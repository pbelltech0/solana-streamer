# SPL Token Lending Flash Loan Implementation

This package implements the [SPL Token Lending Flash Loan specification](https://github.com/solana-labs/solana-program-library/blob/master/token-lending/flash_loan_design.md) as a deployable Solana program package.

## Overview

Flash loans allow users to borrow assets without collateral, as long as the loan is repaid within the same transaction. This implementation provides:

1. **Lending Program** (`programs/lending`) - Issues flash loans from reserve liquidity
2. **Example Receiver** (`programs/example-receiver`) - Demonstrates how to receive and use flash loans

## Quick Start

**Testing:**
```bash
# Local testing (recommended first)
./scripts/test_local.sh

# Deploy to devnet
./scripts/setup_devnet.sh devnet

# See TESTING.md for comprehensive guide
```

**Building:**
```bash
./build.sh
```

For detailed testing instructions, see **[TESTING.md](TESTING.md)**.

## Architecture

### Flash Loan Flow

```
1. User calls FlashLoan instruction on lending program
2. Lending program transfers tokens to receiver
3. Lending program calls receiver program's ReceiveFlashLoan
4. Receiver program executes custom logic (trades, arbitrage, etc.)
5. Receiver program repays loan + fees
6. Lending program verifies repayment
7. Fees are distributed to protocol and host
```

### Lending Program

The lending program implements the `FlashLoan` instruction with the following accounts:

| Index | Account | Writable | Description |
|-------|---------|----------|-------------|
| 0 | Source liquidity | ✓ | Reserve's token account |
| 1 | Destination liquidity | ✓ | Borrower's token account |
| 2 | Reserve | ✓ | Reserve state account |
| 3 | Lending market | | Lending market state |
| 4 | Market authority | | Derived authority PDA |
| 5 | Receiver program | | Flash loan receiver program ID |
| 6 | Token program | | SPL Token program |
| 7 | Fee receiver | ✓ | Protocol fee destination |
| 8 | Host fee receiver | ✓ | Optional host fee destination |
| 9+ | Additional accounts | | Passed to receiver program |

### Receiver Program Interface

Receiver programs must implement instruction tag `0` (ReceiveFlashLoan):

```rust
// Instruction format: [0, amount_le_bytes]
pub fn receive_flash_loan(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    // 1. Verify received tokens
    // 2. Execute custom logic (arbitrage, liquidation, etc.)
    // 3. Repay loan + fees
    Ok(())
}
```

**Requirements:**
- Instruction tag must be `0`
- Must accept `amount: u64` parameter
- Must repay `amount + fees` to source liquidity account
- All operations must complete in the same transaction

## Building

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Add Solana to PATH
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
```

### Build Programs

```bash
cd token-lending-flash-loan

# Build all programs
cargo build-bpf

# Or build individually
cargo build-bpf --manifest-path programs/lending/Cargo.toml
cargo build-bpf --manifest-path programs/example-receiver/Cargo.toml
```

Build artifacts will be in `target/deploy/`:
- `token_lending_flash_loan.so` - Lending program
- `flash_loan_example_receiver.so` - Example receiver program

## Deployment

### 1. Deploy Programs

```bash
# Deploy lending program
solana program deploy target/deploy/token_lending_flash_loan.so

# Deploy example receiver
solana program deploy target/deploy/flash_loan_example_receiver.so
```

### 2. Initialize Lending Market

Create a lending market account and initialize it:

```rust
// Example initialization (pseudo-code)
let lending_market = Keypair::new();
let (authority, bump_seed) = Pubkey::find_program_address(
    &[lending_market.pubkey().as_ref()],
    &program_id,
);

// Create and initialize lending market account
// Set owner, quote currency, etc.
```

### 3. Initialize Reserve

Create a reserve for each token you want to lend:

```rust
// Create reserve account
let reserve = Keypair::new();

// Initialize with:
// - Liquidity mint (token to lend)
// - Liquidity supply (token account holding reserves)
// - Flash loan fee (e.g., 9 bps = 0.09%)
// - Protocol fee percentage
```

### 4. Fund Reserve

Transfer tokens to the reserve's liquidity supply account to provide liquidity for flash loans.

## Usage

### Creating a Flash Loan Receiver

Implement the receiver interface in your program:

```rust
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
};

#[no_mangle]
pub extern "C" fn entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse instruction tag (must be 0)
    let (&tag, rest) = instruction_data.split_first().unwrap();
    if tag != 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse amount
    let amount = u64::from_le_bytes(rest[..8].try_into().unwrap());

    // Your custom logic here
    msg!("Received flash loan: {}", amount);

    // Execute trades, arbitrage, etc.
    execute_your_strategy(accounts, amount)?;

    // Repay loan + fees
    repay_flash_loan(accounts, amount)?;

    Ok(())
}
```

### Executing a Flash Loan

```rust
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    transaction::Transaction,
};

// Build flash loan instruction
let flash_loan_ix = Instruction {
    program_id: lending_program_id,
    accounts: vec![
        AccountMeta::new(source_liquidity, false),
        AccountMeta::new(destination_liquidity, false),
        AccountMeta::new(reserve, false),
        AccountMeta::new_readonly(lending_market, false),
        AccountMeta::new_readonly(market_authority, false),
        AccountMeta::new_readonly(receiver_program_id, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new(fee_receiver, false),
        // Add receiver program accounts...
    ],
    data: [12u8].iter() // Tag 12 for FlashLoan
        .chain(amount.to_le_bytes().iter())
        .copied()
        .collect(),
};

// Send transaction
let tx = Transaction::new_signed_with_payer(
    &[flash_loan_ix],
    Some(&payer.pubkey()),
    &[&payer],
    recent_blockhash,
);

client.send_and_confirm_transaction(&tx)?;
```

## Fee Structure

Flash loan fees are calculated as:

- **Flash Loan Fee**: Configurable per reserve (typically 0.09% = 9 bps)
- **Protocol Fee**: Percentage of flash loan fee (e.g., 20% of fee)
- **Host Fee**: Remainder after protocol fee (e.g., 80% of fee)

Example with 1000 tokens at 0.09% fee:
- Borrow: 1000 tokens
- Fee: 0.9 tokens (0.09%)
- Protocol Fee: 0.18 tokens (20% of fee)
- Host Fee: 0.72 tokens (80% of fee)
- Total Repayment: 1000.9 tokens

## Security Considerations

### For Lending Program Operators

1. **Set appropriate fees** - Too low may encourage unprofitable transactions
2. **Monitor liquidity** - Ensure sufficient reserves for legitimate uses
3. **Validate reserves** - Only add trusted tokens
4. **Access control** - Restrict reserve initialization to authorized accounts

### For Receiver Program Developers

1. **Verify repayment math** - Always calculate fees correctly
2. **Handle edge cases** - Account for rounding, slippage, etc.
3. **Atomic operations** - Ensure all steps succeed or fail together
4. **Test thoroughly** - Flash loan failures revert entire transaction
5. **Gas limits** - Be mindful of compute units

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests (requires local validator)
# Start validator in another terminal:
solana-test-validator

# Then run tests:
cargo test-bpf
```

## Example Use Cases

1. **Arbitrage**: Exploit price differences between DEXes
2. **Liquidations**: Liquidate under-collateralized positions without capital
3. **Collateral Swaps**: Change collateral type without closing position
4. **Refinancing**: Move positions between protocols for better rates
5. **Self-liquidation**: Close your own position before liquidation penalty

## Program IDs

Update these in the source code before deployment:

- **Lending Program**: `F1ashLend1ng111111111111111111111111111111`
- **Example Receiver**: `F1ashRecv1111111111111111111111111111111111`

Generate your own with:
```bash
solana-keygen grind --starts-with Flash:1
```

## Resources

- [SPL Token Lending Flash Loan Design](https://github.com/solana-labs/solana-program-library/blob/master/token-lending/flash_loan_design.md)
- [Solana Program Library](https://github.com/solana-labs/solana-program-library)
- [Solana Documentation](https://docs.solana.com/)

## License

MIT

## Contributing

Contributions are welcome! Please ensure:
- Code follows Rust best practices
- All tests pass
- Documentation is updated
- Security considerations are addressed

## Disclaimer

This is example code for educational purposes. Use at your own risk. Always audit smart contracts before deploying to mainnet with real funds.