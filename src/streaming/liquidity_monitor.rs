/// Liquidity monitoring system for arbitrage detection
/// Tracks pool states, liquidity depth, and price impact for accurate arbitrage execution probability

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents the current state of a liquidity pool
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolState {
    pub pool_address: Pubkey,
    pub dex_type: DexType,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub liquidity: u128,
    pub sqrt_price_x64: Option<u128>, // For CLMM pools
    pub tick_current: Option<i32>,    // For CLMM pools
    pub active_bin_id: Option<i32>,   // For Meteora DLMM pools
    pub bin_step: Option<u16>,        // For Meteora DLMM pools
    pub total_fee_bps: u16,
    pub last_updated: u64,
    pub last_trade_timestamp: Option<u64>,
    pub volume_24h: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DexType {
    RaydiumAmmV4,
    RaydiumClmm,
    RaydiumCpmm,
    OrcaWhirlpool,
    MeteoraDlmm,
}

impl DexType {
    pub fn typical_fee_bps(&self) -> u16 {
        match self {
            DexType::RaydiumAmmV4 => 25,    // 0.25%
            DexType::RaydiumClmm => 25,     // Varies, typically 0.25%
            DexType::RaydiumCpmm => 25,     // 0.25%
            DexType::OrcaWhirlpool => 30,   // Varies, often 0.30%
            DexType::MeteoraDlmm => 20,     // Variable, starting around 0.20%
        }
    }
}

impl PoolState {
    /// Calculate price impact for a given trade size
    /// Returns (output_amount, price_impact_bps)
    pub fn calculate_price_impact(&self, input_amount: u64, is_a_to_b: bool) -> (u64, u16) {
        let (reserve_in, reserve_out) = if is_a_to_b {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };

        // Handle CLMM and DLMM pools differently
        match self.dex_type {
            DexType::RaydiumClmm | DexType::OrcaWhirlpool => {
                self.calculate_clmm_impact(input_amount, is_a_to_b)
            }
            DexType::MeteoraDlmm => {
                self.calculate_dlmm_impact(input_amount, is_a_to_b)
            }
            _ => {
                // Constant product AMM formula: x * y = k
                self.calculate_cpmm_impact(input_amount, reserve_in, reserve_out)
            }
        }
    }

    fn calculate_cpmm_impact(&self, input_amount: u64, reserve_in: u64, reserve_out: u64) -> (u64, u16) {
        if reserve_in == 0 || reserve_out == 0 {
            return (0, 10000); // 100% impact if pool is empty
        }

        // Apply fee
        let fee_multiplier = 10000 - self.total_fee_bps;
        let input_with_fee = (input_amount as u128 * fee_multiplier as u128) / 10000;

        // Calculate output: dy = (y * dx) / (x + dx)
        let numerator = reserve_out as u128 * input_with_fee;
        let denominator = reserve_in as u128 + input_with_fee;
        let output_amount = (numerator / denominator) as u64;

        // Calculate price impact
        let spot_price = (reserve_out as f64) / (reserve_in as f64);
        let execution_price = (output_amount as f64) / (input_amount as f64);
        let impact = ((spot_price - execution_price) / spot_price).abs();
        let impact_bps = (impact * 10000.0) as u16;

        (output_amount, impact_bps)
    }

    fn calculate_clmm_impact(&self, input_amount: u64, _is_a_to_b: bool) -> (u64, u16) {
        // Simplified CLMM calculation - in production, would use tick math
        // For now, estimate based on liquidity
        if let Some(liquidity) = self.liquidity.checked_div(1_000_000) {
            let liquidity_f64 = liquidity as f64;
            let input_f64 = input_amount as f64;

            // Rough estimate: impact proportional to trade size vs liquidity
            let impact_pct = (input_f64 / liquidity_f64).min(1.0);
            let impact_bps = (impact_pct * 10000.0) as u16;

            // Simplified output calculation
            let output = (input_amount as f64 * (1.0 - impact_pct * 0.5)) as u64;

            (output, impact_bps)
        } else {
            (0, 10000)
        }
    }

    fn calculate_dlmm_impact(&self, input_amount: u64, _is_a_to_b: bool) -> (u64, u16) {
        // Meteora DLMM uses bins for concentrated liquidity
        // Simplified calculation - production would iterate through bins
        let liquidity_f64 = self.liquidity as f64 / 1_000_000.0;
        let input_f64 = input_amount as f64;

        let impact_pct = (input_f64 / liquidity_f64).min(1.0);
        let impact_bps = (impact_pct * 10000.0) as u16;

        // Account for bin step
        let bin_step_impact = self.bin_step.unwrap_or(1) as f64 / 100.0;
        let adjusted_impact = impact_bps as f64 * (1.0 + bin_step_impact);

        let output = (input_amount as f64 * (1.0 - impact_pct * 0.3)) as u64;

        (output, adjusted_impact as u16)
    }

    /// Calculate execution probability based on pool state
    /// Returns probability score from 0.0 to 1.0
    pub fn execution_probability(&self, trade_size: u64, is_a_to_b: bool) -> f64 {
        let (_, impact_bps) = self.calculate_price_impact(trade_size, is_a_to_b);

        // Factors affecting execution probability:
        // 1. Price impact (lower is better)
        // 2. Pool liquidity (higher is better)
        // 3. Recent trading activity (more recent is better)
        // 4. Pool age (older/more stable is better)

        let impact_score = match impact_bps {
            0..=50 => 1.0,        // <0.5% impact: excellent
            51..=100 => 0.9,      // 0.5-1%: very good
            101..=200 => 0.7,     // 1-2%: good
            201..=500 => 0.5,     // 2-5%: moderate
            501..=1000 => 0.3,    // 5-10%: risky
            _ => 0.1,             // >10%: very risky
        };

        let liquidity_score = if self.liquidity > 100_000_000 {
            1.0 // High liquidity
        } else if self.liquidity > 10_000_000 {
            0.8 // Medium liquidity
        } else if self.liquidity > 1_000_000 {
            0.6 // Low liquidity
        } else {
            0.3 // Very low liquidity
        };

        let recency_score = if let Some(last_trade) = self.last_trade_timestamp {
            let age_secs = current_timestamp() - last_trade;
            match age_secs {
                0..=60 => 1.0,        // Trade within last minute
                61..=300 => 0.9,      // Last 5 minutes
                301..=3600 => 0.7,    // Last hour
                _ => 0.5,             // Older than 1 hour
            }
        } else {
            0.6 // No recent trade data
        };

        // Weighted average
        (impact_score * 0.5) + (liquidity_score * 0.3) + (recency_score * 0.2)
    }

    /// Check if pool has sufficient liquidity for trade
    pub fn has_sufficient_liquidity(&self, required_output: u64, is_a_to_b: bool) -> bool {
        let available_reserve = if is_a_to_b {
            self.reserve_b
        } else {
            self.reserve_a
        };

        // Reserve should be at least 2x the required output for safety
        available_reserve >= required_output * 2
    }
}

/// Monitors liquidity across multiple pools
pub struct LiquidityMonitor {
    pools: HashMap<Pubkey, PoolState>,
    token_pair_pools: HashMap<TokenPairKey, Vec<Pubkey>>,
    max_pool_age_secs: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TokenPairKey {
    token_a: Pubkey,
    token_b: Pubkey,
}

impl TokenPairKey {
    fn new(token_a: Pubkey, token_b: Pubkey) -> Self {
        // Normalize ordering
        if token_a.to_string() < token_b.to_string() {
            Self { token_a, token_b }
        } else {
            Self { token_a: token_b, token_b: token_a }
        }
    }
}

impl LiquidityMonitor {
    pub fn new(max_pool_age_secs: u64) -> Self {
        Self {
            pools: HashMap::new(),
            token_pair_pools: HashMap::new(),
            max_pool_age_secs,
        }
    }

    /// Update pool state
    pub fn update_pool(&mut self, pool_state: PoolState) {
        let pool_address = pool_state.pool_address;
        let pair_key = TokenPairKey::new(pool_state.token_a, pool_state.token_b);

        // Update pool state
        self.pools.insert(pool_address, pool_state);

        // Update token pair index
        self.token_pair_pools
            .entry(pair_key)
            .or_insert_with(Vec::new)
            .push(pool_address);
    }

    /// Get all pools for a token pair
    pub fn get_pools_for_pair(&self, token_a: Pubkey, token_b: Pubkey) -> Vec<&PoolState> {
        let pair_key = TokenPairKey::new(token_a, token_b);

        if let Some(pool_addresses) = self.token_pair_pools.get(&pair_key) {
            pool_addresses
                .iter()
                .filter_map(|addr| self.pools.get(addr))
                .filter(|pool| {
                    // Filter out stale pools
                    let age = current_timestamp() - pool.last_updated;
                    age <= self.max_pool_age_secs
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Find best pool for a specific trade direction and size
    pub fn find_best_pool(
        &self,
        token_in: Pubkey,
        token_out: Pubkey,
        amount_in: u64,
    ) -> Option<(&PoolState, u64, f64)> {
        let pools = self.get_pools_for_pair(token_in, token_out);

        if pools.is_empty() {
            return None;
        }

        let is_a_to_b = |pool: &&PoolState| pool.token_a == token_in;

        // Evaluate each pool
        let mut best: Option<(&PoolState, u64, f64)> = None;

        for pool in pools {
            let (output, _impact) = pool.calculate_price_impact(amount_in, is_a_to_b(&pool));
            let prob = pool.execution_probability(amount_in, is_a_to_b(&pool));

            // Score = output * probability (expected value)
            let score = output as f64 * prob;

            if let Some((_, _, best_score)) = best {
                if score > best_score {
                    best = Some((pool, output, score));
                }
            } else {
                best = Some((pool, output, score));
            }
        }

        best
    }

    /// Clean stale pool data
    pub fn clean_stale_pools(&mut self) {
        let now = current_timestamp();
        self.pools.retain(|_, pool| {
            now - pool.last_updated <= self.max_pool_age_secs
        });

        // Clean token pair index
        self.token_pair_pools.retain(|_, pools| {
            pools.retain(|addr| self.pools.contains_key(addr));
            !pools.is_empty()
        });
    }

    /// Get pool state by address
    pub fn get_pool(&self, address: &Pubkey) -> Option<&PoolState> {
        self.pools.get(address)
    }

    /// Get statistics
    pub fn stats(&self) -> LiquidityStats {
        LiquidityStats {
            total_pools: self.pools.len(),
            token_pairs: self.token_pair_pools.len(),
            total_liquidity: self.pools.values().map(|p| p.liquidity).sum(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityStats {
    pub total_pools: usize,
    pub token_pairs: usize,
    pub total_liquidity: u128,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_impact_calculation() {
        let pool = PoolState {
            pool_address: Pubkey::default(),
            dex_type: DexType::RaydiumCpmm,
            token_a: Pubkey::default(),
            token_b: Pubkey::default(),
            reserve_a: 1_000_000_000, // 1 SOL
            reserve_b: 100_000_000,   // 100 USDC (assuming 6 decimals)
            liquidity: 10_000_000_000,
            sqrt_price_x64: None,
            tick_current: None,
            active_bin_id: None,
            bin_step: None,
            total_fee_bps: 25,
            last_updated: current_timestamp(),
            last_trade_timestamp: Some(current_timestamp()),
            volume_24h: None,
        };

        // Small trade: 0.1 SOL
        let (output, impact) = pool.calculate_price_impact(100_000_000, true);
        assert!(output > 0);
        assert!(impact < 500); // Should be less than 5% impact

        // Large trade: 10 SOL (relative to pool)
        let (_output_large, impact_large) = pool.calculate_price_impact(10_000_000_000, true);
        assert!(impact_large > impact); // Larger trade should have more impact
    }

    #[test]
    fn test_execution_probability() {
        let pool = PoolState {
            pool_address: Pubkey::default(),
            dex_type: DexType::OrcaWhirlpool,
            token_a: Pubkey::default(),
            token_b: Pubkey::default(),
            reserve_a: 10_000_000_000,
            reserve_b: 1_000_000_000,
            liquidity: 100_000_000_000,
            sqrt_price_x64: Some(1u128 << 64),
            tick_current: Some(0),
            active_bin_id: None,
            bin_step: None,
            total_fee_bps: 30,
            last_updated: current_timestamp(),
            last_trade_timestamp: Some(current_timestamp() - 10), // 10 seconds ago
            volume_24h: Some(1_000_000.0),
        };

        let prob = pool.execution_probability(100_000_000, true);
        assert!(prob > 0.5); // Should be > 50% for reasonable trade
        assert!(prob <= 1.0);
    }
}
