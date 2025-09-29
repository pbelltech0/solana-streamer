use solana_streamer_sdk::{
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

    println!("Logging to: logs/events.log");

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

    let callback = create_event_callback(log_file.clone());

    // Will try to parse corresponding protocol events from transactions
    let protocols = vec![
        // Protocol::PumpFun,
        // Protocol::PumpSwap,
        // Protocol::Bonk,
        // Protocol::RaydiumCpmm, 
        Protocol::RaydiumClmm,
        // Protocol::RaydiumAmmV4,
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

fn create_event_callback(log_file: Arc<Mutex<std::fs::File>>) -> impl Fn(Box<dyn UnifiedEvent>) {
    // Helper macro to log both to console and file
    macro_rules! log_msg {
        ($file:expr, $($arg:tt)*) => {{
            let msg = format!($($arg)*);
            print!("{}", msg);
            if let Ok(mut file) = $file.lock() {
                let _ = file.write_all(msg.as_bytes());
            }
        }};
    }

    move |event: Box<dyn UnifiedEvent>| {
        log_msg!(
            log_file,
            "🎉 Event received! Type: {:?}, transaction_index: {:?}\n",
            event.event_type(),
            event.transaction_index()
        );

        match_event!(event, {
            // -------------------------- block meta -----------------------
            BlockMetaEvent => |e: BlockMetaEvent| {
                log_msg!(log_file, "BlockMetaEvent: {:?}\n", e.metadata.handle_us);
            },
            // -------------------------- bonk -----------------------
            BonkPoolCreateEvent => |e: BonkPoolCreateEvent| {
                log_msg!(log_file, "block_time: {:?}, block_time_ms: {:?}\nBonkPoolCreateEvent: {:?}\n",
                    e.metadata.block_time, e.metadata.block_time_ms, e.base_mint_param.symbol);
            },
            BonkTradeEvent => |e: BonkTradeEvent| {
                log_msg!(log_file, "BonkTradeEvent: {e:?}\n");
            },
            BonkMigrateToAmmEvent => |e: BonkMigrateToAmmEvent| {
                log_msg!(log_file, "BonkMigrateToAmmEvent: {e:?}\n");
            },
            BonkMigrateToCpswapEvent => |e: BonkMigrateToCpswapEvent| {
                log_msg!(log_file, "BonkMigrateToCpswapEvent: {e:?}\n");
            },
            // -------------------------- pumpfun -----------------------
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                log_msg!(log_file, "PumpFunTradeEvent: {e:?}\n");
            },
            PumpFunMigrateEvent => |e: PumpFunMigrateEvent| {
                log_msg!(log_file, "PumpFunMigrateEvent: {e:?}\n");
            },
            PumpFunCreateTokenEvent => |e: PumpFunCreateTokenEvent| {
                log_msg!(log_file, "PumpFunCreateTokenEvent: {e:?}\n");
            },
            // -------------------------- pumpswap -----------------------
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                log_msg!(log_file, "Buy event: {e:?}\n");
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                log_msg!(log_file, "Sell event: {e:?}\n");
            },
            PumpSwapCreatePoolEvent => |e: PumpSwapCreatePoolEvent| {
                log_msg!(log_file, "CreatePool event: {e:?}\n");
            },
            PumpSwapDepositEvent => |e: PumpSwapDepositEvent| {
                log_msg!(log_file, "Deposit event: {e:?}\n");
            },
            PumpSwapWithdrawEvent => |e: PumpSwapWithdrawEvent| {
                log_msg!(log_file, "Withdraw event: {e:?}\n");
            },
            // -------------------------- raydium_cpmm -----------------------
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                log_msg!(log_file, "RaydiumCpmmSwapEvent: {e:?}\n");
            },
            RaydiumCpmmDepositEvent => |e: RaydiumCpmmDepositEvent| {
                log_msg!(log_file, "RaydiumCpmmDepositEvent: {e:?}\n");
            },
            RaydiumCpmmInitializeEvent => |e: RaydiumCpmmInitializeEvent| {
                log_msg!(log_file, "RaydiumCpmmInitializeEvent: {e:?}\n");
            },
            RaydiumCpmmWithdrawEvent => |e: RaydiumCpmmWithdrawEvent| {
                log_msg!(log_file, "RaydiumCpmmWithdrawEvent: {e:?}\n");
            },
            // -------------------------- raydium_clmm -----------------------
            RaydiumClmmSwapEvent => |e: RaydiumClmmSwapEvent| {
                log_msg!(log_file, "RaydiumClmmSwapEvent: {e:?}\n");
            },
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                log_msg!(log_file, "RaydiumClmmSwapV2Event: {e:?}\n");
            },
            RaydiumClmmClosePositionEvent => |e: RaydiumClmmClosePositionEvent| {
                log_msg!(log_file, "RaydiumClmmClosePositionEvent: {e:?}\n");
            },
            RaydiumClmmDecreaseLiquidityV2Event => |e: RaydiumClmmDecreaseLiquidityV2Event| {
                log_msg!(log_file, "RaydiumClmmDecreaseLiquidityV2Event: {e:?}\n");
            },
            RaydiumClmmCreatePoolEvent => |e: RaydiumClmmCreatePoolEvent| {
                log_msg!(log_file, "RaydiumClmmCreatePoolEvent: {e:?}\n");
            },
            RaydiumClmmIncreaseLiquidityV2Event => |e: RaydiumClmmIncreaseLiquidityV2Event| {
                log_msg!(log_file, "RaydiumClmmIncreaseLiquidityV2Event: {e:?}\n");
            },
            RaydiumClmmOpenPositionWithToken22NftEvent => |e: RaydiumClmmOpenPositionWithToken22NftEvent| {
                log_msg!(log_file, "RaydiumClmmOpenPositionWithToken22NftEvent: {e:?}\n");
            },
            RaydiumClmmOpenPositionV2Event => |e: RaydiumClmmOpenPositionV2Event| {
                log_msg!(log_file, "RaydiumClmmOpenPositionV2Event: {e:?}\n");
            },
            // -------------------------- raydium_amm_v4 -----------------------
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                log_msg!(log_file, "RaydiumAmmV4SwapEvent: {e:?}\n");
            },
            RaydiumAmmV4DepositEvent => |e: RaydiumAmmV4DepositEvent| {
                log_msg!(log_file, "RaydiumAmmV4DepositEvent: {e:?}\n");
            },
            RaydiumAmmV4Initialize2Event => |e: RaydiumAmmV4Initialize2Event| {
                log_msg!(log_file, "RaydiumAmmV4Initialize2Event: {e:?}\n");
            },
            RaydiumAmmV4WithdrawEvent => |e: RaydiumAmmV4WithdrawEvent| {
                log_msg!(log_file, "RaydiumAmmV4WithdrawEvent: {e:?}\n");
            },
            RaydiumAmmV4WithdrawPnlEvent => |e: RaydiumAmmV4WithdrawPnlEvent| {
                log_msg!(log_file, "RaydiumAmmV4WithdrawPnlEvent: {e:?}\n");
            },
            // -------------------------- account -----------------------
            BonkPoolStateAccountEvent => |e: BonkPoolStateAccountEvent| {
                log_msg!(log_file, "BonkPoolStateAccountEvent: {e:?}\n");
            },
            BonkGlobalConfigAccountEvent => |e: BonkGlobalConfigAccountEvent| {
                log_msg!(log_file, "BonkGlobalConfigAccountEvent: {e:?}\n");
            },
            BonkPlatformConfigAccountEvent => |e: BonkPlatformConfigAccountEvent| {
                log_msg!(log_file, "BonkPlatformConfigAccountEvent: {e:?}\n");
            },
            PumpSwapGlobalConfigAccountEvent => |e: PumpSwapGlobalConfigAccountEvent| {
                log_msg!(log_file, "PumpSwapGlobalConfigAccountEvent: {e:?}\n");
            },
            PumpSwapPoolAccountEvent => |e: PumpSwapPoolAccountEvent| {
                log_msg!(log_file, "PumpSwapPoolAccountEvent: {e:?}\n");
            },
            PumpFunBondingCurveAccountEvent => |e: PumpFunBondingCurveAccountEvent| {
                log_msg!(log_file, "PumpFunBondingCurveAccountEvent: {e:?}\n");
            },
            PumpFunGlobalAccountEvent => |e: PumpFunGlobalAccountEvent| {
                log_msg!(log_file, "PumpFunGlobalAccountEvent: {e:?}\n");
            },
            RaydiumAmmV4AmmInfoAccountEvent => |e: RaydiumAmmV4AmmInfoAccountEvent| {
                log_msg!(log_file, "RaydiumAmmV4AmmInfoAccountEvent: {e:?}\n");
            },
            RaydiumClmmAmmConfigAccountEvent => |e: RaydiumClmmAmmConfigAccountEvent| {
                log_msg!(log_file, "RaydiumClmmAmmConfigAccountEvent: {e:?}\n");
            },
            RaydiumClmmPoolStateAccountEvent => |e: RaydiumClmmPoolStateAccountEvent| {
                log_msg!(log_file, "RaydiumClmmPoolStateAccountEvent: {e:?}\n");
            },
            RaydiumClmmTickArrayStateAccountEvent => |e: RaydiumClmmTickArrayStateAccountEvent| {
                log_msg!(log_file, "RaydiumClmmTickArrayStateAccountEvent: {e:?}\n");
            },
            RaydiumCpmmAmmConfigAccountEvent => |e: RaydiumCpmmAmmConfigAccountEvent| {
                log_msg!(log_file, "RaydiumCpmmAmmConfigAccountEvent: {e:?}\n");
            },
            RaydiumCpmmPoolStateAccountEvent => |e: RaydiumCpmmPoolStateAccountEvent| {
                log_msg!(log_file, "RaydiumCpmmPoolStateAccountEvent: {e:?}\n");
            },
            TokenAccountEvent => |e: TokenAccountEvent| {
                log_msg!(log_file, "TokenAccountEvent: {e:?}\n");
            },
            NonceAccountEvent => |e: NonceAccountEvent| {
                log_msg!(log_file, "NonceAccountEvent: {e:?}\n");
            },
            TokenInfoEvent => |e: TokenInfoEvent| {
                log_msg!(log_file, "TokenInfoEvent: {e:?}\n");
            },
        });
    }
}
