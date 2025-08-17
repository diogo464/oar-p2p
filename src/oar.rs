use std::collections::{HashMap, HashSet};

use eyre::{Context as _, Result};
use serde::Deserialize;
use tokio::process::Command;

use crate::{
    context::{Context, ExecutionNode},
    machine::Machine,
};

pub async fn job_list_machines(ctx: &Context) -> Result<Vec<Machine>> {
    match ctx.node {
        ExecutionNode::Frontend => {
            let job_id = ctx.job_id().await?;
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
            extract_machines_from_oar_stat_json(stdout, job_id)
        }
        ExecutionNode::Unknown => {
            let job_id = ctx.job_id().await?;
            let frontend_hostname = ctx.frontend_hostname()?;

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
            extract_machines_from_oar_stat_json(stdout, job_id)
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

pub async fn list_user_job_ids(ctx: &Context) -> Result<Vec<u32>> {
    let output = match ctx.node {
        ExecutionNode::Frontend => Command::new("oarstat").arg("-u").arg("-J").output().await?,
        ExecutionNode::Unknown => {
            Command::new("ssh")
                .arg(ctx.frontend_hostname()?)
                .arg("oarstat")
                .arg("-u")
                .arg("-J")
                .output()
                .await?
        }
        ExecutionNode::Machine(_) => {
            return Err(eyre::eyre!(
                "cannot run oarstat from inside a cluster machine"
            ));
        }
    };

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

    let stdout = String::from_utf8(output.stdout)?;
    // for some reason, running oarstat with the -J flag (for json output) when you have no jobs
    // running results in this error message instead of an empty object, so we will just assume it
    // meant an empty object
    let json_string = if stdout
        == "hash- or arrayref expected (not a simple scalar, use allow_nonref to allow this) at /usr/lib/oar/oarstat line 285."
    {
        String::from("{}")
    } else {
        stdout
    };
    extract_job_ids_from_oarstat_output(&json_string)
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

fn extract_job_ids_from_oarstat_output(output: &str) -> Result<Vec<u32>> {
    let value = serde_json::from_str::<serde_json::Value>(output)?;
    let object = match value {
        serde_json::Value::Object(map) => map,
        _ => {
            return Err(eyre::eyre!(
                "expected oar stat output to produce a json object"
            ));
        }
    };

    let mut job_ids = Vec::default();
    for (key, val) in object.iter() {
        if val
            .get("state")
            .expect("job should have a 'state' key")
            .as_str()
            .expect("job state should be a string")
            != "Running"
        {
            continue;
        }
        tracing::trace!("parsing key '{key}'");
        let job_id = key.parse()?;
        job_ids.push(job_id);
    }
    Ok(job_ids)
}

#[cfg(test)]
mod test {
    use super::*;

    const OAR_STAT_JSON_JOB_ID: u32 = 36627;
    const OAR_STAT_JSON_OUTPUT: &str = r#"
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

    const OAR_STAT_ALL_USER_JOBS_OUTPUT: &'static str = r#"
{
   "37030" : {
      "dependencies" : [],
      "jobType" : "PASSIVE",
      "state" : "Running",
      "assigned_network_address" : [
         "moltres-02"
      ],
      "command" : "sleep 365d",
      "submissionTime" : 1752824505,
      "name" : null,
      "Job_Id" : 37030,
      "stderr_file" : "OAR.37030.stderr",
      "queue" : "default",
      "launchingDirectory" : "/home/diogo464",
      "reservation" : "None",
      "properties" : "(( ( dedicated='NO' OR dedicated='protocol-labs' OR dedicated='tardis' )) AND desktop_computing = 'NO') AND drain='NO'",
      "message" : "R=64,W=1:0:0,J=B (Karma=0.106,quota_ok)",
      "stdout_file" : "OAR.37030.stdout",
      "resubmit_job_id" : 0,
      "types" : [],
      "cpuset_name" : "diogo464_37030",
      "array_index" : 1,
      "project" : "default",
      "array_id" : 37030,
      "owner" : "diogo464",
      "startTime" : 1752824506,
      "assigned_resources" : [
         893,
         894,
         895,
         896,
         897,
         898,
         899,
         900,
         901,
         902,
         903,
         904,
         905,
         906,
         907,
         908,
         909,
         910,
         911,
         912,
         913,
         914,
         915,
         916,
         917,
         918,
         919,
         920,
         921,
         922,
         923,
         924,
         925,
         926,
         927,
         928,
         929,
         930,
         931,
         932,
         933,
         934,
         935,
         936,
         937,
         938,
         939,
         940,
         941,
         942,
         943,
         944,
         945,
         946,
         947,
         948,
         949,
         950,
         951,
         952,
         953,
         954,
         955,
         956
      ]
   },
   "37029" : {
      "command" : "sleep 365d",
      "submissionTime" : 1752824490,
      "dependencies" : [],
      "jobType" : "PASSIVE",
      "state" : "Running",
      "assigned_network_address" : [
         "moltres-01"
      ],
      "Job_Id" : 37029,
      "stderr_file" : "OAR.37029.stderr",
      "name" : null,
      "types" : [],
      "cpuset_name" : "diogo464_37029",
      "launchingDirectory" : "/home/diogo464",
      "queue" : "default",
      "reservation" : "None",
      "message" : "R=64,W=1:0:0,J=B (Karma=0.106,quota_ok)",
      "stdout_file" : "OAR.37029.stdout",
      "properties" : "(( ( dedicated='NO' OR dedicated='protocol-labs' OR dedicated='tardis' )) AND desktop_computing = 'NO') AND drain='NO'",
      "resubmit_job_id" : 0,
      "startTime" : 1752824491,
      "assigned_resources" : [
         829,
         830,
         831,
         832,
         833,
         834,
         835,
         836,
         837,
         838,
         839,
         840,
         841,
         842,
         843,
         844,
         845,
         846,
         847,
         848,
         849,
         850,
         851,
         852,
         853,
         854,
         855,
         856,
         857,
         858,
         859,
         860,
         861,
         862,
         863,
         864,
         865,
         866,
         867,
         868,
         869,
         870,
         871,
         872,
         873,
         874,
         875,
         876,
         877,
         878,
         879,
         880,
         881,
         882,
         883,
         884,
         885,
         886,
         887,
         888,
         889,
         890,
         891,
         892
      ],
      "array_index" : 1,
      "project" : "default",
      "array_id" : 37029,
      "owner" : "diogo464"
   }
}
"#;

    #[test]
    fn test_extract_job_ids_from_oarstat_output() {
        let job_ids = extract_job_ids_from_oarstat_output(OAR_STAT_ALL_USER_JOBS_OUTPUT).unwrap();
        assert_eq!(job_ids.len(), 2);
        assert!(job_ids.contains(&37030));
        assert!(job_ids.contains(&37029));
    }
}
