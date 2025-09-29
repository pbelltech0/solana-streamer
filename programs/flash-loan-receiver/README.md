# Flash Loan Receiver Program

This is a Solana program (smart contract) that receives flash loans and executes arbitrage strategies on Raydium CLMM pools.

## Overview

The flash loan receiver program is designed to:
- Receive flash loans from lending protocols (e.g., Solend)
- Execute arbitrage trades across Raydium CLMM pools
- Repay the flash loan with interest
- Keep the profit

## Architecture

This program is intentionally kept separate from the main `solana-streamer-sdk` crate because:
- It uses Anchor framework which depends on older Solana SDK versions
- It will be deployed independently to Solana Mainnet
- It doesn't need to be compiled with the main binary

## Building

This program uses the Anchor framework. To build it:

### Prerequisites

Install Anchor CLI (version 0.30.1+):
```bash
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install 0.30.1
avm use 0.30.1
```

### Build the program

```bash
cd programs/flash-loan-receiver
anchor build
```

The compiled program will be in `target/deploy/flash_loan_receiver.so`

### Deploy to Devnet (for testing)

```bash
anchor deploy --provider.cluster devnet
```

### Deploy to Mainnet

⚠️ **WARNING**: Only deploy to mainnet after thorough testing on devnet!

```bash
# Set your keypair
export ANCHOR_WALLET=~/.config/solana/id.json

# Deploy
anchor deploy --provider.cluster mainnet
```

## Testing

```bash
anchor test
```

## Program Structure

- `src/lib.rs` - Main program logic
  - `receive_flash_loan` - Entry point called by flash loan protocol
  - `execute_arbitrage_strategy` - Implements the arbitrage logic
  - `swap_on_raydium_clmm` - CPI calls to Raydium CLMM

## TODOs

This is a foundational implementation. Before production use:

- [ ] Implement actual Raydium CLMM CPI calls
- [ ] Add comprehensive error handling
- [ ] Implement slippage protection
- [ ] Add transaction simulation
- [ ] Add proper account validation
- [ ] Write comprehensive tests
- [ ] Get security audit
- [ ] Add emergency pause mechanism
- [ ] Implement profit withdrawal mechanism

## Integration with Streamer SDK

The main `solana-streamer-sdk` crate detects arbitrage opportunities and submits transactions that call this program. See the `flash_loan` module in the main crate for the off-chain components:

- `opportunity_detector.rs` - Analyzes streaming events for opportunities
- `transaction_builder.rs` - Builds and submits flash loan transactions

## Resources

- [Anchor Documentation](https://www.anchor-lang.com/)
- [Raydium CLMM SDK](https://github.com/raydium-io/raydium-clmm)
- [Solend Flash Loans](https://github.com/solendprotocol/solana-program-library)
- [Flash Loan Integration Strategy](../../flash_loan_integration_strategy.md)