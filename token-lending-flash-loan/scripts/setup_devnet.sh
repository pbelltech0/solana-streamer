#!/bin/bash

# Setup script for deploying and testing flash loan program on devnet/testnet
# Usage: ./scripts/setup_devnet.sh [devnet|testnet|localnet]

set -e

NETWORK=${1:-devnet}
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "==================================="
echo "Flash Loan Program Setup - $NETWORK"
echo "==================================="
echo ""

# Configure Solana CLI for the network
case $NETWORK in
    devnet)
        solana config set --url https://api.devnet.solana.com
        ;;
    testnet)
        solana config set --url https://api.testnet.solana.com
        ;;
    localnet)
        solana config set --url http://localhost:8899
        ;;
    *)
        echo "Unknown network: $NETWORK"
        echo "Usage: $0 [devnet|testnet|localnet]"
        exit 1
        ;;
esac

echo "✓ Network set to: $NETWORK"
echo ""

# Check/create wallet
if [ ! -f ~/.config/solana/id.json ]; then
    echo "Creating new wallet..."
    solana-keygen new --no-bip39-passphrase
else
    echo "Using existing wallet: $(solana address)"
fi

WALLET=$(solana address)
echo ""

# Airdrop SOL if on devnet/testnet
if [ "$NETWORK" != "localnet" ]; then
    echo "Requesting airdrop..."
    solana airdrop 2 || echo "Airdrop failed - you may need to wait or use a faucet"
    echo ""
fi

# Check balance
BALANCE=$(solana balance)
echo "Wallet balance: $BALANCE"
echo ""

# Build programs
echo "Building programs..."
cd "$ROOT_DIR"
cargo build-bpf --manifest-path programs/lending/Cargo.toml
cargo build-bpf --manifest-path programs/example-receiver/Cargo.toml
echo "✓ Programs built"
echo ""

# Deploy lending program
echo "Deploying lending program..."
LENDING_PROGRAM=$(solana program deploy target/deploy/token_lending_flash_loan.so --output json | jq -r '.programId')
echo "✓ Lending program deployed: $LENDING_PROGRAM"
echo ""

# Deploy receiver program
echo "Deploying receiver program..."
RECEIVER_PROGRAM=$(solana program deploy target/deploy/flash_loan_example_receiver.so --output json | jq -r '.programId')
echo "✓ Receiver program deployed: $RECEIVER_PROGRAM"
echo ""

# Create test token mint
echo "Creating test token mint..."
TEST_MINT=$(spl-token create-token --decimals 6 --output json | jq -r '.commandOutput.address')
echo "✓ Test mint created: $TEST_MINT"
echo ""

# Create token accounts
echo "Creating token accounts..."
SUPPLY_ACCOUNT=$(spl-token create-account $TEST_MINT --output json | jq -r '.commandOutput.address')
echo "✓ Supply account: $SUPPLY_ACCOUNT"

BORROWER_ACCOUNT=$(spl-token create-account $TEST_MINT --output json | jq -r '.commandOutput.address')
echo "✓ Borrower account: $BORROWER_ACCOUNT"

FEE_RECEIVER=$(spl-token create-account $TEST_MINT --output json | jq -r '.commandOutput.address')
echo "✓ Fee receiver: $FEE_RECEIVER"
echo ""

# Mint test tokens
echo "Minting test tokens..."
spl-token mint $TEST_MINT 10000 $SUPPLY_ACCOUNT
echo "✓ Minted 10000 tokens to supply account"
echo ""

# Save configuration
CONFIG_FILE="$ROOT_DIR/.env.$NETWORK"
cat > "$CONFIG_FILE" << EOF
# Flash Loan Test Configuration - $NETWORK
# Generated: $(date)

NETWORK=$NETWORK
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

# Display summary
echo "==================================="
echo "Setup Complete!"
echo "==================================="
echo ""
echo "Network:          $NETWORK"
echo "Lending Program:  $LENDING_PROGRAM"
echo "Receiver Program: $RECEIVER_PROGRAM"
echo "Test Mint:        $TEST_MINT"
echo "Supply Account:   $SUPPLY_ACCOUNT"
echo ""
echo "Next steps:"
echo "1. Initialize lending market and reserve (see TESTING.md)"
echo "2. Run test flash loan transaction"
echo "3. Monitor on explorer:"
if [ "$NETWORK" = "devnet" ]; then
    echo "   https://explorer.solana.com/address/$LENDING_PROGRAM?cluster=devnet"
elif [ "$NETWORK" = "testnet" ]; then
    echo "   https://explorer.solana.com/address/$LENDING_PROGRAM?cluster=testnet"
else
    echo "   http://localhost:8899"
fi
echo ""