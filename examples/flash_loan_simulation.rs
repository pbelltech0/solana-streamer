use solana_streamer_sdk::{
    flash_loan::{OpportunityDetector, FlashLoanTxBuilder, ArbitrageOpportunity, SimulationResult},
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
use solana_sdk::{signature::Keypair, pubkey::Pubkey};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

/// Detailed log entry for each arbitrage opportunity
#[derive(Serialize, Deserialize)]
struct OpportunityLogEntry {
    // Timestamp
    timestamp: String,
    timestamp_unix: i64,

    // Opportunity identification
    opportunity_id: u64,

    // Pool information
    pool_a: String,
    pool_a_protocol: String,
    pool_b: String,
    pool_b_protocol: String,

    // Token information
    base_token: String,
    quote_token: String,

    // Price data
    price_a: f64,
    price_b: f64,
    price_spread_pct: f64,

    // Financial calculations
    loan_amount_lamports: u64,
    loan_amount_sol: f64,
    expected_profit_lamports: u64,
    expected_profit_sol: f64,

    // Fee breakdown
    flash_loan_fee_lamports: u64,
    flash_loan_fee_sol: f64,
    flash_loan_fee_rate: f64,
    swap_fees_lamports: u64,
    swap_fees_sol: f64,
    swap_fee_rate: f64,
    total_fees_lamports: u64,
    total_fees_sol: f64,

    // Simulation results
    would_succeed: bool,
    net_profit_lamports: i64,
    net_profit_sol: f64,
    roi_pct: f64,
    failure_reason: Option<String>,

    // Quality metrics
    confidence: u8,

    // Additional validation data
    gross_profit_lamports: u64,
    gross_profit_sol: f64,
    fee_to_profit_ratio: f64,
}

impl OpportunityLogEntry {
    fn from_simulation(
        opportunity: &ArbitrageOpportunity,
        sim: &SimulationResult,
        opportunity_id: u64,
    ) -> Self {
        let timestamp = chrono::Utc::now();
        let price_spread_pct = (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0;
        let roi_pct = if sim.loan_amount > 0 {
            (sim.net_profit as f64 / sim.loan_amount as f64) * 100.0
        } else {
            0.0
        };
        let fee_to_profit_ratio = if sim.expected_profit > 0 {
            sim.total_fees as f64 / sim.expected_profit as f64
        } else {
            0.0
        };

        let protocol_a = match opportunity.pool_a_protocol {
            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
        };
        let protocol_b = match opportunity.pool_b_protocol {
            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
        };

        Self {
            timestamp: timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            timestamp_unix: timestamp.timestamp(),
            opportunity_id,

            pool_a: opportunity.pool_a.to_string(),
            pool_a_protocol: protocol_a.to_string(),
            pool_b: opportunity.pool_b.to_string(),
            pool_b_protocol: protocol_b.to_string(),

            base_token: opportunity.base_token.to_string(),
            quote_token: opportunity.quote_token.to_string(),

            price_a: opportunity.price_a,
            price_b: opportunity.price_b,
            price_spread_pct,

            loan_amount_lamports: sim.loan_amount,
            loan_amount_sol: sim.loan_amount as f64 / 1e9,
            expected_profit_lamports: sim.expected_profit,
            expected_profit_sol: sim.expected_profit as f64 / 1e9,

            flash_loan_fee_lamports: sim.flash_loan_fee,
            flash_loan_fee_sol: sim.flash_loan_fee as f64 / 1e9,
            flash_loan_fee_rate: 0.0009, // 0.09%
            swap_fees_lamports: sim.swap_fees,
            swap_fees_sol: sim.swap_fees as f64 / 1e9,
            swap_fee_rate: 0.005, // 0.5% total (2x 0.25%)
            total_fees_lamports: sim.total_fees,
            total_fees_sol: sim.total_fees as f64 / 1e9,

            would_succeed: sim.would_succeed,
            net_profit_lamports: sim.net_profit as i64,
            net_profit_sol: sim.net_profit as f64 / 1e9,
            roi_pct,
            failure_reason: if !sim.would_succeed { Some(sim.reason.clone()) } else { None },

            confidence: opportunity.confidence,

            gross_profit_lamports: sim.expected_profit,
            gross_profit_sol: sim.expected_profit as f64 / 1e9,
            fee_to_profit_ratio,
        }
    }

    /// Write as JSON to file
    fn write_json(&self, file: &mut std::fs::File) -> std::io::Result<()> {
        let json = serde_json::to_string(self)?;
        writeln!(file, "{}", json)?;
        file.flush()
    }

    /// Write as human-readable format to file
    fn write_readable(&self, file: &mut std::fs::File) -> std::io::Result<()> {
        writeln!(file, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")?;
        writeln!(file, "OPPORTUNITY #{} - {}", self.opportunity_id, self.timestamp)?;
        writeln!(file, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")?;
        writeln!(file, "STATUS: {}", if self.would_succeed { "✅ SUCCESS" } else { "❌ WOULD FAIL" })?;
        if let Some(ref reason) = self.failure_reason {
            writeln!(file, "FAILURE REASON: {}", reason)?;
        }
        writeln!(file)?;

        writeln!(file, "POOLS & PROTOCOLS:")?;
        writeln!(file, "  Pool A (Buy):  {} [{}]", self.pool_a, self.pool_a_protocol)?;
        writeln!(file, "  Pool B (Sell): {} [{}]", self.pool_b, self.pool_b_protocol)?;
        writeln!(file)?;

        writeln!(file, "TOKENS:")?;
        writeln!(file, "  Base Token:  {}", self.base_token)?;
        writeln!(file, "  Quote Token: {}", self.quote_token)?;
        writeln!(file)?;

        writeln!(file, "PRICE ANALYSIS:")?;
        writeln!(file, "  Price A:       {:.10}", self.price_a)?;
        writeln!(file, "  Price B:       {:.10}", self.price_b)?;
        writeln!(file, "  Price Spread:  {:.4}%", self.price_spread_pct)?;
        writeln!(file, "  Confidence:    {}%", self.confidence)?;
        writeln!(file)?;

        writeln!(file, "FINANCIAL DETAILS:")?;
        writeln!(file, "  Loan Amount:      {:>15} lamports ({:>12.6} SOL)",
            self.loan_amount_lamports, self.loan_amount_sol)?;
        writeln!(file, "  Expected Profit:  {:>15} lamports ({:>12.6} SOL)",
            self.expected_profit_lamports, self.expected_profit_sol)?;
        writeln!(file)?;

        writeln!(file, "FEE BREAKDOWN:")?;
        writeln!(file, "  Flash Loan Fee:   {:>15} lamports ({:>12.6} SOL) [{:.2}%]",
            self.flash_loan_fee_lamports, self.flash_loan_fee_sol, self.flash_loan_fee_rate * 100.0)?;
        writeln!(file, "  Swap Fees:        {:>15} lamports ({:>12.6} SOL) [{:.2}%]",
            self.swap_fees_lamports, self.swap_fees_sol, self.swap_fee_rate * 100.0)?;
        writeln!(file, "  Total Fees:       {:>15} lamports ({:>12.6} SOL)",
            self.total_fees_lamports, self.total_fees_sol)?;
        writeln!(file, "  Fee/Profit Ratio: {:.4}", self.fee_to_profit_ratio)?;
        writeln!(file)?;

        writeln!(file, "NET RESULTS:")?;
        writeln!(file, "  Net Profit:       {:>15} lamports ({:>12.6} SOL)",
            self.net_profit_lamports, self.net_profit_sol)?;
        writeln!(file, "  ROI:              {:.4}%", self.roi_pct)?;
        writeln!(file)?;

        file.flush()
    }
}

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
    // Create both JSON and human-readable log files
    let json_log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/flash_loan_simulations.jsonl")?;
    let json_log_file = Arc::new(Mutex::new(json_log_file));

    let readable_log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/flash_loan_simulations.log")?;
    let readable_log_file = Arc::new(Mutex::new(readable_log_file));

    println!("📝 Logging simulations to:");
    println!("   JSON (machine-readable): logs/flash_loan_simulations.jsonl");
    println!("   Human-readable:          logs/flash_loan_simulations.log\n");

    // Initialize opportunity detector
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        100_000,            // Lower threshold: 0.0001 SOL for testing
        100_000_000_000,    // 100 SOL max
        10_000_000_000,     // 10 SOL min per pool
        50_000_000_000,     // 50 SOL min combined
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
    let json_log_clone = json_log_file.clone();
    let readable_log_clone = readable_log_file.clone();
    let stats_clone = stats.clone();

    let callback = move |event: Box<dyn UnifiedEvent>| {
        let detector = detector_clone.clone();
        let tx_builder = tx_builder_clone.clone();
        let json_log = json_log_clone.clone();
        let readable_log = readable_log_clone.clone();
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
                if let Some(opportunity) = det.analyze_clmm_swap_event(&swap_event) {
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

                    // Create detailed log entry
                    let opportunity_id = {
                        let s = stats.lock().unwrap();
                        s.opportunities_detected
                    };

                    let log_entry = OpportunityLogEntry::from_simulation(
                        &opportunity,
                        &sim,
                        opportunity_id,
                    );

                    // Write to JSON log (one line per entry for easy parsing)
                    if let Ok(mut file) = json_log.lock() {
                        if let Err(e) = log_entry.write_json(&mut *file) {
                            eprintln!("Failed to write JSON log: {}", e);
                        }
                    }

                    // Write to human-readable log
                    if let Ok(mut file) = readable_log.lock() {
                        if let Err(e) = log_entry.write_readable(&mut *file) {
                            eprintln!("Failed to write readable log: {}", e);
                        }
                    }
                }
            },
            RaydiumClmmPoolStateAccountEvent => |pool_event: RaydiumClmmPoolStateAccountEvent| {
                let mut s = stats.lock().unwrap();
                s.pool_state_updates += 1;
                drop(s);

                let mut det = detector.lock().unwrap();
                det.update_clmm_pool_state(&pool_event);
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