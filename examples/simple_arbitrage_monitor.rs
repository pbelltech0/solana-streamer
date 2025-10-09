/// Simple Arbitrage Monitor
/// Streamlined example combining functional streaming with arbitrage detection
/// Focus: Clean, performant, modular

use solana_streamer_sdk::{
    flash_loan::OpportunityDetector,
    match_event,
    streaming::{
        event_parser::{
            protocols::{
                raydium_amm_v4::{
                    parser::RAYDIUM_AMM_V4_PROGRAM_ID,
                    RaydiumAmmV4AmmInfoAccountEvent,
                    RaydiumAmmV4SwapEvent,
                },
                raydium_clmm::{
                    parser::RAYDIUM_CLMM_PROGRAM_ID,
                    RaydiumClmmPoolStateAccountEvent,
                    RaydiumClmmSwapV2Event,
                },
                BlockMetaEvent,
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
    },
};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Stats {
    total_events: u64,
    swaps: u64,
    pool_updates: u64,
    opportunities: u64,
    high_confidence_opps: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║       ARBITRAGE MONITOR - Integrated System              ║");
    println!("║   Raydium CLMM + AMM V4 Flash Loan Detection            ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Create logs directory
    fs::create_dir_all("logs")?;

    // Initialize opportunity detector with optimized settings
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        1_000_000,           // 0.001 SOL min profit
        100_000_000_000,     // 100 SOL max loan
        10_000_000_000,      // 10 SOL min per pool liquidity
        50_000_000_000,      // 50 SOL min combined liquidity
    )));

    println!("✓ Arbitrage detector initialized");
    println!("   Min profit: 0.001 SOL");
    println!("   Max loan: 100 SOL");
    println!("   Min pool liquidity: 10 SOL\n");

    // Configure Yellowstone gRPC client
    let endpoint = std::env::var("YELLOWSTONE_ENDPOINT")
        .unwrap_or_else(|_| "https://solana-yellowstone-grpc.publicnode.com:443".to_string());

    let mut config = ClientConfig::low_latency();
    config.enable_metrics = true;

    let grpc = YellowstoneGrpc::new_with_config(
        endpoint.clone(),
        None,
        config,
    )?;

    println!("✓ gRPC client connected");
    println!("   Endpoint: {}\n", endpoint);

    // Create event callback
    let stats = Arc::new(Mutex::new(Stats::default()));
    let last_report = Arc::new(Mutex::new(Instant::now()));

    let callback = {
        let detector = detector.clone();
        let stats = stats.clone();
        let last_report = last_report.clone();

        move |event: Box<dyn UnifiedEvent>| {
            // Update counters
            {
                let mut s = stats.lock().unwrap();
                s.total_events += 1;
            }

            match_event!(event, {
                BlockMetaEvent => |_e: BlockMetaEvent| {
                    // Track blocks if needed
                },

                // CLMM Swap - Main arbitrage detection
                RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                    {
                        let mut s = stats.lock().unwrap();
                        s.swaps += 1;
                    }

                    // Check for arbitrage opportunity
                    let mut det = detector.lock().unwrap();
                    if let Some(opp) = det.analyze_clmm_swap_event(&e) {
                        let price_spread = (opp.price_b - opp.price_a) / opp.price_a * 100.0;
                        let profit_roi = (opp.expected_profit as f64 / opp.loan_amount as f64) * 100.0;

                        {
                            let mut s = stats.lock().unwrap();
                            s.opportunities += 1;

                            if opp.confidence >= 60 && price_spread >= 1.0 {
                                s.high_confidence_opps += 1;
                            }
                        }

                        // Display high-quality opportunities
                        if opp.confidence >= 60 && price_spread >= 1.0 {
                            let protocol_a = match opp.pool_a_protocol {
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                            };
                            let protocol_b = match opp.pool_b_protocol {
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                            };

                            println!("\n💰 ARBITRAGE OPPORTUNITY");
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                            println!("  Route: {} ↔ {}", protocol_a, protocol_b);
                            println!("  Profit: {:.6} SOL ({:.1}% ROI)",
                                opp.expected_profit as f64 / 1e9,
                                profit_roi);
                            println!("  Loan: {:.3} SOL", opp.loan_amount as f64 / 1e9);
                            println!("  Price Spread: {:.2}%", price_spread);
                            println!("  Confidence: {}%", opp.confidence);
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                        }
                    }
                },

                // AMM V4 Swap
                RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                    {
                        let mut s = stats.lock().unwrap();
                        s.swaps += 1;
                    }

                    let mut det = detector.lock().unwrap();
                    if let Some(opp) = det.analyze_ammv4_swap_event(&e) {
                        let price_spread = (opp.price_b - opp.price_a) / opp.price_a * 100.0;
                        let profit_roi = (opp.expected_profit as f64 / opp.loan_amount as f64) * 100.0;

                        {
                            let mut s = stats.lock().unwrap();
                            s.opportunities += 1;

                            if opp.confidence >= 60 && price_spread >= 1.0 {
                                s.high_confidence_opps += 1;
                            }
                        }

                        if opp.confidence >= 60 && price_spread >= 1.0 {
                            let protocol_a = match opp.pool_a_protocol {
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                            };
                            let protocol_b = match opp.pool_b_protocol {
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                                solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                            };

                            println!("\n💰 ARBITRAGE OPPORTUNITY");
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                            println!("  Route: {} ↔ {}", protocol_a, protocol_b);
                            println!("  Profit: {:.6} SOL ({:.1}% ROI)",
                                opp.expected_profit as f64 / 1e9,
                                profit_roi);
                            println!("  Loan: {:.3} SOL", opp.loan_amount as f64 / 1e9);
                            println!("  Price Spread: {:.2}%", price_spread);
                            println!("  Confidence: {}%", opp.confidence);
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                        }
                    }
                },

                // Pool state updates
                RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {
                    {
                        let mut s = stats.lock().unwrap();
                        s.pool_updates += 1;
                    }
                    detector.lock().unwrap().update_clmm_pool_state(&e);
                },

                RaydiumAmmV4AmmInfoAccountEvent => |e: RaydiumAmmV4AmmInfoAccountEvent| {
                    {
                        let mut s = stats.lock().unwrap();
                        s.pool_updates += 1;
                    }
                    detector.lock().unwrap().update_ammv4_pool_state(&e);
                },
            });

            // Print periodic stats every 30 seconds
            let mut last = last_report.lock().unwrap();
            if last.elapsed() >= Duration::from_secs(30) {
                *last = Instant::now();
                let s = stats.lock().unwrap();

                println!("\n📊 Performance Stats");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("  Total Events: {}", s.total_events);
                println!("  Swaps Processed: {}", s.swaps);
                println!("  Pool Updates: {}", s.pool_updates);
                println!("  Opportunities Found: {}", s.opportunities);
                println!("  High Confidence: {}", s.high_confidence_opps);
                if s.opportunities > 0 {
                    println!("  Quality Rate: {:.1}%",
                        (s.high_confidence_opps as f64 / s.opportunities as f64) * 100.0);
                }
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            }
        }
    };

    // Configure subscription
    let protocols = vec![
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
    ];

    let account_include = vec![
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

    println!("🚀 Starting event subscription...");
    println!("   Monitoring: Raydium CLMM + AMM V4");
    println!("   Press Ctrl+C to stop\n");

    // Subscribe to events
    grpc.subscribe_events_immediate(
        protocols,
        None,
        vec![transaction_filter],
        vec![account_filter],
        None,
        None,
        callback,
    )
    .await?;

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;

    println!("\n\n👋 Shutting down gracefully...");

    // Print final stats
    let final_stats = stats.lock().unwrap();
    println!("\n📊 Final Statistics");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Total Events: {}", final_stats.total_events);
    println!("  Swaps: {}", final_stats.swaps);
    println!("  Pool Updates: {}", final_stats.pool_updates);
    println!("  Total Opportunities: {}", final_stats.opportunities);
    println!("  High Confidence: {}", final_stats.high_confidence_opps);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}