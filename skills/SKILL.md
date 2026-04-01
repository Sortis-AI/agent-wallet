# agent-wallet (`aw`)

## What It Does

`aw` is a Solana wallet CLI that makes HTTP requests and automatically pays for them when the server requires it. If a server responds with HTTP 402 (Payment Required) using the [MPP protocol](https://paymentauth.org), `aw` parses the payment challenge, sends a Solana transaction (USDC, SOL, or any SPL token), and retries the request with a payment credential — all in one command.

It works with **any** MPP-enabled API. No accounts, no subscriptions, no API keys — just a Solana wallet and a budget cap.

## Install

```bash
cargo install agent-wallet
```

The binary is `aw`. Requires Rust — install via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` if needed.

## Setup

```bash
# Generate a new wallet (or use an existing Solana keypair)
aw wallet new --keypair ~/.config/solana/aw.json

# Fund the wallet with SOL (for tx fees) and USDC (for payments)
# Then verify:
aw balance --keypair ~/.config/solana/aw.json
```

Set environment variables to avoid repeating flags:

```bash
export AW_KEYPAIR=~/.config/solana/aw.json
export AW_MAX_COST=0.01
export AW_RPC_URL=https://api.mainnet-beta.solana.com
```

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `AW_KEYPAIR` | Path to Solana keypair JSON file | `~/.config/solana/id.json` |
| `AW_MAX_COST` | Max spend per request in human-readable units (e.g. `0.01` = 1 cent USDC) | None — must be set |
| `AW_RPC_URL` | Solana RPC endpoint | `https://api.mainnet-beta.solana.com` |

## Commands

### HTTP Requests

Make requests to any URL. If the server returns 402, `aw` handles payment automatically.

```bash
aw GET <url> [-H "Header: Value"...] [--max-cost <amount>] [--dry-run] [--json]
aw POST <url> [body] [-H "Header: Value"...] [--max-cost <amount>] [--dry-run] [--json]
aw PUT <url> [body] [-H "Header: Value"...] [--max-cost <amount>] [--dry-run] [--json]
aw DELETE <url> [-H "Header: Value"...] [--max-cost <amount>] [--dry-run] [--json]
```

Methods are case-insensitive (`get` and `GET` both work).

### Wallet

```bash
aw wallet                          # Print wallet public key
aw wallet new                      # Generate new keypair at default path
aw wallet new --keypair <path>     # Generate new keypair at specific path
aw wallet import <source-path>     # Import existing keypair to default path
```

### Balance

```bash
aw balance                         # Human-readable SOL + USDC
aw balance --json                  # {"sol":0.142,"usdc":4.23}
```

## Output Contract

This is critical for scripting and piping:

- **stdout** contains the HTTP response body and nothing else. No status messages, no payment info.
- **stderr** contains everything else: payment reporting, errors, dry-run output.

After each paid request, exactly one JSON line is emitted to stderr:

```json
{"paid":0.001,"currency":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","recipient":"Bx96...","signature":"x25S..."}
```

This means you can capture the response body cleanly:

```bash
RESPONSE=$(aw GET https://api.example.com/data 2>/dev/null)
```

Or track spending by capturing stderr:

```bash
aw GET https://api.example.com/data 2>/tmp/spend.log
```

## Exit Codes

| Code | Meaning | When |
|------|---------|------|
| `0` | Success | Request completed (2xx response) |
| `1` | HTTP error | Non-2xx response, or payment/network failure |
| `2` | Price exceeded budget | Server asked for more than `--max-cost` |
| `3` | Insufficient funds | Wallet balance too low for the payment |
| `4` | Config/wallet error | Missing keypair, bad RPC URL, etc. |

## Examples

### Basic request to a free endpoint

```bash
aw GET https://dns.sortis.dev/health
# stdout: {"status":"ok","timestamp":"2026-04-01T15:14:26.710Z","version":"0.1.0"}
# exit: 0
```

### Paid request with budget cap

```bash
aw GET https://dns.sortis.dev/zones --max-cost 0.01
# stdout: {"zones":[...]}
# stderr: {"paid":0.001,"currency":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","recipient":"Bx96...","signature":"x25S..."}
# exit: 0
```

### Check cost before paying (dry run)

```bash
aw GET https://dns.sortis.dev/zones --max-cost 1 --dry-run
# stderr: dry run: would pay 0.001 EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
#           recipient: Bx96Qf1t2vmrX5jBSAoNs1CtQiVRTUiNcEDxTYDDnTF
#           description: agent-dns: GET /zones
# exit: 0
```

### Budget exceeded (safe fail)

```bash
aw GET https://dns.sortis.dev/zones --max-cost 0.0000001
# stderr: error: price exceeds budget: requested 0.001 EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v, budget 0.0000001
# exit: 2
```

### POST with body and headers

```bash
aw POST https://dns.sortis.dev/zones '{"name":"example.com"}' \
  -H "Content-Type: application/json" \
  --max-cost 10
```

### Pipe response into another tool

```bash
aw GET https://dns.sortis.dev/zones --max-cost 0.01 2>/dev/null | jq '.zones[].name'
```

### Check balance before a series of calls

```bash
aw balance --json 2>/dev/null
# {"sol":0.1,"usdc":8.393}
```

### Use in a script with environment variables

```bash
export AW_KEYPAIR=/path/to/agent-key.json
export AW_MAX_COST=0.05
export AW_RPC_URL=https://my-rpc.example.com

# Now every call uses these defaults
ZONES=$(aw GET https://dns.sortis.dev/zones 2>/dev/null)
echo "$ZONES" | jq '.zones[] | .name'
```

### Track total spend across multiple calls

```bash
aw GET https://api.example.com/a --max-cost 0.01 2>>/tmp/spend.log
aw GET https://api.example.com/b --max-cost 0.01 2>>/tmp/spend.log
aw GET https://api.example.com/c --max-cost 0.01 2>>/tmp/spend.log

# Sum all payments
grep -o '"paid":[0-9.]*' /tmp/spend.log | cut -d: -f2 | paste -sd+ | bc
```

### Probe cost of an endpoint without spending

```bash
aw GET https://dns.sortis.dev/zones --dry-run --max-cost 999 --json 2>&1 >/dev/null
# stderr: {"dry_run":true,"cost":0.001,"currency":"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v","recipient":"Bx96...","description":"agent-dns: GET /zones"}
```

## How MPP Payment Works (Under the Hood)

You don't need to understand this to use `aw` — it handles everything automatically. This is here for debugging.

1. `aw` sends your HTTP request normally
2. If the server returns `402 Payment Required` with a `WWW-Authenticate: Payment ...` header, `aw` parses the challenge
3. The challenge contains: amount (in base units), currency (mint address or `"sol"`), recipient (Solana pubkey), and payment details
4. `aw` converts the amount to human-readable units and checks against `--max-cost`
5. If within budget, `aw` sends a Solana transaction (SPL `TransferChecked` for tokens, `system_instruction::transfer` for SOL)
6. After on-chain confirmation, `aw` builds a credential containing the challenge echo + transaction signature
7. `aw` retries the original request with `Authorization: Payment <base64url-credential>`
8. The server verifies the payment on-chain and returns the response

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| `error: config error: 402 received but no --max-cost set` | Server requires payment but no budget set | Set `AW_MAX_COST` or pass `--max-cost` |
| `error: wallet error: keypair not found` | No keypair at the configured path | Run `aw wallet new` or set `AW_KEYPAIR` |
| `error: insufficient funds` | Wallet doesn't have enough tokens | Fund the wallet address shown by `aw wallet` |
| `error: payment error: transaction failed` | RPC or network issue | Check `AW_RPC_URL`, try a different RPC endpoint |
| Exit code 2 | Server price exceeds your budget | Increase `--max-cost` or use `--dry-run` to check the price first |
