use std::collections::HashMap;
use solana_sdk::pubkey::Pubkey;

use crate::streaming::event_parser::protocols::{
    raydium_clmm::{
        RaydiumClmmSwapV2Event,
        RaydiumClmmPoolStateAccountEvent,
        types::PoolState,
    },
    raydium_amm_v4::{
        RaydiumAmmV4SwapEvent,
        RaydiumAmmV4AmmInfoAccountEvent,
        types::AmmInfo,
    },
};

/// Represents a detected arbitrage opportunity
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    /// Pool to buy from (lower price)
    pub pool_a: Pubkey,
    /// Pool to sell to (higher price)
    pub pool_b: Pubkey,
    /// Protocol type for pool A
    pub pool_a_protocol: PoolProtocol,
    /// Protocol type for pool B
    pub pool_b_protocol: PoolProtocol,
    /// Base token mint (token0)
    pub base_token: Pubkey,
    /// Quote token mint (token1)
    pub quote_token: Pubkey,
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

/// Protocol type for tracking pool types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolProtocol {
    RaydiumClmm,
    RaydiumAmmV4,
}

/// Detects arbitrage opportunities from streaming events
pub struct OpportunityDetector {
    /// Cache of CLMM pool states indexed by pool pubkey
    clmm_pool_states: HashMap<Pubkey, PoolState>,
    /// Cache of AMMv4 pool states indexed by pool pubkey
    ammv4_pool_states: HashMap<Pubkey, AmmInfo>,
    /// Price feeds indexed by token pair (base, quote)
    price_feed: HashMap<TokenPair, Vec<PoolPrice>>,
    /// Minimum profit threshold (in lamports)
    min_profit_threshold: u64,
    /// Maximum loan amount (risk management)
    max_loan_amount: u64,
    /// Minimum liquidity threshold per pool (filters out low-liquidity pools)
    min_liquidity_threshold: u128,
    /// Minimum combined liquidity for both pools
    min_combined_liquidity: u128,
}

/// Represents a token pair for price tracking
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct TokenPair {
    token0: Pubkey,
    token1: Pubkey,
}

impl TokenPair {
    fn new(token0: Pubkey, token1: Pubkey) -> Self {
        // Normalize order: smaller pubkey first
        if token0 < token1 {
            Self { token0, token1 }
        } else {
            Self { token0: token1, token1: token0 }
        }
    }
}

#[derive(Debug, Clone)]
struct PoolPrice {
    pool: Pubkey,
    protocol: PoolProtocol,
    price: f64,      // token1/token0 price
    liquidity: u128,
    token0: Pubkey,
    token1: Pubkey,
    timestamp: i64,
}

impl OpportunityDetector {
    /// Create a new opportunity detector
    ///
    /// # Arguments
    /// * `min_profit_threshold` - Minimum profit in lamports (e.g., 0.001 SOL = 1_000_000)
    /// * `max_loan_amount` - Maximum loan amount in lamports (e.g., 100 SOL = 100_000_000_000)
    /// * `min_liquidity_threshold` - Minimum liquidity per pool (e.g., 10 SOL = 10_000_000_000)
    /// * `min_combined_liquidity` - Minimum combined liquidity for both pools (e.g., 50 SOL = 50_000_000_000)
    pub fn new(
        min_profit_threshold: u64,
        max_loan_amount: u64,
        min_liquidity_threshold: u128,
        min_combined_liquidity: u128,
    ) -> Self {
        Self {
            clmm_pool_states: HashMap::new(),
            ammv4_pool_states: HashMap::new(),
            price_feed: HashMap::new(),
            min_profit_threshold,
            max_loan_amount,
            min_liquidity_threshold,
            min_combined_liquidity,
        }
    }

    /// Create a new opportunity detector with default high-liquidity settings
    /// Targets pools with substantial liquidity to ensure executable trades
    pub fn new_high_liquidity() -> Self {
        Self::new(
            1_000_000,           // 0.001 SOL minimum profit
            100_000_000_000,     // 100 SOL maximum loan
            10_000_000_000,      // 10 SOL minimum per pool
            50_000_000_000,      // 50 SOL minimum combined
        )
    }

    /// Analyze CLMM swap event for arbitrage opportunities
    /// This is triggered by swap events but uses cached pool state prices
    pub fn analyze_clmm_swap_event(
        &mut self,
        event: &RaydiumClmmSwapV2Event
    ) -> Option<ArbitrageOpportunity> {
        // Get the pool state for this swap
        let pool_state = self.clmm_pool_states.get(&event.pool_state)?;

        // Create token pair
        let pair = TokenPair::new(pool_state.token_mint0, pool_state.token_mint1);

        // Look for cross-pool arbitrage on this token pair
        self.find_arbitrage_opportunity(&pair)
    }

    /// Analyze AMMv4 swap event for arbitrage opportunities
    pub fn analyze_ammv4_swap_event(
        &mut self,
        event: &RaydiumAmmV4SwapEvent
    ) -> Option<ArbitrageOpportunity> {
        // Get the pool state for this swap
        let pool_state = self.ammv4_pool_states.get(&event.amm)?;

        // Create token pair
        let pair = TokenPair::new(pool_state.coin_mint, pool_state.pc_mint);

        // Look for cross-pool arbitrage on this token pair
        self.find_arbitrage_opportunity(&pair)
    }

    /// Update CLMM pool state cache from account events and update price feed
    pub fn update_clmm_pool_state(&mut self, event: &RaydiumClmmPoolStateAccountEvent) {
        let pool_state = &event.pool_state;

        // Validate price is non-zero
        if pool_state.sqrt_price_x64 == 0 || pool_state.liquidity == 0 {
            return;
        }

        // Store pool state
        self.clmm_pool_states.insert(event.pubkey, pool_state.clone());

        // Calculate actual price from sqrt_price_x64
        let price = match self.calculate_clmm_price(pool_state) {
            Some(p) => p,
            None => return, // Invalid price, skip this update
        };

        // Update price feed
        let pair = TokenPair::new(pool_state.token_mint0, pool_state.token_mint1);
        self.update_price_feed(
            pair,
            event.pubkey,
            price,
            pool_state.liquidity,
            pool_state.token_mint0,
            pool_state.token_mint1,
            PoolProtocol::RaydiumClmm,
        );
    }

    /// Update AMMv4 pool state cache from account events and update price feed
    pub fn update_ammv4_pool_state(&mut self, event: &RaydiumAmmV4AmmInfoAccountEvent) {
        let pool_state = &event.amm_info;

        // Store pool state
        self.ammv4_pool_states.insert(event.pubkey, pool_state.clone());

        // Calculate price from cumulative swap amounts
        let price = match self.calculate_ammv4_price(pool_state) {
            Some(p) => p,
            None => return, // Invalid price, skip this update
        };

        // Estimate liquidity from cumulative swap volumes
        let liquidity = self.estimate_ammv4_liquidity(pool_state);

        // Update price feed
        let pair = TokenPair::new(pool_state.coin_mint, pool_state.pc_mint);
        self.update_price_feed(
            pair,
            event.pubkey,
            price,
            liquidity,
            pool_state.coin_mint,
            pool_state.pc_mint,
            PoolProtocol::RaydiumAmmV4,
        );
    }

    /// Calculate price (token1/token0) from CLMM pool state
    fn calculate_clmm_price(&self, pool: &PoolState) -> Option<f64> {
        if pool.sqrt_price_x64 == 0 {
            return None;
        }

        // Convert sqrt_price_x64 to actual price
        // sqrt_price_x64 = sqrt(price) * 2^64
        // price = (sqrt_price_x64 / 2^64)^2
        let sqrt_price = pool.sqrt_price_x64 as f64 / (1u128 << 64) as f64;
        let price = sqrt_price * sqrt_price;

        if !price.is_finite() || price <= 0.0 {
            return None;
        }

        Some(price)
    }

    /// Calculate price (pc/coin) from AMMv4 pool state
    /// Uses cumulative swap amounts to estimate current price
    fn calculate_ammv4_price(&self, pool: &AmmInfo) -> Option<f64> {
        let swap_coin_in = pool.out_put.swap_coin_in_amount;
        let swap_pc_out = pool.out_put.swap_pc_out_amount;
        let swap_pc_in = pool.out_put.swap_pc_in_amount;
        let swap_coin_out = pool.out_put.swap_coin_out_amount;

        // Calculate average price from both directions
        // Price = pc/coin
        if swap_coin_in == 0 && swap_pc_in == 0 {
            return None;
        }

        let mut price_sum = 0.0;
        let mut price_count = 0;

        if swap_coin_in > 0 {
            let price_from_coin_in = swap_pc_out as f64 / swap_coin_in as f64;
            if price_from_coin_in.is_finite() && price_from_coin_in > 0.0 {
                price_sum += price_from_coin_in;
                price_count += 1;
            }
        }

        if swap_pc_in > 0 {
            let price_from_pc_in = swap_pc_in as f64 / swap_coin_out as f64;
            if price_from_pc_in.is_finite() && price_from_pc_in > 0.0 {
                price_sum += price_from_pc_in;
                price_count += 1;
            }
        }

        if price_count == 0 {
            return None;
        }

        let avg_price = price_sum / price_count as f64;

        if !avg_price.is_finite() || avg_price <= 0.0 {
            return None;
        }

        Some(avg_price)
    }

    /// Estimate liquidity for AMMv4 pool based on cumulative swap volumes
    /// This uses multiple heuristics to provide a better estimate
    fn estimate_ammv4_liquidity(&self, pool: &AmmInfo) -> u128 {
        // Strategy 1: Use cumulative swap volumes
        let coin_volume = pool.out_put.swap_coin_in_amount + pool.out_put.swap_coin_out_amount;
        let pc_volume = pool.out_put.swap_pc_in_amount + pool.out_put.swap_pc_out_amount;

        // Strategy 2: Use LP token amount as a proxy
        // Higher LP amounts typically indicate more liquidity
        let lp_based_estimate = (pool.lp_amount as u128).saturating_mul(10);

        // Strategy 3: Use the smaller of coin/pc volume * 10
        // This assumes pools maintain some balance and ~10% volume/liquidity ratio
        let volume_based = coin_volume.min(pc_volume).saturating_mul(10);

        // Take the maximum of all estimates for a more optimistic but grounded view
        let estimate = coin_volume
            .max(pc_volume)
            .max(lp_based_estimate)
            .max(volume_based)
            / 2; // Divide by 2 to be conservative

        // Ensure minimum threshold
        estimate.max(1_000_000)
    }

    /// Find arbitrage opportunities across pools for a token pair
    fn find_arbitrage_opportunity(
        &self,
        pair: &TokenPair
    ) -> Option<ArbitrageOpportunity> {
        let prices = self.price_feed.get(pair)?;

        // Need at least 2 pools with different prices
        if prices.len() < 2 {
            return None;
        }

        // Filter out low-liquidity pools immediately
        let high_liquidity_prices: Vec<_> = prices.iter()
            .filter(|p| p.liquidity >= self.min_liquidity_threshold)
            .collect();

        if high_liquidity_prices.len() < 2 {
            return None;
        }

        // Find lowest and highest price among high-liquidity pools
        let (min_price_pool, max_price_pool) = self.find_price_spread(&high_liquidity_prices)?;

        // Check combined liquidity requirement
        let combined_liquidity = min_price_pool.liquidity + max_price_pool.liquidity;
        if combined_liquidity < self.min_combined_liquidity {
            return None;
        }

        // Must have meaningful price difference
        let price_diff = max_price_pool.price - min_price_pool.price;
        if price_diff <= 0.0 || !price_diff.is_finite() {
            return None;
        }

        // Calculate potential profit
        let profit = self.calculate_profit(
            &min_price_pool,
            &max_price_pool
        )?;

        // Filter by minimum profit threshold
        if profit.expected_profit < self.min_profit_threshold {
            return None;
        }

        Some(profit)
    }

    /// Calculate expected profit considering all fees
    ///
    /// Arbitrage flow (borrowing quote token / token1):
    /// 1. Borrow L lamports of token1 (flash loan fee: 0.09%)
    /// 2. Buy token0 at Pool A (low price): spend L token1 → get L/price_a token0 (swap fee: 0.25%)
    /// 3. Sell token0 at Pool B (high price): sell token0 → get token1 (swap fee: 0.25%)
    /// 4. Net received: L * (1 - 0.0025)² * (price_b / price_a)
    /// 5. Must repay: L * (1 + 0.0009)
    /// 6. Profit: received - repayment
    fn calculate_profit(
        &self,
        buy_pool: &PoolPrice,
        sell_pool: &PoolPrice
    ) -> Option<ArbitrageOpportunity> {
        // Validate prices
        if !buy_pool.price.is_finite() || !sell_pool.price.is_finite() {
            return None;
        }
        if buy_pool.price <= 0.0 || sell_pool.price <= 0.0 {
            return None;
        }

        // Price spread
        let price_diff = sell_pool.price - buy_pool.price;
        if price_diff <= 0.0 {
            return None; // No profit possible
        }

        let price_spread_pct = price_diff / buy_pool.price;

        // Must have at least 1% spread to be worth it after fees
        if price_spread_pct < 0.01 {
            return None;
        }

        // Estimate optimal loan amount based on liquidity
        let optimal_loan = self.calculate_optimal_loan_size(buy_pool, sell_pool);

        if optimal_loan == 0 {
            return None;
        }

        // Fee constants
        const FLASH_LOAN_FEE_RATE: f64 = 0.0009; // 0.09% Solend
        const SWAP_FEE_RATE: f64 = 0.0025;       // 0.25% per swap

        // Calculate net amount after fees
        // After two swaps: (1 - 0.0025)² = 0.99500625
        let swap_fee_multiplier = (1.0 - SWAP_FEE_RATE) * (1.0 - SWAP_FEE_RATE);

        // Price multiplier for arbitrage
        let price_multiplier = sell_pool.price / buy_pool.price;

        // Net token1 received after both swaps
        let net_received = optimal_loan as f64 * swap_fee_multiplier * price_multiplier;

        // Amount to repay (loan + flash loan fee)
        let repayment = optimal_loan as f64 * (1.0 + FLASH_LOAN_FEE_RATE);

        // Check for valid calculation
        if !net_received.is_finite() || !repayment.is_finite() {
            return None;
        }

        // Net profit
        if net_received <= repayment {
            return None; // Not profitable after fees
        }

        let expected_profit_f64 = net_received - repayment;
        let expected_profit = expected_profit_f64 as u64;

        // Confidence score based on liquidity and spread
        let confidence = self.calculate_confidence(buy_pool, sell_pool, price_spread_pct);

        Some(ArbitrageOpportunity {
            pool_a: buy_pool.pool,
            pool_b: sell_pool.pool,
            pool_a_protocol: buy_pool.protocol,
            pool_b_protocol: sell_pool.protocol,
            base_token: buy_pool.token0,
            quote_token: buy_pool.token1,
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
        // Conservative approach: use a percentage of minimum liquidity
        // Lower percentages for lower liquidity to minimize slippage
        let min_liquidity = buy_pool.liquidity.min(sell_pool.liquidity);

        let percentage = if min_liquidity > 100_000_000_000 {  // >100 SOL
            15  // Can use up to 15% for very high liquidity
        } else if min_liquidity > 50_000_000_000 {  // >50 SOL
            10  // 10% for good liquidity
        } else if min_liquidity > 20_000_000_000 {  // >20 SOL
            5   // 5% for moderate liquidity
        } else {
            2   // Only 2% for lower liquidity
        };

        let loan = (min_liquidity / percentage as u128) as u64;

        // Ensure loan is reasonable and within limits
        loan.min(self.max_loan_amount).max(100_000) // At least 0.0001 SOL
    }

    fn calculate_confidence(
        &self,
        buy_pool: &PoolPrice,
        sell_pool: &PoolPrice,
        spread_pct: f64
    ) -> u8 {
        let mut confidence = 0u8;

        // Liquidity scoring (up to 50 points) - heavily weighted
        let min_liquidity = buy_pool.liquidity.min(sell_pool.liquidity);
        let combined_liquidity = buy_pool.liquidity + sell_pool.liquidity;

        if min_liquidity > 100_000_000_000 {  // >100 SOL each
            confidence += 30;
        } else if min_liquidity > 50_000_000_000 {  // >50 SOL each
            confidence += 25;
        } else if min_liquidity > 10_000_000_000 {  // >10 SOL each
            confidence += 15;
        } else if min_liquidity > 5_000_000_000 {   // >5 SOL each
            confidence += 10;
        } else if min_liquidity > 1_000_000_000 {   // >1 SOL each
            confidence += 5;
        }

        // Combined liquidity bonus
        if combined_liquidity > 200_000_000_000 {  // >200 SOL combined
            confidence += 20;
        } else if combined_liquidity > 100_000_000_000 {  // >100 SOL combined
            confidence += 15;
        } else if combined_liquidity > 50_000_000_000 {   // >50 SOL combined
            confidence += 10;
        }

        // Price spread scoring (up to 20 points)
        if spread_pct > 0.05 {      // >5%
            confidence += 20;
        } else if spread_pct > 0.03 {  // >3%
            confidence += 15;
        } else if spread_pct > 0.01 {  // >1%
            confidence += 10;
        } else if spread_pct > 0.005 { // >0.5%
            confidence += 5;
        }

        // Data freshness (up to 10 points)
        let now = chrono::Utc::now().timestamp();
        let buy_age = now - buy_pool.timestamp;
        let sell_age = now - sell_pool.timestamp;
        let max_age = buy_age.max(sell_age);

        if max_age < 2 {
            confidence += 10;
        } else if max_age < 5 {
            confidence += 7;
        } else if max_age < 10 {
            confidence += 5;
        } else if max_age < 30 {
            confidence += 2;
        }

        confidence.min(100)
    }

    fn find_price_spread(&self, prices: &[&PoolPrice]) -> Option<(PoolPrice, PoolPrice)> {
        if prices.is_empty() {
            return None;
        }

        let mut min = prices[0].clone();
        let mut max = prices[0].clone();

        for price in prices.iter().skip(1) {
            if price.price < min.price {
                min = (*price).clone();
            }
            if price.price > max.price {
                max = (*price).clone();
            }
        }

        Some((min, max))
    }

    fn update_price_feed(
        &mut self,
        pair: TokenPair,
        pool: Pubkey,
        price: f64,
        liquidity: u128,
        token0: Pubkey,
        token1: Pubkey,
        protocol: PoolProtocol,
    ) {
        let pool_price = PoolPrice {
            pool,
            protocol,
            price,
            liquidity,
            token0,
            token1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let prices = self.price_feed.entry(pair.clone()).or_insert_with(Vec::new);

        // Update existing entry or add new one
        if let Some(existing) = prices.iter_mut().find(|p| p.pool == pool) {
            *existing = pool_price;
        } else {
            prices.push(pool_price);
        }

        // Keep only recent prices (last 30 seconds) and remove stale data
        let now = chrono::Utc::now().timestamp();
        prices.retain(|p| now - p.timestamp < 30);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opportunity_detector_creation() {
        let detector = OpportunityDetector::new(1_000_000, 100_000_000_000, 10_000_000_000, 50_000_000_000);
        assert_eq!(detector.min_profit_threshold, 1_000_000);
        assert_eq!(detector.max_loan_amount, 100_000_000_000);
        assert_eq!(detector.min_liquidity_threshold, 10_000_000_000);
        assert_eq!(detector.min_combined_liquidity, 50_000_000_000);
    }

    #[test]
    fn test_high_liquidity_detector() {
        let detector = OpportunityDetector::new_high_liquidity();
        assert_eq!(detector.min_profit_threshold, 1_000_000);
        assert_eq!(detector.max_loan_amount, 100_000_000_000);
        assert_eq!(detector.min_liquidity_threshold, 10_000_000_000);
        assert_eq!(detector.min_combined_liquidity, 50_000_000_000);
    }

    #[test]
    fn test_optimal_loan_size_calculation() {
        let detector = OpportunityDetector::new(1_000_000, 100_000_000_000, 1_000_000_000, 5_000_000_000);

        let token0 = Pubkey::new_unique();
        let token1 = Pubkey::new_unique();

        let buy_pool = PoolPrice {
            pool: Pubkey::new_unique(),
            protocol: PoolProtocol::RaydiumClmm,
            price: 1.0,
            liquidity: 100_000_000_000,  // 100 SOL
            token0,
            token1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let sell_pool = PoolPrice {
            pool: Pubkey::new_unique(),
            protocol: PoolProtocol::RaydiumAmmV4,
            price: 1.02,
            liquidity: 200_000_000_000,  // 200 SOL
            token0,
            token1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let loan_size = detector.calculate_optimal_loan_size(&buy_pool, &sell_pool);

        // Should be ~15% of minimum liquidity for high liquidity pools
        // 100 SOL * 0.15 = 15 SOL
        assert!(loan_size >= 6_000_000_000 && loan_size <= 7_000_000_000);
    }
}