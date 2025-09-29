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

/// Detects arbitrage opportunities from streaming events
pub struct OpportunityDetector {
    /// Cache of pool states indexed by pool pubkey
    pool_states: HashMap<Pubkey, PoolState>,
    /// Price feeds indexed by token pair (base, quote)
    price_feed: HashMap<TokenPair, Vec<PoolPrice>>,
    /// Minimum profit threshold (in lamports)
    min_profit_threshold: u64,
    /// Maximum loan amount (risk management)
    max_loan_amount: u64,
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
    price: f64,      // token1/token0 price
    liquidity: u128,
    token0: Pubkey,
    token1: Pubkey,
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
    /// This is triggered by swap events but uses cached pool state prices
    pub fn analyze_swap_event(
        &mut self,
        event: &RaydiumClmmSwapV2Event
    ) -> Option<ArbitrageOpportunity> {
        // Get the pool state for this swap
        let pool_state = self.pool_states.get(&event.pool_state)?;

        // Create token pair
        let pair = TokenPair::new(pool_state.token_mint0, pool_state.token_mint1);

        // Look for cross-pool arbitrage on this token pair
        self.find_arbitrage_opportunity(&pair)
    }

    /// Update pool state cache from account events and update price feed
    pub fn update_pool_state(&mut self, event: &RaydiumClmmPoolStateAccountEvent) {
        let pool_state = &event.pool_state;

        // Validate price is non-zero
        if pool_state.sqrt_price_x64 == 0 || pool_state.liquidity == 0 {
            return;
        }

        // Store pool state
        self.pool_states.insert(event.pubkey, pool_state.clone());

        // Calculate actual price from sqrt_price_x64
        let price = match self.calculate_price_from_pool_state(pool_state) {
            Some(p) => p,
            None => return, // Invalid price, skip this update
        };

        // Update price feed
        let pair = TokenPair::new(pool_state.token_mint0, pool_state.token_mint1);
        self.update_price_feed(pair, event.pubkey, price, pool_state);
    }

    /// Calculate price (token1/token0) from pool state
    fn calculate_price_from_pool_state(&self, pool: &PoolState) -> Option<f64> {
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

        // Find lowest and highest price
        let (min_price_pool, max_price_pool) = self.find_price_spread(prices)?;

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

        // Calculate costs (in basis points)
        let flash_loan_fee = (optimal_loan as u128 * 9 / 10000) as u64; // 0.09% Solend
        let swap_fee_a = (optimal_loan as u128 * 25 / 10000) as u64;    // 0.25% swap
        let swap_fee_b = (optimal_loan as u128 * 25 / 10000) as u64;    // 0.25% swap
        let total_fees = flash_loan_fee.saturating_add(swap_fee_a).saturating_add(swap_fee_b);

        // Calculate gross profit (be careful with overflow)
        let gross_profit_f64 = optimal_loan as f64 * price_spread_pct;
        if !gross_profit_f64.is_finite() || gross_profit_f64 < 0.0 {
            return None;
        }

        let gross_profit = gross_profit_f64 as u64;

        // Net profit (check for underflow)
        if gross_profit <= total_fees {
            return None; // Not profitable after fees
        }

        let expected_profit = gross_profit - total_fees;

        // Confidence score based on liquidity and spread
        let confidence = self.calculate_confidence(buy_pool, sell_pool, price_spread_pct);

        Some(ArbitrageOpportunity {
            pool_a: buy_pool.pool,
            pool_b: sell_pool.pool,
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

    fn update_price_feed(
        &mut self,
        pair: TokenPair,
        pool: Pubkey,
        price: f64,
        pool_state: &PoolState
    ) {
        let pool_price = PoolPrice {
            pool,
            price,
            liquidity: pool_state.liquidity,
            token0: pool_state.token_mint0,
            token1: pool_state.token_mint1,
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
        let detector = OpportunityDetector::new(1_000_000, 100_000_000_000);
        assert_eq!(detector.min_profit_threshold, 1_000_000);
        assert_eq!(detector.max_loan_amount, 100_000_000_000);
    }

    #[test]
    fn test_optimal_loan_size_calculation() {
        let detector = OpportunityDetector::new(1_000_000, 100_000_000_000);

        let token0 = Pubkey::new_unique();
        let token1 = Pubkey::new_unique();

        let buy_pool = PoolPrice {
            pool: Pubkey::new_unique(),
            price: 1.0,
            liquidity: 1_000_000_000,
            token0,
            token1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let sell_pool = PoolPrice {
            pool: Pubkey::new_unique(),
            price: 1.02,
            liquidity: 2_000_000_000,
            token0,
            token1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        let loan_size = detector.calculate_optimal_loan_size(&buy_pool, &sell_pool);

        // Should be 10% of minimum liquidity (1B)
        assert_eq!(loan_size, 100_000_000);
    }
}