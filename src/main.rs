use std::collections::{HashMap, HashSet};

use clap::Parser;
use eyre::Context as _;
use machine::Machine;
use serde::Deserialize;
use tokio::process::Command;

pub mod latency_matrix;
pub mod machine;

#[derive(Debug, Parser)]
pub struct Args {
    #[clap(long, env = "OAR_JOB_ID")]
    pub job_id: Option<u32>,
    #[clap(long, env = "FRONTEND_HOSTNAME")]
    pub frontend_hostname: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionNode {
    Frontend,
    Machine(Machine),
    Unknown,
}

pub struct Context {
    pub node: ExecutionNode,
    pub job_id: Option<u32>,
    pub frontend_hostname: Option<String>,
}

pub struct MachineConfig {
    machine
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let args = Args::parse();
    let node = get_execution_node()?;
    let context = Context {
        node,
        job_id: args.job_id,
        frontend_hostname: args.frontend_hostname,
    };
    let machines = list_job_machines(&context).await?;

    // listing oar job machines
    // if we are in the frontend we use oarstat
    // if we are in a job machine we read the cpuset file
    // if we are outside the cluster we use ssh cluster oarstat

    // machine generate configurations
    // this does not require any connections

    // machine cleanup interface
    // requires running commands inside a container inside a machine
    // we could generate a cleanup bash script and execute that in oneshot

    // machine apply configuration
    // this also requires running some scripts inside a container inside the machine

    Ok(())
}

async fn list_job_machines(ctx: &Context) -> eyre::Result<Vec<Machine>> {
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
                        "frontend hostname is requiredwhen running from outside the cluster"
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

fn extract_machines_from_oar_stat_json(output: &str, job_id: u32) -> eyre::Result<Vec<Machine>> {
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

fn get_execution_node() -> eyre::Result<ExecutionNode> {
    let hostname = get_hostname()?;
    let node = match hostname.as_str() {
        "frontend" => ExecutionNode::Frontend,
        _ => match Machine::from_hostname(&hostname) {
            Some(machine) => ExecutionNode::Machine(machine),
            _ => ExecutionNode::Unknown,
        },
    };
    Ok(node)
}

fn get_hostname() -> eyre::Result<String> {
    std::env::var("HOSTNAME").context("reading HOSTNAME env var")
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
