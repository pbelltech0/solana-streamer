# Troubleshooting Guide

Common issues and solutions when running the market-streaming service.

## Connection Issues

### Error: "dns error: failed to lookup address information"

**Cause**: The gRPC endpoint URL is malformed or DNS cannot resolve the hostname.

**Solutions**:

1. **Check URL format** - Ensure you're using the full URL with protocol:
   ```bash
   # ✅ Correct
   GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443

   # ❌ Wrong - missing protocol
   GRPC_ENDPOINT=grpc.mainnet.solana.tools:443

   # ❌ Wrong - invalid protocol
   GRPC_ENDPOINT=grpc://grpc.mainnet.solana.tools:443
   ```

2. **Test DNS resolution**:
   ```bash
   # Check if hostname resolves
   nslookup grpc.mainnet.solana.tools

   # Test with curl
   curl -v https://grpc.mainnet.solana.tools:443
   ```

3. **Try alternative endpoints**:
   ```bash
   # If one endpoint doesn't work, try others
   export GRPC_ENDPOINT="https://yellowstone.rpcpool.com:443"
   ```

### Error: "transport error: Connection refused"

**Cause**: The endpoint is not reachable or the port is blocked.

**Solutions**:

1. **Check firewall settings**:
   ```bash
   # Test port connectivity
   nc -zv grpc.mainnet.solana.tools 443
   telnet grpc.mainnet.solana.tools 443
   ```

2. **Verify endpoint is online**:
   - Check the RPC provider's status page
   - Try accessing from a different network
   - Contact the provider's support

3. **For self-hosted validators**:
   ```bash
   # Ensure the gRPC plugin is running
   ps aux | grep yellowstone

   # Check validator logs
   tail -f validator.log | grep grpc
   ```

### Error: "authentication failed" or "401 Unauthorized"

**Cause**: The endpoint requires authentication but token is invalid or missing.

**Solutions**:

1. **Verify token is set**:
   ```bash
   echo $GRPC_AUTH_TOKEN
   # Should output your token, not empty
   ```

2. **Check token format**:
   ```bash
   # Most providers use bearer token format
   export GRPC_AUTH_TOKEN="your-actual-token-here"

   # Do NOT include "Bearer " prefix
   # ❌ Wrong: export GRPC_AUTH_TOKEN="Bearer xyz123"
   # ✅ Correct: export GRPC_AUTH_TOKEN="xyz123"
   ```

3. **Regenerate token**:
   - Log into your RPC provider dashboard
   - Generate a new API key/token
   - Update your `.env` file

### Error: "TLS handshake failed"

**Cause**: SSL/TLS certificate verification issue.

**Solutions**:

1. **Update system certificates**:
   ```bash
   # macOS
   brew install ca-certificates

   # Ubuntu/Debian
   sudo apt-get update && sudo apt-get install ca-certificates

   # RHEL/CentOS
   sudo yum install ca-certificates
   ```

2. **Check system time**:
   ```bash
   # TLS fails if system clock is off
   date
   # If wrong, sync time
   sudo ntpdate -u time.apple.com
   ```

3. **For self-signed certificates** (self-hosted only):
   - Ensure your validator's certificate is valid
   - Consider using Let's Encrypt for proper certificates

## No Pool Updates

### Service connects but receives no updates

**Possible causes and solutions**:

1. **Pool addresses are incorrect**:
   ```bash
   # Verify pools exist on-chain
   solana account <POOL_PUBKEY>

   # Check pool program
   solana account <POOL_PUBKEY> --output json | jq '.account.owner'
   ```

2. **Wrong program IDs**:
   The service filters by program ID. Verify your pools match the protocols:
   ```rust
   Raydium CLMM:     CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK
   Orca Whirlpool:   whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc
   Meteora DLMM:     LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo
   ```

3. **Pools are inactive**:
   - Low-activity pools may not update frequently
   - Try monitoring high-volume pools like SOL/USDC

4. **Commitment level too strict**:
   ```bash
   # Try a more permissive commitment level
   export COMMITMENT_LEVEL=processed  # Fastest
   # vs
   export COMMITMENT_LEVEL=finalized  # Slowest, most secure
   ```

5. **Enable debug logging**:
   ```bash
   RUST_LOG=debug cargo run --bin market-streaming-service
   ```

## Performance Issues

### High CPU usage

**Solutions**:

1. **Reduce monitored pools**:
   - Start with fewer pools to test
   - Add more pools gradually

2. **Increase stats interval**:
   ```bash
   # Print stats less frequently
   export STATS_INTERVAL=30  # Every 30 seconds instead of 10
   ```

3. **Adjust cache max age**:
   ```bash
   # Increase to reduce cache churn
   export CACHE_MAX_AGE=10000  # 10 seconds
   ```

### Memory usage growing

**Solutions**:

1. **Enable cache cleanup**:
   The service automatically cleans stale entries, but you can adjust timing:
   ```rust
   // In your code
   let cache = PoolStateCache::with_max_age(5000); // 5 second staleness
   ```

2. **Limit pool count**:
   - Monitor only the pools you need
   - Remove inactive pools from monitoring

### Connection drops

**Solutions**:

1. **Check network stability**:
   ```bash
   ping -c 100 grpc.mainnet.solana.tools
   ```

2. **Use a closer/faster endpoint**:
   - Choose an RPC provider with better infrastructure
   - Consider self-hosting for maximum control

3. **Implement automatic reconnection** (for library users):
   ```rust
   loop {
       match client.start().await {
           Ok(_) => log::info!("Stream ended normally"),
           Err(e) => {
               log::error!("Stream error: {:?}", e);
               log::info!("Reconnecting in 5 seconds...");
               tokio::time::sleep(Duration::from_secs(5)).await;
           }
       }
   }
   ```

## RPC Provider Issues

### Rate limiting

**Symptoms**:
- Intermittent connection failures
- "Too many requests" errors
- Sporadic disconnections

**Solutions**:

1. **Upgrade your plan**:
   - Free tiers often have strict rate limits
   - Paid plans offer higher throughput

2. **Use dedicated endpoints**:
   - Shared endpoints have shared limits
   - Get a dedicated gRPC endpoint

3. **Contact provider support**:
   - They may whitelist your IP
   - Request rate limit increase

### Endpoint maintenance/downtime

**Solutions**:

1. **Use multiple endpoints** (failover):
   ```rust
   let endpoints = vec![
       "https://primary-endpoint.com:443",
       "https://backup-endpoint.com:443",
   ];

   for endpoint in endpoints {
       match connect(endpoint).await {
           Ok(client) => return Ok(client),
           Err(e) => log::warn!("Failed to connect to {}: {}", endpoint, e),
       }
   }
   ```

2. **Check provider status pages**:
   - Helius: https://status.helius.dev/
   - QuickNode: https://status.quicknode.com/
   - Triton: Check their status page

## Testing Your Setup

### Quick connection test

```bash
# Test with minimal configuration
export GRPC_ENDPOINT="https://grpc.mainnet.solana.tools:443"
export POOL_PUBKEYS="8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj"  # Known active pool
export DEX_PROTOCOLS="raydium"
export COMMITMENT_LEVEL="processed"

RUST_LOG=info cargo run --bin market-streaming-service
```

You should see:
```
[INFO] Starting pool monitoring...
[INFO] Starting pool stream with 1 pools and 1 protocols
```

Within 10-30 seconds, you should see cache statistics with pool updates.

### Verify pool addresses

```bash
# Check if a pool exists
solana account 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj

# Check pool's program
solana account 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj --output json | jq -r '.account.owner'
# Should output: CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK (Raydium CLMM)
```

### Check gRPC connectivity

```bash
# Install grpcurl if not already installed
brew install grpcurl  # macOS
# or
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest

# Test gRPC endpoint
grpcurl -plaintext grpc.mainnet.solana.tools:443 list
```

## Getting Help

If you're still experiencing issues:

1. **Enable debug logging**:
   ```bash
   RUST_LOG=debug cargo run --bin market-streaming-service 2>&1 | tee debug.log
   ```

2. **Check the logs** for specific error messages

3. **Gather diagnostic info**:
   ```bash
   # System info
   uname -a
   rustc --version
   cargo --version

   # Network connectivity
   curl -v https://grpc.mainnet.solana.tools:443

   # DNS resolution
   nslookup grpc.mainnet.solana.tools
   ```

4. **Open an issue** on GitHub with:
   - Error message
   - Debug logs
   - System info
   - RPC provider (if not sensitive)

## Related Documentation

- [README.md](README.md) - Main documentation
- [INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md) - Integration instructions
- [Yellowstone gRPC GitHub](https://github.com/rpcpool/yellowstone-grpc) - Upstream docs
