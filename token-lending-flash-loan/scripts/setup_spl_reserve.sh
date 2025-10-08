#!/bin/bash

# Setup SPL Token Lending Reserve for Flash Loans
# Uses official spl-token-lending-cli

set -e

NETWORK=${1:-devnet}

echo "========================================"
echo "SPL Token Lending Reserve Setup"
echo "Network: $NETWORK"
echo "========================================"
echo ""

# Configure network
case $NETWORK in
    devnet)
        solana config set --url https://api.devnet.solana.com
        PROGRAM_ID="TokenLending1111111111111111111111111111111"
        ;;
    testnet)
        solana config set --url https://api.testnet.solana.com
        PROGRAM_ID="TokenLending1111111111111111111111111111111"
        ;;
    localnet)
        solana config set --url http://localhost:8899
        PROGRAM_ID="TokenLending1111111111111111111111111111111"
        ;;
    *)
        echo "Usage: $0 [devnet|testnet|localnet]"
        exit 1
        ;;
esac

echo "✓ Network: $NETWORK"
echo "✓ Lending Program: $PROGRAM_ID"
echo ""

# Check wallet
WALLET=$(solana address)
echo "Wallet: $WALLET"
BALANCE=$(solana balance)
echo "Balance: $BALANCE"
echo ""

# Airdrop if needed
if [ "$NETWORK" != "mainnet" ]; then
    echo "Requesting airdrop..."
    solana airdrop 2 || echo "Airdrop may have failed"
    echo ""
fi

# Create test token
echo "Creating test token..."
TEST_MINT=$(spl-token create-token --decimals 6 2>&1 | grep "Creating token" | awk '{print $3}')
echo "✓ Test mint: $TEST_MINT"
echo ""

# Create token account
echo "Creating token account..."
TOKEN_ACCOUNT=$(spl-token create-account $TEST_MINT 2>&1 | grep "Creating account" | awk '{print $3}')
echo "✓ Token account: $TOKEN_ACCOUNT"
echo ""

# Mint tokens
echo "Minting tokens..."
spl-token mint $TEST_MINT 100000 $TOKEN_ACCOUNT
echo "✓ Minted 100,000 tokens"
echo ""

# Create lending market
echo "Creating lending market..."
MARKET_OUTPUT=$(spl-token-lending create-market 2>&1)
MARKET=$(echo "$MARKET_OUTPUT" | grep -oE '[1-9A-HJ-NP-Za-km-z]{32,44}' | head -1)

if [ -z "$MARKET" ]; then
    echo "❌ Failed to create market"
    echo "$MARKET_OUTPUT"
    exit 1
fi

echo "✓ Lending market: $MARKET"
echo ""

# Add reserve
echo "Adding reserve to market..."
echo "  Market: $MARKET"
echo "  Token: $TEST_MINT"
echo "  Initial liquidity: 10,000"
echo ""

# Note: Pyth oracles are required. Using placeholder for testing
PYTH_PRODUCT="PLACEHOLDER11111111111111111111111111"
PYTH_PRICE="PLACEHOLDER11111111111111111111111111"

RESERVE_OUTPUT=$(spl-token-lending add-reserve \
  --market $MARKET \
  --source $TOKEN_ACCOUNT \
  --amount 10000 \
  --pyth-product $PYTH_PRODUCT \
  --pyth-price $PYTH_PRICE 2>&1)

RESERVE=$(echo "$RESERVE_OUTPUT" | grep "Reserve address" | awk '{print $3}')

if [ -z "$RESERVE" ]; then
    # Try alternate parsing
    RESERVE=$(echo "$RESERVE_OUTPUT" | grep -oE '[1-9A-HJ-NP-Za-km-z]{32,44}' | tail -1)
fi

if [ -z "$RESERVE" ]; then
    echo "❌ Failed to create reserve"
    echo "$RESERVE_OUTPUT"
    exit 1
fi

echo "✓ Reserve created: $RESERVE"
echo ""

# Get reserve info
echo "Reserve information:"
spl-token-lending show-reserve $RESERVE || echo "Could not fetch reserve info"
echo ""

# Save configuration
CONFIG_FILE=".env.spl_${NETWORK}"
cat > "$CONFIG_FILE" << EOF
# SPL Token Lending Configuration - $NETWORK
# Generated: $(date)

NETWORK=$NETWORK
WALLET=$WALLET
LENDING_PROGRAM=$PROGRAM_ID
TEST_MINT=$TEST_MINT
TOKEN_ACCOUNT=$TOKEN_ACCOUNT
LENDING_MARKET=$MARKET
RESERVE=$RESERVE
EOF

echo "✓ Configuration saved to: $CONFIG_FILE"
echo ""

echo "========================================"
echo "Setup Complete!"
echo "========================================"
echo ""
echo "Summary:"
echo "  Network:        $NETWORK"
echo "  Lending Market: $MARKET"
echo "  Reserve:        $RESERVE"
echo "  Token Mint:     $TEST_MINT"
echo "  Liquidity:      10,000 tokens"
echo ""
echo "Next steps:"
echo "1. Deploy your flash loan receiver program"
echo "2. Execute flash loan transactions against this reserve"
echo "3. Monitor on explorer:"

if [ "$NETWORK" = "devnet" ]; then
    echo "   https://explorer.solana.com/address/$RESERVE?cluster=devnet"
elif [ "$NETWORK" = "testnet" ]; then
    echo "   https://explorer.solana.com/address/$RESERVE?cluster=testnet"
fi

echo ""
echo "⚠️  Note: This uses placeholder Pyth oracles for testing"
echo "    For production, use real Pyth price feeds"
echo ""