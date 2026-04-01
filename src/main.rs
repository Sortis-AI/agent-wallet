mod balance;
mod cli;
mod config;
mod error;
mod http;
mod mpp;
mod payment;
mod wallet;

use clap::Parser;

use cli::{Cli, Command};
use error::AwError;

fn main() {
    let cli = Cli::parse();

    let config = match config::resolve(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(e.exit_code());
        }
    };

    let result = run(&cli, &config);

    if let Err(e) = result {
        let code = e.exit_code();
        if config.json_output {
            eprintln!(r#"{{"error":"{}"}}"#, e);
        } else {
            eprintln!("error: {e}");
        }
        std::process::exit(code);
    }
}

fn run(cli: &Cli, config: &config::Config) -> Result<(), AwError> {
    match &cli.command {
        Command::Get { .. }
        | Command::Post { .. }
        | Command::Put { .. }
        | Command::Delete { .. } => http::execute_request(&cli.command, config),
        Command::Balance => balance::show(config),
        Command::Skill => {
            print!("{}", include_str!("../skills/agent-wallet/SKILL.md"));
            Ok(())
        }
        Command::Wallet { action } => match action {
            None => wallet::show_pubkey(&config.keypair_path),
            Some(cli::WalletAction::New) => wallet::new_keypair(cli.keypair.as_deref()),
            Some(cli::WalletAction::Import { path }) => wallet::import_keypair(path),
        },
    }
}
