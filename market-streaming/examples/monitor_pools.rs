use market_streaming::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use yellowstone_grpc_proto::prelude::CommitmentLevel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::init();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider().install_default().ok();

    // Create state cache
    let state_cache = Arc::new(PoolStateCache::new());

    // Example pool addresses - replace with actual high-TVL pools
    // Note: These are program IDs, not pool addresses. You need to replace with actual pool pubkeys
    let raydium_pool = Pubkey::from_str("8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj")?; // Example Raydium CLMM pool
    let orca_pool = Pubkey::from_str("HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ")?; // Example Orca Whirlpool
    let meteora_pool = Pubkey::from_str("ARwi1S4DaiTG5DX7S4M4ZsrXqpMD1MrTmbu9ue2tpmEq")?; // Example Meteora DLMM pool

    // Configure streaming
    let config = StreamConfig {
        grpc_endpoint: std::env::var("GRPC_ENDPOINT")
            .unwrap_or_else(|_| "https://grpc.mainnet.solana.tools:443".to_string()),
        auth_token: std::env::var("GRPC_AUTH_TOKEN").ok(),
        pool_pubkeys: vec![
            raydium_pool,
            orca_pool,
            meteora_pool,
        ],
        protocols: vec![
            DexProtocol::RaydiumClmm,
            DexProtocol::OrcaWhirlpool,
            DexProtocol::MeteoraDlmm,
        ],
        commitment: CommitmentLevel::Processed,
    };

    // Create and start pool stream client
    let client = PoolStreamClient::new(config, state_cache.clone());

    println!("Starting DEX pool monitoring...");
    println!("Monitoring {} pools across {} DEXs",
        client.state_cache().len(),
        3
    );
    println!("Press Ctrl+C to stop\n");

    // Spawn a task to periodically print cache statistics
    let cache_clone = state_cache.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let stats = cache_clone.stats();
            println!("\n=== Cache Statistics ===");
            println!("Total entries: {}", stats.total_entries);
            println!("Fresh entries: {}", stats.fresh_entries);
            println!("Stale entries: {}", stats.stale_entries);
            println!("Max age: {}ms", stats.max_age_ms);

            // Print current prices
            for (pubkey, cached) in cache_clone.get_all_fresh() {
                let (token_a, token_b) = cached.state.get_token_pair();
                println!(
                    "\nPool: {}\n  Price: {:.8}\n  Liquidity: {}\n  Tokens: {} / {}",
                    pubkey,
                    cached.state.get_price(),
                    cached.state.get_liquidity(),
                    token_a,
                    token_b
                );
            }
            println!("========================\n");
        }
    });

    // Start streaming (this will run indefinitely)
    client.start().await?;

    Ok(())
}
