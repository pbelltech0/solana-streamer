# Flash Loan Arbitrage - Quick Start Guide

## Running the Opportunity Detector (Test Mode)

To monitor for arbitrage opportunities WITHOUT executing flash loans:

```bash
cargo run --example arbitrage_opportunity_detector
```

### What it does:
- ✅ Connects to Solana via Yellowstone gRPC
- ✅ Monitors Raydium CLMM swap events in real-time
- ✅ Tracks pool state changes for liquidity data
- ✅ Detects price discrepancies between pools
- ✅ Calculates profitability (including all fees)
- ✅ Logs opportunities to console and `logs/arbitrage_opportunities.log`
- ❌ **Does NOT execute flash loans** (safe for testing)

### Example Output

When an opportunity is detected:
```
🎯 ARBITRAGE OPPORTUNITY DETECTED!
   Pool A: 7Z4nN5QsYxHYP...
   Pool B: 9Kf4P2mGxVwT...
   Token: EPjFWdd5AufqSS...
   Price A: $1.002345
   Price B: $1.015678
   Price Spread: 1.33%
   Expected Profit: 2450000 lamports (0.002450 SOL)
   Loan Amount: 50000000000 lamports (50.000 SOL)
   Confidence: 85%
   Timestamp: 1711234567

💭 In production mode, this would trigger a flash loan execution
   Status: TEST MODE - no action taken
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