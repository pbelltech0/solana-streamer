/// Pyth-enhanced arbitrage detector
/// Demonstrates integration of Pyth oracle prices for validation
/// Prevents false arbitrage opportunities from stale/manipulated prices

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::streaming::{
    enhanced_arbitrage::{EnhancedArbitrageDetector, MonitoredPair},
    liquidity_monitor::{LiquidityMonitor, PoolState, DexType},
    pyth_price_monitor::{PythPriceMonitor, presets},
    pyth_arb_validator::{PythArbValidator, OracleValidationConfig},
    yellowstone_grpc::{YellowstoneGrpc, TransactionFilter},
    event_parser::{
        common::EventType,
        UnifiedEvent,
        Protocol,
    },
};
use std::str::FromStr;
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::sync::Mutex;
use std::time::{Instant, Duration};

/// Configuration for the Pyth-enhanced arbitrage detector
struct PythArbConfig {
    // Monitored pairs
    monitored_pairs: Vec<MonitoredPair>,

    // Detection thresholds
    min_net_profit_pct: f64,
    min_execution_prob: f64,
    min_ev_score: f64,

    // Oracle validation
    oracle_validation_config: OracleValidationConfig,

    // RPC endpoints
    solana_rpc_url: String,
    pyth_rpc_url: String,
    yellowstone_endpoint: String,
    yellowstone_token: Option<String>,
}

impl PythArbConfig {
    fn default() -> Self {
        // Define common tokens
        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let usdt = Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap();

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
            min_net_profit_pct: 0.3,
            min_execution_prob: 0.4,
            min_ev_score: 15.0,
            oracle_validation_config: OracleValidationConfig::balanced(),
            solana_rpc_url: std::env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            pyth_rpc_url: std::env::var("PYTH_RPC_URL")
                .unwrap_or_else(|_| "http://pythnet.rpcpool.com".to_string()),
            yellowstone_endpoint: std::env::var("YELLOWSTONE_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:10000".to_string()),
            yellowstone_token: std::env::var("YELLOWSTONE_TOKEN").ok(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  PYTH-ENHANCED ARBITRAGE DETECTOR                        ║");
    println!("║  Oracle-Validated Opportunity Detection                  ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();

    // Check for required environment variables
    if std::env::var("YELLOWSTONE_ENDPOINT").is_err() || std::env::var("YELLOWSTONE_TOKEN").is_err() {
        println!("⚠️  Missing required environment variables!");
        println!();
        println!("📝 Required setup:");
        println!("   export YELLOWSTONE_ENDPOINT=<your-provider-endpoint>");
        println!("   export YELLOWSTONE_TOKEN=<your-api-token>");
        println!();
        println!("📚 Example providers:");
        println!("   - Triton One: https://grpc.triton.one:443 (get token at https://triton.one)");
        println!("   - Local node: http://localhost:10000 (no token needed)");
        println!();
        println!("📖 See examples/yellowstone_config.md for detailed setup instructions");
        println!();
    }

    let config = PythArbConfig::default();

    // Display configuration
    println!("📊 Configuration:");
    println!("  • Monitored Pairs: {}", config.monitored_pairs.len());
    for pair in &config.monitored_pairs {
        println!("    - {} (trade size: {:.2}-{:.2} SOL)",
            pair.name,
            pair.min_trade_size as f64 / 1_000_000_000.0,
            pair.max_trade_size as f64 / 1_000_000_000.0
        );
    }
    println!("  • Min Net Profit: {:.2}%", config.min_net_profit_pct);
    println!("  • Min Execution Prob: {:.0}%", config.min_execution_prob * 100.0);
    println!("  • Min EV Score: {:.1}", config.min_ev_score);
    println!();

    println!("🔮 Oracle Validation:");
    println!("  • Max Price Deviation: {:.1}%", config.oracle_validation_config.max_price_deviation_pct);
    println!("  • Max Confidence Interval: {:.1}%", config.oracle_validation_config.max_oracle_confidence_pct);
    println!("  • Max Staleness: {}s", config.oracle_validation_config.max_staleness_secs);
    println!();

    // Initialize Pyth price monitor
    println!("🔮 Initializing Pyth price oracle...");
    let pyth_monitor = Arc::new(PythPriceMonitor::new(
        config.pyth_rpc_url.clone(),
        2000, // Update every 2 seconds
    ));

    // Add price feeds
    pyth_monitor.add_price_feeds(presets::all_common_feeds());
    println!("✓ Added {} Pyth price feeds", presets::all_common_feeds().len());

    // Start Pyth monitoring in background
    let pyth_monitor_clone = pyth_monitor.clone();
    tokio::spawn(async move {
        if let Err(e) = pyth_monitor_clone.start_monitoring().await {
            log::error!("Pyth monitoring error: {}", e);
        }
    });

    // Wait for initial price fetch
    println!("⏳ Waiting for initial Pyth prices...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Display Pyth prices
    let prices = pyth_monitor.get_all_prices().await;
    println!("✓ Loaded {} Pyth prices:", prices.len());
    for price_data in prices {
        println!("  • {}: ${:.2} (±${:.4}, {:.2}% confidence)",
            price_data.symbol,
            price_data.normalized_price(),
            price_data.confidence * 10f64.powi(price_data.expo),
            price_data.confidence_pct()
        );
    }
    println!();

    // Initialize Pyth validator
    let pyth_validator = Arc::new(PythArbValidator::new(
        pyth_monitor.clone(),
        config.oracle_validation_config,
    ));
    println!("✓ Pyth validator initialized");
    println!();

    // Initialize components
    println!("🔧 Initializing arbitrage detection system...");
    let liquidity_monitor = Arc::new(Mutex::new(LiquidityMonitor::new(300))); // 5 minute max pool age

    let detector = Arc::new(Mutex::new(EnhancedArbitrageDetector::new(
        config.monitored_pairs,
        config.min_net_profit_pct,
        config.min_execution_prob,
    )));

    println!("✓ Liquidity monitor initialized");
    println!("✓ Enhanced arbitrage detector initialized");
    println!();

    // Connect to Yellowstone gRPC
    println!("🔌 Connecting to Yellowstone gRPC...");
    println!("   Endpoint: {}", config.yellowstone_endpoint);
    println!("   Auth: {}", if config.yellowstone_token.is_some() { "Token provided" } else { "No token" });

    let mut grpc = match YellowstoneGrpc::new(
        config.yellowstone_endpoint.clone(),
        config.yellowstone_token.clone(),
    ) {
        Ok(g) => {
            println!("✓ gRPC client initialized");
            g
        }
        Err(e) => {
            println!("❌ Failed to initialize gRPC client: {}", e);
            println!();
            println!("📝 Connection troubleshooting:");
            println!("   1. Verify your Yellowstone endpoint URL is correct");
            println!("   2. Check if you need an authentication token (x-token)");
            println!("   3. Common endpoints:");
            println!("      - Triton/Helius: grpc.PROVIDER.com:443");
            println!("      - Local: 127.0.0.1:10000");
            println!("   4. Ensure your network can reach the endpoint");
            println!("   5. Check if you're behind a firewall/proxy");
            return Err(e);
        }
    };
    println!("✓ Connected successfully");
    println!();

    // Initialize event parser
    // Note: EventParser is handled internally by the streaming infrastructure

    // Subscribe to events
    println!("🚀 Starting event subscription...");
    println!();

    let event_count = Arc::new(AtomicU64::new(0));
    let last_scan = Arc::new(Mutex::new(Instant::now()));
    let scan_interval = Duration::from_secs(5);
    let running = Arc::new(AtomicBool::new(true));

    // Clone references for the callback
    let detector_clone = Arc::clone(&detector);
    let liquidity_monitor_clone = Arc::clone(&liquidity_monitor);
    let validator_clone = Arc::clone(&pyth_validator);
    let event_count_clone = Arc::clone(&event_count);
    let last_scan_clone = Arc::clone(&last_scan);
    let running_clone = Arc::clone(&running);

    // Set up protocols to monitor
    let protocols = vec![
        Protocol::RaydiumCpmm,
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
    ];

    // Set up filters for the DEX programs
    let transaction_filters = protocols.iter().map(|p| {
        TransactionFilter {
            account_include: p.get_program_id().iter().map(|pk| pk.to_string()).collect(),
            account_exclude: vec![],
            account_required: vec![],
        }
    }).collect();

    // No specific account filters
    let account_filters = vec![];

    // Subscribe with callback
    println!("📡 Setting up event subscription...");
    println!("   Monitoring protocols: Raydium CPMM, CLMM, AMM V4");
    println!("   Scan interval: {} seconds", scan_interval.as_secs());
    println!();

    let handle = tokio::spawn(async move {
        match grpc.subscribe_events_immediate(
            protocols,
            None, // No bot wallet filter
            transaction_filters,
            account_filters,
            None, // No event type filter - we want all swap events
            None, // Default commitment
            move |event| {
                event_count_clone.fetch_add(1, Ordering::SeqCst);

                // Process swap events
                if matches!(
                    event.event_type(),
                    EventType::RaydiumClmmSwapV2
                        | EventType::RaydiumCpmmSwapBaseInput
                        | EventType::RaydiumAmmV4SwapBaseIn
                ) {
                    // Convert event to PoolState if possible
                    if let Some(pool_state) = convert_event_to_pool_state(&event) {
                        // Update liquidity monitor
                        liquidity_monitor_clone.lock().unwrap().update_pool(pool_state.clone());

                        println!("💱 Swap detected: {} -> {} ({} -> {})",
                            pool_state.token_a.to_string()[..8].to_string(),
                            pool_state.token_b.to_string()[..8].to_string(),
                            pool_state.reserve_a,
                            pool_state.reserve_b
                        );
                    }
                }

                // Check if it's time to scan for arbitrage opportunities
                let mut last = last_scan_clone.lock().unwrap();
                if last.elapsed() >= scan_interval {
                    *last = Instant::now();

                    println!();
                    println!("🔍 Scanning for arbitrage opportunities...");

                    // Get current opportunities
                    let opportunities = detector_clone.lock().unwrap().get_opportunities().to_vec();

                    if opportunities.is_empty() {
                        println!("  No opportunities found");
                    } else {
                        println!("  Found {} potential opportunities:", opportunities.len());

                        let mut valid_count = 0;
                        let mut invalid_count = 0;

                        // Validate each opportunity with Pyth oracle
                        for opp in opportunities.iter() {
                            let validation = futures::executor::block_on(async {
                                validator_clone
                                    .validate_opportunity(opp)
                                    .await
                            });

                            match validation {
                                Ok(result) if result.is_valid => {
                                    valid_count += 1;
                                    println!();
                                    println!("✅ VALID OPPORTUNITY DETECTED!");
                                    println!("  Pair: {}... <-> {}...",
                                        opp.token_pair.base.to_string()[..8].to_string(),
                                        opp.token_pair.quote.to_string()[..8].to_string()
                                    );
                                    println!("  Buy Pool: {} ({})",
                                        opp.buy_pool.to_string()[..8].to_string(),
                                        format!("{:?}", opp.buy_dex).split("::").last().unwrap_or("Unknown")
                                    );
                                    println!("  Sell Pool: {} ({})",
                                        opp.sell_pool.to_string()[..8].to_string(),
                                        format!("{:?}", opp.sell_dex).split("::").last().unwrap_or("Unknown")
                                    );
                                    println!("  Net Profit: ${:.2} ({:.2}%)",
                                        opp.expected_profit as f64 / 1e9,
                                        opp.net_profit_pct
                                    );
                                    println!("  Execution Prob: {:.1}%", opp.combined_execution_prob * 100.0);
                                    println!("  EV Score: {:.2}", opp.ev_score);
                                    if let Some(deviation) = result.deviation_pct {
                                        println!("  Price Deviation: {:.2}%", deviation);
                                    }
                                }
                                Ok(result) => {
                                    invalid_count += 1;
                                    println!();
                                    println!("❌ Invalid opportunity: {}", result.reason);
                                }
                                Err(e) => {
                                    invalid_count += 1;
                                    println!("⚠️ Validation error: {}", e);
                                }
                            }
                        }

                        println!();
                        println!("╔═══════════════════════════════════════════════════════════╗");
                        println!("║  SCAN COMPLETE - {} valid, {} filtered", valid_count, invalid_count);
                        println!("╚═══════════════════════════════════════════════════════════╝");
                    }
                    println!();
                }

                // Check if we should stop
                if !running_clone.load(Ordering::SeqCst) {
                    println!("Stopping event subscription...");
                }
            }
        ).await {
            Ok(()) => Ok(()),
            Err(e) => {
                println!("❌ Subscription error: {}", e);
                println!();
                println!("📝 Common causes:");
                println!("   1. Invalid authentication token");
                println!("   2. Endpoint not accessible");
                println!("   3. Network timeout");
                println!("   4. TLS/SSL certificate issues");
                println!();
                println!("💡 Solutions:");
                println!("   - For Triton/Helius: Ensure you have a valid API token");
                println!("   - For local node: Check if gRPC server is running");
                println!("   - Try using a different endpoint or provider");
                println!();
                println!("📚 Yellowstone gRPC providers:");
                println!("   - Triton One: https://triton.one");
                println!("   - Helius: https://helius.dev");
                println!("   - Run locally: https://github.com/rpcpool/yellowstone-grpc");
                Err(e)
            }
        }
    });

    // Wait for subscription to complete or error
    println!("⏳ Waiting for events... (Press Ctrl+C to stop)");
    println!();

    match handle.await {
        Ok(Ok(())) => {
            println!("✓ Event subscription completed successfully");
        }
        Ok(Err(e)) => {
            println!("❌ Event subscription error: {}", e);
            return Err(e);
        }
        Err(e) => {
            println!("❌ Task join error: {}", e);
            return Err(anyhow::anyhow!("Task join error: {}", e));
        }
    }

    Ok(())
}

/// Convert a UnifiedEvent to PoolState for liquidity monitoring
fn convert_event_to_pool_state(event: &Box<dyn UnifiedEvent>) -> Option<PoolState> {
    use solana_streamer_sdk::streaming::event_parser::protocols::{
        raydium_cpmm::RaydiumCpmmSwapEvent,
        raydium_clmm::RaydiumClmmSwapEvent,
        raydium_amm_v4::RaydiumAmmV4SwapEvent,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    // Try to downcast to specific swap event types
    if let Some(cpmm_event) = event.as_any().downcast_ref::<RaydiumCpmmSwapEvent>() {
        return Some(PoolState {
            pool_address: cpmm_event.pool_state,
            dex_type: DexType::RaydiumCpmm,
            token_a: cpmm_event.input_token_mint,
            token_b: cpmm_event.output_token_mint,
            reserve_a: cpmm_event.amount_in,
            reserve_b: cpmm_event.amount_out,
            liquidity: 0, // Would need to fetch from account data
            sqrt_price_x64: None,
            tick_current: None,
            active_bin_id: None,
            bin_step: None,
            total_fee_bps: 25, // Standard CPMM fee
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_trade_timestamp: Some(event.slot()),
            volume_24h: None,
        });
    }

    if let Some(clmm_event) = event.as_any().downcast_ref::<RaydiumClmmSwapEvent>() {
        return Some(PoolState {
            pool_address: clmm_event.pool_state,
            dex_type: DexType::RaydiumClmm,
            // TODO: Need to fetch mint addresses from pool account data
            // For now, using vault addresses as placeholders
            token_a: clmm_event.input_vault,
            token_b: clmm_event.output_vault,
            reserve_a: clmm_event.amount,
            reserve_b: clmm_event.other_amount_threshold,
            liquidity: 0, // Would need to fetch from account data
            sqrt_price_x64: Some(clmm_event.sqrt_price_limit_x64),
            tick_current: None,
            active_bin_id: None,
            bin_step: None,
            total_fee_bps: 25, // Standard CLMM fee
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            last_trade_timestamp: Some(event.slot()),
            volume_24h: None,
        });
    }

    if let Some(_amm_event) = event.as_any().downcast_ref::<RaydiumAmmV4SwapEvent>() {
        // TODO: RaydiumAmmV4SwapEvent doesn't contain mint addresses directly
        // Would need to fetch from pool account or track from initialization events
        return None;
    }

    None
}
