# agent-wallet

Solana wallet CLI for AI agents. Makes HTTP requests and automatically handles [MPP](https://paymentauth.org) (HTTP 402 Payment Required) payment flows.

## Install

```bash
cargo install agent-wallet
```

The binary is called `aw`.

## Quick Start

```bash
# Set up a wallet
aw wallet new

# Check balances
aw balance

# Make a request to an MPP-enabled API
aw GET https://api.example.com/resource --max-cost 0.01

# Dry run (see the cost without paying)
aw GET https://api.example.com/resource --max-cost 0.01 --dry-run
```

## How It Works

1. `aw` sends your HTTP request
2. If the server responds with `402 Payment Required` and an MPP challenge, `aw` parses it
3. Checks the price against your budget (`--max-cost`)
4. Sends a Solana payment (USDC, SOL, or any SPL token)
5. Retries the request with a payment credential
6. Prints the response body to stdout

## Commands

### HTTP Requests

```bash
aw GET <url> [-H "Header: Value"] [--max-cost X] [--dry-run] [--json]
aw POST <url> [body] [-H "Header: Value"] [--max-cost X] [--dry-run] [--json]
aw PUT <url> [body] [-H "Header: Value"] [--max-cost X] [--dry-run] [--json]
aw DELETE <url> [-H "Header: Value"] [--max-cost X] [--dry-run] [--json]
```

### Wallet

```bash
aw wallet              # Show public key
aw wallet new          # Generate a new keypair
aw wallet import <path>  # Import an existing keypair
```

### Balance

```bash
aw balance             # Show SOL and USDC balances
aw balance --json      # {"sol":0.142,"usdc":4.23}
```

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `AW_KEYPAIR` | Path to Solana keypair JSON | `~/.config/solana/id.json` |
| `AW_RPC_URL` | Solana RPC endpoint | `https://api.mainnet-beta.solana.com` |
| `AW_MAX_COST` | Default per-call budget (human units) | None (required per-call) |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | HTTP error (non-2xx, non-402) |
| `2` | Price exceeded budget (`--max-cost`) |
| `3` | Insufficient token balance |
| `4` | Config or wallet error |

## Output

- **stdout**: Response body only. Nothing else.
- **stderr**: Payment info, errors, diagnostics.

Each paid request emits one JSON line to stderr:
```json
{"paid":0.001,"currency":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","recipient":"ABC...","signature":"5xK..."}
```

## Agent Usage

```bash
# Set environment, call in a loop
export AW_KEYPAIR=/path/to/key.json
export AW_MAX_COST=0.01
export AW_RPC_URL=https://my-rpc.example.com

RESPONSE=$(aw GET https://api.example.com/data 2>/dev/null)
```

Works with any server implementing the [MPP 402 protocol](https://paymentauth.org).
