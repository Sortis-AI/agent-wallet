use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address;

use crate::config::Config;
use crate::error::AwError;
use crate::wallet;

const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

pub fn show(config: &Config) -> Result<(), AwError> {
    let keypair = wallet::load_keypair(&config.keypair_path)?;
    let pubkey = solana_sdk::signer::Signer::pubkey(&keypair);
    let client = RpcClient::new(&config.rpc_url);

    let sol_balance = client
        .get_balance(&pubkey)
        .map_err(|e| AwError::Config(format!("RPC call failed [{}]: {e}", config.rpc_url)))?;
    let sol = sol_balance as f64 / 1_000_000_000.0;

    let usdc_mint = Pubkey::from_str(USDC_MINT)
        .map_err(|e| AwError::Config(format!("invalid USDC mint: {e}")))?;
    let ata = get_associated_token_address(&pubkey, &usdc_mint);
    let usdc = match client.get_token_account_balance(&ata) {
        Ok(balance) => balance.ui_amount.unwrap_or(0.0),
        Err(_) => 0.0,
    };

    if config.json_output {
        println!(r#"{{"sol":{sol},"usdc":{usdc}}}"#);
    } else {
        println!("SOL:  {sol:.3}");
        println!("USDC: {usdc:.3}");
    }

    Ok(())
}
