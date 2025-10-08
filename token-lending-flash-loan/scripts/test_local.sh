#!/bin/bash

# Local Flash Loan Testing Script
# This script automates the complete local testing workflow

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "========================================"
echo "Flash Loan Local Test Runner"
echo "========================================"
echo ""

# Check if test validator is running
if ! curl -s http://localhost:8899 > /dev/null 2>&1; then
    echo "❌ Solana test validator not running!"
    echo ""
    echo "Please start it in another terminal:"
    echo "  solana-test-validator"
    echo ""
    exit 1
fi

echo "✓ Test validator is running"
echo ""

# Configure for localnet
echo "Configuring for localnet..."
solana config set --url http://localhost:8899 > /dev/null 2>&1
echo "✓ Configured for localnet"
echo ""

# Create test wallet if doesn't exist
WALLET_PATH="$ROOT_DIR/.test-wallet.json"
if [ ! -f "$WALLET_PATH" ]; then
    echo "Creating test wallet..."
    solana-keygen new --no-bip39-passphrase -o "$WALLET_PATH" > /dev/null 2>&1
    echo "✓ Test wallet created"
else
    echo "✓ Using existing test wallet"
fi

solana config set --keypair "$WALLET_PATH" > /dev/null 2>&1
WALLET=$(solana address)
echo "  Address: $WALLET"
echo ""

# Airdrop SOL
echo "Airdropping SOL..."
solana airdrop 10 > /dev/null 2>&1 || echo "Airdrop may have failed (already have balance?)"
BALANCE=$(solana balance)
echo "✓ Balance: $BALANCE"
echo ""

# Build programs
echo "Building programs..."
cd "$ROOT_DIR"
cargo build-bpf --manifest-path programs/lending/Cargo.toml > /dev/null 2>&1
cargo build-bpf --manifest-path programs/example-receiver/Cargo.toml > /dev/null 2>&1
echo "✓ Programs built"
echo ""

# Deploy programs
echo "Deploying programs..."
LENDING_PROGRAM=$(solana program deploy target/deploy/token_lending_flash_loan.so 2>&1 | grep "Program Id" | awk '{print $3}')
echo "✓ Lending program: $LENDING_PROGRAM"

RECEIVER_PROGRAM=$(solana program deploy target/deploy/flash_loan_example_receiver.so 2>&1 | grep "Program Id" | awk '{print $3}')
echo "✓ Receiver program: $RECEIVER_PROGRAM"
echo ""

# Create test token
echo "Creating test token..."
TEST_MINT=$(spl-token create-token --decimals 6 2>&1 | grep "Creating token" | awk '{print $3}')
echo "✓ Test mint: $TEST_MINT"
echo ""

# Create token accounts
echo "Creating token accounts..."
SUPPLY_ACCOUNT=$(spl-token create-account $TEST_MINT 2>&1 | grep "Creating account" | awk '{print $3}')
echo "  Supply account: $SUPPLY_ACCOUNT"

BORROWER_ACCOUNT=$(spl-token create-account $TEST_MINT 2>&1 | grep "Creating account" | awk '{print $3}')
echo "  Borrower account: $BORROWER_ACCOUNT"

FEE_RECEIVER=$(spl-token create-account $TEST_MINT 2>&1 | grep "Creating account" | awk '{print $3}')
echo "  Fee receiver: $FEE_RECEIVER"
echo ""

# Mint tokens
echo "Minting tokens..."
spl-token mint $TEST_MINT 10000 $SUPPLY_ACCOUNT > /dev/null 2>&1
echo "✓ Minted 10000 tokens to supply account"
echo ""

# Check token balances
echo "Token balances:"
spl-token balance $TEST_MINT --address $SUPPLY_ACCOUNT | xargs echo "  Supply:"
spl-token balance $TEST_MINT --address $BORROWER_ACCOUNT | xargs echo "  Borrower:"
spl-token balance $TEST_MINT --address $FEE_RECEIVER | xargs echo "  Fee receiver:"
echo ""

# Save configuration
CONFIG_FILE="$ROOT_DIR/.env.localnet"
cat > "$CONFIG_FILE" << EOF
# Local test configuration
# Generated: $(date)

NETWORK=localnet
WALLET=$WALLET
LENDING_PROGRAM=$LENDING_PROGRAM
RECEIVER_PROGRAM=$RECEIVER_PROGRAM
TEST_MINT=$TEST_MINT
SUPPLY_ACCOUNT=$SUPPLY_ACCOUNT
BORROWER_ACCOUNT=$BORROWER_ACCOUNT
FEE_RECEIVER=$FEE_RECEIVER
EOF

echo "✓ Configuration saved to: $CONFIG_FILE"
echo ""

# Run integration tests
echo "Running integration tests..."
echo ""
cargo test-bpf 2>&1 | tail -20
echo ""

# Summary
echo "========================================"
echo "Local Test Setup Complete!"
echo "========================================"
echo ""
echo "Configuration:"
echo "  Network:         localnet"
echo "  Wallet:          $WALLET"
echo "  Lending Program: $LENDING_PROGRAM"
echo "  Receiver:        $RECEIVER_PROGRAM"
echo "  Test Mint:       $TEST_MINT"
echo ""
echo "Next steps:"
echo "1. Review logs above for any errors"
echo "2. Run custom tests: cargo test-bpf"
echo "3. Try client script: ts-node scripts/test_flash_loan.ts localnet"
echo "4. Monitor logs: solana logs $LENDING_PROGRAM"
echo ""
echo "Note: Remember to initialize lending market and reserve accounts"
echo "      before executing flash loan transactions."
echo ""