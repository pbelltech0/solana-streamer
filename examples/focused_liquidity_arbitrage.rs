/// Focused arbitrage detector for 2-3 specific coin pairs
/// Monitors liquidity events on Raydium, Orca, and Meteora
/// Uses probability-weighted arbitrage detection for optimal execution

use solana_streamer_sdk::{
    match_event,
    streaming::{
        event_parser::{
            common::{filter::EventTypeFilter, EventType},
            protocols::{
                raydium_clmm::{
                    events::{
                        RaydiumClmmPoolStateAccountEvent,
                        RaydiumClmmSwapV2Event,
                    },
                    parser::RAYDIUM_CLMM_PROGRAM_ID,
                },
                raydium_cpmm::{
                    events::RaydiumCpmmSwapEvent,
                    parser::RAYDIUM_CPMM_PROGRAM_ID,
                },
                block::block_meta_event::BlockMetaEvent,
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
        liquidity_monitor::{PoolState, DexType},
        enhanced_arbitrage::{EnhancedArbitrageDetector, MonitoredPair, EnhancedArbitrageOpportunity},
    },
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Configuration for the focused arbitrage system
struct ArbitrageConfig {
    monitored_pairs: Vec<MonitoredPair>,
    min_net_profit_pct: f64,
    min_execution_prob: f64,
    min_ev_score: f64,
}

impl ArbitrageConfig {
    /// Create default configuration for SOL/USDC, SOL/USDT, BONK/SOL
    fn default() -> Self {
        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112")
            .expect("Valid SOL address");
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
            .expect("Valid USDC address");
        let usdt = Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")
            .expect("Valid USDT address");

        Self {
            monitored_pairs: vec![
                MonitoredPair {
                    name: "SOL/USDC".to_string(),
                    token_a: sol,
                    token_b: usdc,
                    min_trade_size: 100_000_000,      // 0.1 SOL
                    max_trade_size: 10_000_000_000,   // 10 SOL
                    target_pools: vec![],
                },
                MonitoredPair {
                    name: "SOL/USDT".to_string(),
                    token_a: sol,
                    token_b: usdt,
                    min_trade_size: 100_000_000,      // 0.1 SOL
                    max_trade_size: 10_000_000_000,   // 10 SOL
                    target_pools: vec![],
                },
            ],
            min_net_profit_pct: 0.3,      // Minimum 0.3% net profit
            min_execution_prob: 0.4,       // Minimum 40% execution probability
            min_ev_score: 15.0,            // Minimum EV score
        }
    }

    /// Create custom configuration (example for your specific needs)
    #[allow(dead_code)]
    fn custom() -> Self {
        // You can customize this for your specific token pairs
        Self::default()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .filter_level(log::LevelFilter::Info)
        .init();

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║   FOCUSED LIQUIDITY-AWARE ARBITRAGE DETECTOR             ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // Load configuration
    let config = ArbitrageConfig::default();

    println!("📊 Configuration:");
    println!("  • Monitored Pairs: {}", config.monitored_pairs.len());
    for pair in &config.monitored_pairs {
        println!("    - {} (trade size: {:.2}-{:.2} SOL)",
            pair.name,
            pair.min_trade_size as f64 / 1e9,
            pair.max_trade_size as f64 / 1e9
        );
    }
    println!("  • Min Net Profit: {:.2}%", config.min_net_profit_pct);
    println!("  • Min Execution Prob: {:.0}%", config.min_execution_prob * 100.0);
    println!("  • Min EV Score: {:.1}", config.min_ev_score);
    println!();

    // Create enhanced arbitrage detector
    let detector = Arc::new(Mutex::new(EnhancedArbitrageDetector::new(
        config.monitored_pairs.clone(),
        config.min_net_profit_pct,
        config.min_execution_prob,
    )));

    // Create GRPC client
    let mut client_config = ClientConfig::low_latency();
    client_config.enable_metrics = true;
    client_config.connection.connect_timeout = 30;
    client_config.connection.request_timeout = 60;

    println!("🔌 Connecting to Yellowstone gRPC...");
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        client_config,
    )?;
    println!("✓ Connected successfully\n");

    // Create callback for processing events
    let callback = create_liquidity_arbitrage_callback(
        detector.clone(),
        config.min_ev_score,
    );

    // Setup filters
    let protocols = vec![
        Protocol::RaydiumClmm,
        Protocol::RaydiumCpmm,
        // TODO: Add Orca and Meteora when event types are implemented
    ];

    // Get token mints to monitor
    let token_mints: Vec<String> = config.monitored_pairs
        .iter()
        .flat_map(|pair| vec![pair.token_a.to_string(), pair.token_b.to_string()])
        .collect();

    println!("🎯 Monitored Tokens:");
    for (i, mint) in token_mints.iter().enumerate() {
        println!("  {}. {}", i + 1, mint);
    }
    println!();

    // Transaction filter - include transactions with our tokens
    let transaction_filter = TransactionFilter {
        account_include: token_mints.clone(),
        account_exclude: vec![],
        account_required: vec![],
    };

    // Account filter - listen to DEX program accounts
    let account_filter = AccountFilter {
        account: vec![],
        owner: vec![
            RAYDIUM_CLMM_PROGRAM_ID.to_string(),
            RAYDIUM_CPMM_PROGRAM_ID.to_string(),
        ],
        filters: vec![],
    };

    // Event type filter - focus on liquidity and swap events
    let event_type_filter = Some(EventTypeFilter {
        include: vec![
            // Raydium CLMM
            EventType::RaydiumClmmSwapV2,
            // Liquidity events (if available)
            // EventType::RaydiumClmmIncreaseLiquidityV2,
            // EventType::RaydiumClmmDecreaseLiquidityV2,
            // Raydium CPMM
            EventType::RaydiumCpmmSwapBaseInput,
            EventType::RaydiumCpmmSwapBaseOutput,
            // Note: Pool state account events are handled separately
        ],
    });

    println!("🚀 Starting event subscription...");
    println!("================================================\n");

    // Subscribe to events
    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![transaction_filter],
        vec![account_filter],
        event_type_filter,
        None,
        callback,
    )
    .await?;

    // Setup graceful shutdown
    let grpc_clone = grpc.clone();
    let detector_clone = detector.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n\n🛑 Received shutdown signal...");
                print_final_stats(&detector_clone);
                grpc_clone.stop().await;
            }
        }
    });

    // Keep running
    tokio::signal::ctrl_c().await?;

    Ok(())
}

fn create_liquidity_arbitrage_callback(
    detector: Arc<Mutex<EnhancedArbitrageDetector>>,
    min_ev_score: f64,
) -> impl Fn(Box<dyn UnifiedEvent>) {
    let last_scan = Arc::new(Mutex::new(Instant::now()));
    let event_count = Arc::new(AtomicU64::new(0));

    move |event: Box<dyn UnifiedEvent>| {
        let count = event_count.fetch_add(1, Ordering::SeqCst);

        match_event!(event, {
            BlockMetaEvent => |_e: BlockMetaEvent| {
                // Periodically scan for arbitrage opportunities
                let should_scan = {
                    let last = last_scan.lock().unwrap();
                    last.elapsed().as_secs() >= 5
                };

                if should_scan {
                    let mut detector = detector.lock().unwrap();
                    let opportunities = detector.scan_arbitrage_opportunities();

                    if !opportunities.is_empty() {
                        println!("\n╔═══════════════════════════════════════════════════════════╗");
                        println!("║  ARBITRAGE SCAN COMPLETE - {} opportunities found", opportunities.len());
                        println!("╚═══════════════════════════════════════════════════════════╝\n");

                        for (i, opp) in opportunities.iter().take(5).enumerate() {
                            if opp.ev_score >= min_ev_score {
                                print_opportunity(i + 1, opp);
                            }
                        }
                    }

                    *last_scan.lock().unwrap() = Instant::now();

                    // Print stats every 50 events
                    if count % 50 == 0 {
                        let stats = detector.stats();
                        println!("\n📊 Detector Stats:");
                        println!("  • Events Processed: {}", count);
                        println!("  • Active Opportunities: {}", stats.active_opportunities);
                        println!("  • High Confidence: {}", stats.high_confidence_count);
                        println!("  • Total Pools: {}", stats.liquidity_stats.total_pools);
                        println!();
                    }
                }
            },

            // Raydium CLMM Pool State Update
            RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {
                let pool_state = PoolState {
                    pool_address: e.pubkey,
                    dex_type: DexType::RaydiumClmm,
                    token_a: e.pool_state.token_mint0,
                    token_b: e.pool_state.token_mint1,
                    reserve_a: 0, // Would need to query token vaults
                    reserve_b: 0, // Would need to query token vaults
                    liquidity: e.pool_state.liquidity,
                    sqrt_price_x64: Some(e.pool_state.sqrt_price_x64),
                    tick_current: Some(e.pool_state.tick_current),
                    active_bin_id: None,
                    bin_step: None,
                    total_fee_bps: (e.pool_state.tick_spacing * 10) as u16, // Approximate fee from tick spacing
                    last_updated: e.metadata.block_time as u64,
                    last_trade_timestamp: None,
                    volume_24h: None,
                };

                println!("🔄 Pool Update: Raydium CLMM {} (liquidity: {})",
                    e.pubkey, e.pool_state.liquidity);

                let mut detector = detector.lock().unwrap();
                detector.update_pool_state(pool_state);
            },

            // Raydium CLMM Swap V2
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                println!("💱 Raydium CLMM Swap: {} -> {} ({} -> {})",
                    e.input_vault_mint,
                    e.output_vault_mint,
                    e.amount,
                    e.other_amount_threshold
                );

                // Update last trade timestamp for the pool
                // This would ideally update the pool state with the latest trade info
            },

            // Raydium CPMM Swap
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                let (in_amt, out_amt) = if e.amount_in > 0 {
                    (e.amount_in, e.minimum_amount_out)
                } else {
                    (e.max_amount_in, e.amount_out)
                };

                println!("💱 Raydium CPMM Swap: {} -> {} ({} -> {})",
                    e.input_token_mint,
                    e.output_token_mint,
                    in_amt,
                    out_amt
                );
            },
        });
    }
}

fn print_opportunity(rank: usize, opp: &EnhancedArbitrageOpportunity) {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  Opportunity #{} - {}",
        rank,
        opp.recommendation()
    );
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║ Pair: {} <-> {}", opp.token_pair.base, opp.token_pair.quote);
    println!("║ Buy:  {:?} @ {:.6} (impact: {:.2}%)",
        opp.buy_dex,
        opp.buy_price,
        opp.buy_pool_impact_bps as f64 / 100.0
    );
    println!("║ Sell: {:?} @ {:.6} (impact: {:.2}%)",
        opp.sell_dex,
        opp.sell_price,
        opp.sell_pool_impact_bps as f64 / 100.0
    );
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║ Optimal Trade Size: {:.4} SOL", opp.optimal_trade_size as f64 / 1e9);
    println!("║ Gross Profit: {:.2}%", opp.gross_profit_pct);
    println!("║ Total Fees: {:.2}% ({} lamports)", opp.total_fee_pct, opp.total_fees);
    println!("║ Gas Cost: ~{} lamports", opp.estimated_gas_lamports);
    println!("║ Net Profit: {:.2}% ({} lamports)", opp.net_profit_pct, opp.net_profit);
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║ Buy Execution Prob: {:.1}%", opp.buy_execution_prob * 100.0);
    println!("║ Sell Execution Prob: {:.1}%", opp.sell_execution_prob * 100.0);
    println!("║ Combined Prob: {:.1}%", opp.combined_execution_prob * 100.0);
    println!("║ Expected Value: {:.2} lamports", opp.expected_value);
    println!("║ EV Score: {:.2}", opp.ev_score);
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

fn print_final_stats(detector: &Arc<Mutex<EnhancedArbitrageDetector>>) {
    let detector = detector.lock().unwrap();
    let stats = detector.stats();
    let top_opps = detector.get_top_opportunities(10);

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║  FINAL STATISTICS");
    println!("╠═══════════════════════════════════════════════════════════╣");
    println!("║ Monitored Pairs: {}", stats.monitored_pairs);
    println!("║ Total Pools Tracked: {}", stats.liquidity_stats.total_pools);
    println!("║ Active Opportunities: {}", stats.active_opportunities);
    println!("║ High Confidence Opportunities: {}", stats.high_confidence_count);
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    if !top_opps.is_empty() {
        println!("Top {} Opportunities by EV Score:\n", top_opps.len().min(10));
        for (i, opp) in top_opps.iter().enumerate() {
            println!("{}. EV={:.2}, Net={:.2}%, Prob={:.0}% - {} <-> {}",
                i + 1,
                opp.ev_score,
                opp.net_profit_pct,
                opp.combined_execution_prob * 100.0,
                opp.token_pair.base,
                opp.token_pair.quote
            );
        }
    }
    println!();
}
