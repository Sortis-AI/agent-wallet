use std::path::PathBuf;

use crate::cli::Cli;
use crate::error::AwError;

const DEFAULT_RPC_URL: &str = "https://api.mainnet-beta.solana.com";

pub struct Config {
    pub keypair_path: PathBuf,
    pub rpc_url: String,
    pub max_cost: Option<f64>,
    pub dry_run: bool,
    pub json_output: bool,
}

pub fn resolve(cli: &Cli) -> Result<Config, AwError> {
    let keypair_path = match &cli.keypair {
        Some(p) => p.clone(),
        None => default_keypair_path()?,
    };

    let rpc_url = cli
        .rpc_url
        .clone()
        .unwrap_or_else(|| DEFAULT_RPC_URL.to_string());

    Ok(Config {
        keypair_path,
        rpc_url,
        max_cost: cli.max_cost,
        dry_run: cli.dry_run,
        json_output: cli.json,
    })
}

fn default_keypair_path() -> Result<PathBuf, AwError> {
    let home = std::env::var("HOME")
        .map_err(|_| AwError::Config("HOME environment variable not set".into()))?;
    Ok(PathBuf::from(home).join(".config/solana/id.json"))
}
