use eyre::Result;

use crate::machine::Machine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionNode {
    Frontend,
    Machine(Machine),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub node: ExecutionNode,
    job_id: Option<u32>,
    infer_job_id: bool,
    frontend_hostname: Option<String>,
    cluster_username: Option<String>
}

impl Context {
    pub async fn new(
        job_id: Option<u32>,
        infer_job_id: bool,
        frontend_hostname: Option<String>,
        cluster_username: Option<String>
    ) -> Result<Self> {
        Ok(Self {
            node: get_execution_node().await?,
            job_id,
            infer_job_id,
            frontend_hostname,
            cluster_username
        })
    }

    pub async fn job_id(&self) -> Result<u32> {
        tracing::debug!("obtaining job id");
        if let Some(job_id) = self.job_id {
            tracing::debug!("job id was set, using {job_id}");
            Ok(job_id)
        } else if self.infer_job_id {
            tracing::debug!("job id was not set but inference is enabled, finding job id");
            let job_ids = crate::oar::list_user_job_ids(self).await?;
            match job_ids.len() {
                0 => Err(eyre::eyre!("cannot infer job id, no jobs are running")),
                1 => Ok(job_ids[0]),
                _ => Err(eyre::eyre!(
                    "cannot infer job id, multiple jobs are running"
                )),
            }
        } else {
            tracing::debug!("inference was disabled and job id is not set");
            Err(eyre::eyre!("missing job id"))
        }
    }

    pub fn frontend_hostname(&self) -> Result<&str> {
        self.frontend_hostname
            .as_deref()
            .ok_or_else(|| eyre::eyre!("missing frontend hostname"))
    }

    pub fn cluster_username(&self) -> Result<&str> {
        self.cluster_username
            .as_deref()
            .ok_or_else(|| eyre::eyre!("missing cluster username"))
    }
}

async fn get_execution_node() -> Result<ExecutionNode> {
    let hostname = get_hostname().await;
    let node = match hostname.as_str() {
        "frontend" => ExecutionNode::Frontend,
        _ => match Machine::from_hostname(&hostname) {
            Some(machine) => ExecutionNode::Machine(machine),
            _ => ExecutionNode::Unknown,
        },
    };
    Ok(node)
}

async fn get_hostname() -> String {
    let hostname = if let Ok(hostname) = tokio::fs::read_to_string("/etc/hostname").await {
        hostname
    } else if let Ok(hostname) = std::env::var("HOSTNAME") {
        hostname
    } else {
        tracing::warn!("unable to obtain hostname, using empty string");
        String::default()
    };
    hostname.trim().to_string()
}
