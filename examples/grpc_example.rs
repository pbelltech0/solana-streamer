use solana_streamer_sdk::{
    flash_loan::OpportunityDetector,
    match_event,
    streaming::{
        event_parser::{
            core::account_event_parser::{NonceAccountEvent, TokenAccountEvent, TokenInfoEvent},
            protocols::{
                bonk::{
                    parser::BONK_PROGRAM_ID, BonkGlobalConfigAccountEvent, BonkMigrateToAmmEvent,
                    BonkMigrateToCpswapEvent, BonkPlatformConfigAccountEvent, BonkPoolCreateEvent,
                    BonkPoolStateAccountEvent, BonkTradeEvent,
                },
                pumpfun::{
                    parser::PUMPFUN_PROGRAM_ID, PumpFunBondingCurveAccountEvent,
                    PumpFunCreateTokenEvent, PumpFunGlobalAccountEvent, PumpFunMigrateEvent,
                    PumpFunTradeEvent,
                },
                pumpswap::{
                    parser::PUMPSWAP_PROGRAM_ID, PumpSwapBuyEvent, PumpSwapCreatePoolEvent,
                    PumpSwapDepositEvent, PumpSwapGlobalConfigAccountEvent,
                    PumpSwapPoolAccountEvent, PumpSwapSellEvent, PumpSwapWithdrawEvent,
                },
                raydium_amm_v4::{
                    parser::RAYDIUM_AMM_V4_PROGRAM_ID, RaydiumAmmV4AmmInfoAccountEvent,
                    RaydiumAmmV4DepositEvent, RaydiumAmmV4Initialize2Event, RaydiumAmmV4SwapEvent,
                    RaydiumAmmV4WithdrawEvent, RaydiumAmmV4WithdrawPnlEvent,
                },
                raydium_clmm::{
                    parser::RAYDIUM_CLMM_PROGRAM_ID, RaydiumClmmAmmConfigAccountEvent,
                    RaydiumClmmClosePositionEvent, RaydiumClmmCreatePoolEvent,
                    RaydiumClmmDecreaseLiquidityV2Event, RaydiumClmmIncreaseLiquidityV2Event,
                    RaydiumClmmOpenPositionV2Event, RaydiumClmmOpenPositionWithToken22NftEvent,
                    RaydiumClmmPoolStateAccountEvent, RaydiumClmmSwapEvent, RaydiumClmmSwapV2Event,
                    RaydiumClmmTickArrayStateAccountEvent,
                },
                raydium_cpmm::{
                    parser::RAYDIUM_CPMM_PROGRAM_ID, RaydiumCpmmAmmConfigAccountEvent,
                    RaydiumCpmmDepositEvent, RaydiumCpmmInitializeEvent,
                    RaydiumCpmmPoolStateAccountEvent, RaydiumCpmmSwapEvent,
                    RaydiumCpmmWithdrawEvent,
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
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct EventStats {
    total_events: u64,
    swap_events: u64,
    pool_updates: u64,
    opportunities_found: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create logs directory if it doesn't exist
    fs::create_dir_all("logs")?;

    println!("Starting Yellowstone gRPC Streamer...");
    test_grpc().await?;
    Ok(())
}

async fn test_grpc() -> Result<(), Box<dyn std::error::Error>> {
    println!("Subscribing to Yellowstone gRPC events...");

    // Create log file in logs directory (overwrites if exists)
    let log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("logs/events.log")?;
    let log_file = Arc::new(Mutex::new(log_file));

    // Create flash loan opportunities log file
    let opportunities_log = OpenOptions::new()
        .create(true)
        .append(true)
        .open("logs/flash_loan_opportunities.log")?;
    let opportunities_log = Arc::new(Mutex::new(opportunities_log));

    println!("Logging to: logs/events.log");
    println!("Flash loan opportunities: logs/flash_loan_opportunities.log");

    // Initialize opportunity detector with high-liquidity requirements
    // This ensures we only find opportunities in pools with substantial liquidity
    // Min profit: 0.001 SOL (1,000,000 lamports)
    // Max loan: 100 SOL (100,000,000,000 lamports)
    // Min liquidity per pool: 10 SOL (10,000,000,000 lamports)
    // Min combined liquidity: 50 SOL (50,000_000_000 lamports)
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        1_000_000,           // 0.001 SOL min profit
        100_000_000_000,     // 100 SOL max loan
        10_000_000_000,      // 10 SOL min per pool
        50_000_000_000,      // 50 SOL min combined
    )));

    // Create low-latency configuration
    let mut config: ClientConfig = ClientConfig::low_latency();
    // Enable performance monitoring, has performance overhead, disabled by default
    config.enable_metrics = true;
    let grpc = YellowstoneGrpc::new_with_config(
        "https://solana-yellowstone-grpc.publicnode.com:443".to_string(),
        None,
        config,
    )?;

    println!("GRPC client created successfully");

    let callback = create_event_callback(log_file.clone(), opportunities_log.clone(), detector.clone());

    // Will try to parse corresponding protocol events from transactions
    let protocols = vec![
        // Protocol::PumpFun,
        // Protocol::PumpSwap,
        // Protocol::Bonk,
        // Protocol::RaydiumCpmm, 
        Protocol::RaydiumClmm,
        Protocol::RaydiumAmmV4,
    ];

    println!("Protocols to monitor: {:?}", protocols);

    // Filter accounts
    let account_include = vec![
        PUMPFUN_PROGRAM_ID.to_string(),        // Listen to pumpfun program ID
        PUMPSWAP_PROGRAM_ID.to_string(),       // Listen to pumpswap program ID
        BONK_PROGRAM_ID.to_string(),           // Listen to bonk program ID
        RAYDIUM_CPMM_PROGRAM_ID.to_string(),   // Listen to raydium_cpmm program ID
        RAYDIUM_CLMM_PROGRAM_ID.to_string(),   // Listen to raydium_clmm program ID
        RAYDIUM_AMM_V4_PROGRAM_ID.to_string(), // Listen to raydium_amm_v4 program ID
    ];
    let account_exclude = vec![];
    let account_required = vec![];

    // Listen to transaction data
    let transaction_filter = TransactionFilter {
        account_include: account_include.clone(),
        account_exclude,
        account_required,
    };

    // Listen to account data belonging to owner programs -> account event monitoring
    let account_filter = AccountFilter { account: vec![], owner: account_include.clone(), filters: vec![] };

    // Event filtering
    // No event filtering, includes all events
    let event_type_filter = None;
    // Only include PumpSwapBuy events and PumpSwapSell events
    // let event_type_filter = Some(EventTypeFilter { include: vec![EventType::PumpFunTrade] });

    println!("Starting to listen for events, press Ctrl+C to stop...");
    println!("Monitoring programs: {:?}", account_include);

    println!("Starting subscription...");

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

    // 支持 stop 方法，测试代码 -  异步1000秒之后停止
    let grpc_clone = grpc.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
        grpc_clone.stop().await;
    });

    println!("Waiting for Ctrl+C to stop...");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

fn create_event_callback(
    log_file: Arc<Mutex<std::fs::File>>,
    opportunities_log: Arc<Mutex<std::fs::File>>,
    detector: Arc<Mutex<OpportunityDetector>>,
) -> impl Fn(Box<dyn UnifiedEvent>) {
    // Event counter for periodic stats
    let event_counter = Arc::new(Mutex::new(EventStats::default()));

    // Helper macro to log to file only (not console)
    macro_rules! log_file_only {
        ($file:expr, $($arg:tt)*) => {{
            let msg = format!($($arg)*);
            if let Ok(mut file) = $file.lock() {
                let _ = file.write_all(msg.as_bytes());
            }
        }};
    }

    move |event: Box<dyn UnifiedEvent>| {
        // Only log to file, not console (reduces noise)
        log_file_only!(
            log_file,
            "Event: {:?}, tx_index: {:?}\n",
            event.event_type(),
            event.transaction_index()
        );

        // Update event counter
        {
            let mut stats = event_counter.lock().unwrap();
            stats.total_events += 1;
        }

        match_event!(event, {
            // -------------------------- block meta -----------------------
            BlockMetaEvent => |e: BlockMetaEvent| {
                log_file_only!(log_file, "BlockMetaEvent: {:?}\n", e.metadata.handle_us);
            },
            // -------------------------- bonk -----------------------
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                log_file_only!(log_file, "BonkPoolCreateEvent: {:?}\n", e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                log_file_only!(log_file, "BonkTradeEvent: {e:?}\n");
            },
            BonkMigrateToAmmEvent => |e: BonkMigrateToAmmEvent| {
                log_file_only!(log_file, "BonkMigrateToAmmEvent: {e:?}\n");
            },
            BonkMigrateToCpswapEvent => |e: BonkMigrateToCpswapEvent| {
                log_file_only!(log_file, "BonkMigrateToCpswapEvent: {e:?}\n");
            },
            // -------------------------- pumpfun -----------------------
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                log_file_only!(log_file, "PumpFunTradeEvent: {e:?}\n");
            },
            PumpFunMigrateEvent => |e: PumpFunMigrateEvent| {
                log_file_only!(log_file, "PumpFunMigrateEvent: {e:?}\n");
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                log_file_only!(log_file, "PumpFunCreateTokenEvent: {e:?}\n");
            },
            // -------------------------- pumpswap -----------------------
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                log_file_only!(log_file, "PumpSwapBuyEvent: {e:?}\n");
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                log_file_only!(log_file, "PumpSwapSellEvent: {e:?}\n");
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                log_file_only!(log_file, "PumpSwapCreatePoolEvent: {e:?}\n");
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                log_file_only!(log_file, "PumpSwapDepositEvent: {e:?}\n");
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                log_file_only!(log_file, "PumpSwapWithdrawEvent: {e:?}\n");
            },
            // -------------------------- raydium_cpmm -----------------------
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                log_file_only!(log_file, "RaydiumCpmmSwapEvent: {e:?}\n");
            },
            RaydiumCpmmDepositEvent => |e: RaydiumCpmmDepositEvent| {
                log_file_only!(log_file, "RaydiumCpmmDepositEvent: {e:?}\n");
            },
            RaydiumCpmmInitializeEvent => |e: RaydiumCpmmInitializeEvent| {
                log_file_only!(log_file, "RaydiumCpmmInitializeEvent: {e:?}\n");
            },
            RaydiumCpmmWithdrawEvent => |e: RaydiumCpmmWithdrawEvent| {
                log_file_only!(log_file, "RaydiumCpmmWithdrawEvent: {e:?}\n");
            },
            // -------------------------- raydium_clmm -----------------------
            RaydiumClmmSwapEvent => |e: RaydiumClmmSwapEvent| {
                log_file_only!(log_file, "RaydiumClmmSwapEvent: {e:?}\n");
            },
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                log_file_only!(log_file, "RaydiumClmmSwapV2Event: {e:?}\n");

                // Update swap event counter
                {
                    let mut stats = event_counter.lock().unwrap();
                    stats.swap_events += 1;
                }

                // Analyze swap for flash loan opportunities
                let mut det = detector.lock().unwrap();
                if let Some(opportunity) = det.analyze_clmm_swap_event(&e) {
                    drop(det); // Release lock before logging

                    // Update opportunities counter
                    {
                        let mut stats = event_counter.lock().unwrap();
                        stats.opportunities_found += 1;
                    }

                    // Calculate price spread percentage
                    let spread_pct = (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0;

                    // Only show high-quality opportunities (confidence >= 60% and spread >= 1%)
                    if opportunity.confidence >= 60 && spread_pct >= 1.0 {
                        // Format protocol names for display
                        let protocol_a = match opportunity.pool_a_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let protocol_b = match opportunity.pool_b_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };

                        // Log to console with clear formatting
                        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("💰 FLASH LOAN OPPORTUNITY DETECTED");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("📊 Arbitrage Details:");
                        println!("   Pool A (buy):    {} [{}]", opportunity.pool_a, protocol_a);
                        println!("   Pool B (sell):   {} [{}]", opportunity.pool_b, protocol_b);
                        println!("   Base Token:      {}", opportunity.base_token);
                        println!("   Quote Token:     {}", opportunity.quote_token);
                        println!("   Price A:         {:.10}", opportunity.price_a);
                        println!("   Price B:         {:.10}", opportunity.price_b);
                        println!("   Price Spread:    {:.2}%\n", spread_pct);

                        println!("💵 Financial Breakdown:");
                        println!("   Loan Amount:     {:>15} lamports ({:.6} SOL)",
                            opportunity.loan_amount, opportunity.loan_amount as f64 / 1e9);
                        println!("   Expected Profit: {:>15} lamports ({:.6} SOL)",
                            opportunity.expected_profit, opportunity.expected_profit as f64 / 1e9);
                        println!("   Confidence:      {}%\n", opportunity.confidence);

                        println!("🎯 EXECUTION READY");
                        println!("   Cross-protocol arb: {} ↔ {}", protocol_a, protocol_b);
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                    }

                    // Write to opportunities log file
                    if let Ok(mut file) = opportunities_log.lock() {
                        let protocol_a = match opportunity.pool_a_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let protocol_b = match opportunity.pool_b_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let log_entry = format!(
                            "{} | {} ↔ {} | Profit: {:.6} SOL | Spread: {:.2}% | Confidence: {}% | Loan: {:.6} SOL | Pool A: {} | Pool B: {}\n",
                            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                            protocol_a,
                            protocol_b,
                            opportunity.expected_profit as f64 / 1e9,
                            spread_pct,
                            opportunity.confidence,
                            opportunity.loan_amount as f64 / 1e9,
                            opportunity.pool_a,
                            opportunity.pool_b
                        );
                        let _ = file.write_all(log_entry.as_bytes());
                        let _ = file.flush();
                    }

                    // ═════════════════════════════════════════════════════════════════
                    // TODO: FLASH LOAN EXECUTION LOGIC GOES HERE
                    // ═════════════════════════════════════════════════════════════════
                    //
                    // When ready to execute flash loans, implement the following:
                    //
                    // 1. Create FlashLoanTxBuilder instance (once, at startup):
                    //    ```
                    //    use solana_streamer_sdk::flash_loan::FlashLoanTxBuilder;
                    //    use solana_sdk::signature::Keypair;
                    //
                    //    let payer_keypair = Keypair::from_bytes(&secret_key)?;
                    //    let receiver_program = solana_sdk::pubkey!("YourReceiverProgramID");
                    //    let tx_builder = Arc::new(FlashLoanTxBuilder::new(
                    //        "https://api.mainnet-beta.solana.com".to_string(),
                    //        payer_keypair,
                    //        receiver_program,
                    //    ));
                    //    ```
                    //
                    // 2. Execute flash loan transaction (here in the callback):
                    //    ```
                    //    let tx_builder_clone = tx_builder.clone();
                    //    tokio::spawn(async move {
                    //        match tx_builder_clone.execute_flash_loan(&opportunity).await {
                    //            Ok(signature) => {
                    //                println!("✅ Flash loan executed! Signature: {}", signature);
                    //            }
                    //            Err(e) => {
                    //                eprintln!("❌ Flash loan failed: {}", e);
                    //            }
                    //        }
                    //    });
                    //    ```
                    //
                    // 3. Before executing on mainnet:
                    //    - Deploy and test your flash loan receiver program
                    //    - Ensure sufficient SOL balance for transaction fees
                    //    - Test on devnet/testnet first
                    //    - Implement proper error handling and monitoring
                    //    - Consider slippage protection and timeout limits
                    //
                    // See: src/flash_loan/transaction_builder.rs for implementation details
                    // See: programs/flash-loan-receiver/src/lib.rs for receiver program
                    // See: FLASH_LOAN_QUICKSTART.md for deployment guide
                    //
                    // ═════════════════════════════════════════════════════════════════
                }
            },
            RaydiumClmmClosePositionEvent => |e: RaydiumClmmClosePositionEvent| {
                log_file_only!(log_file, "RaydiumClmmClosePositionEvent: {e:?}\n");
            },
            RaydiumClmmDecreaseLiquidityV2Event => |e: RaydiumClmmDecreaseLiquidityV2Event| {
                log_file_only!(log_file, "RaydiumClmmDecreaseLiquidityV2Event: {e:?}\n");
            },
            RaydiumClmmCreatePoolEvent => |e: RaydiumClmmCreatePoolEvent| {
                log_file_only!(log_file, "RaydiumClmmCreatePoolEvent: {e:?}\n");
            },
            RaydiumClmmIncreaseLiquidityV2Event => |e: RaydiumClmmIncreaseLiquidityV2Event| {
                log_file_only!(log_file, "RaydiumClmmIncreaseLiquidityV2Event: {e:?}\n");
            },
            RaydiumClmmOpenPositionWithToken22NftEvent => |e: RaydiumClmmOpenPositionWithToken22NftEvent| {
                log_file_only!(log_file, "RaydiumClmmOpenPositionWithToken22NftEvent: {e:?}\n");
            },
            RaydiumClmmOpenPositionV2Event => |e: RaydiumClmmOpenPositionV2Event| {
                log_file_only!(log_file, "RaydiumClmmOpenPositionV2Event: {e:?}\n");
            },
            // -------------------------- raydium_amm_v4 -----------------------
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                log_file_only!(log_file, "RaydiumAmmV4SwapEvent: {e:?}\n");

                // Update swap event counter
                {
                    let mut stats = event_counter.lock().unwrap();
                    stats.swap_events += 1;
                }

                // Analyze swap for flash loan opportunities
                let mut det = detector.lock().unwrap();
                if let Some(opportunity) = det.analyze_ammv4_swap_event(&e) {
                    drop(det); // Release lock before logging

                    // Update opportunities counter
                    {
                        let mut stats = event_counter.lock().unwrap();
                        stats.opportunities_found += 1;
                    }

                    // Calculate price spread percentage
                    let spread_pct = (opportunity.price_b - opportunity.price_a) / opportunity.price_a * 100.0;

                    // Only show high-quality opportunities (confidence >= 60% and spread >= 1%)
                    if opportunity.confidence >= 60 && spread_pct >= 1.0 {
                        // Format protocol names for display
                        let protocol_a = match opportunity.pool_a_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let protocol_b = match opportunity.pool_b_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };

                        // Log to console with clear formatting
                        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("💰 FLASH LOAN OPPORTUNITY DETECTED");
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        println!("📊 Arbitrage Details:");
                        println!("   Pool A (buy):    {} [{}]", opportunity.pool_a, protocol_a);
                        println!("   Pool B (sell):   {} [{}]", opportunity.pool_b, protocol_b);
                        println!("   Base Token:      {}", opportunity.base_token);
                        println!("   Quote Token:     {}", opportunity.quote_token);
                        println!("   Price A:         {:.10}", opportunity.price_a);
                        println!("   Price B:         {:.10}", opportunity.price_b);
                        println!("   Price Spread:    {:.2}%\n", spread_pct);

                        println!("💵 Financial Breakdown:");
                        println!("   Loan Amount:     {:>15} lamports ({:.6} SOL)",
                            opportunity.loan_amount, opportunity.loan_amount as f64 / 1e9);
                        println!("   Expected Profit: {:>15} lamports ({:.6} SOL)",
                            opportunity.expected_profit, opportunity.expected_profit as f64 / 1e9);
                        println!("   Confidence:      {}%\n", opportunity.confidence);

                        println!("🎯 EXECUTION READY");
                        println!("   Cross-protocol arb: {} ↔ {}", protocol_a, protocol_b);
                        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
                    }

                    // Write to opportunities log file
                    if let Ok(mut file) = opportunities_log.lock() {
                        let protocol_a = match opportunity.pool_a_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let protocol_b = match opportunity.pool_b_protocol {
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumClmm => "CLMM",
                            solana_streamer_sdk::flash_loan::PoolProtocol::RaydiumAmmV4 => "AMMv4",
                        };
                        let log_entry = format!(
                            "{} | {} ↔ {} | Profit: {:.6} SOL | Spread: {:.2}% | Confidence: {}% | Loan: {:.6} SOL | Pool A: {} | Pool B: {}\n",
                            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
                            protocol_a,
                            protocol_b,
                            opportunity.expected_profit as f64 / 1e9,
                            spread_pct,
                            opportunity.confidence,
                            opportunity.loan_amount as f64 / 1e9,
                            opportunity.pool_a,
                            opportunity.pool_b
                        );
                        let _ = file.write_all(log_entry.as_bytes());
                        let _ = file.flush();
                    }
                }
            },
            RaydiumAmmV4DepositEvent => |e: RaydiumAmmV4DepositEvent| {
                log_file_only!(log_file, "RaydiumAmmV4DepositEvent: {e:?}\n");
            },
            RaydiumAmmV4Initialize2Event => |e: RaydiumAmmV4Initialize2Event| {
                log_file_only!(log_file, "RaydiumAmmV4Initialize2Event: {e:?}\n");
            },
            RaydiumAmmV4WithdrawEvent => |e: RaydiumAmmV4WithdrawEvent| {
                log_file_only!(log_file, "RaydiumAmmV4WithdrawEvent: {e:?}\n");
            },
            RaydiumAmmV4WithdrawPnlEvent => |e: RaydiumAmmV4WithdrawPnlEvent| {
                log_file_only!(log_file, "RaydiumAmmV4WithdrawPnlEvent: {e:?}\n");
            },
            // -------------------------- account -----------------------
            BonkPoolStateAccountEvent => |e: BonkPoolStateAccountEvent| {
                log_file_only!(log_file, "BonkPoolStateAccountEvent: {e:?}\n");
            },
            BonkGlobalConfigAccountEvent => |e: BonkGlobalConfigAccountEvent| {
                log_file_only!(log_file, "BonkGlobalConfigAccountEvent: {e:?}\n");
            },
            BonkPlatformConfigAccountEvent => |e: BonkPlatformConfigAccountEvent| {
                log_file_only!(log_file, "BonkPlatformConfigAccountEvent: {e:?}\n");
            },
            PumpSwapGlobalConfigAccountEvent => |e: PumpSwapGlobalConfigAccountEvent| {
                log_file_only!(log_file, "PumpSwapGlobalConfigAccountEvent: {e:?}\n");
            },
            PumpSwapPoolAccountEvent => |e: PumpSwapPoolAccountEvent| {
                log_file_only!(log_file, "PumpSwapPoolAccountEvent: {e:?}\n");
            },
            PumpFunBondingCurveAccountEvent => |e: PumpFunBondingCurveAccountEvent| {
                log_file_only!(log_file, "PumpFunBondingCurveAccountEvent: {e:?}\n");
            },
            PumpFunGlobalAccountEvent => |e: PumpFunGlobalAccountEvent| {
                log_file_only!(log_file, "PumpFunGlobalAccountEvent: {e:?}\n");
            },
            RaydiumAmmV4AmmInfoAccountEvent => |e: RaydiumAmmV4AmmInfoAccountEvent| {
                log_file_only!(log_file, "RaydiumAmmV4AmmInfoAccountEvent: {e:?}\n");

                // Update pool state counter
                {
                    let mut stats = event_counter.lock().unwrap();
                    stats.pool_updates += 1;
                }

                // Update pool state in detector for arbitrage analysis
                let mut det = detector.lock().unwrap();
                det.update_ammv4_pool_state(&e);
            },
            RaydiumClmmAmmConfigAccountEvent => |e: RaydiumClmmAmmConfigAccountEvent| {
                log_file_only!(log_file, "RaydiumClmmAmmConfigAccountEvent: {e:?}\n");
            },
            RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {
                log_file_only!(log_file, "RaydiumClmmPoolStateAccountEvent: {e:?}\n");

                // Update pool state counter
                {
                    let mut stats = event_counter.lock().unwrap();
                    stats.pool_updates += 1;
                }

                // Update pool state in detector for arbitrage analysis
                let mut det = detector.lock().unwrap();
                det.update_clmm_pool_state(&e);
            },
            RaydiumClmmTickArrayStateAccountEvent => |e: RaydiumClmmTickArrayStateAccountEvent| {
                log_file_only!(log_file, "RaydiumClmmTickArrayStateAccountEvent: {e:?}\n");
            },
            RaydiumCpmmAmmConfigAccountEvent => |e: RaydiumCpmmAmmConfigAccountEvent| {
                log_file_only!(log_file, "RaydiumCpmmAmmConfigAccountEvent: {e:?}\n");
            },
            RaydiumCpmmPoolStateAccountEvent => |e: RaydiumCpmmPoolStateAccountEvent| {
                log_file_only!(log_file, "RaydiumCpmmPoolStateAccountEvent: {e:?}\n");
            },
            TokenAccountEvent => |e: TokenAccountEvent| {
                log_file_only!(log_file, "TokenAccountEvent: {e:?}\n");
            },
            NonceAccountEvent => |e: NonceAccountEvent| {
                log_file_only!(log_file, "NonceAccountEvent: {e:?}\n");
            },
            TokenInfoEvent => |e: TokenInfoEvent| {
                log_file_only!(log_file, "TokenInfoEvent: {e:?}\n");
            },
        });

        // Print periodic stats every 100 events (console only)
        {
            let stats = event_counter.lock().unwrap();
            if stats.total_events % 100 == 0 {
                println!("📊 Stats | Events: {} | Swaps: {} | Pool Updates: {} | Opportunities: {}",
                    stats.total_events, stats.swap_events, stats.pool_updates, stats.opportunities_found);
            }
        }
    }
}
