use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aw",
    version,
    about = "Agent wallet — MPP-aware HTTP client for Solana"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Solana keypair path
    #[arg(long, global = true, env = "AW_KEYPAIR")]
    pub keypair: Option<PathBuf>,

    /// Max cost per call in human-readable units
    #[arg(long, global = true, env = "AW_MAX_COST")]
    pub max_cost: Option<f64>,

    /// Solana RPC endpoint
    #[arg(long, global = true, env = "AW_RPC_URL")]
    pub rpc_url: Option<String>,

    /// JSON output mode
    #[arg(long, global = true)]
    pub json: bool,

    /// Show payment details without paying
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Make an MPP-aware HTTP GET request
    #[command(name = "GET", alias = "get")]
    Get {
        url: String,
        #[arg(long, short = 'H', num_args = 1)]
        header: Vec<String>,
    },
    /// Make an MPP-aware HTTP POST request
    #[command(name = "POST", alias = "post")]
    Post {
        url: String,
        body: Option<String>,
        #[arg(long, short = 'H', num_args = 1)]
        header: Vec<String>,
    },
    /// Make an MPP-aware HTTP PUT request
    #[command(name = "PUT", alias = "put")]
    Put {
        url: String,
        body: Option<String>,
        #[arg(long, short = 'H', num_args = 1)]
        header: Vec<String>,
    },
    /// Make an MPP-aware HTTP DELETE request
    #[command(name = "DELETE", alias = "delete")]
    Delete {
        url: String,
        #[arg(long, short = 'H', num_args = 1)]
        header: Vec<String>,
    },
    /// Show wallet balances (SOL + USDC)
    Balance,
    /// Print agent-wallet skill install instructions
    Skill,
    /// Wallet management
    Wallet {
        #[command(subcommand)]
        action: Option<WalletAction>,
    },
}

#[derive(Subcommand)]
pub enum WalletAction {
    /// Generate a new keypair
    New,
    /// Import a keypair from a file
    Import { path: PathBuf },
}
