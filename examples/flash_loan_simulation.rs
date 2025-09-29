use solana_streamer_sdk::{
    flash_loan::{OpportunityDetector, FlashLoanTxBuilder},
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
use solana_sdk::signature::Keypair;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    fs::create_dir_all("logs")?;

    println!("🧪 Flash Loan Simulation Mode");
    println!("================================");
    println!("This mode runs full flash loan logic WITHOUT executing on-chain");
    println!("✅ Detects opportunities");
    println!("✅ Calculates fees and profit");
    println!("✅ Shows detailed simulation results");
    println!("❌ Does NOT submit transactions\n");

    run_simulation().await?;
    Ok(())
}

async fn run_simulation() -> Result<(), Box<dyn std::error::Error>> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/flash_loan_simulations.log")?;
    let log_file = Arc::new(Mutex::new(log_file));

    println!("📝 Logging simulations to: logs/flash_loan_simulations.log\n");

    // Initialize opportunity detector
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        100_000,            // Lower threshold: 0.0001 SOL for testing
        100_000_000_000,    // 100 SOL max
    )));

    // Initialize transaction builder in SIMULATION MODE
    let dummy_keypair = Keypair::new();
    // Use a valid dummy pubkey for simulation (doesn't matter since we're not executing)
    let receiver_program = Pubkey::new_unique();

    let tx_builder = Arc::new(FlashLoanTxBuilder::new_simulation_mode(
        "https://api.mainnet-beta.solana.com".to_string(),
        dummy_keypair,
        receiver_program,
    ));

    let stats = Arc::new(Mutex::new(SimStats::default()));

    println!("⚙️  Configuration:");
    println!("   Mode: SIMULATION (safe)");
    println!("   Min profit: 0.0001 SOL");
    println!("   Max loan: 100 SOL\n");

    let mut config = ClientConfig::low_latency();
    config.enable_metrics = true;

    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        config,
    )?;

    println!("✅ Connected to Yellowstone gRPC\n");

    let detector_clone = detector.clone();
    let tx_builder_clone = tx_builder.clone();
    let log_file_clone = log_file.clone();
    let stats_clone = stats.clone();

    let callback = move |event: Box<dyn UnifiedEvent>| {
        let detector = detector_clone.clone();
        let tx_builder = tx_builder_clone.clone();
        let log_file = log_file_clone.clone();
        let stats = stats_clone.clone();

        {
            let mut s = stats.lock().unwrap();
            s.total_events += 1;
        }

        match_event!(event, {
            RaydiumClmmSwapV2Event => |swap_event: RaydiumClmmSwapV2Event| {
                let mut s = stats.lock().unwrap();
                s.swap_events += 1;
                drop(s);

                let mut det = detector.lock().unwrap();
                if let Some(opportunity) = det.analyze_swap_event(&swap_event) {
                    let mut s = stats.lock().unwrap();
                    s.opportunities_detected += 1;
                    drop(s);

                    // Run simulation!
                    let sim = tx_builder.simulate_flash_loan_detailed(&opportunity);

                    // Print detailed simulation
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("🧪 FLASH LOAN SIMULATION #{}", {
                        let s = stats.lock().unwrap();
                        s.opportunities_detected
                    });
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                    println!("📊 Opportunity Details:");
                    println!("   Pool A (buy):  {}", opportunity.pool_a);
                    println!("   Pool B (sell): {}", opportunity.pool_b);
                    println!("   Base Token:    {}", opportunity.base_token);
                    println!("   Quote Token:   {}", opportunity.quote_token);
                    println!("   Price A:       {:.10}", opportunity.price_a);
                    println!("   Price B:       {:.10}", opportunity.price_b);
                    println!("   Price Spread:  {:.2}%\n",
                        (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0);

                    println!("💰 Financial Breakdown:");
                    println!("   Loan Amount:      {:>15} lamports ({:.6} SOL)",
                        sim.loan_amount, sim.loan_amount as f64 / 1e9);
                    println!("   Expected Profit:  {:>15} lamports ({:.6} SOL)",
                        sim.expected_profit, sim.expected_profit as f64 / 1e9);
                    println!("\n   📝 Fee Breakdown:");
                    println!("      Flash Loan Fee: {:>15} lamports ({:.6} SOL) [0.09%]",
                        sim.flash_loan_fee, sim.flash_loan_fee as f64 / 1e9);
                    println!("      Swap Fees:      {:>15} lamports ({:.6} SOL) [0.50%]",
                        sim.swap_fees, sim.swap_fees as f64 / 1e9);
                    println!("      Total Fees:     {:>15} lamports ({:.6} SOL)\n",
                        sim.total_fees, sim.total_fees as f64 / 1e9);

                    if sim.would_succeed {
                        println!("✅ SIMULATION RESULT: SUCCESS");
                        println!("   Net Profit:       {:>15} lamports ({:.6} SOL)",
                            sim.net_profit, sim.net_profit as f64 / 1e9);
                        println!("   ROI:              {:.2}%",
                            (sim.net_profit as f64 / sim.loan_amount as f64) * 100.0);

                        let mut s = stats.lock().unwrap();
                        s.successful_simulations += 1;
                        s.total_simulated_profit += sim.net_profit;
                    } else {
                        println!("❌ SIMULATION RESULT: WOULD FAIL");
                        println!("   Reason: {}", sim.reason);

                        let mut s = stats.lock().unwrap();
                        s.failed_simulations += 1;
                    }

                    println!("   Confidence: {}%", opportunity.confidence);
                    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

                    // Log to file
                    if let Ok(mut file) = log_file.lock() {
                        let log_entry = format!(
                            "{} | {} | Profit: {:.6} SOL | Spread: {:.2}% | Confidence: {}% | Pool A: {} | Pool B: {}\n",
                            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                            if sim.would_succeed { "SUCCESS" } else { "FAIL   " },
                            sim.net_profit as f64 / 1e9,
                            (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0,
                            opportunity.confidence,
                            opportunity.pool_a,
                            opportunity.pool_b
                        );
                        let _ = file.write_all(log_entry.as_bytes());
                        let _ = file.flush();
                    }
                }
            },
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
                println!("📊 Running Statistics:");
                println!("   Total Events:        {}", s.total_events);
                println!("   Swap Events:         {}", s.swap_events);
                println!("   Pool Updates:        {}", s.pool_state_updates);
                println!("   Opportunities:       {}", s.opportunities_detected);
                println!("   Successful Sims:     {} ✅", s.successful_simulations);
                println!("   Failed Sims:         {} ❌", s.failed_simulations);
                if s.successful_simulations > 0 {
                    println!("   Total Profit (sim):  {:.6} SOL", s.total_simulated_profit as f64 / 1e9);
                    println!("   Avg Profit:          {:.6} SOL",
                        (s.total_simulated_profit as f64 / s.successful_simulations as f64) / 1e9);
                }
                println!();
            }
        }
    };

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

    println!("🔄 Subscribing to events...\n");

    grpc.subscribe_events_immediate(
        protocols,
        None,
        transaction_filters,
        account_filters,
        None,
        None,
        callback,
    )
    .await?;

    println!("Press Ctrl+C to stop...");
    tokio::signal::ctrl_c().await?;

    // Print final stats
    let s = stats.lock().unwrap();
    println!("\n📊 Final Statistics:");
    println!("   Total Events:        {}", s.total_events);
    println!("   Opportunities:       {}", s.opportunities_detected);
    println!("   Successful Sims:     {}", s.successful_simulations);
    println!("   Failed Sims:         {}", s.failed_simulations);
    if s.successful_simulations > 0 {
        println!("   Total Profit (sim):  {:.6} SOL", s.total_simulated_profit as f64 / 1e9);
        println!("   Success Rate:        {:.1}%",
            (s.successful_simulations as f64 / s.opportunities_detected as f64) * 100.0);
    }

    Ok(())
}

#[derive(Default)]
struct SimStats {
    total_events: u64,
    swap_events: u64,
    pool_state_updates: u64,
    opportunities_detected: u64,
    successful_simulations: u64,
    failed_simulations: u64,
    total_simulated_profit: u64,
}