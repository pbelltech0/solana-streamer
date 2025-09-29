use solana_streamer_sdk::{
    flash_loan::OpportunityDetector,
    match_event,
    streaming::{
        event_parser::{
            protocols::raydium_clmm::{
                parser::RAYDIUM_CLMM_PROGRAM_ID, RaydiumClmmPoolStateAccountEvent,
                RaydiumClmmSwapV2Event,
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
    },
};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create logs directory if it doesn't exist
    fs::create_dir_all("logs")?;

    println!("🔍 Starting Arbitrage Opportunity Detector...");
    println!("📊 Monitoring Raydium CLMM pools for arbitrage opportunities");
    println!("💡 This is a test mode - NO flash loans will be executed\n");

    detect_opportunities().await?;
    Ok(())
}

async fn detect_opportunities() -> Result<(), Box<dyn std::error::Error>> {
    // Create log file for opportunities
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/arbitrage_opportunities.log")?;
    let log_file = Arc::new(Mutex::new(log_file));

    println!("📝 Logging opportunities to: logs/arbitrage_opportunities.log\n");

    // Initialize opportunity detector
    // Parameters:
    // - min_profit_threshold: 1,000,000 lamports = 0.001 SOL
    // - max_loan_amount: 100,000,000,000 lamports = 100 SOL
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        1_000_000,           // 0.001 SOL minimum profit
        100_000_000_000,     // 100 SOL max loan
    )));

    // Statistics tracking
    let stats = Arc::new(Mutex::new(DetectorStats::default()));

    println!("⚙️  Configuration:");
    println!("   Min profit threshold: 0.001 SOL");
    println!("   Max loan amount: 100 SOL");
    println!("   Monitoring: Raydium CLMM swaps and pool states\n");

    // Create gRPC client with low-latency configuration
    let mut config = ClientConfig::low_latency();
    config.enable_metrics = true;

    // Use a public endpoint or your own
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None, // No auth token for public endpoint
        config,
    )?;

    println!("✅ GRPC client created successfully");

    let detector_clone = detector.clone();
    let log_file_clone = log_file.clone();
    let stats_clone = stats.clone();

    let callback = move |event: Box<dyn UnifiedEvent>| {
        let detector = detector_clone.clone();
        let log_file = log_file_clone.clone();
        let stats = stats_clone.clone();

        // Update stats
        {
            let mut s = stats.lock().unwrap();
            s.total_events += 1;
        }

        // Handle Swap events - these are the primary trigger for opportunities
        match_event!(event, {
            RaydiumClmmSwapV2Event => |swap_event: RaydiumClmmSwapV2Event| {
                let mut s = stats.lock().unwrap();
                s.swap_events += 1;
                drop(s);

                // Analyze the swap for arbitrage opportunities
                let mut det = detector.lock().unwrap();
                if let Some(opportunity) = det.analyze_swap_event(&swap_event) {
                    let mut s = stats.lock().unwrap();
                    s.opportunities_detected += 1;
                    drop(s);

                    // Log to console
                    println!("🎯 ARBITRAGE OPPORTUNITY DETECTED!");
                    println!("   Pool A (buy): {}", opportunity.pool_a);
                    println!("   Pool B (sell): {}", opportunity.pool_b);
                    println!("   Base Token: {}", opportunity.base_token);
                    println!("   Quote Token: {}", opportunity.quote_token);
                    println!("   Price A: {:.10}", opportunity.price_a);
                    println!("   Price B: {:.10}", opportunity.price_b);
                    println!("   Price Spread: {:.2}%",
                        (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0);
                    println!("   Expected Profit: {} lamports ({:.6} SOL)",
                        opportunity.expected_profit,
                        opportunity.expected_profit as f64 / 1_000_000_000.0);
                    println!("   Loan Amount: {} lamports ({:.3} SOL)",
                        opportunity.loan_amount,
                        opportunity.loan_amount as f64 / 1_000_000_000.0);
                    println!("   Confidence: {}%", opportunity.confidence);
                    println!("   Timestamp: {}\n", opportunity.timestamp);

                    // Log to file
                    if let Ok(mut file) = log_file.lock() {
                        let log_entry = format!(
                            "{} | Profit: {:.6} SOL | Spread: {:.2}% | Confidence: {}% | Pool A: {} | Pool B: {}\n",
                            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                            opportunity.expected_profit as f64 / 1_000_000_000.0,
                            (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0,
                            opportunity.confidence,
                            opportunity.pool_a,
                            opportunity.pool_b
                        );
                        let _ = file.write_all(log_entry.as_bytes());
                        let _ = file.flush();
                    }

                    // In production, this is where you would execute the flash loan
                    // For now, we just log it
                    println!("💭 In production mode, this would trigger a flash loan execution");
                    println!("   Status: TEST MODE - no action taken\n");
                }
            },
            // Handle Pool State updates - these keep our liquidity/price data fresh
            RaydiumClmmPoolStateAccountEvent => |pool_event: RaydiumClmmPoolStateAccountEvent| {
                let mut s = stats.lock().unwrap();
                s.pool_state_updates += 1;
                drop(s);

                let mut det = detector.lock().unwrap();
                det.update_pool_state(&pool_event);
            }
        });

        // Print stats every 100 events
        {
            let s = stats.lock().unwrap();
            if s.total_events % 100 == 0 {
                println!("📈 Statistics (total events: {})", s.total_events);
                println!("   Swap events: {}", s.swap_events);
                println!("   Pool state updates: {}", s.pool_state_updates);
                println!("   Opportunities detected: {}\n", s.opportunities_detected);
            }
        }
    };

    // Subscribe to Raydium CLMM events
    let protocols = vec![Protocol::RaydiumClmm];

    let transaction_filters = vec![TransactionFilter {
        account_include: vec![RAYDIUM_CLMM_PROGRAM_ID.to_string()],
        account_exclude: vec![],
        account_required: vec![],
    }];

    let account_filters = vec![AccountFilter {
        account: vec![],
        owner: vec![RAYDIUM_CLMM_PROGRAM_ID.to_string()],
        filters: vec![],
    }];

    println!("🔄 Subscribing to Raydium CLMM events...");
    println!("   Waiting for swap events and pool state updates...\n");

    grpc.subscribe_events_immediate(
        protocols,
        None,           // commitment level
        transaction_filters,
        account_filters,
        None,           // event type filter (include all)
        None,           // ping interval
        callback,
    )
    .await?;

    println!("Press Ctrl+C to stop...");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

#[derive(Default)]
struct DetectorStats {
    total_events: u64,
    swap_events: u64,
    pool_state_updates: u64,
    opportunities_detected: u64,
}