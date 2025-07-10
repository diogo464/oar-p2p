#![feature(exit_status_error)]
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::Output,
};

use clap::{Args, Parser, Subcommand};
use eyre::Context as _;
use eyre::Result;
use futures::{StreamExt as _, stream::FuturesUnordered};
use machine::Machine;
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    process::Command,
    task::JoinSet,
};

use crate::latency_matrix::LatencyMatrix;

pub mod latency_matrix;
pub mod machine;

const CONTAINER_IMAGE_NAME: &'static str = "local/oar-p2p-networking";

#[derive(Debug, Parser)]
struct Cli {
    #[clap(subcommand)]
    cmd: SubCmd,
}

#[derive(Debug, Args)]
struct Common {
    #[clap(long, env = "OAR_JOB_ID")]
    job_id: Option<u32>,

    #[clap(long, env = "FRONTEND_HOSTNAME")]
    frontend_hostname: Option<String>,
}

#[derive(Debug, Subcommand)]
enum SubCmd {
    Net(NetArgs),
    Run(RunArgs),
}

#[derive(Debug, Args)]
struct NetArgs {
    #[clap(subcommand)]
    cmd: NetSubCmd,
}

#[derive(Debug, Subcommand)]
enum NetSubCmd {
    Up(NetUpArgs),
    Down(NetDownArgs),
    Show(NetShowArgs),
    Preview(NetPreviewArgs),
}

#[derive(Debug, Args)]
struct NetUpArgs {
    #[clap(flatten)]
    common: Common,
    #[clap(long)]
    addr_per_cpu: u32,
    #[clap(long)]
    latency_matrix: PathBuf,
}

#[derive(Debug, Args)]
struct NetDownArgs {
    #[clap(flatten)]
    common: Common,
}

#[derive(Debug, Args)]
struct NetShowArgs {
    #[clap(flatten)]
    common: Common,
}

#[derive(Debug, Args)]
struct NetPreviewArgs {
    #[clap(long)]
    machine: Vec<Machine>,

    #[clap(long)]
    addr_per_cpu: u32,

    #[clap(long)]
    latency_matrix: PathBuf,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[clap(flatten)]
    common: Common,

    #[clap(long)]
    output_dir: PathBuf,

    schedule: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExecutionNode {
    Frontend,
    Machine(Machine),
    Unknown,
}

#[derive(Debug, Clone)]
struct Context {
    node: ExecutionNode,
    job_id: Option<u32>,
    frontend_hostname: Option<String>,
}

#[derive(Debug, Clone)]
struct MachineConfig {
    machine: Machine,
    addresses: Vec<Ipv4Addr>,
    nft_script: String,
    tc_commands: Vec<String>,
    ip_commands: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    color_eyre::install()?;

    let cli = Cli::parse();
    match cli.cmd {
        SubCmd::Net(args) => match args.cmd {
            NetSubCmd::Up(args) => cmd_net_up(args).await,
            NetSubCmd::Down(args) => cmd_net_down(args).await,
            NetSubCmd::Show(args) => cmd_net_show(args).await,
            NetSubCmd::Preview(args) => cmd_net_preview(args).await,
        },
        SubCmd::Run(args) => cmd_run(args).await,
    }
}

async fn context_from_common(common: &Common) -> Result<Context> {
    let node = get_execution_node().await?;
    Ok(Context {
        node,
        job_id: common.job_id,
        frontend_hostname: common.frontend_hostname.clone(),
    })
}

async fn cmd_net_up(args: NetUpArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let matrix_content = tokio::fs::read_to_string(&args.latency_matrix)
        .await
        .context("reading latecy matrix")?;
    let matrix = LatencyMatrix::parse(&matrix_content, latency_matrix::TimeUnit::Milliseconds)
        .context("parsing latency matrix")?;
    let machines = job_list_machines(&context).await?;
    let configs = machine_generate_configs(&matrix, &machines, args.addr_per_cpu);
    machines_net_container_build(&context, &machines).await?;
    machines_clean(&context, &machines).await?;
    machines_configure(&context, &configs).await?;
    Ok(())
}

async fn cmd_net_down(args: NetDownArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let machines = job_list_machines(&context).await?;
    machines_net_container_build(&context, &machines).await?;
    machines_clean(&context, &machines).await?;
    Ok(())
}

async fn cmd_net_show(args: NetShowArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let machines = job_list_machines(&context).await?;
    let mut set = JoinSet::default();
    for machine in machines {
        let context = context.clone();
        set.spawn(async move { (machine, machine_list_addresses(&context, machine).await) });
    }
    let mut addresses = Vec::default();
    for (machine, result) in set.join_all().await {
        let addrs = result?;
        for addr in addrs {
            addresses.push((machine, addr));
        }
    }
    addresses.sort();
    for (machine, addr) in addresses {
        println!("{} {}", machine, addr);
    }
    Ok(())
}

async fn cmd_net_preview(args: NetPreviewArgs) -> Result<()> {
    let matrix_content = tokio::fs::read_to_string(&args.latency_matrix)
        .await
        .context("reading latecy matrix")?;
    let matrix = LatencyMatrix::parse(&matrix_content, latency_matrix::TimeUnit::Milliseconds)
        .context("parsing latency matrix")?;
    let machines = args.machine;
    let configs = machine_generate_configs(&matrix, &machines, args.addr_per_cpu);

    for config in configs {
        (0..20).for_each(|_| print!("-"));
        print!(" {} ", config.machine);
        (0..20).for_each(|_| print!("-"));
        println!();
        println!("{}", machine_configuration_script(&config));
    }
    Ok(())
}

fn machine_from_addr(addr: Ipv4Addr) -> Result<Machine> {
    let machine_index = usize::from(addr.octets()[1]);
    Machine::from_index(machine_index)
        .ok_or_else(|| eyre::eyre!("failed to resolve machine from address {addr}"))
}

#[derive(Debug, Clone)]
struct ScheduledContainer {
    name: String,
    image: String,
    machine: Machine,
    address: Ipv4Addr,
    variables: HashMap<String, String>,
}

fn parse_schedule(schedule: &str) -> Result<Vec<ScheduledContainer>> {
    #[derive(Debug, Deserialize)]
    struct ScheduleItem {
        name: Option<String>,
        address: Ipv4Addr,
        image: String,
        env: HashMap<String, String>,
    }

    let items = serde_json::from_str::<Vec<ScheduleItem>>(schedule)?;
    let mut containers = Vec::default();
    for item in items {
        let name = match item.name {
            Some(name) => name,
            None => item.address.to_string(),
        };
        let machine = machine_from_addr(item.address)?;

        containers.push(ScheduledContainer {
            name,
            image: item.image,
            machine,
            address: item.address,
            variables: item.env,
        });
    }
    Ok(containers)
}

async fn cmd_run(args: RunArgs) -> Result<()> {
    let ctx = context_from_common(&args.common).await?;
    let machines = job_list_machines(&ctx).await?;
    let schedule = match args.schedule {
        Some(path) => tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("reading schedule file: {}", path.display()))?,
        None => {
            let mut stdin = String::default();
            tokio::io::stdin()
                .read_to_string(&mut stdin)
                .await
                .context("reading schedule from stdin")?;
            stdin
        }
    };
    let containers = parse_schedule(&schedule)?;

    machines_foreach(&machines, |machine| machine_containers_clean(&ctx, machine)).await?;
    machines_foreach(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        let mut script = String::default();
        for (idx, container) in containers.iter().enumerate() {
            script.push_str("docker create \\\n");
            script.push_str("\t--pull=always \\\n");
            script.push_str("\t--network=host \\\n");
            script.push_str("\t--restart=no \\\n");
            script.push_str(&format!("\t--name {} \\\n", container.name));
            for (key, val) in container.variables.iter() {
                script.push_str("\t-e ");
                script.push_str(key);
                script.push_str("=");
                script.push_str(val);
                script.push_str(" \\\n");
            }
            script.push_str("\t");
            script.push_str(&container.image);
            script.push_str(" &\n");
            script.push_str(&format!("pid_{idx}=$!\n\n"));
        }

        for (idx, container) in containers.iter().enumerate() {
            let name = &container.name;
            script.push_str(&format!(
                "wait $pid_{idx} || {{ echo Failed to create container {name} ; exit 1 ; }}\n"
            ));
        }
        tracing::debug!("container creation script:\n{script}");
        async move { machine_run_script(&ctx, machine, &script).await }
    })
    .await?;

    tracing::info!("starting all containers on all machines");
    machines_foreach(
        machines
            .iter()
            .filter(|&machine| containers.iter().any(|c| c.machine == *machine)),
        |machine| {
            machine_run_script(
                &ctx,
                machine,
                "docker container ls -aq | xargs docker container start",
            )
        },
    )
    .await?;

    tracing::info!("waiting for all containers to exit");
    machines_foreach(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        let mut script = String::default();
        for container in containers {
            let name = &container.name;
            script.push_str(&format!("if [ \"$(docker wait {name})\" -ne \"0\" ] ; then\n"));
            script.push_str(&format!("\techo Container {name} failed\n"));
            script.push_str(&format!("\tdocker logs {name} 2>1\n"));
            script.push_str("\texit 1\n");
            script.push_str("fi\n\n");
        }
        script.push_str("exit 0\n");
        async move { machine_run_script(&ctx, machine, &script).await }
    })
    .await?;

    tracing::info!("saving logs to disk on all machines");
    machines_foreach(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        let mut script = String::default();
        script.push_str("set -e\n");
        script.push_str("mkdir -p /tmp/oar-p2p-logs\n");
        script.push_str("find /tmp/oar-p2p-logs -maxdepth 1 -type f -delete\n");
        for container in containers {
            let name = &container.name;
            script.push_str(&format!("docker logs {name} 1> /tmp/oar-p2p-logs/{name}.stdout 2> /tmp/oar-p2p-logs/{name}.stderr\n"));
        }
        script.push_str("exit 0\n");
        async move { machine_run_script(&ctx, machine, &script).await }
    })
    .await?;

    machines_foreach(
        machines
            .iter()
            .filter(|&machine| containers.iter().any(|c| c.machine == *machine)),
        |machine| machine_copy_logs_dir(&ctx, machine, &args.output_dir),
    )
    .await?;

    Ok(())
}

async fn machine_copy_logs_dir(ctx: &Context, machine: Machine, output_dir: &Path) -> Result<()> {
    let scp_common = &[
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
    ];

    let mut args = vec![];
    args.extend(scp_common);
    if ctx.node == ExecutionNode::Unknown {
        args.push("-J");
        args.push(ctx.frontend_hostname.as_ref().expect("TODO"));
    }
    args.push("-r");

    let source_path = format!("{}:/tmp/oar-p2p-logs", machine.hostname());
    let destination_path = output_dir.display().to_string();
    args.push(&source_path);
    args.push(&destination_path);

    let output = Command::new("scp").args(args).output().await?;
    output.exit_ok()?;
    Ok(())
}

async fn machines_foreach<F, FUT, RET>(
    machines: impl IntoIterator<Item = &Machine>,
    f: F,
) -> Result<()>
where
    F: Fn(Machine) -> FUT,
    FUT: std::future::Future<Output = Result<RET>>,
{
    let mut futures = FuturesUnordered::new();

    for &machine in machines {
        let fut = f(machine);
        let fut = async move { (machine, fut.await) };
        futures.push(fut);
    }

    while let Some((machine, result)) = futures.next().await {
        if let Err(err) = result {
            tracing::error!("error on machine {machine}: {err}");
            return Err(err);
        }
    }
    Ok(())
}

#[tracing::instrument(ret, err, skip_all, fields(machine = machine.to_string()))]
async fn machine_containers_clean(ctx: &Context, machine: Machine) -> Result<()> {
    tracing::info!("removing all containers...");
    machine_run_script(ctx, machine, "docker ps -aq | xargs -r docker rm -f").await?;
    Ok(())
}

#[tracing::instrument(ret, err, skip_all)]
async fn machines_clean(ctx: &Context, machines: &[Machine]) -> Result<()> {
    tracing::info!("cleaning machines: {machines:?}");
    let mut set = JoinSet::default();
    for &machine in machines {
        let ctx = ctx.clone();
        set.spawn(async move { machine_clean(&ctx, machine).await });
    }
    let results = set.join_all().await;
    for result in results {
        result?;
    }
    Ok(())
}

#[tracing::instrument(ret, err, skip_all)]
async fn machines_net_container_build(ctx: &Context, machines: &[Machine]) -> Result<()> {
    tracing::info!("building networking container for machines: {machines:?}");
    let mut set = JoinSet::default();
    for &machine in machines {
        let ctx = ctx.clone();
        set.spawn(async move { machine_net_container_build(&ctx, machine).await });
    }
    for result in set.join_all().await {
        result?;
    }
    Ok(())
}

#[tracing::instrument(ret, err, skip_all)]
async fn machines_configure(ctx: &Context, configs: &[MachineConfig]) -> Result<()> {
    tracing::info!("configuring machines");
    let mut set = JoinSet::default();
    for config in configs {
        let ctx = ctx.clone();
        let config = config.clone();
        set.spawn(async move { machine_configure(&ctx, &config).await });
    }
    for result in set.join_all().await {
        result?;
    }
    Ok(())
}

async fn machine_list_addresses(ctx: &Context, machine: Machine) -> Result<Vec<Ipv4Addr>> {
    let interface = machine.interface();
    let script = format!("ip addr show {interface} | grep -oE '10\\.[0-9]+\\.[0-9]+\\.[0-9]+'");
    let output = machine_run_script(ctx, machine, &script).await?;
    let stdout = std::str::from_utf8(&output.stdout)?;
    let mut addresses = Vec::default();
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        tracing::trace!("parsing address from line: '{line}'");
        addresses.push(line.parse()?);
    }
    Ok(addresses)
}

async fn machine_run(
    ctx: &Context,
    machine: Machine,
    args: &[&str],
    stdin: Option<&str>,
) -> Result<Output> {
    let ssh_common = &[
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
    ];

    let mut arguments = match ctx.node {
        ExecutionNode::Frontend => {
            let mut arguments = Vec::default();
            arguments.push("ssh");
            arguments.extend(ssh_common);
            arguments.push(machine.hostname());
            arguments
        }
        ExecutionNode::Machine(m) => {
            if m == machine {
                vec![]
            } else {
                let mut arguments = Vec::default();
                arguments.push("ssh");
                arguments.extend(ssh_common);
                arguments.push(machine.hostname());
                arguments
            }
        }
        ExecutionNode::Unknown => {
            let frontend = ctx.frontend_hostname.as_ref().unwrap();
            let mut arguments = Vec::default();
            arguments.push("ssh");
            arguments.extend(ssh_common);
            arguments.push("-J");
            arguments.push(frontend);
            arguments.push(machine.hostname());
            arguments
        }
    };
    if args.is_empty() {
        arguments.push("bash");
    }
    arguments.extend(args);

    let mut proc = Command::new(arguments[0])
        .args(&arguments[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("spawning process")?;

    if let Some(stdin) = stdin {
        let proc_stdin = proc.stdin.as_mut().unwrap();
        proc_stdin
            .write_all(stdin.as_bytes())
            .await
            .context("writing stdin")?;
    }

    let output = proc
        .wait_with_output()
        .await
        .context("waiting for process to exit")?;

    Ok(output)
}

async fn machine_run_script(ctx: &Context, machine: Machine, script: &str) -> Result<Output> {
    tracing::trace!("running script on machine {machine}:\n{script}");
    let output = machine_run(ctx, machine, &[], Some(script)).await?;
    tracing::trace!(
        "stdout:\n{}",
        std::str::from_utf8(&output.stdout).unwrap_or("<invalid utf-8>")
    );
    tracing::trace!(
        "stderr:\n{}",
        std::str::from_utf8(&output.stderr).unwrap_or("<invalid utf-8>")
    );
    Ok(output.exit_ok()?)
}

async fn machine_net_container_run_script(
    ctx: &Context,
    machine: Machine,
    script: &str,
) -> Result<Output> {
    machine_run(
        ctx,
        machine,
        &[
            "docker",
            "run",
            "--rm",
            "-i",
            "--net=host",
            "--privileged",
            CONTAINER_IMAGE_NAME,
        ],
        Some(script),
    )
    .await
}

#[tracing::instrument(ret, err, skip_all, fields(machine = machine.to_string()))]
async fn machine_net_container_build(ctx: &Context, machine: Machine) -> Result<()> {
    let script = r#"
set -e
cat << EOF > /tmp/oar-p2p.containerfile
FROM alpine:latest
RUN apk update && \
    apk add --no-cache bash grep iproute2 iproute2-tc nftables && \
    rm -rf /var/cache/apk/*

WORKDIR /work
EOF

docker build -t local/oar-p2p-networking:latest -f /tmp/oar-p2p.containerfile .
"#;
    machine_run_script(ctx, machine, script).await?;
    Ok(())
}

#[tracing::instrument(ret, err, skip_all, fields(machine = machine.to_string()))]
async fn machine_clean(ctx: &Context, machine: Machine) -> Result<()> {
    let interface = machine.interface();
    let mut script = String::default();
    script.push_str(&format!(
        "ip route del 10.0.0.0/8 dev {interface} || true\n"
    ));
    script.push_str(&format!("ip addr show {interface} | grep -oE '10\\.[0-9]+\\.[0-9]+\\.[0-9]+/32' | sed 's/\\(.*\\)/addr del \\1 dev {interface}/' | ip -b -\n"));
    script.push_str(&format!(
        "tc qdisc del dev {interface} root 2>/dev/null || true\n"
    ));
    script.push_str(&format!(
        "tc qdisc del dev {interface} ingress 2>/dev/null || true\n"
    ));
    script.push_str("tc qdisc del dev lo root 2>/dev/null || true\n");
    script.push_str("tc qdisc del dev lo ingress 2>/dev/null || true\n");
    script.push_str("nft delete table oar-p2p 2>/dev/null || true\n");
    let output = machine_net_container_run_script(&ctx, machine, &script).await?;
    Ok(())
}

fn machine_configuration_script(config: &MachineConfig) -> String {
    let mut script = String::default();
    // ip configuration
    script.push_str("cat << EOF | ip -b -\n");
    for command in config.ip_commands.iter() {
        script.push_str(command);
        script.push_str("\n");
    }
    script.push_str("\nEOF\n");

    // tc configuration
    script.push_str("cat << EOF | tc -b -\n");
    for command in config.tc_commands.iter() {
        script.push_str(command);
        script.push_str("\n");
    }
    script.push_str("\nEOF\n");

    // nft configuration
    script.push_str("cat << EOF | nft -f -\n");
    script.push_str(&config.nft_script);
    script.push_str("\nEOF\n");
    script
}

#[tracing::instrument(ret, err, skip_all, fields(machine = config.machine.to_string()))]
async fn machine_configure(ctx: &Context, config: &MachineConfig) -> Result<()> {
    let script = machine_configuration_script(config);
    tracing::debug!("configuration script:\n{script}");
    machine_net_container_run_script(ctx, config.machine, &script).await?;
    Ok(())
}

fn machine_address_for_idx(machine: Machine, idx: u32) -> Ipv4Addr {
    let c = u8::try_from(idx / 254).unwrap();
    let d = u8::try_from(idx % 254 + 1).unwrap();
    Ipv4Addr::new(10, machine.index().try_into().unwrap(), c, d)
}

fn machine_generate_configs(
    matrix: &LatencyMatrix,
    machines: &[Machine],
    addr_per_cpu: u32,
) -> Vec<MachineConfig> {
    let mut configs = Vec::default();
    let mut addresses = Vec::default();
    let mut address_to_index = HashMap::<Ipv4Addr, usize>::default();

    // gather all addresses across all machines
    for &machine in machines {
        for i in 0..(addr_per_cpu * machine.cpus()) {
            let address = machine_address_for_idx(machine, i);
            addresses.push(address);
            address_to_index.insert(address, addresses.len() - 1);
        }
    }

    for &machine in machines {
        let mut machine_addresses = Vec::default();
        let mut machine_ip_commands = Vec::default();
        let mut machine_tc_commands = Vec::default();
        let mut machine_nft_script = String::default();

        machine_ip_commands.push(format!("route add 10.0.0.0/8 dev {}", machine.interface()));
        for i in 0..(addr_per_cpu * machine.cpus()) {
            let address = machine_address_for_idx(machine, i);
            machine_addresses.push(address);
            machine_ip_commands.push(format!("addr add {address}/32 dev {}", machine.interface()));
        }

        let mut latencies_set = HashSet::<u32>::default();
        let mut latencies_buckets = Vec::<u32>::default();
        let mut latencies_addr_pairs = HashMap::<u32, Vec<(Ipv4Addr, Ipv4Addr)>>::default();
        for &addr in &machine_addresses {
            let addr_idx = address_to_index[&addr];
            for other_idx in (0..addresses.len()).filter(|i| *i != addr_idx) {
                let other = addresses[other_idx];
                let latency = matrix.latency(addr_idx, other_idx);
                let latency_millis = u32::try_from(latency.as_millis()).unwrap();
                if !latencies_set.contains(&latency_millis) {
                    latencies_set.insert(latency_millis);
                    latencies_buckets.push(latency_millis);
                }
                latencies_addr_pairs
                    .entry(latency_millis)
                    .or_default()
                    .push((addr, other));
            }
        }

        for iface in &["lo", machine.interface()] {
            machine_tc_commands.push(format!(
                "qdisc add dev {iface} root handle 1: htb default 9999"
            ));
            machine_tc_commands.push(format!(
                "class add dev {iface} parent 1: classid 1:9999 htb rate 10gbit"
            ));
            for (idx, &latency_millis) in latencies_buckets.iter().enumerate() {
                // tc class for latency at idx X is X + 1
                let latency_class_id = idx + 1;
                // mark for latency at idx X is X + 1
                let latency_mark = idx + 1;

                machine_tc_commands.push(format!(
                    "class add dev {iface} parent 1: classid 1:{} htb rate 10gbit",
                    latency_class_id
                ));
                // why idx + 2 here? I dont remember anymore and forgot to comment
                machine_tc_commands.push(format!(
                    "qdisc add dev {iface} parent 1:{} handle {}: netem delay {latency_millis}ms",
                    latency_class_id,
                    idx + 2
                ));
                // TODO: is the order of these things correct?
                machine_tc_commands.push(format!(
                    "filter add dev {iface} parent 1:0 prio 1 handle {} fw flowid 1:{}",
                    latency_mark, latency_class_id,
                ));
            }
        }

        machine_nft_script.push_str("table ip oar-p2p {\n");
        machine_nft_script.push_str("\tmap mark_pairs {\n");
        machine_nft_script.push_str("\t\ttype ipv4_addr . ipv4_addr : mark\n");
        machine_nft_script.push_str("\t\telements = {\n");
        for (latency_idx, &latency_millis) in latencies_buckets.iter().enumerate() {
            let latency_mark = latency_idx + 1;
            let pairs = match latencies_addr_pairs.get(&latency_millis) {
                Some(pairs) => pairs,
                None => continue,
            };

            for (src, dst) in pairs {
                assert_ne!(src, dst);
                machine_nft_script.push_str(&format!("\t\t\t{src} . {dst} : {latency_mark},\n"));
            }
        }
        machine_nft_script.push_str("\t\t}\n");
        machine_nft_script.push_str("\t}\n");
        machine_nft_script.push_str("\n");
        machine_nft_script.push_str("\tchain postrouting {\n");
        machine_nft_script.push_str("\t\ttype filter hook postrouting priority mangle -1\n");
        machine_nft_script.push_str("\t\tpolicy accept\n");
        machine_nft_script
            .push_str("\t\tmeta mark set ip saddr . ip daddr map @mark_pairs counter\n");
        machine_nft_script.push_str("\t}\n");
        machine_nft_script.push_str("}\n");

        configs.push(MachineConfig {
            machine,
            addresses: machine_addresses,
            nft_script: machine_nft_script,
            tc_commands: machine_tc_commands,
            ip_commands: machine_ip_commands,
        });
    }
    configs
}

async fn job_list_machines(ctx: &Context) -> Result<Vec<Machine>> {
    match ctx.node {
        ExecutionNode::Frontend => {
            let job_id = match ctx.job_id {
                Some(job_id) => job_id,
                None => return Err(eyre::eyre!("job id is required when running from cluster")),
            };

            let output = Command::new("oarstat")
                .arg("-j")
                .arg(job_id.to_string())
                .arg("-J")
                .output()
                .await?;

            if !output.status.success() {
                tracing::error!(
                    "stdout: {}",
                    std::str::from_utf8(&output.stdout).unwrap_or("stderr contains invalid uft-8")
                );
                tracing::error!(
                    "stderr: {}",
                    std::str::from_utf8(&output.stderr).unwrap_or("stderr contains invalid uft-8")
                );
                return Err(eyre::eyre!("failed to run oarstat"));
            }

            let stdout = std::str::from_utf8(&output.stdout)?;
            extract_machines_from_oar_stat_json(&stdout, job_id)
        }
        ExecutionNode::Unknown => {
            let frontend_hostname = match ctx.frontend_hostname.as_ref() {
                Some(hostname) => hostname,
                None => {
                    return Err(eyre::eyre!(
                        "frontend hostname is required when running from outside the cluster"
                    ));
                }
            };

            let job_id = match ctx.job_id {
                Some(job_id) => job_id,
                None => return Err(eyre::eyre!("job id is required when running from cluster")),
            };

            let output = Command::new("ssh")
                .arg(frontend_hostname)
                .arg("oarstat")
                .arg("-j")
                .arg(job_id.to_string())
                .arg("-J")
                .output()
                .await?;

            if !output.status.success() {
                return Err(eyre::eyre!("failed to run oarstat"));
            }

            let stdout = std::str::from_utf8(&output.stdout)?;
            extract_machines_from_oar_stat_json(&stdout, job_id)
        }
        ExecutionNode::Machine(_) => {
            let nodefile = std::env::var("OAR_NODEFILE").context("reading OAR_NODEFILE env var")?;
            let content = tokio::fs::read_to_string(&nodefile).await?;
            let unique_lines = content
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<HashSet<_>>();
            let mut machines = Vec::default();
            for hostname in unique_lines {
                let machine = match Machine::from_hostname(hostname) {
                    Some(machine) => machine,
                    None => return Err(eyre::eyre!("unknown machine: {hostname}")),
                };
                machines.push(machine);
            }
            Ok(machines)
        }
    }
}

fn extract_machines_from_oar_stat_json(output: &str, job_id: u32) -> Result<Vec<Machine>> {
    #[derive(Debug, Deserialize)]
    struct JobSchema {
        assigned_network_address: Vec<String>,
    }
    let map = serde_json::from_str::<HashMap<String, JobSchema>>(output)?;
    let key = job_id.to_string();
    let data = map
        .get(&key)
        .ok_or_else(|| eyre::eyre!("missing job key"))?;
    let mut machines = Vec::default();
    for hostname in data.assigned_network_address.iter() {
        match Machine::from_hostname(hostname) {
            Some(machine) => machines.push(machine),
            None => return Err(eyre::eyre!("unknown machine: '{hostname}'")),
        }
    }
    Ok(machines)
}

async fn get_execution_node() -> Result<ExecutionNode> {
    let hostname = get_hostname().await?;
    let node = match hostname.as_str() {
        "frontend" => ExecutionNode::Frontend,
        _ => match Machine::from_hostname(&hostname) {
            Some(machine) => ExecutionNode::Machine(machine),
            _ => ExecutionNode::Unknown,
        },
    };
    Ok(node)
}

async fn get_hostname() -> Result<String> {
    if let Ok(hostname) = tokio::fs::read_to_string("/etc/hostname").await {
        Ok(hostname)
    } else {
        std::env::var("HOSTNAME").context("reading HOSTNAME env var")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const OAR_STAT_JSON_JOB_ID: u32 = 36627;
    const OAR_STAT_JSON_OUTPUT: &'static str = r#"
{
   "36627" : {
      "types" : [],
      "reservation" : "None",
      "dependencies" : [],
      "Job_Id" : 36627,
      "assigned_network_address" : [
         "gengar-1",
         "gengar-2"
      ],
      "owner" : "diogo464",
      "properties" : "(( ( dedicated='NO' OR dedicated='protocol-labs' )) AND desktop_computing = 'NO') AND drain='NO'",
      "startTime" : 1751979909,
      "cpuset_name" : "diogo464_36627",
      "stderr_file" : "OAR.36627.stderr",
      "queue" : "default",
      "state" : "Running",
      "stdout_file" : "OAR.36627.stdout",
      "array_index" : 1,
      "array_id" : 36627,
      "assigned_resources" : [
         419,
         420,
         421,
         422,
         423,
         424,
         425,
         426,
         427,
         428,
         429,
         430,
         431,
         432,
         433,
         434
      ],
      "name" : null,
      "resubmit_job_id" : 0,
      "message" : "R=16,W=12:0:0,J=B (Karma=0.087,quota_ok)",
      "launchingDirectory" : "/home/diogo464",
      "jobType" : "PASSIVE",
      "submissionTime" : 1751979897,
      "project" : "default",
      "command" : "sleep 365d"
   }
}
"#;

    #[test]
    fn test_extract_machines_from_oar_stat_json() {
        let machines =
            extract_machines_from_oar_stat_json(OAR_STAT_JSON_OUTPUT, OAR_STAT_JSON_JOB_ID)
                .unwrap();
        assert_eq!(machines.len(), 2);
        assert_eq!(machines[0], Machine::Gengar1);
        assert_eq!(machines[1], Machine::Gengar2);
    }
}
