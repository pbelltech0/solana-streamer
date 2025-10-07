use solana_streamer_sdk::{
    match_event,
    streaming::{
        ArbitrageDetector, ArbitrageOpportunity,
        event_parser::{
            common::{filter::EventTypeFilter, EventType},
            protocols::{
                jupiter_agg_v6::{
                    events::{JupiterAggV6ExactOutRouteEvent, JupiterAggV6RouteEvent, JupiterAggV6FeeEvent},
                    parser::JUPITER_AGG_V6_PROGRAM_ID,
                },
                raydium_amm_v4::{
                    events::RaydiumAmmV4SwapEvent, parser::RAYDIUM_AMM_V4_PROGRAM_ID,
                },
                raydium_clmm::{
                    events::{RaydiumClmmSwapEvent, RaydiumClmmSwapV2Event},
                    parser::RAYDIUM_CLMM_PROGRAM_ID,
                },
                raydium_cpmm::{events::RaydiumCpmmSwapEvent, parser::RAYDIUM_CPMM_PROGRAM_ID},
                block::block_meta_event::BlockMetaEvent,
            },
            Protocol, UnifiedEvent,
        },
        grpc::ClientConfig,
        yellowstone_grpc::{AccountFilter, TransactionFilter},
        YellowstoneGrpc,
    },
};
use log::info;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::{sleep, interval};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger with custom format
    env_logger::Builder::from_default_env()
        .format_timestamp_secs()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting Arbitrage Detector...");
    info!("Monitoring Jupiter and Raydium DEXes for arbitrage opportunities");
    println!("================================================\n");

    // Run with automatic reconnection
    loop {
        match run_arbitrage_detector_with_reconnect().await {
            Ok(_) => {
                println!("Arbitrage detector completed successfully");
                break;
            }
            Err(e) => {
                eprintln!("Error in arbitrage detector: {:?}", e);
                eprintln!("Attempting to reconnect in 5 seconds...");
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    Ok(())
}

async fn run_arbitrage_detector_with_reconnect() -> Result<(), Box<dyn std::error::Error>> {
    const MAX_RETRIES: u32 = 3;
    let mut retry_count = 0;

    loop {
        match run_arbitrage_detector().await {
            Ok(_) => return Ok(()),
            Err(e) => {
                retry_count += 1;
                eprintln!("Connection error (attempt {}/{}): {:?}", retry_count, MAX_RETRIES, e);

                if retry_count >= MAX_RETRIES {
                    return Err(e);
                }

                // Exponential backoff
                let delay = Duration::from_secs(2u64.pow(retry_count));
                eprintln!("Retrying in {} seconds...", delay.as_secs());
                sleep(delay).await;
            }
        }
    }
}

async fn run_arbitrage_detector() -> Result<(), Box<dyn std::error::Error>> {
    // Create arbitrage detector with:
    // - Minimum 0.5% profit threshold
    // - 30 second maximum quote age
    let detector = Arc::new(Mutex::new(ArbitrageDetector::new(0.5, 30)));

    // Connection health monitoring
    let is_connected = Arc::new(AtomicBool::new(true));
    let last_event_time = Arc::new(Mutex::new(Instant::now()));
    let event_count = Arc::new(AtomicU64::new(0));

    // Create low-latency configuration with connection resilience
    let mut config: ClientConfig = ClientConfig::low_latency();
    config.enable_metrics = true;
    // Set reasonable timeouts for better connection stability
    config.connection.connect_timeout = 30;
    config.connection.request_timeout = 60;
    config.connection.max_decoding_message_size = 64 * 1024 * 1024; // 64MB

    let grpc = match YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        config,
    ) {
        Ok(client) => {
            println!("✓ GRPC client created successfully");
            client
        }
        Err(e) => {
            eprintln!("Failed to create GRPC client: {:?}", e);
            return Err(e.into());
        }
    };

    // Start connection health monitor
    let is_connected_clone = is_connected.clone();
    let last_event_clone = last_event_time.clone();
    let event_count_clone = event_count.clone();
    let health_monitor = tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(10));
        loop {
            interval.tick().await;

            let last_event = last_event_clone.lock().unwrap().clone();
            let time_since_last = Instant::now().duration_since(last_event);

            if time_since_last > Duration::from_secs(60) {
                eprintln!("⚠️ Warning: No events received for {} seconds", time_since_last.as_secs());
                is_connected_clone.store(false, Ordering::SeqCst);
            } else {
                let events = event_count_clone.load(Ordering::SeqCst);
                println!("📊 Connection healthy - Events received: {} | Last event: {}s ago",
                         events, time_since_last.as_secs());
            }
        }
    });

    let callback = create_arbitrage_callback_with_monitoring(
        detector.clone(),
        last_event_time.clone(),
        event_count.clone(),
    );

    // Monitor Jupiter and Raydium protocols
    let protocols = vec![
        Protocol::JupiterAggV6,
        Protocol::RaydiumAmmV4,
        Protocol::RaydiumClmm,
        Protocol::RaydiumCpmm,
    ];

    println!("✓ Monitoring protocols: {:?}", protocols);

    // Filter accounts - listen to all DEX programs
    let account_include = vec![
        JUPITER_AGG_V6_PROGRAM_ID.to_string(),
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(),
        RAYDIUM_CLMM_PROGRAM_ID.to_string(),
        RAYDIUM_CPMM_PROGRAM_ID.to_string(),
    ];

    // Listen to transaction data
    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude: vec![],
        account_required: vec![],
    };

    // Listen to account data
    let account_filter = AccountFilter {
        account: vec![],
        owner: account_include.clone(),
        filters: vec![],
    };

    // Event filtering - Include all swap events and fee events
    let event_type_filter = Some(EventTypeFilter {
        include: vec![
            // Jupiter events
            EventType::JupiterAggV6Route,
            EventType::JupiterAggV6ExactOutRoute,
            EventType::JupiterAggV6Fee, // Fee tracking for accurate profit calculation
            // Raydium events
            EventType::RaydiumAmmV4SwapBaseIn,
            EventType::RaydiumAmmV4SwapBaseOut,
            EventType::RaydiumClmmSwap,
            EventType::RaydiumClmmSwapV2,
            EventType::RaydiumCpmmSwapBaseInput,
            EventType::RaydiumCpmmSwapBaseOutput,
        ],
    });

    println!("\n================================================");
    println!("Starting subscription to DEX events...");
    println!("Monitoring programs:");
    println!("  - Jupiter Agg V6:  {}", JUPITER_AGG_V6_PROGRAM_ID);
    println!("  - Raydium AMM V4:  {}", RAYDIUM_AMM_V4_PROGRAM_ID);
    println!("  - Raydium CLMM:    {}", RAYDIUM_CLMM_PROGRAM_ID);
    println!("  - Raydium CPMM:    {}", RAYDIUM_CPMM_PROGRAM_ID);
    println!("\nPress Ctrl+C to stop...");
    println!("================================================\n");

    // Subscribe with timeout
    let subscribe_result = tokio::time::timeout(
        Duration::from_secs(30),
        grpc.subscribe_events_immediate(
            protocols,
            None,
            vec![transaction_filter],
            vec![account_filter],
            event_type_filter,
            None,
            callback,
        )
    ).await;

    match subscribe_result {
        Ok(Ok(_)) => {
            println!("✓ Successfully subscribed to events");
        }
        Ok(Err(e)) => {
            eprintln!("✗ Subscription failed: {:?}", e);
            return Err(e.into());
        }
        Err(_) => {
            eprintln!("✗ Subscription timeout after 30 seconds");
            return Err("Subscription timeout".into());
        }
    }

    // Set up graceful shutdown
    let grpc_clone = grpc.clone();
    let is_connected_clone = is_connected.clone();
    let shutdown_handle = tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1000)) => {
                println!("\nAuto-stopping after 1000 seconds...");
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n\nReceived Ctrl+C signal, shutting down gracefully...");
            }
            _ = async {
                // Monitor connection health
                loop {
                    sleep(Duration::from_secs(5)).await;
                    if !is_connected_clone.load(Ordering::SeqCst) {
                        eprintln!("\n❌ Connection lost, initiating shutdown...");
                        break;
                    }
                }
            } => {
                println!("\nConnection issue detected, stopping...");
            }
        }
        grpc_clone.stop().await;
    });

    // Wait for shutdown
    let _shutdown_result = shutdown_handle.await;

    // Cleanup health monitor
    health_monitor.abort();

    // Print final statistics
    println!("\n================================================");
    println!("Arbitrage Detector Statistics");
    println!("================================================");
    let detector_lock = detector.lock().unwrap();
    let tracked_pairs = detector_lock.get_tracked_pairs();
    println!("Total tracked token pairs: {}", tracked_pairs.len());
    if !tracked_pairs.is_empty() {
        println!("\nTop 5 most active pairs:");
        let pairs: Vec<_> = tracked_pairs.iter().take(5).collect();
        for (i, pair) in pairs.iter().enumerate() {
            println!("  {}. {} <-> {}", i + 1, pair.base, pair.quote);
        }
    }
    println!("================================================");

    Ok(())
}

fn create_arbitrage_callback_with_monitoring(
    detector: Arc<Mutex<ArbitrageDetector>>,
    last_event_time: Arc<Mutex<Instant>>,
    event_count: Arc<AtomicU64>,
) -> impl Fn(Box<dyn UnifiedEvent>) {
    move |event: Box<dyn UnifiedEvent>| {
        // Update monitoring metrics
        *last_event_time.lock().unwrap() = Instant::now();
        event_count.fetch_add(1, Ordering::SeqCst);

        let mut opportunities = Vec::new();

        match_event!(event, {
            BlockMetaEvent => |_e: BlockMetaEvent| {
                // Ignore block meta events for arbitrage detection
            },
            // Jupiter Fee Event (for accurate profit calculation)
            JupiterAggV6FeeEvent => |e: JupiterAggV6FeeEvent| {
                println!("💰 Jupiter Fee: {} lamports (mint: {})",
                    e.amount,
                    e.mint
                );

                let mut detector = detector.lock().unwrap();
                detector.process_fee_event(&e);
            },
            // Jupiter Aggregator V6 Route Event
            JupiterAggV6RouteEvent => |e: JupiterAggV6RouteEvent| {
                println!("🔵 Jupiter Swap: {} {} -> {} {}",
                    e.in_amount,
                    e.source_mint,
                    e.quoted_out_amount,
                    e.destination_mint
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_jupiter_route(&e));
            },
            // Jupiter Aggregator V6 Exact Out Route Event
            JupiterAggV6ExactOutRouteEvent => |e: JupiterAggV6ExactOutRouteEvent| {
                println!("🔵 Jupiter ExactOut Swap: {} {} -> {} {}",
                    e.quoted_in_amount,
                    e.source_mint,
                    e.out_amount,
                    e.destination_mint
                );

                // Convert to route event format for processing
                let route_event = JupiterAggV6RouteEvent {
                    metadata: e.metadata,
                    in_amount: e.quoted_in_amount,
                    quoted_out_amount: e.out_amount,
                    slippage_bps: e.slippage_bps,
                    platform_fee_bps: e.platform_fee_bps,
                    token_program: e.token_program,
                    user_transfer_authority: e.user_transfer_authority,
                    user_source_token_account: e.user_source_token_account,
                    user_destination_token_account: e.user_destination_token_account,
                    destination_token_account: e.destination_token_account,
                    source_mint: e.source_mint,
                    destination_mint: e.destination_mint,
                    platform_fee_account: e.platform_fee_account,
                    event_authority: e.event_authority,
                    program: e.program,
                };

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_jupiter_route(&route_event));
            },
            // Raydium AMM V4 Swap Event
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                println!("🟣 Raydium AMM V4 Swap: pool {}", e.amm);

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_amm_v4_swap(&e));
            },
            // Raydium CLMM Swap Event
            RaydiumClmmSwapEvent => |e: RaydiumClmmSwapEvent| {
                println!("🟣 Raydium CLMM Swap: {} -> {} (pool: {})",
                    e.amount,
                    e.other_amount_threshold,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_clmm_swap(&e));
            },
            // Raydium CLMM Swap V2 Event
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                println!("🟣 Raydium CLMM V2 Swap: {} -> {} (pool: {})",
                    e.amount,
                    e.other_amount_threshold,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_clmm_swap_v2(&e));
            },
            // Raydium CPMM Swap Event
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                let (in_amt, out_amt) = if e.amount_in > 0 {
                    (e.amount_in, e.minimum_amount_out)
                } else {
                    (e.max_amount_in, e.amount_out)
                };

                println!("🟣 Raydium CPMM Swap: {} {} -> {} {} (pool: {})",
                    in_amt,
                    e.input_token_mint,
                    out_amt,
                    e.output_token_mint,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_cpmm_swap(&e));
            },
        });

        // Print arbitrage opportunities
        for opp in opportunities {
            print_arbitrage_opportunity(&opp);
        }
    }
}

// Keep the original callback for backward compatibility
#[allow(dead_code)]
fn create_arbitrage_callback(
    detector: Arc<Mutex<ArbitrageDetector>>,
) -> impl Fn(Box<dyn UnifiedEvent>) {
    move |event: Box<dyn UnifiedEvent>| {
        let mut opportunities = Vec::new();

        match_event!(event, {
            BlockMetaEvent => |_e: BlockMetaEvent| {
                // Ignore block meta events for arbitrage detection
            },
            // Jupiter Fee Event (for accurate profit calculation)
            JupiterAggV6FeeEvent => |e: JupiterAggV6FeeEvent| {
                println!("💰 Jupiter Fee: {} lamports (mint: {})",
                    e.amount,
                    e.mint
                );

                let mut detector = detector.lock().unwrap();
                detector.process_fee_event(&e);
            },
            // Jupiter Aggregator V6 Route Event
            JupiterAggV6RouteEvent => |e: JupiterAggV6RouteEvent| {
                println!("🔵 Jupiter Swap: {} {} -> {} {}",
                    e.in_amount,
                    e.source_mint,
                    e.quoted_out_amount,
                    e.destination_mint
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_jupiter_route(&e));
            },
            // Jupiter Aggregator V6 Exact Out Route Event
            JupiterAggV6ExactOutRouteEvent => |e: JupiterAggV6ExactOutRouteEvent| {
                println!("🔵 Jupiter ExactOut Swap: {} {} -> {} {}",
                    e.quoted_in_amount,
                    e.source_mint,
                    e.out_amount,
                    e.destination_mint
                );

                // Convert to route event format for processing
                let route_event = JupiterAggV6RouteEvent {
                    metadata: e.metadata,
                    in_amount: e.quoted_in_amount,
                    quoted_out_amount: e.out_amount,
                    slippage_bps: e.slippage_bps,
                    platform_fee_bps: e.platform_fee_bps,
                    token_program: e.token_program,
                    user_transfer_authority: e.user_transfer_authority,
                    user_source_token_account: e.user_source_token_account,
                    user_destination_token_account: e.user_destination_token_account,
                    destination_token_account: e.destination_token_account,
                    source_mint: e.source_mint,
                    destination_mint: e.destination_mint,
                    platform_fee_account: e.platform_fee_account,
                    event_authority: e.event_authority,
                    program: e.program,
                };

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_jupiter_route(&route_event));
            },
            // Raydium AMM V4 Swap Event
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                println!("🟣 Raydium AMM V4 Swap: pool {}", e.amm);

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_amm_v4_swap(&e));
            },
            // Raydium CLMM Swap Event
            RaydiumClmmSwapEvent => |e: RaydiumClmmSwapEvent| {
                println!("🟣 Raydium CLMM Swap: {} -> {} (pool: {})",
                    e.amount,
                    e.other_amount_threshold,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_clmm_swap(&e));
            },
            // Raydium CLMM Swap V2 Event
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                println!("🟣 Raydium CLMM V2 Swap: {} -> {} (pool: {})",
                    e.amount,
                    e.other_amount_threshold,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_clmm_swap_v2(&e));
            },
            // Raydium CPMM Swap Event
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                let (in_amt, out_amt) = if e.amount_in > 0 {
                    (e.amount_in, e.minimum_amount_out)
                } else {
                    (e.max_amount_in, e.amount_out)
                };

                println!("🟣 Raydium CPMM Swap: {} {} -> {} {} (pool: {})",
                    in_amt,
                    e.input_token_mint,
                    out_amt,
                    e.output_token_mint,
                    e.pool_state
                );

                let mut detector = detector.lock().unwrap();
                opportunities.extend(detector.process_raydium_cpmm_swap(&e));
            },
        });

        // Print arbitrage opportunities
        for opp in opportunities {
            print_arbitrage_opportunity(&opp);
        }
    }
}

fn print_arbitrage_opportunity(opp: &ArbitrageOpportunity) {
    // Only show opportunities that are profitable after fees
    let is_profitable = opp.is_profitable_after_fees();
    let icon = if is_profitable { "🚀" } else { "⚠️" };

    println!("\n{} ARBITRAGE OPPORTUNITY DETECTED! {}", icon, icon);
    println!("================================================");
    println!("Token Pair: {} <-> {}", opp.token_pair.base, opp.token_pair.quote);
    println!("Buy on:  {:?} at price {:.6}", opp.buy_dex, opp.buy_price);
    println!("Sell on: {:?} at price {:.6}", opp.sell_dex, opp.sell_price);
    println!("\n--- Profit Analysis ---");
    println!("Gross Profit:     {:.2}%", opp.profit_percentage);
    println!("Total Fees:       {:.2}%", opp.total_fee_percentage);
    println!("Est. Gas Cost:    {:.2}%", opp.estimated_gas_cost / 100.0);
    println!("Net Profit:       {:.2}%", opp.net_profit_percentage);

    if !is_profitable {
        println!("\n⚠️  WARNING: Not profitable after fees and gas costs!");
    }

    // Calculate example profit for 1 SOL (1_000_000_000 lamports)
    let example_input = 1_000_000_000.0;
    let example_gross_profit = opp.calculate_profit(example_input);
    let example_net_profit = opp.calculate_net_profit(example_input);

    println!("\n--- Example (1 SOL input) ---");
    println!("Gross Profit: {:.6} SOL ({:.2} lamports)",
        example_gross_profit / 1_000_000_000.0,
        example_gross_profit
    );
    println!("Net Profit:   {:.6} SOL ({:.2} lamports)",
        example_net_profit / 1_000_000_000.0,
        example_net_profit
    );

    println!("\n--- Quote Details ---");
    println!("Buy:  {} in -> {} out (fee: {}bps)",
        opp.buy_quote.input_amount,
        opp.buy_quote.output_amount,
        opp.buy_quote.platform_fee_bps.unwrap_or(0)
    );
    println!("Sell: {} in -> {} out (fee: {}bps)",
        opp.sell_quote.input_amount,
        opp.sell_quote.output_amount,
        opp.sell_quote.platform_fee_bps.unwrap_or(0)
    );
    println!("================================================\n");
}
