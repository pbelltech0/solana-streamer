# Yellowstone gRPC Configuration Guide

## Connection Setup

The Yellowstone gRPC connection requires:
1. A valid endpoint URL
2. Authentication token (for most providers)

## Provider Options

### 1. Triton One (Recommended)
- **Endpoint**: `https://grpc.triton.one:443` or `grpc.triton.one:443`
- **Token**: Required (get from https://triton.one)
- **Cost**: Free tier available

### 2. Helius
- **Endpoint**: Contact Helius for endpoint
- **Token**: Required (get from https://helius.dev)
- **Cost**: Paid plans

### 3. Local Yellowstone Node
- **Endpoint**: `127.0.0.1:10000` or `localhost:10000`
- **Token**: Not required
- **Setup**: https://github.com/rpcpool/yellowstone-grpc

## Example Configuration

```rust
// For Triton One
let config = PythArbConfig {
    yellowstone_endpoint: "https://grpc.triton.one:443".to_string(),
    yellowstone_token: Some("YOUR_TRITON_TOKEN".to_string()),
    rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
    // ... rest of config
};

// For local node
let config = PythArbConfig {
    yellowstone_endpoint: "127.0.0.1:10000".to_string(),
    yellowstone_token: None,
    rpc_endpoint: "https://api.mainnet-beta.solana.com".to_string(),
    // ... rest of config
};
```

## Environment Variables

You can also use environment variables:

```bash
export YELLOWSTONE_ENDPOINT="https://grpc.triton.one:443"
export YELLOWSTONE_TOKEN="your-token-here"
export SOLANA_RPC_URL="https://api.mainnet-beta.solana.com"
```

Then in your code:
```rust
let config = PythArbConfig {
    yellowstone_endpoint: std::env::var("YELLOWSTONE_ENDPOINT")
        .unwrap_or_else(|_| "127.0.0.1:10000".to_string()),
    yellowstone_token: std::env::var("YELLOWSTONE_TOKEN").ok(),
    rpc_endpoint: std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
    // ... rest of config
};
```

## Troubleshooting

### "transport error" or "gRPC transport error"
- **Cause**: Cannot connect to the endpoint
- **Solutions**:
  - Verify endpoint URL is correct (include port if needed)
  - Check network connectivity
  - Ensure firewall allows outbound connections

### "authentication failed" or "unauthenticated"
- **Cause**: Missing or invalid token
- **Solutions**:
  - Verify token is correct
  - Check token hasn't expired
  - Ensure token is passed in x-token header

### Certificate errors
- **Cause**: TLS/SSL certificate validation issues
- **Solutions**:
  - Use `https://` prefix for secure connections
  - Update system certificates
  - For local development, may need to disable cert validation (not recommended for production)

## Testing Connection

Run a simple test to verify connection:

```bash
cargo run --example pyth_enhanced_arbitrage
```

You should see:
```
🔌 Connecting to Yellowstone gRPC...
   Endpoint: https://grpc.triton.one:443
   Auth: Token provided
✓ gRPC client initialized
✓ Connected successfully
```

If connection fails, you'll see detailed troubleshooting information.