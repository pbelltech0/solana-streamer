# Quick Start Guide

## 🚀 Run the Arbitrage Detector (5 minutes)

### Step 1: Build the Project
```bash
cd /Users/pblaze/Documents/Solana/solana-streamer
cargo build --release
```

Expected: Compiles successfully with some warnings (normal)

### Step 2: Run the Detector
```bash
cargo run --example focused_liquidity_arbitrage
```

### Step 3: Watch for Opportunities

You should see output like:

```
╔═══════════════════════════════════════════════════════════╗
║   FOCUSED LIQUIDITY-AWARE ARBITRAGE DETECTOR             ║
╚═══════════════════════════════════════════════════════════╝

📊 Configuration:
  • Monitored Pairs: 2
    - SOL/USDC (trade size: 0.10-10.00 SOL)
    - SOL/USDT (trade size: 0.10-10.00 SOL)
  • Min Net Profit: 0.30%
  • Min Execution Prob: 40%
  • Min EV Score: 15.0

🔌 Connecting to Yellowstone gRPC...
✓ Connected successfully

🚀 Starting event subscription...

🔄 Pool Update: Raydium CLMM CAMMCzo... (liquidity: 12345678900)
💱 Raydium CLMM Swap: So11... -> EPjF... (1000000 -> 950000)

╔═══════════════════════════════════════════════════════════╗
║  ARBITRAGE SCAN COMPLETE - 3 opportunities found
╚═══════════════════════════════════════════════════════════╝

╔═══════════════════════════════════════════════════════════╗
║  Opportunity #1 - 🟢 EXECUTE: High confidence, EV=42.3...
╠═══════════════════════════════════════════════════════════╣
...
```

---

## 🎯 Customize for Your Needs

### Change Token Pairs

Edit `examples/focused_liquidity_arbitrage.rs` around line 60:

```rust
impl ArbitrageConfig {
    fn default() -> Self {
        // Define your tokens
        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let bonk = Pubkey::from_str("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263").unwrap();

        Self {
            monitored_pairs: vec![
                MonitoredPair {
                    name: "SOL/USDC".to_string(),
                    token_a: sol,
                    token_b: usdc,
                    min_trade_size: 100_000_000,      // 0.1 SOL
                    max_trade_size: 10_000_000_000,   // 10 SOL
                    target_pools: vec![],
                },
                MonitoredPair {
                    name: "BONK/SOL".to_string(),
                    token_a: bonk,
                    token_b: sol,
                    min_trade_size: 1_000_000_000,    // Adjust for BONK decimals
                    max_trade_size: 100_000_000_000,
                    target_pools: vec![],
                },
            ],
            min_net_profit_pct: 0.3,
            min_execution_prob: 0.4,
            min_ev_score: 15.0,
        }
    }
}
```

### Adjust Risk Parameters

**Conservative** (safer, fewer opportunities):
```rust
min_net_profit_pct: 0.5,      // 0.5% min profit
min_execution_prob: 0.6,       // 60% min probability
min_ev_score: 20.0,            // Higher EV threshold
```

**Balanced** (recommended):
```rust
min_net_profit_pct: 0.3,      // 0.3% min profit
min_execution_prob: 0.4,       // 40% min probability
min_ev_score: 15.0,            // Default
```

**Aggressive** (more opportunities, higher risk):
```rust
min_net_profit_pct: 0.2,      // 0.2% min profit
min_execution_prob: 0.3,       // 30% min probability
min_ev_score: 10.0,            // Lower threshold
```

---

## 📊 Understanding the Output

### Confidence Levels

- **🟢 VeryHigh**: >80% execution prob, >1% net profit → **EXECUTE**
- **🟡 High**: >60% execution prob, >0.5% net profit → **CONSIDER**
- **🟠 Medium**: >40% execution prob, >0.3% net profit → **MONITOR**
- **🔴 Low**: >20% execution prob → **SKIP**
- **⛔ VeryLow**: <20% execution prob → **AVOID**

### Key Metrics

- **EV Score**: Expected value score (0-100). Higher is better.
  - `>40`: Excellent opportunity
  - `20-40`: Good opportunity
  - `10-20`: Marginal opportunity
  - `<10`: Skip

- **Net Profit %**: Profit after all fees and gas
  - `>1%`: Excellent
  - `0.5-1%`: Good
  - `0.3-0.5%`: Acceptable
  - `<0.3%`: Too risky

- **Combined Execution Prob**: Likelihood both swaps succeed
  - `>70%`: Very likely
  - `50-70%`: Likely
  - `30-50%`: Moderate
  - `<30%`: Risky

---

## 🔍 What to Look For

### Good Signs ✅
- EV scores consistently >15
- Finding 2-5 high-confidence opportunities per hour
- Net profits >0.3% after fees
- Execution probabilities >40%

### Warning Signs ⚠️
- No opportunities for >30 minutes (check connection)
- All opportunities have EV <10 (adjust parameters)
- Execution probabilities all <30% (pools too small)
- Net profits all negative (fees too high)

---

## 🛠️ Troubleshooting

### "No opportunities found"

**Possible causes**:
1. Parameters too strict → Loosen `min_net_profit_pct` or `min_ev_score`
2. Not enough market volatility → Normal, wait for activity
3. Token pairs have no liquidity → Choose different pairs

**Fix**: Lower thresholds temporarily to see if opportunities exist

### "Connection error"

**Possible causes**:
1. Yellowstone gRPC endpoint down → Try different endpoint
2. Network issues → Check internet connection
3. Rate limiting → Wait 30 seconds, retry

**Fix**: Edit example and change RPC endpoint

### "All opportunities have negative profit"

**Possible causes**:
1. Gas estimates too high → Expected, adjust in production
2. Fees eating all profit → Normal for small opportunities
3. Price impact too large → Reduce `max_trade_size`

**Fix**: This is informational - it's correctly rejecting unprofitable trades

---

## 📈 Next Steps

### Week 1: Paper Trading
1. ✅ Run detector for 1 week
2. ✅ Track hypothetical performance
3. ✅ Log top 10 opportunities per day
4. ✅ Calculate "would-be" profits
5. ✅ Tune parameters based on results

**Goal**: Validate detection accuracy

### Week 2: DEX SDK Integration (Optional)
If paper trading shows consistent profitability:

1. Add Raydium SDK dependencies
2. Implement swap instruction builders
3. Test on devnet first
4. Deploy to mainnet with 0.1 SOL

**Goal**: Enable actual execution

### Week 3-4: Production (Optional)
1. Add Jito bundle submission
2. Implement monitoring/alerts
3. Add risk management
4. Scale up capital gradually

**Goal**: Full production system

---

## 💡 Tips for Success

1. **Start Small**: Monitor first, execute later
2. **Be Patient**: Good opportunities come in waves
3. **Track Everything**: Log all opportunities, analyze patterns
4. **Iterate**: Adjust parameters based on actual results
5. **Risk Management**: Never risk more than you can afford to lose

---

## 📞 Need Help?

Check the documentation:
- **Architecture**: `ARBITRAGE_REFACTORING.md`
- **Full Guide**: `IMPLEMENTATION_GUIDE.md`
- **Status**: `FINAL_STATUS.md`

---

## 🎉 You're Ready!

Run this now:
```bash
cargo run --example focused_liquidity_arbitrage
```

Watch for opportunities, and good luck! 🚀
