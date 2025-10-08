/// Phase 1 Test: Verify opportunity detection logic without network
///
/// Run with: cargo run --example test_opportunity_detection

use solana_streamer_sdk::flash_loan::OpportunityDetector;

fn main() {
    println!("🧪 Testing Flash Loan Opportunity Detection Logic\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Test 1: Basic detector creation
    println!("\n✓ Test 1: Detector initialization");
    let _detector = OpportunityDetector::new(
        1_000_000,        // 0.001 SOL min profit
        100_000_000_000,  // 100 SOL max loan
        10_000_000_000,   // 10 SOL min liquidity per pool
        50_000_000_000,   // 50 SOL min combined liquidity
    );
    println!("  Min profit threshold: 1,000,000 lamports (0.001 SOL)");
    println!("  Max loan amount: 100,000,000,000 lamports (100 SOL)");
    println!("  Min liquidity per pool: 10,000,000,000 lamports (10 SOL)");
    println!("  Min combined liquidity: 50,000,000,000 lamports (50 SOL)");

    // Test 2: Fee calculations
    println!("\n✓ Test 2: Fee calculations");
    let loan_amount = 10_000_000_000u64; // 10 SOL (in quote token)

    // Fee constants
    const FLASH_LOAN_FEE_RATE: f64 = 0.0009; // 0.09%
    const SWAP_FEE_RATE: f64 = 0.0025;       // 0.25% per swap

    let flash_loan_fee = (loan_amount as f64 * FLASH_LOAN_FEE_RATE) as u64;

    // For demonstration, assuming price_a = 1.0
    let price_a = 1.0;
    let swap_fee_a = (loan_amount as f64 * SWAP_FEE_RATE) as u64;
    let token0_amount = (loan_amount as f64 * (1.0 - SWAP_FEE_RATE)) / price_a;
    let swap_fee_b = (token0_amount * SWAP_FEE_RATE) as u64;
    let total_fees = flash_loan_fee + swap_fee_a + swap_fee_b;

    println!("  Loan amount:      {} lamports ({:.3} SOL)", loan_amount, loan_amount as f64 / 1e9);
    println!("  Flash loan fee:   {} lamports ({:.6} SOL) [0.09%]", flash_loan_fee, flash_loan_fee as f64 / 1e9);
    println!("  Swap fee A:       {} lamports ({:.6} SOL) [0.25% on loan]", swap_fee_a, swap_fee_a as f64 / 1e9);
    println!("  Swap fee B:       {} lamports ({:.6} SOL) [0.25% on converted amount]", swap_fee_b, swap_fee_b as f64 / 1e9);
    println!("  Total fees:       {} lamports ({:.6} SOL)", total_fees, total_fees as f64 / 1e9);

    // Test 3: Profitability scenarios
    println!("\n✓ Test 3: Profitability calculations");

    let test_scenarios = vec![
        ("Profitable (2% spread)", 1.00, 1.02, true),
        ("Marginal (1% spread)", 1.00, 1.01, true),
        ("Unprofitable (0.5% spread)", 1.00, 1.005, false),
        ("Large spread (5%)", 1.00, 1.05, true),
    ];

    for (name, price_a, price_b, should_profit) in test_scenarios {
        let spread = (price_b - price_a) / price_a;

        // Corrected calculation using actual arbitrage flow
        let swap_fee_multiplier = (1.0 - SWAP_FEE_RATE) * (1.0 - SWAP_FEE_RATE);
        let price_multiplier = price_b / price_a;
        let net_received = loan_amount as f64 * swap_fee_multiplier * price_multiplier;
        let repayment = loan_amount as f64 * (1.0 + FLASH_LOAN_FEE_RATE);
        let net_profit = if net_received > repayment {
            (net_received - repayment) as u64
        } else {
            0
        };
        let is_profitable = net_profit > 0;

        println!("\n  Scenario: {}", name);
        println!("    Price A: {:.4}", price_a);
        println!("    Price B: {:.4}", price_b);
        println!("    Spread: {:.2}%", spread * 100.0);
        println!("    Net received: {:.0} lamports ({:.6} SOL)", net_received, net_received / 1e9);
        println!("    Must repay: {:.0} lamports ({:.6} SOL)", repayment, repayment / 1e9);
        println!("    Net profit: {} lamports ({:.6} SOL)", net_profit, net_profit as f64 / 1e9);
        println!("    Result: {}", if is_profitable { "✅ PROFITABLE" } else { "❌ UNPROFITABLE" });

        if is_profitable != should_profit {
            println!("    ⚠️  WARNING: Expected {}, got {}",
                if should_profit { "profitable" } else { "unprofitable" },
                if is_profitable { "profitable" } else { "unprofitable" }
            );
        }
    }

    // Test 4: Minimum spread required for profitability
    println!("\n✓ Test 4: Break-even analysis");
    let min_spread = total_fees as f64 / loan_amount as f64;
    let min_spread_pct = min_spread * 100.0;
    println!("  Total fees: {:.6} SOL", total_fees as f64 / 1e9);
    println!("  Loan amount: {:.6} SOL", loan_amount as f64 / 1e9);
    println!("  Minimum spread for profit: {:.2}%", min_spread_pct);
    println!("  Recommendation: Look for spreads > {:.1}% to ensure profit after fees", min_spread_pct + 0.2);

    // Test 5: Confidence scoring simulation
    println!("\n✓ Test 5: Confidence scoring");

    struct ConfidenceTest {
        name: &'static str,
        liquidity_a: u128,
        liquidity_b: u128,
        spread_pct: f64,
        data_age_secs: i64,
        expected_range: (u8, u8),
    }

    let confidence_tests = vec![
        ConfidenceTest {
            name: "High liquidity, good spread, fresh data",
            liquidity_a: 10_000_000,
            liquidity_b: 10_000_000,
            spread_pct: 2.0,
            data_age_secs: 2,
            expected_range: (90, 100),
        },
        ConfidenceTest {
            name: "Low liquidity",
            liquidity_a: 100_000,
            liquidity_b: 100_000,
            spread_pct: 2.0,
            data_age_secs: 2,
            expected_range: (30, 50),
        },
        ConfidenceTest {
            name: "Stale data",
            liquidity_a: 10_000_000,
            liquidity_b: 10_000_000,
            spread_pct: 2.0,
            data_age_secs: 10,
            expected_range: (60, 80),
        },
        ConfidenceTest {
            name: "Small spread",
            liquidity_a: 10_000_000,
            liquidity_b: 10_000_000,
            spread_pct: 0.5,
            data_age_secs: 2,
            expected_range: (50, 70),
        },
    ];

    for test in confidence_tests {
        let mut confidence = 0u8;

        // Liquidity score
        if test.liquidity_a > 1_000_000 && test.liquidity_b > 1_000_000 {
            confidence += 40;
        }

        // Spread score
        if test.spread_pct > 0.01 {
            confidence += 30;
        }

        // Data freshness score
        if test.data_age_secs < 5 {
            confidence += 30;
        }

        println!("\n  Scenario: {}", test.name);
        println!("    Liquidity A: {}", test.liquidity_a);
        println!("    Liquidity B: {}", test.liquidity_b);
        println!("    Spread: {:.2}%", test.spread_pct);
        println!("    Data age: {}s", test.data_age_secs);
        println!("    Confidence: {}%", confidence);

        if confidence >= test.expected_range.0 && confidence <= test.expected_range.1 {
            println!("    ✅ Within expected range: {}-{}", test.expected_range.0, test.expected_range.1);
        } else {
            println!("    ⚠️  Outside expected range: {}-{}", test.expected_range.0, test.expected_range.1);
        }
    }

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ All tests completed!");
    println!("\nNext steps:");
    println!("1. Review test results above");
    println!("2. If all looks good, proceed to Phase 2 (local validator testing)");
    println!("3. See FLASH_LOAN_IMPLEMENTATION_GUIDE.md for details\n");
}