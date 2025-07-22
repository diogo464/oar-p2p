#![feature(exit_status_error)]
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::Output,
    time::{Duration, Instant},
};

use clap::{Args, Parser, Subcommand};
use eyre::Context as _;
use eyre::Result;
use machine::Machine;
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    process::Command,
    task::JoinSet,
};

use crate::{
    address_allocation_policy::AddressAllocationPolicy,
    context::{Context, ExecutionNode},
    latency_matrix::LatencyMatrix,
    signal::{Signal, SignalSpec},
};

pub mod address_allocation_policy;
pub mod context;
pub mod latency_matrix;
pub mod machine;
pub mod oar;
pub mod signal;

const CONTAINER_IMAGE_NAME: &str = "local/oar-p2p-networking";

#[derive(Debug, Parser)]
struct Cli {
    #[clap(subcommand)]
    cmd: SubCmd,
}

#[derive(Debug, Args)]
struct Common {
    /// oar job id
    #[clap(long, env = "OAR_JOB_ID")]
    job_id: Option<u32>,

    /// infer the oar job id
    ///
    /// if you have a single job running on the cluster, that job will be used, otherwise,
    /// inference fails. the job_id flag takes precedence of over inference.
    #[clap(long, env = "OAR_P2P_INFER_JOB_ID")]
    infer_job_id: bool,

    /// hostname used to access the frontend using ssh.
    /// i.e. `ssh <frontend-hostname>` should work.
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
    /// specify how addresses will be created.
    ///
    /// the address allocation policy specifies how addresses will be created. there are 3
    /// different ways to allocate addresses.
    ///
    /// 1. total number of addresses: in this policy, a fixed number of addresses will be allocated
    ///    between all machines in the job. this is represented by a single number, for example,
    ///    `64` will allocated a total of 64 addresses evenly across all machines in the job.
    ///
    /// 2. per cpu: in this policy, a set number of addresses will be allocated per cpu. machines
    ///    with more cpus will have more addresses. this is represented by `<n>/cpu`, for example,
    ///    `4/cpu` will allocated 4 addresses per cpu on every machine.
    ///
    /// 3. per machine: in this policy, a set number of addresses will be allocated per machine.
    ///    each machine gets the same amount of addresses. this is represented by `<n>/machine`,
    ///    for example, `64/machine` will allocate 64 addresses per machine on every machine.
    #[clap(long)]
    addresses: AddressAllocationPolicy,

    /// path to the latency matrix.
    ///
    /// the latency matrix is a square matrix of latency values in milliseconds.
    /// here is an example latency matrix:{n}
    /// {n}
    /// 0.0 25.5687 78.64806 83.50032 99.91315 {n}
    /// 25.5687 0.0 63.165894 66.74037 110.71518 {n}
    /// 78.64806 63.165894 0.0 2.4708898 93.90618 {n}
    /// 83.50032 66.74037 2.4708898 0.0 84.67561 {n}
    /// 99.91315 110.71518 93.90618 84.67561 0.0 {n}
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
    addresses: AddressAllocationPolicy,

    #[clap(long)]
    latency_matrix: PathBuf,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[clap(flatten)]
    common: Common,

    /// directory where all the log files will be placed.
    ///
    /// this directory will be created if it does not exist.
    /// for each container, there will be a seperate file for the stdout and sterr.
    #[clap(long)]
    output_dir: PathBuf,

    /// declare a signal. this flag can be used more than once to declare multiple signals.
    ///
    /// a signal is an empty file that will be come visible to all containers after some amount of
    /// time under the `/oar-p2p/` directory. a sginal has the format `<signal name>:<delay>`,
    /// where the delay is given in seconds. using the signal `start:10` as an example, this means
    /// that after all containers are started, a 10 second timer will start and when that timer
    /// expires the file `/oar-p2p/start` will become visible to all containers at roughtly the
    /// same time allowing them to synchronize their start-ups to within milliseconds. to make use
    /// of this, your program running in the container must somehow wait for this file to come into
    /// existance and this can be as simple as having a while loop checking for the file's existing
    /// with a short sleep, here is an example in java:{n}
    ///```java{n}
    ///01.  import java.nio.file.Files;{n}
    ///02.  import java.nio.file.Path;{n}
    ///03. {n}
    ///04.  public static void waitForStartFile() {{n}
    ///05.      Path startFile = Path.of("/oar-p2p/start");{n}
    ///06.      while (!Files.exists(startFile)) {{n}
    ///07.          try {{n}
    ///08.              Thread.sleep(250);{n}
    ///09.          } catch (InterruptedException e) {{n}
    ///10.              Thread.currentThread().interrupt();{n}
    ///11.              break;{n}
    ///12.          }{n}
    ///13.      }{n}
    ///14.  }{n}
    ///```{n}
    #[clap(long)]
    signal: Vec<SignalSpec>,

    /// the schedule used for execution. if not specified, it will be read from stdin.
    schedule: Option<PathBuf>,
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
    Context::new(
        common.job_id,
        common.infer_job_id,
        common.frontend_hostname.clone(),
    )
    .await
}

async fn cmd_net_up(args: NetUpArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let matrix_content = tokio::fs::read_to_string(&args.latency_matrix)
        .await
        .context("reading latecy matrix")?;
    let matrix = LatencyMatrix::parse(&matrix_content, latency_matrix::TimeUnit::Milliseconds)
        .context("parsing latency matrix")?;
    let machines = oar::job_list_machines(&context).await?;
    let configs = machine_generate_configs(&matrix, &machines, &args.addresses)?;
    machines_net_container_build(&context, &machines).await?;
    machines_clean(&context, &machines).await?;
    machines_configure(&context, &configs).await?;
    Ok(())
}

async fn cmd_net_down(args: NetDownArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let machines = oar::job_list_machines(&context).await?;
    machines_net_container_build(&context, &machines).await?;
    machines_clean(&context, &machines).await?;
    Ok(())
}

async fn cmd_net_show(args: NetShowArgs) -> Result<()> {
    let context = context_from_common(&args.common).await?;
    let machines = oar::job_list_machines(&context).await?;
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
        println!("{machine} {addr}");
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
    let configs = machine_generate_configs(&matrix, &machines, &args.addresses)?;

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

    tracing::trace!("parsing schedule:\n{schedule}");
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
    tracing::debug!(
        "creating output directory if it does not exist at {}",
        args.output_dir.display()
    );
    tokio::fs::create_dir_all(&args.output_dir)
        .await
        .context("creating output directory")?;

    let ctx = context_from_common(&args.common).await?;
    let schedule = match args.schedule {
        Some(path) => {
            tracing::debug!("reading schedule from {}", path.display());
            tokio::fs::read_to_string(&path)
                .await
                .with_context(|| format!("reading schedule file: {}", path.display()))?
        }
        None => {
            tracing::debug!("reading schedule from stdin");
            let mut stdin = String::default();
            tokio::io::stdin()
                .read_to_string(&mut stdin)
                .await
                .context("reading schedule from stdin")?;
            stdin
        }
    };
    let containers = parse_schedule(&schedule)?;
    let machines = oar::job_list_machines(&ctx).await?;

    machine::for_each(&machines, |machine| machine_containers_clean(&ctx, machine)).await?;
    machine::for_each(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        async move { machine_create_containers(&ctx, machine, &containers).await }
    })
    .await?;

    tracing::info!("starting all containers on all machines");
    machine::for_each(
        machines
            .iter()
            .filter(|&machine| containers.iter().any(|c| c.machine == *machine)),
        |machine| machine_start_containers(&ctx, machine),
    )
    .await?;

    let signal_start_instant = Instant::now();
    let signal_specs = {
        let mut specs = args.signal.clone();
        specs.sort_by_key(|s| s.delay);
        specs
    };

    for spec in signal_specs {
        tracing::info!("waiting to trigger signal {}", spec.signal);
        let expire = signal_start_instant + spec.delay;
        tokio::time::sleep_until(expire.into()).await;

        tracing::info!("triggering signal {}", spec.signal);
        machine::for_each(
            machines
                .iter()
                .filter(|&machine| containers.iter().any(|c| c.machine == *machine)),
            |machine| machine_signal_containers(&ctx, machine, &spec.signal),
        )
        .await?;
    }

    tracing::info!("waiting for all containers to exit");
    machine::for_each(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        async move {
            machine_containers_wait(&ctx, machine, &containers)
                .await
                .with_context(|| format!("waiting for containers on {machine}"))
        }
    })
    .await?;

    tracing::info!("saving logs to disk on all machines");
    machine::for_each(&machines, |machine| {
        let ctx = ctx.clone();
        let containers = containers
            .iter()
            .filter(|c| c.machine == machine)
            .cloned()
            .collect::<Vec<_>>();
        async move { machine_containers_save_logs(&ctx, machine, &containers).await }
    })
    .await?;

    tracing::info!("copying logs from all machines");
    machine::for_each(
        machines
            .iter()
            .filter(|&machine| containers.iter().any(|c| c.machine == *machine)),
        |machine| machine_copy_logs_dir(&ctx, machine, &args.output_dir),
    )
    .await?;

    Ok(())
}

fn machine_containers_create_script(containers: &[ScheduledContainer]) -> String {
    let mut script = String::default();
    for (idx, container) in containers.iter().enumerate() {
        // remove the start signal file if it exists
        script.push_str("mkdir -p /tmp/oar-p2p-signal\n");
        script.push_str("rm /tmp/oar-p2p-signal/start 2>/dev/null || true\n");

        script.push_str("docker create \\\n");
        script.push_str("\t--pull=always \\\n");
        script.push_str("\t--network=host \\\n");
        script.push_str("\t--restart=no \\\n");
        script.push_str("\t--volume /tmp/oar-p2p-signal:/oar-p2p\\\n");
        script.push_str(&format!("\t--name {} \\\n", container.name));
        for (key, val) in container.variables.iter() {
            script.push_str("\t-e ");
            script.push_str(key);
            script.push('=');
            script.push('\'');
            script.push_str(val);
            script.push('\'');
            script.push_str(" \\\n");
        }
        script.push('\t');
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

    script
}

#[tracing::instrument(ret, err, skip(ctx, containers))]
async fn machine_create_containers(
    ctx: &Context,
    machine: Machine,
    containers: &[ScheduledContainer],
) -> Result<()> {
    tracing::info!("creating {} containers", containers.len());
    let script = machine_containers_create_script(containers);
    machine_run_script(ctx, machine, &script).await?;
    tracing::info!("containers created");
    Ok(())
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_start_containers(ctx: &Context, machine: Machine) -> Result<()> {
    tracing::info!("starting all containers");
    machine_run_script(
        ctx,
        machine,
        "docker container ls -aq | xargs docker container start",
    )
    .await?;
    tracing::info!("all containers started");
    Ok(())
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_signal_containers(ctx: &Context, machine: Machine, signal: &Signal) -> Result<()> {
    tracing::info!("signaling containers");
    machine_run_script(ctx, machine, &format!("touch /tmp/oar-p2p-signal/{signal}")).await?;
    tracing::info!("containers signaled");
    Ok(())
}

fn machine_containers_wait_script(containers: &[ScheduledContainer]) -> String {
    let mut script = String::default();
    for container in containers {
        let name = &container.name;
        script.push_str(&format!(
            "if [ \"$(docker wait {name})\" -ne \"0\" ] ; then\n"
        ));
        script.push_str(&format!("\techo Container {name} failed\n"));
        script.push_str(&format!("\tdocker logs {name} 2>&1\n"));
        script.push_str("\texit 1\n");
        script.push_str("fi\n\n");
    }
    script.push_str("exit 0\n");
    script
}

#[tracing::instrument(ret, err, skip(ctx, containers))]
async fn machine_containers_wait(
    ctx: &Context,
    machine: Machine,
    containers: &[ScheduledContainer],
) -> Result<()> {
    tracing::info!("waiting for {} containers to exit", containers.len());
    let script = machine_containers_wait_script(containers);
    machine_run_script(ctx, machine, &script).await?;
    tracing::info!("all containers exited");
    Ok(())
}

fn machine_containers_save_logs_script(containers: &[ScheduledContainer]) -> String {
    let mut script = String::default();
    script.push_str("set -e\n");
    script.push_str("mkdir -p /tmp/oar-p2p-logs\n");
    script.push_str("find /tmp/oar-p2p-logs -maxdepth 1 -type f -delete\n");
    for container in containers {
        let name = &container.name;
        script.push_str(&format!("docker logs {name} 1> /tmp/oar-p2p-logs/{name}.stdout 2> /tmp/oar-p2p-logs/{name}.stderr\n"));
    }
    script.push_str("exit 0\n");
    script
}

#[tracing::instrument(ret, err, skip(ctx, containers))]
async fn machine_containers_save_logs(
    ctx: &Context,
    machine: Machine,
    containers: &[ScheduledContainer],
) -> Result<()> {
    tracing::info!("saving logs from {} containers", containers.len());
    let script = machine_containers_save_logs_script(containers);
    machine_run_script(ctx, machine, &script).await?;
    tracing::info!("logs saved");
    Ok(())
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_copy_logs_dir(ctx: &Context, machine: Machine, output_dir: &Path) -> Result<()> {
    tracing::info!("copying container logs from machine");
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
        args.push(ctx.frontend_hostname()?);
    }
    args.push("-r");

    let source_path = format!("{}:/tmp/oar-p2p-logs/*", machine.hostname());
    let destination_path = output_dir.display().to_string();
    args.push(&source_path);
    args.push(&destination_path);

    let output = Command::new("scp").args(args).output().await?;
    let stdout = std::str::from_utf8(&output.stdout).unwrap_or("<invalid utf-8>");
    let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<invalid utf-8>");
    if output.status.success() {
        tracing::trace!("scp stdout:\n{stdout}");
        tracing::trace!("scp stderr:\n{stderr}");
    } else {
        tracing::error!("scp stdout:\n{stdout}");
        tracing::error!("scp stderr:\n{stderr}");
    }
    output.exit_ok()?;
    tracing::info!("logs finished copying");
    Ok(())
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_containers_clean(ctx: &Context, machine: Machine) -> Result<()> {
    tracing::info!("removing all containers...");
    machine_run_script(ctx, machine, "docker ps -aq | xargs -r docker rm -f").await?;
    tracing::info!("all containers removed");
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

#[tracing::instrument(err, skip(ctx))]
async fn machine_list_addresses(ctx: &Context, machine: Machine) -> Result<Vec<Ipv4Addr>> {
    tracing::info!("listing machine addresses");
    let interface = machine.interface();
    let script =
        format!("ip addr show {interface} | grep -oE '10\\.[0-9]+\\.[0-9]+\\.[0-9]+' || true");
    let output = machine_run_script(ctx, machine, &script).await?;
    let stdout = std::str::from_utf8(&output.stdout)?;
    let mut addresses = Vec::default();
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        tracing::trace!("parsing address from line: '{line}'");
        addresses.push(line.parse()?);
    }
    tracing::trace!("addresses: {addresses:#?}");
    Ok(addresses)
}

#[tracing::instrument(ret, err, level = tracing::Level::TRACE)]
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
            let frontend = ctx.frontend_hostname()?;
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

    tracing::trace!("running command: {arguments:?}");
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
    tracing::debug!("script body:\n{script}");
    let output = machine_run(ctx, machine, &[], Some(script)).await?;
    let stdout = std::str::from_utf8(&output.stdout).unwrap_or("<invalid utf-8>");
    let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<invalid utf-8>");
    if output.status.success() {
        tracing::trace!("stdout:\n{stdout}",);
        tracing::trace!("stderr:\n{stderr}",);
    } else {
        tracing::error!("stdout:\n{stdout}",);
        tracing::error!("stderr:\n{stderr}",);
    }
    Ok(output.exit_ok()?)
}

async fn machine_net_container_run_script(
    ctx: &Context,
    machine: Machine,
    script: &str,
) -> Result<Output> {
    tracing::debug!("network container script body:\n{script}");
    let output = machine_run(
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
    .await?;

    let stdout = std::str::from_utf8(&output.stdout).unwrap_or("<invalid utf-8>");
    let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<invalid utf-8>");
    if output.status.success() {
        tracing::trace!("stdout:\n{stdout}",);
        tracing::trace!("stderr:\n{stderr}",);
    } else {
        tracing::error!("stdout:\n{stdout}",);
        tracing::error!("stderr:\n{stderr}",);
    }

    Ok(output.exit_ok()?)
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_net_container_build(ctx: &Context, machine: Machine) -> Result<()> {
    tracing::info!("building network container...");
    let script = r#"
set -e
cat << EOF > /tmp/oar-p2p.containerfile
FROM alpine:latest
RUN apk update && \
    apk add --no-cache bash grep iproute2 iproute2-tc nftables && \
    rm -rf /var/cache/apk/*

WORKDIR /work
EOF

rm -rf /tmp/oar-p2p || true
mkdir -p /tmp/oar-p2p
docker build -t local/oar-p2p-networking:latest -f /tmp/oar-p2p.containerfile /tmp/oar-p2p
"#;
    machine_run_script(ctx, machine, script).await?;
    tracing::info!("network container built");
    Ok(())
}

#[tracing::instrument(ret, err, skip(ctx))]
async fn machine_clean(ctx: &Context, machine: Machine) -> Result<()> {
    tracing::info!("cleaning network interfaces");
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
    machine_net_container_run_script(ctx, machine, &script).await?;
    tracing::info!("network interfaces clean");
    Ok(())
}

fn machine_configuration_script(config: &MachineConfig) -> String {
    let mut script = String::default();
    // ip configuration
    script.push_str("cat << EOF | ip -b -\n");
    for command in config.ip_commands.iter() {
        script.push_str(command);
        script.push('\n');
    }
    script.push_str("\nEOF\n");

    // tc configuration
    script.push_str("cat << EOF | tc -b -\n");
    for command in config.tc_commands.iter() {
        script.push_str(command);
        script.push('\n');
    }
    script.push_str("\nEOF\n");

    // nft configuration
    script.push_str("cat << EOF | nft -f -\n");
    script.push_str(&config.nft_script);
    script.push_str("\nEOF\n");
    script
}

#[tracing::instrument(ret, err, skip_all, fields(machine = ?config.machine))]
async fn machine_configure(ctx: &Context, config: &MachineConfig) -> Result<()> {
    tracing::info!(
        "configuring machine with {} addresses",
        config.addresses.len()
    );
    let script = machine_configuration_script(config);
    machine_net_container_run_script(ctx, config.machine, &script).await?;
    tracing::info!("machine configured");
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
    addr_policy: &AddressAllocationPolicy,
) -> Result<Vec<MachineConfig>> {
    let mut configs = Vec::default();
    let mut addresses = Vec::default();
    let mut address_to_index = HashMap::<Ipv4Addr, usize>::default();
    let mut addresses_per_machine = HashMap::<Machine, Vec<Ipv4Addr>>::default();
    machines.iter().for_each(|&m| {
        addresses_per_machine.insert(m, Default::default());
    });

    // gather all addresses across all machines
    match addr_policy {
        AddressAllocationPolicy::PerCpu(n) => {
            for &machine in machines {
                for i in 0..(n * machine.cpus()) {
                    let address = machine_address_for_idx(machine, i);
                    addresses.push(address);
                }
            }
        }
        AddressAllocationPolicy::PerMachine(n) => {
            for &machine in machines {
                for i in 0..*n {
                    let address = machine_address_for_idx(machine, i);
                    addresses.push(address);
                }
            }
        }
        AddressAllocationPolicy::Total(n) => {
            let mut counter = 0;
            while counter < *n {
                let machine = machines[(counter as usize) % machines.len()];
                let address = machine_address_for_idx(machine, counter / (machines.len() as u32));
                addresses.push(address);
                counter += 1;
            }
        }
    }
    for (idx, &address) in addresses.iter().enumerate() {
        let machine = machine_from_addr(address).expect("we should only generate valid addresses");
        address_to_index.insert(address, idx);
        addresses_per_machine
            .entry(machine)
            .or_default()
            .push(address);
    }

    if addresses.len() > matrix.dimension() {
        return Err(eyre::eyre!(
            "latency matrix is too small, size is {} but {} was required",
            matrix.dimension(),
            addresses.len()
        ));
    }

    for &machine in machines {
        let machine_addresses = &addresses_per_machine[&machine];
        let mut machine_ip_commands = Vec::default();
        let mut machine_tc_commands = Vec::default();
        let mut machine_nft_script = String::default();

        machine_ip_commands.push(format!("route add 10.0.0.0/8 dev {}", machine.interface()));
        for address in machine_addresses.iter() {
            machine_ip_commands.push(format!("addr add {address}/32 dev {}", machine.interface()));
        }

        let mut latencies_set = HashSet::<u32>::default();
        let mut latencies_buckets = Vec::<u32>::default();
        let mut latencies_addr_pairs = HashMap::<u32, Vec<(Ipv4Addr, Ipv4Addr)>>::default();
        for &addr in machine_addresses {
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
                    "class add dev {iface} parent 1: classid 1:{latency_class_id} htb rate 10gbit"
                ));
                // why idx + 2 here? I dont remember anymore and forgot to comment
                machine_tc_commands.push(format!(
                    "qdisc add dev {iface} parent 1:{} handle {}: netem delay {latency_millis}ms",
                    latency_class_id,
                    idx + 2
                ));
                // TODO: is the order of these things correct?
                machine_tc_commands.push(format!(
                    "filter add dev {iface} parent 1:0 prio 1 handle {latency_mark} fw flowid 1:{latency_class_id}",
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
        machine_nft_script.push('\n');
        machine_nft_script.push_str("\tchain postrouting {\n");
        machine_nft_script.push_str("\t\ttype filter hook postrouting priority mangle -1\n");
        machine_nft_script.push_str("\t\tpolicy accept\n");
        machine_nft_script
            .push_str("\t\tmeta mark set ip saddr . ip daddr map @mark_pairs counter\n");
        machine_nft_script.push_str("\t}\n");
        machine_nft_script.push_str("}\n");

        configs.push(MachineConfig {
            machine,
            addresses: machine_addresses.clone(),
            nft_script: machine_nft_script,
            tc_commands: machine_tc_commands,
            ip_commands: machine_ip_commands,
        });
    }
    Ok(configs)
}
