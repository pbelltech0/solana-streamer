#!/bin/bash
# Market Streaming Service Runner
# This script helps you run the market streaming service with proper configuration

set -e

# Colors for output
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Parse command line arguments
BUILD_MODE="debug"
SKIP_BUILD=false
SHOW_HELP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--release)
            BUILD_MODE="release"
            shift
            ;;
        -s|--skip-build)
            SKIP_BUILD=true
            shift
            ;;
        -d|--debug)
            export RUST_LOG=debug
            shift
            ;;
        -h|--help)
            SHOW_HELP=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            SHOW_HELP=true
            shift
            ;;
    esac
done

if [ "$SHOW_HELP" = true ]; then
    echo -e "${CYAN}Market Streaming Service Runner${NC}\n"
    echo "Usage: ./run.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -r, --release      Build and run in release mode (optimized)"
    echo "  -s, --skip-build   Skip building, run existing binary"
    echo "  -d, --debug        Enable debug logging (RUST_LOG=debug)"
    echo "  -h, --help         Show this help message"
    echo ""
    echo "Examples:"
    echo "  ./run.sh                  # Run in debug mode"
    echo "  ./run.sh --release        # Run optimized build"
    echo "  ./run.sh --debug          # Run with debug logging"
    echo "  ./run.sh -r -s            # Run release build, skip rebuilding"
    exit 0
fi

echo -e "${GREEN}=== Market Streaming Service ===${NC}\n"

# Check if .env file exists
if [ ! -f .env ]; then
    echo -e "${YELLOW}Warning: .env file not found${NC}"
    echo "Creating .env from .env.example..."
    cp .env.example .env
    echo -e "${GREEN}✓ Created .env file${NC}"
    echo -e "${YELLOW}Opening .env in your default editor...${NC}"
    echo -e "${YELLOW}Please configure POOL_PUBKEYS and GRPC_ENDPOINT${NC}\n"
    ${EDITOR:-nano} .env
    echo ""
fi

# Load environment variables
if [ -f .env ]; then
    echo -e "${CYAN}Loading configuration from .env...${NC}"
    set -a
    source .env
    set +a
fi

# Validate required configuration
if [ -z "$GRPC_ENDPOINT" ]; then
    echo -e "${RED}✗ GRPC_ENDPOINT not set in .env${NC}"
    echo "Please set your Yellowstone gRPC endpoint."
    echo ""
    echo "Example:"
    echo "  GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443"
    exit 1
fi

if [ -z "$POOL_PUBKEYS" ]; then
    echo -e "${YELLOW}⚠ Warning: POOL_PUBKEYS not set in .env${NC}"
    echo "Service will run but won't monitor any pools."
    echo ""
    echo "Example pool addresses:"
    echo "  Raydium SOL/USDC: 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj"
    echo "  Orca SOL/USDC: HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ"
    echo ""
    read -p "Continue anyway? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Display configuration
echo -e "\n${GREEN}✓ Configuration:${NC}"
echo "  Endpoint: $GRPC_ENDPOINT"
echo "  Auth Token: ${GRPC_AUTH_TOKEN:+***SET***}${GRPC_AUTH_TOKEN:-Not set}"
if [ -n "$POOL_PUBKEYS" ]; then
    POOL_COUNT=$(echo $POOL_PUBKEYS | tr ',' '\n' | wc -l | tr -d ' ')
    echo "  Pools: $POOL_COUNT configured"
else
    echo "  Pools: None (warning)"
fi
echo "  Protocols: ${DEX_PROTOCOLS:-raydium,orca,meteora}"
echo "  Commitment: ${COMMITMENT_LEVEL:-processed}"
echo "  Build Mode: $BUILD_MODE"
echo "  Log Level: ${RUST_LOG:-info}"
echo ""

# Build if needed
if [ "$SKIP_BUILD" = false ]; then
    echo -e "${CYAN}Building service ($BUILD_MODE mode)...${NC}"
    if [ "$BUILD_MODE" = "release" ]; then
        cargo build --release --bin market-streaming-service
    else
        cargo build --bin market-streaming-service
    fi
    echo -e "${GREEN}✓ Build complete${NC}\n"
else
    echo -e "${YELLOW}Skipping build (--skip-build flag)${NC}\n"
fi

# Run the service
echo -e "${GREEN}Starting Market Streaming Service...${NC}"
echo -e "${YELLOW}Press Ctrl+C to stop${NC}\n"

# Set default log level if not set
export RUST_LOG=${RUST_LOG:-info}

# Run based on build mode
if [ "$BUILD_MODE" = "release" ]; then
    cargo run --release --bin market-streaming-service
else
    cargo run --bin market-streaming-service
fi
