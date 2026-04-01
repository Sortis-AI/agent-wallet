use std::str::FromStr;

use solana_client::rpc_client::RpcClient;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use solana_sdk::signer::keypair::Keypair;
#[allow(deprecated)]
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::instruction::transfer_checked;

use crate::error::AwError;
use crate::mpp::PaymentRequest;

pub fn send_payment(
    keypair: &Keypair,
    request: &PaymentRequest,
    challenge_id: &str,
    rpc_url: &str,
) -> Result<String, AwError> {
    if request.currency == "sol" {
        send_sol(keypair, request, challenge_id, rpc_url)
    } else {
        send_spl(keypair, request, challenge_id, rpc_url)
    }
}

fn send_spl(
    keypair: &Keypair,
    request: &PaymentRequest,
    challenge_id: &str,
    rpc_url: &str,
) -> Result<String, AwError> {
    let mint = Pubkey::from_str(&request.currency).map_err(|e| {
        AwError::Payment(format!("invalid mint address '{}': {e}", request.currency))
    })?;
    let recipient = Pubkey::from_str(&request.recipient)
        .map_err(|e| AwError::Payment(format!("invalid recipient '{}': {e}", request.recipient)))?;
    let amount: u64 = request
        .amount
        .parse()
        .map_err(|e| AwError::Payment(format!("invalid amount '{}': {e}", request.amount)))?;

    let sender = keypair.pubkey();
    let token_program_id = match &request.method_details.token_program {
        Some(tp) => Pubkey::from_str(tp)
            .map_err(|e| AwError::Payment(format!("invalid token program '{tp}': {e}")))?,
        None => spl_token::id(),
    };
    let sender_ata = get_associated_token_address(&sender, &mint);
    let recipient_ata = get_associated_token_address(&recipient, &mint);

    let client = RpcClient::new(rpc_url);

    // Pre-flight balance check
    let sender_balance =
        client
            .get_token_account_balance(&sender_ata)
            .map_err(|_| AwError::InsufficientFunds {
                needed: amount as f64 / 10f64.powi(request.method_details.decimals as i32),
                available: 0.0,
                currency: request.currency.clone(),
            })?;
    let available: u64 = sender_balance.amount.parse().unwrap_or(0);
    if available < amount {
        return Err(AwError::InsufficientFunds {
            needed: amount as f64 / 10f64.powi(request.method_details.decimals as i32),
            available: available as f64 / 10f64.powi(request.method_details.decimals as i32),
            currency: request.currency.clone(),
        });
    }

    let mut instructions = Vec::new();

    // Create recipient ATA if needed (idempotent — safe even if it exists)
    instructions.push(create_associated_token_account_idempotent(
        &sender,
        &recipient,
        &mint,
        &token_program_id,
    ));

    // TransferChecked
    instructions.push(
        transfer_checked(
            &token_program_id,
            &sender_ata,
            &mint,
            &recipient_ata,
            &sender,
            &[],
            amount,
            request.method_details.decimals,
        )
        .map_err(|e| AwError::Payment(format!("failed to build transfer instruction: {e}")))?,
    );

    // Memo (optional — don't fail if it errors)
    if let Ok(memo_ix) =
        std::panic::catch_unwind(|| spl_memo::build_memo(challenge_id.as_bytes(), &[&sender]))
    {
        instructions.push(memo_ix);
    }

    let blockhash = resolve_blockhash(&client)?;
    let tx =
        Transaction::new_signed_with_payer(&instructions, Some(&sender), &[keypair], blockhash);

    let signature = client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| AwError::Payment(format!("transaction failed [{}]: {e}", rpc_url)))?;

    Ok(signature.to_string())
}

fn send_sol(
    keypair: &Keypair,
    request: &PaymentRequest,
    challenge_id: &str,
    rpc_url: &str,
) -> Result<String, AwError> {
    let recipient = Pubkey::from_str(&request.recipient)
        .map_err(|e| AwError::Payment(format!("invalid recipient '{}': {e}", request.recipient)))?;
    let lamports: u64 = request
        .amount
        .parse()
        .map_err(|e| AwError::Payment(format!("invalid amount '{}': {e}", request.amount)))?;

    let sender = keypair.pubkey();
    let client = RpcClient::new(rpc_url);

    // Pre-flight balance check (need lamports + fee headroom)
    let balance = client
        .get_balance(&sender)
        .map_err(|e| AwError::Payment(format!("RPC call failed [{}]: {e}", rpc_url)))?;
    let fee_headroom = 10_000; // ~0.00001 SOL for fees
    if balance < lamports + fee_headroom {
        return Err(AwError::InsufficientFunds {
            needed: lamports as f64 / 1_000_000_000.0,
            available: balance as f64 / 1_000_000_000.0,
            currency: "sol".to_string(),
        });
    }

    let mut instructions = vec![system_instruction::transfer(&sender, &recipient, lamports)];

    if let Ok(memo_ix) =
        std::panic::catch_unwind(|| spl_memo::build_memo(challenge_id.as_bytes(), &[&sender]))
    {
        instructions.push(memo_ix);
    }

    let blockhash = resolve_blockhash(&client)?;
    let tx =
        Transaction::new_signed_with_payer(&instructions, Some(&sender), &[keypair], blockhash);

    let signature = client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| AwError::Payment(format!("transaction failed [{}]: {e}", rpc_url)))?;

    Ok(signature.to_string())
}

fn resolve_blockhash(client: &RpcClient) -> Result<Hash, AwError> {
    // Always fetch a fresh blockhash. The challenge's recentBlockhash is for pull mode
    // (server co-sign); in push mode (client pays) it's likely stale by the time we build the tx.
    client
        .get_latest_blockhash()
        .map_err(|e| AwError::Payment(format!("failed to get blockhash: {e}")))
}
