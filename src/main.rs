#![feature(exit_status_error)]

use std::{collections::HashMap, path::PathBuf};

use clap::Parser;
use eyre::Result;
use serde::Deserialize;
use tokio::process::Command;

const MACHINES: &'static [(&'static str, Option<&'static str>)] = &[
    ("alakazam-01", None),
    ("alakazam-02", None),
    ("alakazam-03", None),
    ("alakazam-04", None),
    ("alakazam-05", None),
    ("alakazam-06", None),
    ("alakazam-07", None),
    ("alakazam-08", None),
    ("bulbasaur-1", None),
    ("bulbasaur-2", None),
    ("bulbasaur-3", None),
    ("charmander-1", Some("bond0")),
    ("charmander-2", Some("bond0")),
    ("charmander-3", Some("bond0")),
    ("charmander-4", Some("bond0")),
    ("charmander-5", Some("bond0")),
    ("gengar-1", None),
    ("gengar-2", None),
    ("gengar-3", None),
    ("gengar-4", None),
    ("gengar-5", None),
    ("kadabra-01", None),
    ("kadabra-02", None),
    ("kadabra-03", None),
    ("kadabra-04", None),
    ("kadabra-05", None),
    ("kadabra-06", None),
    ("kadabra-07", None),
    ("kadabra-08", None),
    ("lugia-1", None),
    ("lugia-2", None),
    ("lugia-3", None),
    ("lugia-4", None),
    ("lugia-5", None),
    ("magikarp-1", None),
    ("moltres-01", None),
    ("moltres-02", None),
    ("moltres-03", None),
    ("moltres-04", None),
    ("moltres-05", None),
    ("moltres-06", None),
    ("moltres-07", None),
    ("moltres-08", None),
    ("moltres-09", None),
    ("moltres-10", None),
    ("oddish-1", None),
    ("psyduck-1", None),
    ("psyduck-2", None),
    ("psyduck-3", None),
    ("shelder-1", None),
    ("squirtle-1", None),
    ("squirtle-2", None),
    ("squirtle-3", None),
    ("squirtle-4", None),
    ("staryu-1", None),
    ("sudowoodo-1", None),
    ("vulpix-1", None),
];

#[derive(Debug, Parser)]
struct Args {
    job_id: u32,
    addresses: usize,
    latency_matrix: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let machines = oar_network_addresses(args.job_id).await?;
    let addresses_per_machine = (args.addresses + machines.len() - 1) / machines.len();

    let mut machine_addresses = Vec::default();

    for (machine_idx, _machine) in machines.iter().enumerate() {
        let mut addresses = Vec::default();
        for addr_idx in 0..addresses_per_machine {
            let ip = address_from_index(machine_idx * addresses_per_machine + addr_idx);
            addresses.push(ip);
        }

        machine_addresses.push(addresses);
    }

    println!("{machines:#?}");

    Ok(())
}

fn address_from_index(address: usize) -> String {
    let d = address % 254;
    let c = (address / 254) % 254;
    assert!(c <= 254);
    format!("10.0.{}.{}", c, d + 1)
}

async fn clear_addresses(_machine: &str) -> Result<()> {
    Ok(())
}

async fn oar_network_addresses(job_id: u32) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct JobSchema {
        assigned_network_address: Vec<String>,
    }

    let output = Command::new("oarstat")
        .arg("-j")
        .arg(job_id.to_string())
        .arg("-J")
        .output()
        .await?
        .exit_ok()?;
    let mut output_map = serde_json::from_slice::<HashMap<String, JobSchema>>(&output.stdout)?;
    let job_key = job_id.to_string();
    let job = output_map
        .remove(&job_key)
        .ok_or(eyre::eyre!("TODO: better message"))?;
    Ok(job.assigned_network_address)
}
