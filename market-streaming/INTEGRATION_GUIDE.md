# Market Streaming Integration Guide

This guide shows how to integrate the market-streaming service with your RPC endpoint and use it in your existing Solana streaming project.

## Quick Start

### 1. Configure Your RPC Endpoint

```bash
cd market-streaming

# Copy the example environment file
cp .env.example .env

# Edit .env with your configuration
nano .env
```

Example `.env` configuration:

```bash
# Your Yellowstone gRPC endpoint
GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443

# Optional authentication token (if required by your provider)
GRPC_AUTH_TOKEN=your_token_here

# Pool addresses to monitor (comma-separated)
POOL_PUBKEYS=8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj,HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ

# Protocols to monitor
DEX_PROTOCOLS=raydium,orca,meteora

# Commitment level
COMMITMENT_LEVEL=processed
```

### 2. Run the Service

```bash
# Using the convenience script
./run.sh

# Or directly with cargo
cargo run --release --bin market-streaming-service
```

### 3. Test the Connection

The service will output logs showing:
- Connection status
- Pool updates received
- Cache statistics every 10 seconds

Expected output:

```
=== Market Streaming Service ===
Endpoint: https://grpc.mainnet.solana.tools:443
Auth Token: Set
Pools: 2
Protocols: Raydium CLMM, Orca Whirlpool, Meteora DLMM
Commitment: Processed
================================

Starting pool monitoring...

=== Cache Statistics ===
Total entries: 2
Fresh entries: 2
Stale entries: 0

--- Pool States ---
Pool: 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj
  Price: 0.00512345
  Liquidity: 1234567890
  Tokens: So11111111111111111111111111111111111111112 / EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
  Slot: 245678901
========================
```

## Integration with Main Project

### Option 1: Use as a Library

Integrate directly into your existing code:

```rust
use solana_streamer_sdk::streaming::YellowstoneGrpc;
use market_streaming::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Your existing streaming setup
    let grpc_client = YellowstoneGrpc::new(
        "https://grpc.mainnet.solana.tools:443".to_string(),
        None,
    )?;

    // Add pool monitoring
    let cache = Arc::new(PoolStateCache::new());
    let pool_config = StreamConfig {
        grpc_endpoint: "https://grpc.mainnet.solana.tools:443".to_string(),
        auth_token: None,
        pool_pubkeys: vec![/* your pools */],
        protocols: vec![DexProtocol::RaydiumClmm],
        commitment: yellowstone_grpc_proto::prelude::CommitmentLevel::Processed,
    };

    let pool_client = PoolStreamClient::new(pool_config, cache.clone());

    // Run both concurrently
    tokio::try_join!(
        async {
            // Your existing streaming logic
            Ok(())
        },
        async {
            pool_client.start().await
        }
    )?;

    Ok(())
}
```

### Option 2: Run as a Separate Service

Run the market-streaming service as a standalone microservice and communicate via shared state or IPC:

```bash
# Terminal 1: Run market streaming service
cd market-streaming
./run.sh

# Terminal 2: Run your main application
cd ..
cargo run --example your_app
```

### Option 3: Combine with Existing Arbitrage Logic

Integrate pool monitoring with your arbitrage detector:

```rust
use market_streaming::prelude::*;
use solana_streamer_sdk::streaming::arbitrage::*;

// Get fresh pool states for arbitrage calculations
let cache = Arc::new(PoolStateCache::new());

// In your arbitrage logic
for (pool_pubkey, cached_state) in cache.get_all_fresh() {
    let price = cached_state.state.get_price();
    let liquidity = cached_state.state.get_liquidity();

    // Use price and liquidity for arbitrage detection
    check_arbitrage_opportunity(pool_pubkey, price, liquidity)?;
}
```

## Finding Pool Addresses

### Raydium CLMM

```bash
# Using Raydium API
curl https://api-v3.raydium.io/pools/info/list | jq '.data.data[] | select(.programId == "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK")'
```

Popular Raydium pools:
- SOL/USDC: `8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj`
- SOL/USDT: `5r9wYBVqLmr5NJfvNiUqTWvPnJYHK7ZxeXAZJeXMvPnz`

### Orca Whirlpool

```bash
# Using Orca SDK or API
# Visit https://www.orca.so/pools for pool addresses
```

Popular Orca pools:
- SOL/USDC: `HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ`
- SOL/mSOL: `3ne4mWqdYuNiYrYZC9TrA3FcfuFdErghH97vNPbjicr1`

### Meteora DLMM

```bash
# Using Meteora API
curl https://dlmm-api.meteora.ag/pair/all | jq '.[] | select(.name | contains("SOL"))'
```

Popular Meteora pools:
- SOL/USDC: `ARwi1S4DaiTG5DX7S4M4ZsrXqpMD1MrTmbu9ue2tpmEq`

## RPC Provider Setup

### Using Helius

1. Sign up at https://www.helius.dev/
2. Create an API key
3. Use endpoint: `https://mainnet.helius-rpc.com`
4. Add your API key as `GRPC_AUTH_TOKEN`

### Using QuickNode

1. Sign up at https://www.quicknode.com/
2. Create a Solana endpoint with "Geyser" enabled
3. Use your dedicated endpoint URL
4. Add authentication token if provided

### Using Triton

1. Contact https://triton.one/ for access
2. Get your dedicated endpoint
3. Configure authentication as provided

### Self-Hosting

```bash
# Clone yellowstone-grpc
git clone https://github.com/rpcpool/yellowstone-grpc.git
cd yellowstone-grpc

# Build the plugin
cargo build --release

# Configure your validator
cat > yellowstone-grpc-geyser/config.json <<EOF
{
  "libpath": "target/release/libyellowstone_grpc_geyser.so",
  "grpc_listen_address": "0.0.0.0:10000",
  "grpc_max_decoding_message_size": 134217728
}
EOF

# Start validator with plugin
solana-validator \
  --geyser-plugin-config yellowstone-grpc-geyser/config.json \
  # ... other validator flags
```

Then use `http://localhost:10000` as your endpoint.

## Monitoring and Debugging

### Enable Debug Logging

```bash
RUST_LOG=debug cargo run --bin market-streaming-service
```

### Check Connection

```bash
# Test gRPC endpoint connectivity
grpcurl -plaintext grpc.mainnet.solana.tools:443 list
```

### Monitor Performance

The service outputs statistics every 10 seconds (configurable via `STATS_INTERVAL`):

```
=== Cache Statistics ===
Total entries: 10
Fresh entries: 10      # Recently updated pools
Stale entries: 0       # Pools not updated recently
Max age: 5000ms        # Cache staleness threshold
```

### Common Issues

#### "Connection refused"
- Check if endpoint URL is correct
- Verify firewall settings
- Ensure port 443 is accessible

#### "Authentication failed"
- Verify `GRPC_AUTH_TOKEN` is correct
- Check if token has expired
- Contact your RPC provider

#### "No pool updates"
- Verify pool addresses are correct
- Check if pools are active
- Try different commitment level
- Ensure correct program IDs

## Production Deployment

### Using systemd

Create `/etc/systemd/system/market-streaming.service`:

```ini
[Unit]
Description=Market Streaming Service
After=network.target

[Service]
Type=simple
User=solana
WorkingDirectory=/opt/market-streaming
EnvironmentFile=/opt/market-streaming/.env
ExecStart=/opt/market-streaming/target/release/market-streaming-service
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable market-streaming
sudo systemctl start market-streaming
sudo systemctl status market-streaming
```

### Using Docker

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin market-streaming-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/market-streaming-service /usr/local/bin/
COPY --from=builder /app/market-streaming/.env.example /.env.example

ENTRYPOINT ["market-streaming-service"]
```

Build and run:

```bash
docker build -t market-streaming .
docker run -it --env-file .env market-streaming
```

## Performance Tuning

### Optimize Cache Settings

```bash
# Adjust cache staleness threshold
CACHE_MAX_AGE=2000  # 2 seconds for high-frequency updates
```

### Connection Settings

For high-throughput scenarios, consider:
- Using `finalized` commitment for guaranteed data
- Increasing stats interval to reduce log overhead
- Running on dedicated hardware

## Next Steps

1. ✅ Configure your RPC endpoint in `.env`
2. ✅ Add pool addresses you want to monitor
3. ✅ Run the service with `./run.sh`
4. ✅ Integrate with your arbitrage or trading logic
5. ✅ Monitor performance and adjust settings

## Support

For issues or questions:
- Check the main project README
- Review troubleshooting section in market-streaming/README.md
- Open an issue on GitHub

## License

MIT
