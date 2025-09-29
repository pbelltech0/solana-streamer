# Flash Loan Arbitrage - Quick Start Guide

## Three Modes of Operation

### 1. Opportunity Detection Only (Safest)

Monitor for arbitrage opportunities WITHOUT any transaction logic:

```bash
cargo run --example arbitrage_opportunity_detector
```

**What it does:**
- ✅ Detects price discrepancies between pools
- ✅ Calculates profitability
- ✅ Logs opportunities
- ❌ NO transaction building
- ❌ NO flash loan logic

### 2. Full Simulation Mode (Recommended for Testing)

Run complete flash loan logic WITHOUT submitting to blockchain:

```bash
cargo run --example flash_loan_simulation
```

**What it does:**
- ✅ Detects arbitrage opportunities
- ✅ Builds flash loan transaction logic
- ✅ Calculates all fees (flash loan + swaps)
- ✅ Shows detailed profit/loss breakdown
- ✅ Simulates execution success/failure
- ✅ Tracks simulated profits
- ❌ **Does NOT submit transactions to chain** (100% safe)

### 3. Production Mode (Real Transactions)

Execute actual flash loans on mainnet:

```bash
# Coming soon - requires deployed program
cargo run --example flash_loan_production
```

**What it does:**
- ✅ Everything from simulation mode
- ⚠️ **PLUS: Submits real transactions**
- ⚠️ **Costs real SOL for fees**
- ⚠️ **Requires deployed program**

## Example Output

### Mode 1: Detection Only
```
🎯 ARBITRAGE OPPORTUNITY DETECTED!
   Pool A (buy): 7Z4nN5QsYxHYP...
   Pool B (sell): 9Kf4P2mGxVwT...
   Price Spread: 1.33%
   Expected Profit: 0.002450 SOL
   Confidence: 85%

💭 In production mode, this would trigger a flash loan
   Status: TEST MODE - no action taken
```

### Mode 2: Full Simulation
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🧪 FLASH LOAN SIMULATION #42
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Opportunity Details:
   Pool A (buy):  7Z4nN5QsYxHYP...
   Pool B (sell): 9Kf4P2mGxVwT...
   Price A:       1.0023450000
   Price B:       1.0156780000
   Price Spread:  1.33%

💰 Financial Breakdown:
   Loan Amount:       50000000000 lamports (50.000000 SOL)
   Expected Profit:    665000000 lamports (0.665000 SOL)

   📝 Fee Breakdown:
      Flash Loan Fee:    45000 lamports (0.000045 SOL) [0.09%]
      Swap Fees:        250000 lamports (0.000250 SOL) [0.50%]
      Total Fees:       295000 lamports (0.000295 SOL)

✅ SIMULATION RESULT: SUCCESS
   Net Profit:       370000 lamports (0.000370 SOL)
   ROI:              0.74%
   Confidence: 85%
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📊 Running Statistics:
   Total Events:        1000
   Opportunities:       15
   Successful Sims:     8 ✅
   Failed Sims:         7 ❌
   Total Profit (sim):  0.003240 SOL
   Avg Profit:          0.000405 SOL
```

### Configuration

Edit the detector parameters in `examples/arbitrage_opportunity_detector.rs`:

```rust
let detector = Arc::new(Mutex::new(OpportunityDetector::new(
    1_000_000,           // Min profit: 0.001 SOL
    100_000_000_000,     // Max loan: 100 SOL
)));
```

### Using Your Own gRPC Endpoint

For lower latency, use a private gRPC endpoint:

```rust
let grpc = YellowstoneGrpc::new_with_config(
    "https://your-private-grpc-endpoint.com:443".to_string(),
    Some("your-auth-token".to_string()),
    config,
)?;
```

### Logs

All detected opportunities are logged to:
- **Console**: Real-time output with details
- **File**: `logs/arbitrage_opportunities.log` (timestamped, structured)

### Next Steps

Once you're confident in the detector:

1. **Deploy the on-chain program**:
   ```bash
   cd programs/flash-loan-receiver
   anchor build
   anchor deploy --provider.cluster devnet
   ```

2. **Update transaction builder** with your deployed program ID

3. **Enable flash loan execution** by integrating `FlashLoanTxBuilder`

4. **Test on devnet** with small amounts

5. **Deploy to mainnet** after thorough testing and audits

## Testing Strategy

### Phase 1: Monitoring Only (Current)
- Run the detector for 24-48 hours
- Analyze opportunity frequency
- Validate profit calculations
- No capital at risk

### Phase 2: Simulation
- Add transaction simulation
- Validate success rate
- Test with historical data
- Still no actual execution

### Phase 3: Devnet Deployment
- Deploy flash loan receiver to devnet
- Execute small test transactions
- Monitor success/failure rates
- Debug edge cases

### Phase 4: Mainnet (Production)
- Start with minimal capital
- Gradually increase limits
- Monitor closely for 7+ days
- Scale up slowly

## Troubleshooting

### No opportunities detected?

This is normal! Profitable arbitrage opportunities on established markets are rare because:
- MEV bots are competing
- Markets are efficient
- Opportunity windows are <1 second

To increase detection:
1. Lower `min_profit_threshold` (may include unprofitable ops)
2. Monitor more DEXs (add Orca, Meteora)
3. Use faster gRPC endpoint
4. Target newer/volatile token pairs

### Getting connection errors?

Free public endpoints may have rate limits. Consider:
- Using your own RPC node
- Getting a paid Yellowstone gRPC subscription
- Running your own validator

## Cost Analysis

### Fees to Consider

1. **Flash Loan Fee**: ~0.09% (Solend)
2. **Swap Fees**: ~0.25% per swap × 2 = 0.5%
3. **Solana Transaction**: ~0.000005 SOL
4. **Priority Fees**: Variable (MEV competition)

**Total Cost**: ~0.6% + gas

**Minimum Profitable Spread**: >1% to be safe

### Capital Requirements

Flash loans require NO upfront capital, but you need:
- SOL for transaction fees (~0.1 SOL buffer)
- Deployed program (~1-2 SOL for rent)

### Expected Returns

Highly variable and competitive:
- **Good opportunity**: 0.1-0.5% profit on loan amount
- **Frequency**: 1-20 per day (depends on markets)
- **Competition**: High from other MEV bots

## Safety Features

The detector includes:
- ✅ Fee-adjusted profit calculations
- ✅ Confidence scoring
- ✅ Maximum loan limits (risk management)
- ✅ Liquidity depth validation
- ✅ Stale data filtering (10s window)

## Questions?

See the comprehensive guide: `flash_loan_integration_strategy.md`