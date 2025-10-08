/// Integrated Arbitrage Streamer
/// Combines real-time market streaming, enhanced arbitrage detection, and Pyth price validation
/// This is the main example showcasing the full integrated system

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use solana_streamer_sdk::{
    flash_loan::OpportunityDetector,
    match_event,
    streaming::{
        enhanced_arbitrage::{
            DexType, EnhancedArbitrageDetector, MonitoredPair, PoolState
        },
        event_parser::{
            core::account_event_parser::{TokenAccountEvent, TokenInfoEvent},
            protocols::{
                raydium_amm_v4::{
                    parser::RAYDIUM_AMM_V4_PROGRAM_ID, RaydiumAmmV4AmmInfoAccountEvent,
                    RaydiumAmmV4SwapEvent,
                },
                raydium_clmm::{
                    parser::RAYDIUM_CLMM_PROGRAM_ID, RaydiumClmmPoolStateAccountEvent,
                    RaydiumClmmSwapV2Event,
                },
                raydium_cpmm::{
                    parser::RAYDIUM_CPMM_PROGRAM_ID, RaydiumCpmmPoolStateAccountEvent,
                    RaydiumCpmmSwapEvent,
                },
                BlockMetaEvent,
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        pyth_price_monitor::{PythPriceMonitor, presets},
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
    },
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct EventStats {
    total_events: u64,
    swap_events: u64,
    pool_updates: u64,
    opportunities_found: u64,
    pyth_validations: u64,
}

/// Configuration for the integrated system
struct IntegratedConfig {
    // Yellowstone gRPC endpoint
    grpc_endpoint: String,
    grpc_token: Option<String>,

    // RPC endpoint for Pyth prices
    rpc_url: String,

    // Arbitrage detection parameters
    min_profit_pct: f64,
    min_execution_prob: f64,

    // Pyth validation
    max_price_deviation: f64,

    // Logging
    enable_file_logging: bool,
}

impl Default for IntegratedConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: std::env::var("YELLOWSTONE_ENDPOINT")
                .unwrap_or_else(|_| "https://solana-yellowstone-grpc.publicnode.com:443".to_string()),
            grpc_token: std::env::var("YELLOWSTONE_TOKEN").ok(),
            rpc_url: std::env::var("RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string()),
            min_profit_pct: 0.3,      // 0.3% minimum net profit
            min_execution_prob: 0.4,   // 40% minimum execution probability
            max_price_deviation: 5.0,  // 5% max deviation from Pyth oracle
            enable_file_logging: true,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║      INTEGRATED ARBITRAGE STREAMING SYSTEM               ║");
    println!("║  Real-time Market Events + Enhanced Arbitrage + Pyth     ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!();

    let config = IntegratedConfig::default();

    // Create logs directory
    if config.enable_file_logging {
        fs::create_dir_all("logs")?;
    }

    // Initialize system components
    println!("🔧 Initializing system components...");

    // 1. Flash loan detector (for legacy compatibility)
    let flash_detector = Arc::new(Mutex::new(OpportunityDetector::new(
        1_000_000,           // 0.001 SOL min profit
        100_000_000_000,     // 100 SOL max loan
        10_000_000_000,      // 10 SOL min per pool
        50_000_000_000,      // 50 SOL min combined
    )));
    println!("✓ Flash loan detector initialized");

    // 2. Enhanced arbitrage detector
    let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
    let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
    let usdt = Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap();

    let monitored_pairs = vec![
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
            min_trade_size: 100_000_000,
            max_trade_size: 10_000_000_000,
            target_pools: vec![],
        },
    ];

    let enhanced_detector = Arc::new(Mutex::new(EnhancedArbitrageDetector::new(
        monitored_pairs,
        config.min_profit_pct,
        config.min_execution_prob,
    )));
    println!("✓ Enhanced arbitrage detector initialized");

    // 3. Pyth price monitor
    let pyth_monitor = Arc::new(PythPriceMonitor::new(
        config.rpc_url.clone(),
        2000, // Update every 2 seconds
    ));
    pyth_monitor.add_price_feeds(presets::all_common_feeds());
    println!("✓ Pyth price monitor initialized");

    // Start Pyth monitoring in background
    let pyth_monitor_clone = pyth_monitor.clone();
    tokio::spawn(async move {
        if let Err(e) = pyth_monitor_clone.start_monitoring().await {
            log::error!("Pyth monitoring error: {}", e);
        }
    });

    // 4. Yellowstone gRPC client
    println!("\n📡 Connecting to Yellowstone gRPC...");
    println!("   Endpoint: {}", config.grpc_endpoint);

    let mut grpc_config = ClientConfig::low_latency();
    grpc_config.enable_metrics = true;

    let grpc = YellowstoneGrpc::new_with_config(
        config.grpc_endpoint,
        config.grpc_token,
        grpc_config,
    )?;
    println!("✓ gRPC client connected");

    // Setup log files
    let log_file = if config.enable_file_logging {
        Some(Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open("logs/integrated_events.log")?
        )))
    } else {
        None
    };

    let opportunities_log = if config.enable_file_logging {
        Some(Arc::new(Mutex::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open("logs/integrated_opportunities.log")?
        )))
    } else {
        None
    };

    // Create event callback
    let callback = create_integrated_callback(
        flash_detector,
        enhanced_detector.clone(),
        pyth_monitor.clone(),
        log_file,
        opportunities_log,
        config.max_price_deviation,
    );

    // Configure protocols to monitor
    let protocols = vec![
        Protocol::RaydiumCpmm,
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
    ];

    println!("\n🚀 Starting event subscription...");
    println!("   Protocols: Raydium CPMM, CLMM, AMM V4");
    println!("   Min profit: {:.2}%", config.min_profit_pct);
    println!("   Min execution prob: {:.0}%", config.min_execution_prob * 100.0);
    println!("   Max price deviation: {:.1}%", config.max_price_deviation);

    // Setup filters
    let account_include = vec![
        RAYDIUM_CPMM_PROGRAM_ID.to_string(),
        RAYDIUM_CLMM_PROGRAM_ID.to_string(),
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(),
    ];

    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude: vec![],
        account_required: vec![],
    };

    let account_filter = AccountFilter {
        account: vec![],
        owner: account_include,
        filters: vec![],
    };

    // Start periodic arbitrage scanning
    let enhanced_detector_scan = enhanced_detector.clone();
    let pyth_monitor_scan = pyth_monitor.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;

            // Scan for opportunities
            let opportunities = enhanced_detector_scan.lock().unwrap()
                .scan_arbitrage_opportunities();

            if !opportunities.is_empty() {
                println!("\n📊 Arbitrage Scan Results:");
                println!("   Found {} potential opportunities", opportunities.len());

                // Validate top opportunities with Pyth
                for (i, opp) in opportunities.iter().take(3).enumerate() {
                    println!("\n   #{} Opportunity:", i + 1);
                    println!("      Pair: {:?}", opp.token_pair);
                    println!("      Net Profit: {:.3}%", opp.net_profit_pct);
                    println!("      Execution Prob: {:.1}%", opp.combined_execution_prob * 100.0);
                    println!("      EV Score: {:.2}", opp.ev_score);
                    println!("      Confidence: {:?}", opp.confidence_level);

                    // Try to validate with Pyth
                    if let Some(price_data) = pyth_monitor_scan
                        .get_price(&opp.token_pair.base, &opp.token_pair.quote)
                        .await
                    {
                        let pool_price = opp.buy_price;
                        let deviation = price_data.calculate_pool_deviation(pool_price);
                        println!("      Pyth Validation: {:.2}% deviation", deviation);
                    }
                }
            }
        }
    });

    println!("\n⏳ Listening for events... (Press Ctrl+C to stop)\n");

    // Subscribe to events
    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![transaction_filter],
        vec![account_filter],
        None, // No event filtering
        None,
        callback,
    )
    .await?;

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    println!("\n👋 Shutting down...");

    Ok(())
}

fn create_integrated_callback(
    flash_detector: Arc<Mutex<OpportunityDetector>>,
    enhanced_detector: Arc<Mutex<EnhancedArbitrageDetector>>,
    _pyth_monitor: Arc<PythPriceMonitor>,
    log_file: Option<Arc<Mutex<std::fs::File>>>,
    opportunities_log: Option<Arc<Mutex<std::fs::File>>>,
    _max_price_deviation: f64,
) -> impl Fn(Box<dyn UnifiedEvent>) {
    let event_counter = Arc::new(Mutex::new(EventStats::default()));
    let last_report = Arc::new(Mutex::new(Instant::now()));

    // Helper macro for file logging
    macro_rules! log_to_file {
        ($file:expr, $($arg:tt)*) => {{
            if let Some(ref file) = $file {
                let msg = format!($($arg)*);
                if let Ok(mut f) = file.lock() {
                    let _ = f.write_all(msg.as_bytes());
                    let _ = f.flush();
                }
            }
        }};
    }

    move |event: Box<dyn UnifiedEvent>| {
        // Update event counter
        {
            let mut stats = event_counter.lock().unwrap();
            stats.total_events += 1;
        }

        // Log event to file
        log_to_file!(
            log_file,
            "[{}] Event: {:?}, Slot: {}\n",
            chrono::Utc::now().format("%H:%M:%S"),
            event.event_type(),
            event.slot()
        );

        match_event!(event, {
            // Block metadata
            BlockMetaEvent => |_e: BlockMetaEvent| {
                // Track block timing if needed
            },

            // Raydium CPMM Swap
            RaydiumCpmmSwapEvent => |_e: RaydiumCpmmSwapEvent| {
                let mut stats = event_counter.lock().unwrap();
                stats.swap_events += 1;

                // CPMM swap events don't contain reserve data
                // Pool state is updated from account events instead
            },

            // Raydium CLMM Swap
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                let mut stats = event_counter.lock().unwrap();
                stats.swap_events += 1;

                // Also check with flash loan detector
                let mut detector = flash_detector.lock().unwrap();
                if let Some(opp) = detector.analyze_clmm_swap_event(&e) {
                    let mut stats = event_counter.lock().unwrap();
                    stats.opportunities_found += 1;

                    // Log opportunity
                    let spread = (opp.price_b - opp.price_a) / opp.price_a * 100.0;
                    if opp.confidence >= 60 && spread >= 1.0 {
                        println!("\n💎 FLASH LOAN OPPORTUNITY");
                        println!("   Profit: {:.6} SOL", opp.expected_profit as f64 / 1e9);
                        println!("   Spread: {:.2}%", spread);
                        println!("   Confidence: {}%", opp.confidence);

                        log_to_file!(
                            opportunities_log,
                            "{} | Flash | Profit: {:.6} SOL | Spread: {:.2}% | Confidence: {}%\n",
                            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                            opp.expected_profit as f64 / 1e9,
                            spread,
                            opp.confidence
                        );
                    }
                }
            },

            // Raydium AMM V4 Swap
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                let mut stats = event_counter.lock().unwrap();
                stats.swap_events += 1;

                // Check with flash loan detector
                let mut detector = flash_detector.lock().unwrap();
                if let Some(opp) = detector.analyze_ammv4_swap_event(&e) {
                    let mut stats = event_counter.lock().unwrap();
                    stats.opportunities_found += 1;

                    let spread = (opp.price_b - opp.price_a) / opp.price_a * 100.0;
                    if opp.confidence >= 60 && spread >= 1.0 {
                        println!("\n💎 FLASH LOAN OPPORTUNITY");
                        println!("   Profit: {:.6} SOL", opp.expected_profit as f64 / 1e9);
                        println!("   Spread: {:.2}%", spread);
                        println!("   Confidence: {}%", opp.confidence);
                    }
                }
            },

            // Pool State Updates
            RaydiumCpmmPoolStateAccountEvent => |e: RaydiumCpmmPoolStateAccountEvent| {
                let mut stats = event_counter.lock().unwrap();
                stats.pool_updates += 1;

                // Convert to pool state - CPMM pool state doesn't include reserve amounts
                // Only the vault pubkeys and LP supply are available
                let pool_state = PoolState {
                    pool_address: e.pubkey,
                    dex_type: DexType::RaydiumCpmm,
                    token_a: e.pool_state.token0_mint,
                    token_b: e.pool_state.token1_mint,
                    reserve_a: 0,  // Not available in pool state account
                    reserve_b: 0,  // Would need to fetch vault token accounts
                    liquidity: e.pool_state.lp_supply,
                    sqrt_price_x64: None,
                    total_fee_bps: 25,
                    last_updated: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                enhanced_detector.lock().unwrap().update_pool_state(pool_state);
            },

            RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {
                let mut stats = event_counter.lock().unwrap();
                stats.pool_updates += 1;

                // Update flash loan detector
                flash_detector.lock().unwrap().update_clmm_pool_state(&e);

                // Convert to pool state for enhanced detector
                let pool_state = PoolState {
                    pool_address: e.pubkey,
                    dex_type: DexType::RaydiumClmm,
                    token_a: e.pool_state.token_mint0,
                    token_b: e.pool_state.token_mint1,
                    reserve_a: 0, // Would need vault balances
                    reserve_b: 0,
                    liquidity: e.pool_state.liquidity as u64,
                    sqrt_price_x64: Some(e.pool_state.sqrt_price_x64),
                    total_fee_bps: 25,
                    last_updated: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                enhanced_detector.lock().unwrap().update_pool_state(pool_state);
            },

            RaydiumAmmV4AmmInfoAccountEvent => |e: RaydiumAmmV4AmmInfoAccountEvent| {
                let mut stats = event_counter.lock().unwrap();
                stats.pool_updates += 1;

                // Update flash loan detector
                flash_detector.lock().unwrap().update_ammv4_pool_state(&e);
            },

            // Token events
            TokenAccountEvent => |_e: TokenAccountEvent| {
                // Track token balances if needed
            },
            TokenInfoEvent => |_e: TokenInfoEvent| {
                // Track token info if needed
            },
        });

        // Print periodic stats
        let mut last = last_report.lock().unwrap();
        if last.elapsed() >= Duration::from_secs(30) {
            *last = Instant::now();

            let stats = event_counter.lock().unwrap();
            println!("\n📊 Stats Update:");
            println!("   Total Events: {}", stats.total_events);
            println!("   Swap Events: {}", stats.swap_events);
            println!("   Pool Updates: {}", stats.pool_updates);
            println!("   Opportunities: {}", stats.opportunities_found);
        }
    }
}