#!/bin/bash

# Pyth-Enhanced Arbitrage Detector - Startup Script
# This script helps set up and run the pyth_enhanced_arbitrage example

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║  PYTH-ENHANCED ARBITRAGE DETECTOR - SETUP                ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo

# Check if environment variables are set
if [ -z "$YELLOWSTONE_ENDPOINT" ] || [ -z "$YELLOWSTONE_TOKEN" ]; then
    echo "⚠️  Missing required environment variables!"
    echo
    echo "Please choose a provider:"
    echo "1) Triton One (recommended - free tier available)"
    echo "2) Local Yellowstone node"
    echo "3) Manual configuration"
    echo
    read -p "Enter choice (1-3): " choice

    case $choice in
        1)
            echo
            echo "📝 Setting up for Triton One..."
            export YELLOWSTONE_ENDPOINT="https://grpc.triton.one:443"
            echo "✓ Endpoint set to: $YELLOWSTONE_ENDPOINT"
            echo
            echo "You need an API token from https://triton.one"
            read -p "Enter your Triton API token: " token
            export YELLOWSTONE_TOKEN="$token"
            echo "✓ Token configured"
            ;;
        2)
            echo
            echo "📝 Setting up for local Yellowstone node..."
            export YELLOWSTONE_ENDPOINT="http://localhost:10000"
            echo "✓ Endpoint set to: $YELLOWSTONE_ENDPOINT"
            echo "✓ No token needed for local node"
            ;;
        3)
            echo
            echo "📝 Manual configuration..."
            read -p "Enter Yellowstone endpoint URL: " endpoint
            export YELLOWSTONE_ENDPOINT="$endpoint"
            read -p "Enter API token (or press Enter if none): " token
            if [ ! -z "$token" ]; then
                export YELLOWSTONE_TOKEN="$token"
            fi
            ;;
        *)
            echo "Invalid choice. Exiting."
            exit 1
            ;;
    esac
fi

# Optional: Set RPC endpoints
if [ -z "$SOLANA_RPC_URL" ]; then
    export SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"
    echo "✓ Using default Solana RPC: $SOLANA_RPC_URL"
fi

if [ -z "$PYTH_RPC_URL" ]; then
    export PYTH_RPC_URL="http://pythnet.rpcpool.com"
    echo "✓ Using default Pyth RPC: $PYTH_RPC_URL"
fi

echo
echo "📊 Configuration Summary:"
echo "   YELLOWSTONE_ENDPOINT: $YELLOWSTONE_ENDPOINT"
echo "   YELLOWSTONE_TOKEN: ${YELLOWSTONE_TOKEN:0:10}..."
echo "   SOLANA_RPC_URL: $SOLANA_RPC_URL"
echo "   PYTH_RPC_URL: $PYTH_RPC_URL"
echo

echo "🚀 Starting Pyth-Enhanced Arbitrage Detector..."
echo

# Run the example with environment variables
RUST_LOG=info cargo run --example pyth_enhanced_arbitrage