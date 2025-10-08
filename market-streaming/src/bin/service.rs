//! Market Streaming Service - Standalone gRPC Pool Monitor
//!
//! This binary runs a standalone service that monitors DEX pool states via Yellowstone gRPC.
//!
//! # Usage
//!
//! ```bash
//! # Basic usage with environment variables
//! export GRPC_ENDPOINT="https://grpc.mainnet.solana.tools:443"
//! export GRPC_AUTH_TOKEN="your-token-here"
//! cargo run --bin market-streaming-service
//!
//! # Or with command line arguments
//! cargo run --bin market-streaming-service -- \
//!   --endpoint https://grpc.mainnet.solana.tools:443 \
//!   --token your-token-here \
//!   --pools 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj,HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ
//! ```

use clap::Parser;
use market_streaming::prelude::*;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use yellowstone_grpc_proto::prelude::CommitmentLevel;

#[derive(Parser, Debug)]
#[command(name = "market-streaming-service")]
#[command(about = "Real-time DEX pool monitoring service via Yellowstone gRPC", long_about = None)]
struct Args {
    /// Yellowstone gRPC endpoint
    #[arg(
        short = 'e',
        long = "endpoint",
        env = "GRPC_ENDPOINT",
        default_value = "https://grpc.mainnet.solana.tools:443"
    )]
    endpoint: String,

    /// Optional authentication token
    #[arg(short = 't', long = "token", env = "GRPC_AUTH_TOKEN")]
    auth_token: Option<String>,

    /// Comma-separated list of pool pubkeys to monitor
    #[arg(
        short = 'p',
        long = "pools",
        env = "POOL_PUBKEYS",
        value_delimiter = ',',
        help = "Pool pubkeys to monitor (comma-separated)"
    )]
    pools: Vec<String>,

    /// DEX protocols to monitor
    #[arg(
        long = "protocols",
        env = "DEX_PROTOCOLS",
        value_delimiter = ',',
        default_value = "raydium,orca,meteora",
        help = "DEX protocols to monitor: raydium, orca, meteora"
    )]
    protocols: Vec<String>,

    /// Commitment level
    #[arg(
        short = 'c',
        long = "commitment",
        env = "COMMITMENT_LEVEL",
        default_value = "processed",
        help = "Commitment level: processed, confirmed, or finalized"
    )]
    commitment: String,

    /// Statistics interval in seconds
    #[arg(
        short = 's',
        long = "stats-interval",
        env = "STATS_INTERVAL",
        default_value = "10",
        help = "Interval for printing statistics (seconds)"
    )]
    stats_interval: u64,

    /// Cache max age in milliseconds
    #[arg(
        long = "cache-max-age",
        env = "CACHE_MAX_AGE",
        default_value = "5000",
        help = "Maximum age for cached pool states (milliseconds)"
    )]
    cache_max_age: u64,
}

impl Args {
    fn parse_pools(&self) -> anyhow::Result<Vec<Pubkey>> {
        self.pools
            .iter()
            .map(|s| {
                Pubkey::from_str(s.trim())
                    .map_err(|e| anyhow::anyhow!("Invalid pubkey '{}': {}", s, e))
            })
            .collect()
    }

    fn parse_protocols(&self) -> Vec<DexProtocol> {
        self.protocols
            .iter()
            .filter_map(|s| match s.trim().to_lowercase().as_str() {
                "raydium" | "raydium-clmm" => Some(DexProtocol::RaydiumClmm),
                "orca" | "whirlpool" => Some(DexProtocol::OrcaWhirlpool),
                "meteora" | "meteora-dlmm" => Some(DexProtocol::MeteoraDlmm),
                "crema" => Some(DexProtocol::CremaFinance),
                "tuna" | "defi-tuna" => Some(DexProtocol::DefiTuna),
                _ => {
                    log::warn!("Unknown protocol: {}", s);
                    None
                }
            })
            .collect()
    }

    fn parse_commitment(&self) -> CommitmentLevel {
        match self.commitment.to_lowercase().as_str() {
            "finalized" => CommitmentLevel::Finalized,
            "confirmed" => CommitmentLevel::Confirmed,
            "processed" | _ => CommitmentLevel::Processed,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Initialize rustls crypto provider
    let _ = rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // Parse command line arguments
    let args = Args::parse();

    // Parse pool pubkeys
    let pool_pubkeys = args.parse_pools()?;
    if pool_pubkeys.is_empty() {
        log::warn!("No pool pubkeys provided. Service will not monitor any pools.");
        log::info!("Use --pools flag or POOL_PUBKEYS environment variable to add pools.");
        log::info!("Example: --pools 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj,HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ");
    }

    // Parse protocols
    let protocols = args.parse_protocols();
    if protocols.is_empty() {
        anyhow::bail!("No valid protocols specified");
    }

    // Create state cache with custom max age
    let state_cache = Arc::new(PoolStateCache::with_max_age(args.cache_max_age));

    // Configure streaming
    let config = StreamConfig {
        grpc_endpoint: args.endpoint.clone(),
        auth_token: args.auth_token.clone(),
        pool_pubkeys,
        protocols: protocols.clone(),
        commitment: args.parse_commitment(),
    };

    log::info!("=== Market Streaming Service ===");
    log::info!("Endpoint: {}", args.endpoint);
    log::info!("Auth Token: {}", if args.auth_token.is_some() { "Set" } else { "None" });
    log::info!("Pools: {}", config.pool_pubkeys.len());
    log::info!("Protocols: {}", protocols.iter().map(|p| p.name()).collect::<Vec<_>>().join(", "));
    log::info!("Commitment: {:?}", config.commitment);
    log::info!("Cache Max Age: {}ms", args.cache_max_age);
    log::info!("Stats Interval: {}s", args.stats_interval);
    log::info!("================================\n");

    // Create pool stream client
    let client = PoolStreamClient::new(config, state_cache.clone());

    // Spawn statistics task
    let cache_clone = state_cache.clone();
    let stats_interval = args.stats_interval;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(stats_interval));
        loop {
            interval.tick().await;

            let stats = cache_clone.stats();
            log::info!("\n=== Cache Statistics ===");
            log::info!("Total entries: {}", stats.total_entries);
            log::info!("Fresh entries: {}", stats.fresh_entries);
            log::info!("Stale entries: {}", stats.stale_entries);
            log::info!("Max age: {}ms", stats.max_age_ms);

            // Print current pool states
            let fresh_pools = cache_clone.get_all_fresh();
            if !fresh_pools.is_empty() {
                log::info!("\n--- Pool States ---");
                for (pubkey, cached) in fresh_pools {
                    let (token_a, token_b) = cached.state.get_token_pair();
                    log::info!(
                        "Pool: {}\n  Price: {:.8}\n  Liquidity: {}\n  Tokens: {} / {}\n  Slot: {}",
                        pubkey,
                        cached.state.get_price(),
                        cached.state.get_liquidity(),
                        token_a,
                        token_b,
                        cached.slot
                    );
                }
            } else {
                log::warn!("No fresh pool data available");
            }
            log::info!("========================\n");
        }
    });

    // Start streaming (runs indefinitely)
    log::info!("Starting pool monitoring...\n");
    match client.start().await {
        Ok(_) => log::info!("Stream ended normally"),
        Err(e) => {
            log::error!("Stream error: {:?}", e);
            return Err(e);
        }
    }

    Ok(())
}
