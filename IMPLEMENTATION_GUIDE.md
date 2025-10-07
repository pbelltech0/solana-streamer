# Implementation Guide: Next Steps

## Quick Start

### 1. Test the Current System

```bash
# Build the project
cargo build --release

# Run the focused arbitrage example
cargo run --example focused_liquidity_arbitrage
```

**Expected Output**: You should see:
- Connection to Yellowstone gRPC
- Pool state updates from Raydium CLMM/CPMM
- Swap events
- Periodic arbitrage opportunity scans

### 2. Customize for Your Token Pairs

Edit `examples/focused_liquidity_arbitrage.rs`:

```rust
// Line ~60: Add your specific tokens
let bonk = Pubkey::from_str("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263").unwrap();
let jup = Pubkey::from_str("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN").unwrap();

monitored_pairs: vec![
    MonitoredPair {
        name: "BONK/SOL".to_string(),
        token_a: bonk,
        token_b: sol,
        min_trade_size: 1_000_000_000,    // Adjust based on token decimals
        max_trade_size: 100_000_000_000,
        target_pools: vec![],
    },
]
```

## Priority Implementation Tasks

### Phase 1: Enhanced Pool State Tracking (High Priority)

**Problem**: Currently we only get partial pool state from account events. We need full reserve data.

**Solution**: Query token vault accounts for actual reserves.

**File**: Create `src/streaming/pool_state_fetcher.rs`

```rust
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::state::Account as TokenAccount;

pub struct PoolStateFetcher {
    rpc_client: RpcClient,
}

impl PoolStateFetcher {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_client: RpcClient::new(rpc_url),
        }
    }

    /// Fetch token account balance
    pub async fn get_token_balance(&self, token_account: &Pubkey) -> Result<u64, Error> {
        let account = self.rpc_client.get_account(token_account)?;
        let token_account = TokenAccount::unpack(&account.data)?;
        Ok(token_account.amount)
    }

    /// Update pool state with actual reserves
    pub async fn enrich_pool_state(&self, pool_state: &mut PoolState) -> Result<(), Error> {
        // For Raydium CLMM: query token_vault0 and token_vault1
        // For Raydium CPMM: query pool token accounts
        // Update pool_state.reserve_a and pool_state.reserve_b
        Ok(())
    }
}
```

**Integration Point**: `examples/focused_liquidity_arbitrage.rs`
```rust
// After receiving pool state event
let pool_state_fetcher = PoolStateFetcher::new(rpc_url);
pool_state_fetcher.enrich_pool_state(&mut pool_state).await?;
detector.update_pool_state(pool_state);
```

**Estimated Time**: 2-3 hours
**Impact**: Critical - enables accurate price impact calculations

---

### Phase 2: Add Orca & Meteora Support (Medium Priority)

**Goal**: Parse Orca Whirlpool and Meteora DLMM events using the IDL parser.

#### Step 2a: Orca Whirlpool Events

**File**: Create `src/streaming/event_parser/protocols/orca_whirlpool/events.rs`

```rust
use crate::streaming::event_parser::common::EventMetadata;
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrcaWhirlpoolSwapEvent {
    pub metadata: EventMetadata,
    pub whirlpool: Pubkey,
    pub token_authority: Pubkey,
    pub token_owner_account_a: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_owner_account_b: Pubkey,
    pub token_vault_b: Pubkey,
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool, // Trade direction!
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrcaLiquidityEvent {
    pub metadata: EventMetadata,
    pub whirlpool: Pubkey,
    pub position: Pubkey,
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
    pub liquidity: u128,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}
```

**File**: `src/streaming/event_parser/protocols/orca_whirlpool/parser.rs`

```rust
// Use the IDL parser from dex-idl-parser
use dex_idl_parser::prelude::*;

pub const ORCA_WHIRLPOOL_PROGRAM_ID: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

pub fn parse_orca_event(instruction_data: &[u8], accounts: &[Pubkey])
    -> Option<Box<dyn UnifiedEvent>> {
    // Use DexStreamParser from dex-idl-parser
    // Parse instruction and create event
}
```

**Estimated Time**: 4-6 hours
**Impact**: High - doubles DEX coverage

#### Step 2b: Meteora DLMM Events

**File**: Create `src/streaming/event_parser/protocols/meteora_dlmm/events.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeteoraSwapEvent {
    pub metadata: EventMetadata,
    pub lb_pair: Pubkey,  // Important: LbPair address
    pub from: Pubkey,
    pub start_bin_id: i32,
    pub end_bin_id: i32,
    pub amount_in: u64,
    pub amount_out: u64,
    pub swap_for_y: bool, // Trade direction
    pub fee: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeteoraLiquidityEvent {
    pub metadata: EventMetadata,
    pub lb_pair: Pubkey,
    pub position: Pubkey,
    pub amounts: [u64; 2], // [token_x, token_y]
    pub active_bin_id: i32,
}
```

**Estimated Time**: 4-6 hours
**Impact**: High - adds DLMM pools (often best prices)

---

### Phase 3: Jupiter V6 Integration (High Priority)

**Goal**: Use Jupiter Aggregator to find optimal multi-hop routes when direct pools aren't profitable.

**Dependencies**:
```toml
# Add to Cargo.toml
jupiter-swap-api-client = "0.1"
reqwest = { version = "0.11", features = ["json"] }
```

**File**: Create `src/execution/jupiter_router.rs`

```rust
use solana_sdk::pubkey::Pubkey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JupiterQuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JupiterQuote {
    pub in_amount: u64,
    pub out_amount: u64,
    pub price_impact_pct: f64,
    pub route_plan: Vec<RoutePlanStep>,
}

pub struct JupiterRouter {
    client: reqwest::Client,
    api_url: String,
}

impl JupiterRouter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: "https://quote-api.jup.ag/v6".to_string(),
        }
    }

    pub async fn get_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<JupiterQuote, Error> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}",
            self.api_url, input_mint, output_mint, amount
        );

        let response = self.client.get(&url).send().await?;
        let quote: JupiterQuote = response.json().await?;

        Ok(quote)
    }

    /// Compare Jupiter route vs direct DEX swap
    pub async fn is_jupiter_better(
        &self,
        opportunity: &EnhancedArbitrageOpportunity,
    ) -> Result<bool, Error> {
        let jupiter_quote = self.get_quote(
            &opportunity.token_pair.base,
            &opportunity.token_pair.quote,
            opportunity.optimal_trade_size,
        ).await?;

        // Compare:
        // 1. Price (Jupiter out_amount vs direct swap)
        // 2. Fees (Jupiter fees vs DEX fees)
        // 3. Success probability (multi-hop is riskier)

        let jupiter_net = jupiter_quote.out_amount as f64 * 0.99; // Account for risk
        let direct_net = opportunity.expected_output as f64;

        Ok(jupiter_net > direct_net)
    }
}
```

**Integration**: `src/streaming/enhanced_arbitrage.rs`

```rust
impl EnhancedArbitrageDetector {
    pub async fn optimize_with_jupiter(
        &self,
        opportunity: &EnhancedArbitrageOpportunity,
        jupiter: &JupiterRouter,
    ) -> EnhancedArbitrageOpportunity {
        if jupiter.is_jupiter_better(opportunity).await.unwrap_or(false) {
            // Use Jupiter route instead
            // Update opportunity with Jupiter pricing
        }
        opportunity.clone()
    }
}
```

**Estimated Time**: 6-8 hours
**Impact**: Very High - can significantly improve profitability

---

### Phase 4: Jito Bundle Execution (Critical for Production)

**Goal**: Atomic, MEV-protected trade execution.

**Dependencies**:
```toml
jito-searcher-client = "0.1"
solana-transaction-status = "3.0"
```

**File**: Create `src/execution/jito_executor.rs`

```rust
use jito_searcher_client::SearcherClient;
use solana_sdk::{
    signature::Keypair,
    transaction::Transaction,
};

pub struct JitoExecutor {
    searcher_client: SearcherClient,
    searcher_keypair: Keypair,
    block_engine_url: String,
}

impl JitoExecutor {
    pub fn new(searcher_keypair: Keypair) -> Self {
        Self {
            searcher_client: SearcherClient::new(
                "mainnet.block-engine.jito.wtf".to_string()
            ),
            searcher_keypair,
            block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
        }
    }

    pub async fn execute_arbitrage(
        &self,
        opportunity: &EnhancedArbitrageOpportunity,
    ) -> Result<Signature, Error> {
        // 1. Build swap transactions
        let buy_tx = self.build_swap_tx(
            &opportunity.buy_pool,
            &opportunity.token_pair.base,
            &opportunity.token_pair.quote,
            opportunity.optimal_trade_size,
        )?;

        let sell_tx = self.build_swap_tx(
            &opportunity.sell_pool,
            &opportunity.token_pair.quote,
            &opportunity.token_pair.base,
            opportunity.expected_output,
        )?;

        // 2. Calculate optimal tip
        let tip = self.calculate_tip(opportunity.expected_profit);

        let tip_tx = self.build_tip_tx(tip)?;

        // 3. Create bundle (all-or-nothing execution)
        let bundle = vec![buy_tx, sell_tx, tip_tx];

        // 4. Submit to Jito
        let bundle_id = self.searcher_client
            .send_bundle(bundle)
            .await?;

        // 5. Monitor bundle status
        self.wait_for_bundle_confirmation(bundle_id).await
    }

    fn calculate_tip(&self, expected_profit: u64) -> u64 {
        // Start with 10% of expected profit
        // Increase if bundle rejections are high
        (expected_profit as f64 * 0.10) as u64
    }

    fn build_swap_tx(
        &self,
        pool: &Pubkey,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
    ) -> Result<Transaction, Error> {
        // Build swap transaction for specific DEX
        // Handle Raydium, Orca, Meteora differently
        todo!("Implement swap tx builder")
    }
}
```

**Safety Checks**:
```rust
impl JitoExecutor {
    fn validate_opportunity(&self, opp: &EnhancedArbitrageOpportunity) -> bool {
        // Don't execute if:
        opp.combined_execution_prob > 0.5      // Min 50% success chance
            && opp.net_profit > 1_000_000      // Min 0.001 SOL profit
            && opp.ev_score > 20.0             // Min EV score
    }

    fn has_sufficient_balance(&self, required: u64) -> bool {
        // Check wallet balance before executing
        todo!()
    }
}
```

**Estimated Time**: 8-12 hours
**Impact**: Critical - enables actual trade execution

---

## Testing Strategy

### 1. Unit Tests

```bash
cargo test --lib
```

**Key Tests**:
- `test_price_impact_calculation` ✅
- `test_execution_probability` ✅
- `test_opportunity_evaluation` ✅
- `test_pool_state_updates` (TODO)
- `test_jupiter_integration` (TODO)

### 2. Integration Tests

**File**: `tests/arbitrage_integration_test.rs`

```rust
#[tokio::test]
async fn test_end_to_end_arbitrage_detection() {
    // 1. Set up detector with test config
    // 2. Simulate pool state updates
    // 3. Simulate swap events
    // 4. Verify opportunities are detected
    // 5. Validate EV calculations
}

#[tokio::test]
async fn test_jupiter_fallback() {
    // Verify Jupiter is used when direct swap isn't profitable
}
```

### 3. Live Testing (Devnet)

```bash
# Point to devnet RPC
export SOLANA_RPC_URL="https://api.devnet.solana.com"

# Run with test wallet
cargo run --example focused_liquidity_arbitrage
```

**Devnet Limitations**: Fewer pools, lower liquidity, but safe for testing

### 4. Mainnet Simulation (No Execution)

```bash
# Monitor mainnet but don't execute
cargo run --example focused_liquidity_arbitrage
```

**What to Monitor**:
- Opportunity frequency (should see 5-20 per minute with good params)
- EV scores (few should be >30, many 10-20)
- Execution probabilities (should cluster 30-70%)
- Net profit after fees (most should be 0.3-2%)

---

## Production Checklist

Before going live with real funds:

### Security
- [ ] Wallet private keys stored securely (not in code!)
- [ ] Max trade size limits enforced
- [ ] Emergency stop mechanism implemented
- [ ] Failed transaction handling
- [ ] Rate limiting for RPC calls

### Risk Management
- [ ] Position size limits (max % of portfolio per trade)
- [ ] Daily loss limits
- [ ] Minimum balance checks
- [ ] Slippage protection
- [ ] Failed trade cooldown period

### Monitoring
- [ ] Metrics export (Prometheus/Grafana)
- [ ] Alert system (Discord/Telegram webhook)
- [ ] Trade logging (SQLite/PostgreSQL)
- [ ] Performance dashboard
- [ ] Error tracking (Sentry)

### Performance
- [ ] Connection pooling optimized
- [ ] Event processing <100ms
- [ ] Opportunity detection <500ms
- [ ] Bundle submission <1s

---

## Recommended Development Order

**Week 1**: Pool State Enrichment
- Implement `PoolStateFetcher`
- Test with Raydium CLMM pools
- Verify accurate reserve data

**Week 2**: Orca & Meteora
- Add Orca event types
- Add Meteora event types
- Test multi-DEX arbitrage detection

**Week 3**: Jupiter Integration
- Implement Jupiter API client
- Add route comparison logic
- Test with real quotes

**Week 4**: Jito Execution (Devnet)
- Build transaction builders for each DEX
- Implement bundle creation
- Test on devnet

**Week 5**: Testing & Optimization
- Integration tests
- Mainnet simulation
- Performance tuning

**Week 6**: Production Launch
- Start with small trade sizes
- Monitor for 1 week
- Gradually increase size if profitable

---

## Cost Estimates

### Development Time
- Phase 1: 2-3 hours
- Phase 2: 8-12 hours
- Phase 3: 6-8 hours
- Phase 4: 8-12 hours
- Testing: 8-16 hours
**Total: 32-51 hours**

### Infrastructure Costs (Monthly)
- Yellowstone gRPC: $0-100 (depending on provider)
- RPC calls: $50-200 (QuickNode/Helius)
- Server/VM: $20-100 (AWS/GCP)
- Jito tips: Variable (5-15% of profits)
**Total: ~$100-400/month**

### Capital Requirements
- **Minimum**: 1 SOL ($150-200) for testing
- **Recommended**: 5-10 SOL for meaningful profits
- **Optimal**: 50-100 SOL for best opportunities

---

## Success Metrics

### Alpha Testing (Devnet)
- ✅ System runs for 24 hours without crashes
- ✅ Detects >10 opportunities per hour
- ✅ EV calculations are consistent

### Beta Testing (Mainnet Simulation)
- ✅ Opportunities have >0.5% net profit
- ✅ Execution probability >40% average
- ✅ <1% of opportunities cause errors

### Production (Real Trading)
- 🎯 **Profitability**: >5% monthly ROI after all fees
- 🎯 **Win Rate**: >60% of executed trades profitable
- 🎯 **Uptime**: >99% system availability
- 🎯 **Risk**: <2% max daily drawdown

---

## Support & Resources

- **Solana Docs**: https://docs.solana.com
- **Jupiter API**: https://station.jup.ag/docs/apis/swap-api
- **Jito Docs**: https://jito-labs.gitbook.io/mev
- **Raydium SDK**: https://github.com/raydium-io/raydium-sdk
- **Orca SDK**: https://github.com/orca-so/whirlpools

**Questions? Issues?**
Open an issue in the GitHub repo with detailed logs and configuration.

Good luck! 🚀
