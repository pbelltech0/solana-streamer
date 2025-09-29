# Flash Loan Integration Strategy for Solana Streamer

## Overview

This document outlines the architectural design for integrating flash loan capabilities with the existing Solana Streamer system to enable automated arbitrage opportunities on Raydium CLMM pools.

---

## Architecture Decision: Hybrid On-Chain + Off-Chain

### **Critical Question: Where Does the Flash Loan Agent Run?**

**Answer: You need BOTH components working together:**

1. **On-Chain Program** (Solana program in Rust/Anchor)
   - Implements the `ReceiveFlashLoan` instruction (Tag 0)
   - Executes arbitrage logic via CPI to Raydium CLMM
   - Lives on Solana blockchain
   - Called by lending protocols via CPI during flash loan execution

2. **Local Binary/Agent** (Your current Rust binary)
   - Analyzes streaming data for opportunities
   - Builds and submits transactions
   - Runs locally or on a server
   - Uses existing `solana-streamer` infrastructure

**Why Both?**
- **Local agent** detects opportunities using real-time streaming data (your competitive advantage)
- **On-chain program** executes the actual flash loan arbitrage atomically (trustless execution)

---

## System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    LOCAL COMPONENTS (Off-Chain)                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │         Real-Time Event Streaming (Existing)              │  │
│  │  - Yellowstone gRPC / ShredStream                         │  │
│  │  - Raydium CLMM Swap Events (price changes)              │  │
│  │  - Pool State Updates (liquidity, sqrt_price_x64)        │  │
│  │  - Tick Array Updates (concentrated liquidity ranges)     │  │
│  └──────────────┬───────────────────────────────────────────┘  │
│                 │                                                 │
│                 ▼                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │      NEW: Arbitrage Opportunity Detector                  │  │
│  │  - Price discrepancy detection across pools               │  │
│  │  - Liquidity depth analysis from PoolState               │  │
│  │  - Slippage calculation from tick arrays                  │  │
│  │  - MEV opportunity identification                         │  │
│  │  - Profitability estimation (fees vs profit)              │  │
│  └──────────────┬───────────────────────────────────────────┘  │
│                 │                                                 │
│                 ▼                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │      NEW: Flash Loan Transaction Builder                  │  │
│  │  - Builds flash loan transaction                          │  │
│  │  - Calculates optimal loan amounts                        │  │
│  │  - Constructs instruction sequence                        │  │
│  │  - Signs and submits to Solana                           │  │
│  └──────────────┬───────────────────────────────────────────┘  │
│                 │                                                 │
└─────────────────┼─────────────────────────────────────────────┘
                  │ Submit Transaction
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                 ON-CHAIN COMPONENTS (Solana)                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │          Solend Flash Loan Program (Existing)             │  │
│  │  - Lends tokens without collateral                        │  │
│  │  - Calls your receiver program via CPI                    │  │
│  │  - Validates repayment + fees                             │  │
│  └──────────────┬───────────────────────────────────────────┘  │
│                 │ CPI Call: ReceiveFlashLoan                     │
│                 ▼                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │    NEW: Your Flash Loan Receiver Program (Anchor/Rust)    │  │
│  │                                                            │  │
│  │  pub fn receive_flash_loan(                               │  │
│  │      ctx: Context<ReceiveFlashLoan>,                      │  │
│  │      repay_amount: u64                                     │  │
│  │  ) -> Result<()> {                                         │  │
│  │      // 1. Receive borrowed tokens                        │  │
│  │      // 2. Execute arbitrage on Raydium CLMM             │  │
│  │      // 3. Repay loan + fees                              │  │
│  │  }                                                         │  │
│  └──────────────┬───────────────────────────────────────────┘  │
│                 │                                                 │
│                 ▼                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │         Raydium CLMM Program (Existing)                   │  │
│  │  - Executes swaps via CPI from your program               │  │
│  │  - swap_v2 instruction                                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Flash Loan Flow Sequence

```
1. [Local Agent] Detects arbitrage opportunity from streaming data
                ↓
2. [Local Agent] Builds flash loan transaction with instructions:
                - FlashLoan instruction (Solend)
                - ReceiveFlashLoan instruction (Your program)
                ↓
3. [Local Agent] Signs and submits transaction to Solana
                ↓
4. [Solend Program] Transfers borrowed tokens to your program
                ↓
5. [Solend Program] Calls your program via CPI: receive_flash_loan()
                ↓
6. [Your Program] Executes arbitrage:
                - Swap on Pool A (buy low)
                - Swap on Pool B (sell high)
                ↓
7. [Your Program] Repays loan + fees to Solend
                ↓
8. [Solend Program] Verifies repayment, completes transaction
                ↓
9. [Result] Profit deposited to your wallet (all atomic!)
```

---

## Key Streaming Data for Flash Loan Opportunities

### 1. Swap Events (`RaydiumClmmSwapV2Event`)

**Location:** `src/streaming/event_parser/protocols/raydium_clmm/events.rs:34`

```rust
pub struct RaydiumClmmSwapV2Event {
    pub amount: u64,                     // ← Large swap = price impact
    pub other_amount_threshold: u64,     // ← Expected output
    pub sqrt_price_limit_x64: u128,      // ← Price limit
    pub is_base_input: bool,             // ← Direction
    pub pool_state: Pubkey,              // ← Pool identifier
    pub input_vault_mint: Pubkey,        // ← Token A
    pub output_vault_mint: Pubkey,       // ← Token B
}
```

**Opportunity Detection Signals:**
- **Large swaps** → Temporary price inefficiency (slippage creates arb)
- **Multiple pools** with same token pair → Direct arbitrage opportunity
- **Cross-DEX** (Raydium vs Orca vs Meteora) → Price discrepancies
- **Swap direction imbalance** → One-sided pressure = opportunity

### 2. Pool State Updates (`RaydiumClmmPoolStateAccountEvent`)

**Location:** `src/streaming/event_parser/protocols/raydium_clmm/types.rs:78`

```rust
pub struct PoolState {
    pub liquidity: u128,                  // ← Available liquidity
    pub sqrt_price_x64: u128,             // ← Current price
    pub tick_current: i32,                // ← Current tick
    pub token_vault0: Pubkey,             // ← Token reserves
    pub token_vault1: Pubkey,
    pub protocol_fees_token0: u64,        // ← Fee tracking
    pub protocol_fees_token1: u64,
    pub swap_in_amount_token0: u128,      // ← Cumulative volumes
    pub swap_out_amount_token1: u128,
    pub token_mint0: Pubkey,
    pub token_mint1: Pubkey,
}
```

**Critical Metrics for Flash Loans:**
- **Liquidity depth** → Maximum loan amount possible
- **Current price** → Arbitrage profit calculation
- **Fee rates** → Profitability threshold
- **Token vaults** → Available reserves for borrowing

### 3. Pool Configuration (`AmmConfig`)

**Location:** `src/streaming/event_parser/protocols/raydium_clmm/types.rs:18`

```rust
pub struct AmmConfig {
    pub protocol_fee_rate: u32,
    pub trade_fee_rate: u32,
    pub tick_spacing: u16,
}
```

**Cost Calculation:**
- Fee rates affect profit margins
- Must account for: Flash loan fee + Swap fees (Pool A + Pool B)

---

## Implementation Roadmap

### Phase 1: Local Opportunity Detector Module

**New File:** `src/flash_loan/opportunity_detector.rs`

```rust
use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;

use crate::streaming::event_parser::protocols::raydium_clmm::{
    RaydiumClmmSwapV2Event,
    RaydiumClmmPoolStateAccountEvent,
    types::PoolState,
};

/// Represents a detected arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    /// Pool to buy from (lower price)
    pub pool_a: Pubkey,
    /// Pool to sell to (higher price)
    pub pool_b: Pubkey,
    /// Token to arbitrage
    pub token_mint: Pubkey,
    /// Price in Pool A
    pub price_a: f64,
    /// Price in Pool B
    pub price_b: f64,
    /// Expected profit after all fees (in lamports)
    pub expected_profit: u64,
    /// Optimal loan amount to maximize profit
    pub loan_amount: u64,
    /// Timestamp of opportunity detection
    pub timestamp: i64,
    /// Confidence score (0-100)
    pub confidence: u8,
}

/// Detects arbitrage opportunities from streaming events
pub struct OpportunityDetector {
    /// Cache of pool states indexed by pool pubkey
    pool_states: HashMap<Pubkey, PoolState>,
    /// Price feeds indexed by token mint
    price_feed: HashMap<Pubkey, Vec<PoolPrice>>,
    /// Minimum profit threshold (in lamports)
    min_profit_threshold: u64,
    /// Maximum loan amount (risk management)
    max_loan_amount: u64,
}

#[derive(Debug, Clone)]
struct PoolPrice {
    pool: Pubkey,
    price: f64,
    liquidity: u128,
    timestamp: i64,
}

impl OpportunityDetector {
    pub fn new(min_profit_threshold: u64, max_loan_amount: u64) -> Self {
        Self {
            pool_states: HashMap::new(),
            price_feed: HashMap::new(),
            min_profit_threshold,
            max_loan_amount,
        }
    }

    /// Analyze swap event for arbitrage opportunities
    pub fn analyze_swap_event(
        &mut self,
        event: &RaydiumClmmSwapV2Event
    ) -> Option<ArbitrageOpportunity> {
        // 1. Extract price from swap event
        let price = self.calculate_swap_price(event)?;

        // 2. Update price feed
        self.update_price_feed(&event.input_vault_mint, event.pool_state, price);

        // 3. Look for cross-pool arbitrage
        self.find_arbitrage_opportunity(&event.input_vault_mint)
    }

    /// Update pool state cache from account events
    pub fn update_pool_state(&mut self, event: &RaydiumClmmPoolStateAccountEvent) {
        self.pool_states.insert(event.pubkey, event.pool_state.clone());
    }

    /// Calculate effective price from swap
    fn calculate_swap_price(&self, event: &RaydiumClmmSwapV2Event) -> Option<f64> {
        // Use sqrt_price_x64 to calculate effective price
        let sqrt_price = event.sqrt_price_limit_x64 as f64 / (1u128 << 64) as f64;
        Some(sqrt_price * sqrt_price)
    }

    /// Find arbitrage opportunities across pools
    fn find_arbitrage_opportunity(
        &self,
        token_mint: &Pubkey
    ) -> Option<ArbitrageOpportunity> {
        let prices = self.price_feed.get(token_mint)?;

        if prices.len() < 2 {
            return None;
        }

        // Find lowest and highest price
        let (min_price_pool, max_price_pool) = self.find_price_spread(prices)?;

        // Calculate potential profit
        let profit = self.calculate_profit(
            &min_price_pool,
            &max_price_pool
        )?;

        if profit.expected_profit < self.min_profit_threshold {
            return None;
        }

        Some(profit)
    }

    /// Calculate expected profit considering all fees
    fn calculate_profit(
        &self,
        buy_pool: &PoolPrice,
        sell_pool: &PoolPrice
    ) -> Option<ArbitrageOpportunity> {
        // Price spread
        let price_diff = sell_pool.price - buy_pool.price;
        let price_spread_pct = price_diff / buy_pool.price;

        // Estimate optimal loan amount based on liquidity
        let optimal_loan = self.calculate_optimal_loan_size(buy_pool, sell_pool);

        // Calculate costs
        let flash_loan_fee = optimal_loan * 9 / 10000; // 0.09% Solend fee
        let swap_fee_a = optimal_loan * 25 / 10000; // ~0.25% typical
        let swap_fee_b = optimal_loan * 25 / 10000;
        let total_fees = flash_loan_fee + swap_fee_a + swap_fee_b;

        // Calculate gross profit
        let gross_profit = (optimal_loan as f64 * price_spread_pct) as u64;

        // Net profit
        let expected_profit = gross_profit.saturating_sub(total_fees);

        // Confidence score based on liquidity and spread
        let confidence = self.calculate_confidence(buy_pool, sell_pool, price_spread_pct);

        Some(ArbitrageOpportunity {
            pool_a: buy_pool.pool,
            pool_b: sell_pool.pool,
            token_mint: Pubkey::default(), // Fill from context
            price_a: buy_pool.price,
            price_b: sell_pool.price,
            expected_profit,
            loan_amount: optimal_loan,
            timestamp: chrono::Utc::now().timestamp(),
            confidence,
        })
    }

    fn calculate_optimal_loan_size(
        &self,
        buy_pool: &PoolPrice,
        sell_pool: &PoolPrice
    ) -> u64 {
        // Use Kelly Criterion or simpler heuristic
        // Take smaller of: 10% of min liquidity or max_loan_amount
        let min_liquidity = buy_pool.liquidity.min(sell_pool.liquidity);
        let loan = (min_liquidity / 10) as u64;
        loan.min(self.max_loan_amount)
    }

    fn calculate_confidence(
        &self,
        buy_pool: &PoolPrice,
        sell_pool: &PoolPrice,
        spread_pct: f64
    ) -> u8 {
        let mut confidence = 0u8;

        // High liquidity = higher confidence
        if buy_pool.liquidity > 1_000_000 && sell_pool.liquidity > 1_000_000 {
            confidence += 40;
        }

        // Larger spread = higher confidence
        if spread_pct > 0.01 {
            confidence += 30;
        }

        // Recent data = higher confidence
        let now = chrono::Utc::now().timestamp();
        if now - buy_pool.timestamp < 5 {
            confidence += 30;
        }

        confidence
    }

    fn find_price_spread(&self, prices: &[PoolPrice]) -> Option<(PoolPrice, PoolPrice)> {
        let mut min = prices[0].clone();
        let mut max = prices[0].clone();

        for price in prices.iter().skip(1) {
            if price.price < min.price {
                min = price.clone();
            }
            if price.price > max.price {
                max = price.clone();
            }
        }

        Some((min, max))
    }

    fn update_price_feed(&mut self, token: &Pubkey, pool: Pubkey, price: f64) {
        let pool_price = PoolPrice {
            pool,
            price,
            liquidity: self.pool_states
                .get(&pool)
                .map(|s| s.liquidity)
                .unwrap_or(0),
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.price_feed
            .entry(*token)
            .or_insert_with(Vec::new)
            .push(pool_price);
    }
}
```

---

### Phase 2: On-Chain Flash Loan Receiver Program

**New Anchor Project:** `programs/flash-loan-receiver/`

**File:** `programs/flash-loan-receiver/src/lib.rs`

```rust
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("YOUR_PROGRAM_ID_HERE");

#[program]
pub mod flash_loan_receiver {
    use super::*;

    /// Receives flash loan from Solend and executes arbitrage
    /// This instruction is called via CPI from Solend's flash loan program
    pub fn receive_flash_loan(
        ctx: Context<ReceiveFlashLoan>,
        repay_amount: u64,
    ) -> Result<()> {
        msg!("Flash loan received: {} tokens", repay_amount);

        // Calculate borrowed amount (repay_amount includes fee)
        let borrowed_amount = calculate_borrowed_amount(repay_amount);

        // Verify we received the borrowed tokens
        let token_balance = ctx.accounts.token_account.amount;
        require!(
            token_balance >= borrowed_amount,
            ErrorCode::InsufficientBorrowedFunds
        );

        // Execute arbitrage strategy
        execute_arbitrage_strategy(
            &ctx,
            borrowed_amount,
        )?;

        // Verify we have enough to repay
        ctx.accounts.token_account.reload()?;
        require!(
            ctx.accounts.token_account.amount >= repay_amount,
            ErrorCode::InsufficientRepaymentFunds
        );

        msg!("Arbitrage executed, repaying {} tokens", repay_amount);

        Ok(())
    }
}

/// Execute the arbitrage strategy: buy low on Pool A, sell high on Pool B
fn execute_arbitrage_strategy(
    ctx: &Context<ReceiveFlashLoan>,
    amount: u64,
) -> Result<()> {
    // Step 1: Swap on Pool A (buy at lower price)
    swap_on_raydium_clmm(
        ctx.accounts.raydium_program.to_account_info(),
        ctx.accounts.pool_a.to_account_info(),
        ctx.accounts.token_account.to_account_info(),
        ctx.accounts.intermediate_token_account.to_account_info(),
        amount,
        0, // min output (calculate based on slippage)
        true, // is_base_input
    )?;

    // Step 2: Swap on Pool B (sell at higher price)
    let intermediate_amount = ctx.accounts.intermediate_token_account.amount;
    swap_on_raydium_clmm(
        ctx.accounts.raydium_program.to_account_info(),
        ctx.accounts.pool_b.to_account_info(),
        ctx.accounts.intermediate_token_account.to_account_info(),
        ctx.accounts.token_account.to_account_info(),
        intermediate_amount,
        0, // min output
        false, // is_base_input
    )?;

    Ok(())
}

/// Call Raydium CLMM swap via CPI
fn swap_on_raydium_clmm(
    raydium_program: AccountInfo,
    pool: AccountInfo,
    input_account: AccountInfo,
    output_account: AccountInfo,
    amount: u64,
    min_output: u64,
    is_base_input: bool,
) -> Result<()> {
    // Build Raydium swap instruction
    // This requires understanding Raydium CLMM's CPI interface

    msg!("Executing swap: amount={}, is_base_input={}", amount, is_base_input);

    // TODO: Implement actual CPI call to Raydium CLMM
    // See Raydium SDK for instruction format

    Ok(())
}

fn calculate_borrowed_amount(repay_amount: u64) -> u64 {
    // Solend flash loan fee is typically 0.09%
    // borrowed_amount = repay_amount / 1.0009
    (repay_amount * 10000) / 10009
}

#[derive(Accounts)]
pub struct ReceiveFlashLoan<'info> {
    /// The account receiving/repaying the flash loan
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,

    /// Intermediate token account for multi-hop swaps
    #[account(mut)]
    pub intermediate_token_account: Account<'info, TokenAccount>,

    /// Pool A (buy at lower price)
    /// CHECK: Validated by Raydium program
    #[account(mut)]
    pub pool_a: AccountInfo<'info>,

    /// Pool B (sell at higher price)
    /// CHECK: Validated by Raydium program
    #[account(mut)]
    pub pool_b: AccountInfo<'info>,

    /// Raydium CLMM program
    /// CHECK: Hardcoded program ID
    pub raydium_program: AccountInfo<'info>,

    /// Authority (program signer)
    pub authority: Signer<'info>,

    /// Token program
    pub token_program: Program<'info, Token>,

    // Add more accounts as needed for Raydium swaps
}

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient borrowed funds received")]
    InsufficientBorrowedFunds,
    #[msg("Insufficient funds to repay flash loan")]
    InsufficientRepaymentFunds,
    #[msg("Arbitrage execution failed")]
    ArbitrageFailed,
}
```

**File:** `programs/flash-loan-receiver/Cargo.toml`

```toml
[package]
name = "flash-loan-receiver"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]
name = "flash_loan_receiver"

[dependencies]
anchor-lang = "0.30.0"
anchor-spl = "0.30.0"
```

---

### Phase 3: Local Transaction Builder Module

**New File:** `src/flash_loan/transaction_builder.rs`

```rust
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use anyhow::Result;

use crate::flash_loan::opportunity_detector::ArbitrageOpportunity;

/// Builds and submits flash loan transactions
pub struct FlashLoanTxBuilder {
    client: RpcClient,
    payer: Keypair,
    flash_loan_receiver_program: Pubkey,
}

impl FlashLoanTxBuilder {
    pub fn new(
        rpc_url: String,
        payer: Keypair,
        flash_loan_receiver_program: Pubkey,
    ) -> Self {
        Self {
            client: RpcClient::new(rpc_url),
            payer,
            flash_loan_receiver_program,
        }
    }

    /// Build and submit flash loan transaction
    pub async fn execute_flash_loan(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<Signature> {
        // 1. Build flash loan instruction (from Solend)
        let flash_loan_ix = self.build_solend_flash_loan_instruction(opportunity)?;

        // 2. Get recent blockhash
        let recent_blockhash = self.client.get_latest_blockhash()?;

        // 3. Create transaction
        let tx = Transaction::new_signed_with_payer(
            &[flash_loan_ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        // 4. Submit transaction
        let signature = self.client.send_and_confirm_transaction(&tx)?;

        Ok(signature)
    }

    /// Build Solend flash loan instruction
    fn build_solend_flash_loan_instruction(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<Instruction> {
        // Solend flash loan instruction format
        // Reference: https://github.com/solendprotocol/solana-program-library

        let solend_program_id = solana_sdk::pubkey!("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo");

        // TODO: Build actual Solend flash loan instruction
        // This requires:
        // 1. Solend reserve account (liquidity source)
        // 2. Your receiver program ID
        // 3. Loan amount
        // 4. All required accounts

        Ok(Instruction {
            program_id: solend_program_id,
            accounts: vec![
                // Add Solend accounts
            ],
            data: vec![
                // Flash loan instruction data
            ],
        })
    }
}
```

---

### Phase 4: Integration with Existing Streaming System

**Modified File:** `examples/flash_loan_arbitrage_example.rs`

```rust
use solana_streamer_sdk::streaming::{
    event_parser::{
        common::{filter::EventTypeFilter, EventType},
        protocols::raydium_clmm::{
            RaydiumClmmSwapV2Event, RaydiumClmmPoolStateAccountEvent,
        },
        UnifiedEvent,
    },
    grpc::YellowstoneGrpc,
};
use std::sync::Arc;
use tokio::sync::Mutex;

mod flash_loan;
use flash_loan::{
    opportunity_detector::OpportunityDetector,
    transaction_builder::FlashLoanTxBuilder,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize streaming client
    let endpoint = "http://your-grpc-endpoint.com";
    let token = "your-auth-token";
    let grpc = YellowstoneGrpc::new_low_latency(endpoint, token)?;

    // Initialize opportunity detector
    let detector = Arc::new(Mutex::new(OpportunityDetector::new(
        1_000_000, // min profit: 0.001 SOL
        100_000_000_000, // max loan: 100 SOL
    )));

    // Initialize transaction builder
    let payer = load_keypair("path/to/keypair.json")?;
    let receiver_program = solana_sdk::pubkey!("YOUR_RECEIVER_PROGRAM_ID");
    let tx_builder = Arc::new(FlashLoanTxBuilder::new(
        "https://api.mainnet-beta.solana.com".to_string(),
        payer,
        receiver_program,
    ));

    // Filter for relevant events
    let event_filter = Some(EventTypeFilter {
        include: vec![
            EventType::RaydiumClmmSwapV2,
            EventType::RaydiumClmmPoolStateAccount,
        ],
    });

    // Subscribe to Raydium CLMM events
    grpc.subscribe_immediate(
        vec![
            // Transaction filter for swaps
            TransactionFilter {
                account_include: vec![
                    "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string() // Raydium CLMM
                ],
                account_exclude: vec![],
                account_required: vec![],
            },
        ],
        vec![
            // Account filter for pool states
            AccountFilter {
                account: vec![],
                owner: vec!["CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()],
                filters: vec![],
            },
        ],
        event_filter,
        move |events| {
            let detector = detector.clone();
            let tx_builder = tx_builder.clone();

            tokio::spawn(async move {
                handle_events(events, detector, tx_builder).await;
            });
        },
    )
    .await?;

    Ok(())
}

async fn handle_events(
    events: Vec<Box<dyn UnifiedEvent>>,
    detector: Arc<Mutex<OpportunityDetector>>,
    tx_builder: Arc<FlashLoanTxBuilder>,
) {
    for event in events {
        // Handle swap events
        if let Some(swap_event) = event.as_any().downcast_ref::<RaydiumClmmSwapV2Event>() {
            let mut detector = detector.lock().await;

            if let Some(opportunity) = detector.analyze_swap_event(swap_event) {
                println!("🎯 Opportunity detected!");
                println!("   Pool A: {}", opportunity.pool_a);
                println!("   Pool B: {}", opportunity.pool_b);
                println!("   Expected profit: {} lamports", opportunity.expected_profit);
                println!("   Confidence: {}%", opportunity.confidence);

                // Execute flash loan if confidence is high
                if opportunity.confidence >= 70 {
                    match tx_builder.execute_flash_loan(&opportunity).await {
                        Ok(sig) => println!("✅ Flash loan executed: {}", sig),
                        Err(e) => println!("❌ Flash loan failed: {}", e),
                    }
                }
            }
        }

        // Handle pool state updates
        if let Some(pool_event) = event.as_any().downcast_ref::<RaydiumClmmPoolStateAccountEvent>() {
            let mut detector = detector.lock().await;
            detector.update_pool_state(pool_event);
        }
    }
}

fn load_keypair(path: &str) -> anyhow::Result<solana_sdk::signature::Keypair> {
    // Load keypair from file
    todo!()
}
```

---

## Most Powerful Data Streams for Flash Loans

### 1. Real-Time Swap Events
**Purpose:** Detect price movements and opportunities
```rust
let event_filter = Some(EventTypeFilter {
    include: vec![EventType::RaydiumClmmSwapV2]
});
```

**Signals:**
- Large swaps (>$10k) → Price impact creates temporary inefficiency
- Multiple swaps in same direction → Momentum for cross-pool arb
- Unusual swap patterns → Potential MEV opportunities

### 2. Pool State Updates
**Purpose:** Monitor liquidity and pricing
```rust
let account_filter = AccountFilter {
    owner: vec!["CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()],
    filters: vec![],
};
```

**Signals:**
- Liquidity changes → Affects maximum loan size
- Price updates → Real-time arbitrage calculation
- Fee accumulation → Pool profitability indicator

### 3. Multi-Pool Monitoring
**Purpose:** Cross-pool arbitrage detection
```rust
// Subscribe to multiple pools with same token pairs
let pools = vec![
    "Pool1_USDC_SOL",
    "Pool2_USDC_SOL",
    "Pool3_USDC_SOL",
];
```

**Strategy:** Detect price discrepancies between pools trading the same pair

### 4. Large Transaction Detection
**Purpose:** MEV and front-running opportunities
```rust
// Filter for large swaps only
if swap_event.amount > 1_000_000_000 { // >1 SOL
    analyze_for_sandwich_opportunity(swap_event);
}
```

---

## Risk Management Considerations

### 1. **Transaction Failure Risk**
- Flash loan reverts if arbitrage doesn't profit
- **Mitigation:** Simulate transactions before submission

### 2. **Slippage Risk**
- Prices may move between detection and execution
- **Mitigation:** Set conservative slippage limits, require minimum profit margin

### 3. **Gas/Fee Risk**
- Transaction fees can eat into small profits
- **Mitigation:** Set minimum profit threshold above all fees

### 4. **Competition Risk**
- Other bots may detect same opportunity
- **Mitigation:** Optimize latency, use private RPC endpoints

### 5. **Smart Contract Risk**
- Bugs in receiver program = lost funds
- **Mitigation:** Thorough testing, audits, start with small amounts

---

## Performance Optimization Tips

1. **Use Low-Latency Configuration**
   ```rust
   let grpc = YellowstoneGrpc::new_low_latency(endpoint, token)?;
   ```

2. **Event Filtering**
   - Only subscribe to relevant events
   - Reduces processing overhead by 60-80%

3. **Pre-compute Accounts**
   - Cache pool state accounts
   - Pre-build transaction templates

4. **Parallel Processing**
   - Analyze multiple opportunities concurrently
   - Use async/await effectively

5. **Private RPC**
   - Use dedicated Solana RPC node
   - Reduces latency and rate limits

---

## Testing Strategy

### 1. Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opportunity_detection() {
        let mut detector = OpportunityDetector::new(1_000_000, 100_000_000_000);
        // Test price spread detection
    }

    #[test]
    fn test_profit_calculation() {
        // Test fee calculations
    }
}
```

### 2. Simulation Testing
- Use Solana localnet
- Simulate flash loan scenarios
- Test with real historical data

### 3. Devnet Testing
- Deploy receiver program to devnet
- Test with small amounts
- Validate end-to-end flow

### 4. Mainnet Testing
- Start with very small amounts
- Monitor closely for first 24 hours
- Gradually increase limits

---

## Deployment Checklist

- [ ] Opportunity detector module implemented
- [ ] Flash loan receiver program written and tested
- [ ] Transaction builder module implemented
- [ ] Integration with streaming system complete
- [ ] Unit tests passing
- [ ] Devnet testing successful
- [ ] Security audit completed (recommended)
- [ ] Risk management parameters configured
- [ ] Monitoring and alerting set up
- [ ] Emergency shutdown mechanism implemented
- [ ] Documentation complete
- [ ] Mainnet deployment with small test amount

---

## Resources

- **Solend Flash Loan Docs:** https://github.com/solendprotocol/solana-program-library/blob/mainnet/token-lending/flash_loan_design.md
- **Raydium CLMM SDK:** https://github.com/raydium-io/raydium-clmm
- **Anchor Framework:** https://www.anchor-lang.com/
- **Solana CPI Guide:** https://docs.solana.com/developing/programming-model/calling-between-programs

---

## Next Steps

1. **Implement Opportunity Detector** - Start with basic price spread detection
2. **Build On-Chain Program** - Use Anchor framework for receiver program
3. **Test on Devnet** - Validate with small test transactions
4. **Optimize Performance** - Fine-tune detection algorithms and latency
5. **Deploy to Mainnet** - Start small and scale gradually

---

## Questions & Considerations

**Q: How much capital is needed to start?**
A: Start with 1-10 SOL for testing. Flash loans don't require upfront capital, but you need gas fees and a buffer.

**Q: What are typical flash loan fees?**
A: Solend charges ~0.09%, plus you pay swap fees (~0.25% per swap), so total costs ~0.6% round-trip.

**Q: How fast do opportunities disappear?**
A: Usually 1-5 seconds. Low latency is critical.

**Q: Can I use this with other DEXs?**
A: Yes! Expand to Orca, Meteora, Phoenix for more opportunities.

**Q: Do I need to be a liquidity provider?**
A: No, flash loans let you borrow without collateral.