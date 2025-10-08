# Flash Loan Testing Guide

This guide covers testing the flash loan program from local development to mainnet deployment.

## Table of Contents

1. [Local Testing (Recommended First)](#local-testing)
2. [Devnet Testing](#devnet-testing)
3. [Testnet Testing](#testnet-testing)
4. [Mainnet Preparation](#mainnet-preparation)
5. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Required Tools

```bash
# Rust and Cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/stable/install)"

# Node.js (for TypeScript client)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18

# TypeScript
npm install -g typescript ts-node

# SPL Token CLI
cargo install spl-token-cli
```

### Verify Installation

```bash
solana --version
cargo --version
spl-token --version
node --version
```

---

## Local Testing

Local testing with `solana-test-validator` is the fastest way to iterate and debug.

### Step 1: Start Local Validator

```bash
# In a separate terminal window
solana-test-validator
```

Keep this running throughout your testing session.

### Step 2: Configure Solana CLI for Localnet

```bash
solana config set --url http://localhost:8899
```

### Step 3: Create Test Wallet

```bash
# Create new wallet (or use existing)
solana-keygen new --no-bip39-passphrase -o test-wallet.json

# Set as default
solana config set --keypair test-wallet.json

# Airdrop test SOL
solana airdrop 10
```

### Step 4: Build Programs

```bash
cd token-lending-flash-loan
./build.sh
```

Expected output:
```
✓ Build complete!

Programs:
  - Lending: target/deploy/token_lending_flash_loan.so
  - Receiver: target/deploy/flash_loan_example_receiver.so
```

### Step 5: Deploy Programs

```bash
# Deploy lending program
solana program deploy target/deploy/token_lending_flash_loan.so

# Deploy receiver program
solana program deploy target/deploy/flash_loan_example_receiver.so
```

Save the program IDs from the output.

### Step 6: Initialize Test Environment

```bash
# Create test token mint
TEST_MINT=$(spl-token create-token --decimals 6 | grep "Creating token" | awk '{print $3}')
echo "Test Mint: $TEST_MINT"

# Create token accounts
SUPPLY_ACCOUNT=$(spl-token create-account $TEST_MINT | grep "Creating account" | awk '{print $3}')
BORROWER_ACCOUNT=$(spl-token create-account $TEST_MINT | grep "Creating account" | awk '{print $3}')
FEE_RECEIVER=$(spl-token create-account $TEST_MINT | grep "Creating account" | awk '{print $3}')

# Mint tokens to supply account
spl-token mint $TEST_MINT 10000 $SUPPLY_ACCOUNT
```

### Step 7: Initialize Lending Market (Manual)

Since the program doesn't have an init instruction yet, you'll need to manually create and write the account data:

```bash
# Create lending market account
solana-keygen new --no-bip39-passphrase -o lending-market.json
LENDING_MARKET=$(solana-keygen pubkey lending-market.json)

# Derive authority PDA
# You'll need to calculate this using the program
# See scripts/calculate_pda.ts for helper
```

**Note:** For full testing, you should add initialization instructions to the program. See [Adding Init Instructions](#adding-init-instructions) below.

### Step 8: Run Integration Tests

```bash
# Run Rust integration tests
cargo test-bpf

# Or specific test
cargo test-bpf test_flash_loan_basic_flow
```

### Step 9: Test with Client Script

```bash
# Install dependencies
npm install @solana/web3.js @solana/spl-token dotenv

# Create configuration
cat > .env.localnet << EOF
NETWORK=localnet
LENDING_PROGRAM=YourLendingProgramId
RECEIVER_PROGRAM=YourReceiverProgramId
TEST_MINT=$TEST_MINT
SUPPLY_ACCOUNT=$SUPPLY_ACCOUNT
BORROWER_ACCOUNT=$BORROWER_ACCOUNT
FEE_RECEIVER=$FEE_RECEIVER
EOF

# Run test
ts-node scripts/test_flash_loan.ts localnet
```

---

## Devnet Testing

Devnet is a public testnet that closely mimics mainnet behavior.

### Quick Setup

```bash
# Automated setup
./scripts/setup_devnet.sh devnet
```

This script will:
- ✓ Configure Solana CLI for devnet
- ✓ Create/use wallet
- ✓ Request SOL airdrop
- ✓ Build and deploy programs
- ✓ Create test token accounts
- ✓ Save configuration to `.env.devnet`

### Manual Setup

If you prefer manual setup:

```bash
# 1. Configure for devnet
solana config set --url https://api.devnet.solana.com

# 2. Create wallet
solana-keygen new

# 3. Airdrop SOL
solana airdrop 2

# 4. Build programs
./build.sh

# 5. Deploy programs
LENDING_PROGRAM=$(solana program deploy target/deploy/token_lending_flash_loan.so --output json | jq -r '.programId')
RECEIVER_PROGRAM=$(solana program deploy target/deploy/flash_loan_example_receiver.so --output json | jq -r '.programId')

echo "Lending: $LENDING_PROGRAM"
echo "Receiver: $RECEIVER_PROGRAM"
```

### Testing on Devnet

```bash
# Use the client script
ts-node scripts/test_flash_loan.ts devnet

# View transaction on explorer
# The script will output the explorer link
```

### Monitoring on Devnet

```bash
# Watch program logs
solana logs $LENDING_PROGRAM

# Check account info
solana account $LENDING_PROGRAM

# View recent transactions
solana transaction-history
```

---

## Testnet Testing

Testnet is more stable than devnet and closer to mainnet conditions.

### Setup

```bash
./scripts/setup_devnet.sh testnet
```

Or manually:

```bash
solana config set --url https://api.testnet.solana.com
# Follow same steps as devnet
```

### Important Notes

- **Airdrops may be limited** - Use faucets if needed
- **More stable** than devnet, less resets
- **Better for long-term testing** and demos
- **Rate limits** similar to mainnet

---

## Mainnet Preparation

Before deploying to mainnet, ensure you've completed thorough testing.

### Pre-Deployment Checklist

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Tested on localnet extensively
- [ ] Tested on devnet for at least 1 week
- [ ] Tested on testnet for at least 1 week
- [ ] Security audit completed (recommended)
- [ ] Program IDs are properly set
- [ ] Emergency procedures documented
- [ ] Monitoring/alerting configured
- [ ] Initial liquidity secured
- [ ] Fee parameters validated

### Security Considerations

1. **Audit the Code**
   - Consider professional audit (Neodyme, OtterSec, Sec3, etc.)
   - Review all arithmetic for overflow/underflow
   - Validate all PDAs and account ownership checks
   - Test edge cases exhaustively

2. **Program Upgrade Authority**
   ```bash
   # Deploy with upgrade authority
   solana program deploy target/deploy/token_lending_flash_loan.so

   # Later, you can make it immutable
   solana program set-upgrade-authority <PROGRAM_ID> --final
   ```

3. **Start Small**
   - Launch with limited liquidity
   - Monitor closely for first week
   - Gradually increase caps

### Mainnet Deployment

```bash
# 1. Configure for mainnet
solana config set --url https://api.mainnet-beta.solana.com

# 2. Ensure wallet has sufficient SOL
solana balance
# Need ~5-10 SOL for deployment

# 3. Build with optimizations
cargo build-bpf --release

# 4. Deploy
solana program deploy target/deploy/token_lending_flash_loan.so

# 5. Verify deployment
solana program show <PROGRAM_ID>
```

### Monitoring Mainnet

```bash
# Real-time logs
solana logs <PROGRAM_ID>

# Set up alerts (example with custom script)
while true; do
    # Check for errors
    solana logs <PROGRAM_ID> --limit 100 | grep "Error" && \
        notify-send "Flash Loan Error Detected"
    sleep 60
done
```

---

## Adding Init Instructions

The current implementation is missing initialization instructions. Here's how to add them:

### 1. Add InitLendingMarket Instruction

```rust
// In src/instruction.rs
pub enum LendingInstruction {
    InitLendingMarket {
        owner: Pubkey,
        quote_currency: [u8; 32],
    },
    InitReserve {
        liquidity_amount: u64,
        flash_loan_fee_bps: u64,
        protocol_fee_bps: u64,
    },
    FlashLoan {
        amount: u64,
    },
}
```

### 2. Implement Processors

```rust
// In src/processor.rs
impl Processor {
    pub fn process_init_lending_market(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        owner: Pubkey,
        quote_currency: [u8; 32],
    ) -> ProgramResult {
        // Implementation
    }

    pub fn process_init_reserve(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        config: ReserveConfig,
    ) -> ProgramResult {
        // Implementation
    }
}
```

### 3. Add Helper Script

```typescript
// scripts/init_market.ts
async function initLendingMarket() {
  const data = Buffer.alloc(1 + 32 + 32);
  data.writeUInt8(0, 0); // InitLendingMarket tag
  // ... write owner and quote currency

  const ix = new TransactionInstruction({
    keys: [/* accounts */],
    programId: LENDING_PROGRAM,
    data,
  });

  await sendAndConfirmTransaction(connection, new Transaction().add(ix), [wallet]);
}
```

---

## Troubleshooting

### Program Deployment Issues

**Error: "Insufficient funds"**
```bash
# Check balance
solana balance

# Airdrop more (devnet/testnet)
solana airdrop 2

# Or transfer from another wallet
solana transfer <WALLET> 2
```

**Error: "Program account not found"**
```bash
# Verify program is deployed
solana program show <PROGRAM_ID>

# Redeploy if needed
solana program deploy target/deploy/token_lending_flash_loan.so
```

### Transaction Failures

**Error: "Custom program error: 0x1"**

This typically means `InvalidInstruction`. Check:
- Instruction data format is correct
- Account order matches expected
- All required accounts are included

```bash
# Enable detailed logs
export RUST_LOG=solana_runtime::system_instruction_processor=trace
solana logs <PROGRAM_ID>
```

**Error: "Flash loan not repaid"**

The receiver didn't return enough tokens:
- Check fee calculation
- Verify receiver has enough tokens after operations
- Ensure transfer instruction succeeds

### Account Issues

**Error: "InvalidAccountData"**
```bash
# Check account exists and has correct size
solana account <ACCOUNT_PUBKEY>

# Verify account owner
# Should be your program for reserve/market accounts
```

**Error: "InvalidAccountOwner"**
```bash
# Verify PDA derivation
# Check bump seed matches expected
# Ensure using correct seeds for PDA
```

### Testing Issues

**Integration tests fail with "BanksClient error"**
```bash
# Clean and rebuild
cargo clean
cargo build-bpf

# Run tests with verbose output
cargo test-bpf -- --nocapture
```

### Performance Testing

```bash
# Measure transaction size
solana transaction-size <TRANSACTION_FILE>

# Should be < 1232 bytes

# Measure compute units
# Add logging in program:
msg!("Compute units consumed: {}",
    compute_budget::get_compute_unit_price());
```

---

## Best Practices

### 1. Always Test Locally First
```bash
# Quick iteration cycle
solana-test-validator &
cargo build-bpf && solana program deploy ... && cargo test-bpf
```

### 2. Use Separate Wallets
```bash
# Dev wallet
solana-keygen new -o wallets/dev.json

# Test wallet
solana-keygen new -o wallets/test.json

# Production wallet (hardware wallet recommended)
```

### 3. Version Control Deployments
```bash
# Tag deployments
git tag -a v1.0.0-devnet -m "Devnet deployment"
git push --tags

# Track program IDs
echo "v1.0.0: F1ashLending..." >> DEPLOYMENTS.md
```

### 4. Monitor Gas Costs
```bash
# Log deployment costs
solana program deploy ... 2>&1 | tee deploy.log

# Track over time for budgeting
```

### 5. Automated Testing
```bash
# CI/CD with GitHub Actions
# See .github/workflows/test.yml example
```

---

## Resources

- **Solana Cookbook**: https://solanacookbook.com
- **Program Examples**: https://github.com/solana-labs/solana-program-library
- **Discord Support**: https://discord.gg/solana
- **Explorer (Devnet)**: https://explorer.solana.com/?cluster=devnet
- **Faucet**: https://solfaucet.com

---

## Next Steps

1. ✅ Complete local testing
2. ✅ Add initialization instructions
3. ✅ Deploy to devnet
4. ✅ Write comprehensive tests
5. ✅ Deploy to testnet
6. ✅ Security audit
7. ✅ Mainnet deployment

Happy testing! 🚀