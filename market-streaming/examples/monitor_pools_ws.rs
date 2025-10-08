use market_streaming::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    dotenv::dotenv().ok();

    // Initialize logger
    env_logger::init();

    // Create state cache
    let state_cache = Arc::new(PoolStateCache::new());

    // Load configuration from environment
    let wss_endpoint = std::env::var("WSS_ENDPOINT")
        .unwrap_or_else(|_| "wss://atlas-mainnet.helius-rpc.com/?api-key=YOUR_KEY".to_string());
    let rpc_endpoint = std::env::var("RPC_ENDPOINT")
        .unwrap_or_else(|_| "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY".to_string());

    // Debug: Print endpoints (hide API key for display)
    println!("Full WSS URL length: {}", wss_endpoint.len());
    println!("Using WebSocket: {}", wss_endpoint.split("api-key=").next().unwrap_or(""));
    println!("API key present: {}", wss_endpoint.contains("api-key=e5c72776"));

    // Parse pool addresses from environment
    let pool_pubkeys = std::env::var("POOL_PUBKEYS")
        .unwrap_or_else(|_| String::new())
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| Pubkey::from_str(s.trim()).ok())
        .collect::<Vec<_>>();

    // Parse DEX protocols from environment
    let protocols = std::env::var("DEX_PROTOCOLS")
        .unwrap_or_else(|_| "raydium,orca,meteora".to_string())
        .split(',')
        .filter_map(|s| match s.trim().to_lowercase().as_str() {
            "raydium" => Some(DexProtocol::RaydiumClmm),
            "orca" => Some(DexProtocol::OrcaWhirlpool),
            "meteora" => Some(DexProtocol::MeteoraDlmm),
            _ => None,
        })
        .collect::<Vec<_>>();

    // Get commitment level from environment
    let commitment = std::env::var("COMMITMENT_LEVEL")
        .unwrap_or_else(|_| "confirmed".to_string());

    // Configure WebSocket streaming
    let config = WsStreamConfig {
        wss_endpoint,
        rpc_endpoint,
        pool_pubkeys: pool_pubkeys.clone(),
        protocols: protocols.clone(),
        commitment,
    };

    // Create WebSocket client
    let client = WsPoolStreamClient::new(config, state_cache.clone());

    println!("Starting DEX pool monitoring via WebSocket...");
    println!("WebSocket endpoint: {}",
        client.config.wss_endpoint.split("api-key=").next().unwrap_or(""));
    println!("Monitoring {} pools across {} DEXs",
        pool_pubkeys.len(),
        protocols.len()
    );

    if pool_pubkeys.is_empty() {
        println!("\n⚠️  Warning: No pool addresses specified!");
        println!("Add pool addresses to POOL_PUBKEYS in your .env file");
        println!("Example pools you can monitor:");
        println!("  - Raydium SOL/USDC: 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj");
        println!("  - Orca SOL/USDC: HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ");
    }

    println!("\nPress Ctrl+C to stop\n");

    // Spawn a task to periodically print cache statistics
    let cache_clone = state_cache.clone();
    tokio::spawn(async move {
        let stats_interval = std::env::var("STATS_INTERVAL")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u64>()
            .unwrap_or(10);

        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(stats_interval));

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
    match client.start().await {
        Ok(_) => println!("WebSocket stream ended normally"),
        Err(e) => eprintln!("WebSocket stream error: {}", e),
    }

    Ok(())
}