use eyre::{Context as _, Result};

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
    frontend_hostname: Option<String>,
}

impl Context {
    pub async fn new(job_id: Option<u32>, frontend_hostname: Option<String>) -> Result<Self> {
        Ok(Self {
            node: get_execution_node().await?,
            job_id,
            frontend_hostname,
        })
    }

    pub fn job_id(&self) -> Result<u32> {
        self.job_id.ok_or_else(|| eyre::eyre!("missing job id"))
    }

    pub fn frontend_hostname(&self) -> Result<&str> {
        self.frontend_hostname
            .as_ref()
            .map(|s| s.as_str())
            .ok_or_else(|| eyre::eyre!("missing frontend hostname"))
    }
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
