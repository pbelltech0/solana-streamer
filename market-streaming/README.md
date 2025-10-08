# Market Streaming - DEX Pool Monitoring Service

Real-time DEX pool state monitoring via Yellowstone gRPC. This subcrate provides both a library and standalone service for monitoring liquidity pool states across multiple DEX protocols on Solana.

## ⚡ Quick Start (30 seconds)

```bash
cd market-streaming

# 1. Setup and configure
make setup              # Creates .env file
nano .env               # Add your GRPC_ENDPOINT and POOL_PUBKEYS

# 2. Run
make run                # Starts the service

# Done! You're now streaming pool data
```

**Alternative:** Use `./run.sh` or `just run` instead of `make run`

---

## 📚 Documentation

- **[COMMANDS.md](COMMANDS.md)** - Complete command reference for all tools
- **[BUILD_TOOLS_SUMMARY.md](BUILD_TOOLS_SUMMARY.md)** - Comparison of Makefile, run.sh, just, and cargo
- **[INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md)** - Step-by-step integration with your RPC
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** - Solutions for common issues

---

## Supported DEX Protocols

- **Raydium CLMM** - Concentrated Liquidity Market Maker
- **Orca Whirlpool** - Concentrated liquidity pools
- **Meteora DLMM** - Dynamic Liquidity Market Maker
- **Crema Finance** - Concentrated liquidity
- **DefiTuna** - Automated market maker

## Features

- ✅ Real-time pool state streaming via Yellowstone gRPC
- ✅ Thread-safe pool state caching with staleness detection
- ✅ Support for multiple DEX protocols
- ✅ Configurable commitment levels
- ✅ Standalone service with CLI
- ✅ Library for integration into other projects

## Installation

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
market-streaming = { path = "path/to/market-streaming" }
```

### As a Standalone Service

Build the binary:

```bash
cd market-streaming
cargo build --release --bin market-streaming-service
```

## Usage

### Quick Start Options

You can run the service using any of these methods:

#### **Option 1: Using the Makefile (Recommended)**

```bash
# First-time setup
make setup          # Create .env file
# Edit .env with your configuration

# Build and run
make run            # Run in debug mode
make run-release    # Run optimized build
make run-debug      # Run with debug logging

# Other useful commands
make help           # Show all available commands
make status         # Show current configuration
make quick-start    # Interactive setup guide
```

#### **Option 2: Using the run.sh Script**

```bash
# Make executable (first time only)
chmod +x run.sh

# Run with options
./run.sh                # Run in debug mode
./run.sh --release      # Run optimized build
./run.sh --debug        # Run with debug logging
./run.sh --help         # Show all options
```

#### **Option 3: Using Just (Modern Alternative to Make)**

```bash
# Install just: https://github.com/casey/just
# brew install just (macOS)
# cargo install just (Cross-platform)

just setup          # First-time setup
just run            # Build and run
just run-release    # Optimized build
just status         # Show configuration
```

#### **Option 4: Using Environment Variables (Manual)

```bash
export GRPC_ENDPOINT="https://grpc.mainnet.solana.tools:443"
export GRPC_AUTH_TOKEN="your-token-here"  # Optional
export POOL_PUBKEYS="8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj,HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ"
export DEX_PROTOCOLS="raydium,orca,meteora"
export COMMITMENT_LEVEL="processed"

cargo run --release --bin market-streaming-service
```

#### Using Command Line Arguments

```bash
cargo run --release --bin market-streaming-service -- \
  --endpoint https://grpc.mainnet.solana.tools:443 \
  --token your-token-here \
  --pools 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj,HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ \
  --protocols raydium,orca,meteora \
  --commitment processed \
  --stats-interval 10
```

#### Command Line Options

```
Options:
  -e, --endpoint <ENDPOINT>
          Yellowstone gRPC endpoint [env: GRPC_ENDPOINT=]
          [default: https://grpc.mainnet.solana.tools:443]

  -t, --token <AUTH_TOKEN>
          Optional authentication token [env: GRPC_AUTH_TOKEN=]

  -p, --pools <POOLS>
          Pool pubkeys to monitor (comma-separated) [env: POOL_PUBKEYS=]

      --protocols <PROTOCOLS>
          DEX protocols to monitor: raydium, orca, meteora
          [env: DEX_PROTOCOLS=] [default: raydium,orca,meteora]

  -c, --commitment <COMMITMENT>
          Commitment level: processed, confirmed, or finalized
          [env: COMMITMENT_LEVEL=] [default: processed]

  -s, --stats-interval <STATS_INTERVAL>
          Interval for printing statistics (seconds)
          [env: STATS_INTERVAL=] [default: 10]

      --cache-max-age <CACHE_MAX_AGE>
          Maximum age for cached pool states (milliseconds)
          [env: CACHE_MAX_AGE=] [default: 5000]

  -h, --help
          Print help

  -V, --version
          Print version
```

### Using as a Library

```rust
use market_streaming::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use yellowstone_grpc_proto::prelude::CommitmentLevel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize rustls
    let _ = rustls::crypto::ring::default_provider().install_default().ok();

    // Create state cache
    let state_cache = Arc::new(PoolStateCache::new());

    // Configure streaming
    let config = StreamConfig {
        grpc_endpoint: "https://grpc.mainnet.solana.tools:443".to_string(),
        auth_token: None,
        pool_pubkeys: vec![
            Pubkey::from_str("8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj")?,
        ],
        protocols: vec![
            DexProtocol::RaydiumClmm,
            DexProtocol::OrcaWhirlpool,
        ],
        commitment: CommitmentLevel::Processed,
    };

    // Create and start client
    let client = PoolStreamClient::new(config, state_cache.clone());

    // Access pool states from cache
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            for (pubkey, cached) in state_cache.get_all_fresh() {
                println!("Pool {}: Price={:.8}, Liquidity={}",
                    pubkey,
                    cached.state.get_price(),
                    cached.state.get_liquidity()
                );
            }
        }
    });

    client.start().await?;
    Ok(())
}
```

## Configuration

### Yellowstone gRPC Endpoints

You need access to a Yellowstone gRPC endpoint. Here are some options:

#### 1. Public Endpoints (Free)

```bash
# Public endpoint (may have rate limits)
https://grpc.mainnet.solana.tools:443
```

#### 2. Paid RPC Providers

- **Helius**: https://docs.helius.dev/solana-rpc-nodes/geyser-enhanced-websockets
- **Triton**: https://triton.one/
- **QuickNode**: https://www.quicknode.com/
- **RunNode**: https://runnode.com/

#### 3. Self-Hosted

Run your own Solana validator with the yellowstone-grpc plugin:

```bash
git clone https://github.com/rpcpool/yellowstone-grpc.git
cd yellowstone-grpc

# Follow setup instructions in the repository
solana-validator --geyser-plugin-config yellowstone-grpc-geyser/config.json
```

### Finding Pool Addresses

To monitor specific pools, you need their on-chain addresses:

#### Raydium CLMM
- API: https://api-v3.raydium.io/pools/info/list
- Explorer: https://raydium.io/clmm/pools

#### Orca Whirlpool
- SDK: https://github.com/orca-so/whirlpools
- Explorer: https://www.orca.so/pools

#### Meteora DLMM
- API: https://dlmm-api.meteora.ag/pair/all
- Explorer: https://app.meteora.ag/pools

## Examples

See the `examples/` directory for more usage examples:

```bash
# Run the pool monitoring example
cargo run --example monitor_pools
```

## Architecture

```
market-streaming/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── pool_states.rs      # DEX pool state definitions
│   ├── state_cache.rs      # Thread-safe caching layer
│   ├── stream_client.rs    # gRPC streaming client
│   └── bin/
│       └── service.rs      # Standalone service binary
├── examples/
│   └── monitor_pools.rs    # Example usage
└── Cargo.toml
```

## Integration with Main Project

The main `solana-streamer-sdk` project already has yellowstone-grpc integration. You can use `market-streaming` for dedicated pool monitoring or integrate it with existing streaming logic:

```rust
use solana_streamer_sdk::streaming::YellowstoneGrpc;
use market_streaming::prelude::*;

// Use both together for comprehensive monitoring
let grpc_client = YellowstoneGrpc::new(endpoint, token)?;
let pool_monitor = PoolStreamClient::new(config, cache)?;
```

## Performance

- Supports thousands of concurrent pool subscriptions
- Sub-second update latency from on-chain changes
- Configurable cache staleness detection (default: 5 seconds)
- Efficient concurrent access via DashMap

## Troubleshooting

### Connection Issues

If you can't connect to the gRPC endpoint:

1. Verify the endpoint URL is correct
2. Check if authentication token is required and valid
3. Ensure rustls crypto provider is initialized
4. Check firewall settings for port 443

### No Updates Received

If the stream connects but receives no updates:

1. Verify pool addresses are correct
2. Check if pools exist and are active
3. Ensure the correct program IDs for each protocol
4. Try a different commitment level

### Authentication Errors

Some endpoints require authentication:

```bash
export GRPC_AUTH_TOKEN="your-token-here"
```

Contact your RPC provider for token details.

## License

MIT

## Contributing

See the main project README for contribution guidelines.
