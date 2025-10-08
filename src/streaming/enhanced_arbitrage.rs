/// Enhanced arbitrage detection with liquidity-aware probability scoring
/// Focuses on maximizing expected value: profit * execution_probability

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a potential arbitrage opportunity with execution probability
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnhancedArbitrageOpportunity {
    pub token_pair: TokenPair,
    pub buy_pool: Pubkey,
    pub sell_pool: Pubkey,
    pub buy_dex: DexType,
    pub sell_dex: DexType,

    // Price information
    pub buy_price: f64,
    pub sell_price: f64,
    pub gross_profit_pct: f64,

    // Trade execution details
    pub optimal_trade_size: u64,
    pub expected_input: u64,
    pub expected_output: u64,
    pub expected_profit: u64,

    // Fee analysis
    pub total_fees: u64,
    pub total_fee_pct: f64,
    pub estimated_gas_lamports: u64,

    // Profit after costs
    pub net_profit: i64,
    pub net_profit_pct: f64,

    // Execution probability analysis
    pub buy_pool_impact_bps: u16,
    pub sell_pool_impact_bps: u16,
    pub buy_execution_prob: f64,
    pub sell_execution_prob: f64,
    pub combined_execution_prob: f64,

    // Expected value (profit * probability)
    pub expected_value: f64,
    pub ev_score: f64, // Normalized 0-100

    // Metadata
    pub timestamp: u64,
    pub confidence_level: ConfidenceLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    VeryHigh,  // >80% execution prob, >1% net profit
    High,      // >60% execution prob, >0.5% net profit
    Medium,    // >40% execution prob, >0.3% net profit
    Low,       // >20% execution prob, any profit
    VeryLow,   // <20% execution prob
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenPair {
    pub base: Pubkey,
    pub quote: Pubkey,
}

impl TokenPair {
    pub fn new(token_a: Pubkey, token_b: Pubkey) -> Self {
        if token_a.to_string() < token_b.to_string() {
            Self { base: token_a, quote: token_b }
        } else {
            Self { base: token_b, quote: token_a }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DexType {
    RaydiumCpmm,
    RaydiumClmm,
    RaydiumAmmV4,
    PumpFun,
    PumpSwap,
    Bonk,
}

/// Pool state for liquidity tracking
#[derive(Clone, Debug)]
pub struct PoolState {
    pub pool_address: Pubkey,
    pub dex_type: DexType,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub liquidity: u64,
    pub sqrt_price_x64: Option<u128>,
    pub total_fee_bps: u16,
    pub last_updated: u64,
}

impl PoolState {
    /// Calculate price impact for a given trade size
    pub fn calculate_price_impact(&self, amount_in: u64, is_a_to_b: bool) -> (u64, u16) {
        let (reserve_in, reserve_out) = if is_a_to_b {
            (self.reserve_a, self.reserve_b)
        } else {
            (self.reserve_b, self.reserve_a)
        };

        if reserve_in == 0 || reserve_out == 0 {
            return (0, 10000); // 100% impact if no reserves
        }

        // Calculate output amount using constant product formula
        let amount_in_with_fee = amount_in * (10000 - self.total_fee_bps as u64) / 10000;
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in + amount_in_with_fee;

        if denominator == 0 {
            return (0, 10000);
        }

        let amount_out = numerator / denominator;

        // Calculate price impact in basis points
        let spot_price = reserve_out as f64 / reserve_in as f64;
        let execution_price = amount_out as f64 / amount_in as f64;
        let impact_pct = ((spot_price - execution_price) / spot_price * 10000.0).abs();
        let impact_bps = impact_pct.min(10000.0) as u16;

        (amount_out, impact_bps)
    }

    /// Calculate execution probability based on liquidity and trade size
    pub fn execution_probability(&self, trade_size: u64, is_a_to_b: bool) -> f64 {
        let reserve = if is_a_to_b { self.reserve_a } else { self.reserve_b };

        if reserve == 0 {
            return 0.0;
        }

        let size_ratio = trade_size as f64 / reserve as f64;

        // Probability decreases exponentially with size relative to liquidity
        // 1% of liquidity = 95% prob, 5% = 80% prob, 10% = 60% prob, 20% = 30% prob
        let prob = (-5.0 * size_ratio).exp();
        prob.max(0.0).min(1.0)
    }
}

/// Configuration for monitored token pairs
#[derive(Clone, Debug)]
pub struct MonitoredPair {
    pub name: String,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub min_trade_size: u64,
    pub max_trade_size: u64,
    pub target_pools: Vec<Pubkey>, // Optional: specific pools to watch
}

/// Enhanced arbitrage detector with liquidity awareness
pub struct EnhancedArbitrageDetector {
    pool_states: HashMap<Pubkey, PoolState>,
    monitored_pairs: Vec<MonitoredPair>,
    opportunities: Vec<EnhancedArbitrageOpportunity>,

    // Configuration
    min_net_profit_pct: f64,
    min_execution_prob: f64,
    min_ev_score: f64,
    max_opportunities: usize,

    // Gas estimation
    base_gas_per_tx: u64,      // lamports
    jito_bundle_tip: u64,       // lamports
}

impl EnhancedArbitrageDetector {
    pub fn new(
        monitored_pairs: Vec<MonitoredPair>,
        min_net_profit_pct: f64,
        min_execution_prob: f64,
    ) -> Self {
        Self {
            pool_states: HashMap::new(),
            monitored_pairs,
            opportunities: Vec::new(),
            min_net_profit_pct,
            min_execution_prob,
            min_ev_score: 10.0, // Minimum EV score to consider
            max_opportunities: 100,
            base_gas_per_tx: 10_000,       // ~0.00001 SOL per transaction
            jito_bundle_tip: 1_000_000,    // ~0.001 SOL tip for Jito
        }
    }

    /// Update pool state from event
    pub fn update_pool_state(&mut self, pool_state: PoolState) {
        self.pool_states.insert(pool_state.pool_address, pool_state);
    }

    /// Scan for arbitrage opportunities across all monitored pairs
    pub fn scan_arbitrage_opportunities(&mut self) -> Vec<EnhancedArbitrageOpportunity> {
        let mut new_opportunities = Vec::new();

        for pair_config in &self.monitored_pairs {
            let opportunities = self.find_opportunities_for_pair(pair_config);
            new_opportunities.extend(opportunities);
        }

        // Sort by EV score (highest first)
        new_opportunities.sort_by(|a, b| {
            b.ev_score.partial_cmp(&a.ev_score).unwrap()
        });

        // Keep only top N opportunities
        new_opportunities.truncate(self.max_opportunities);

        // Update stored opportunities
        self.opportunities = new_opportunities.clone();

        new_opportunities
    }

    /// Find arbitrage opportunities for a specific token pair
    fn find_opportunities_for_pair(
        &self,
        pair_config: &MonitoredPair,
    ) -> Vec<EnhancedArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        // Get all pools for this pair
        let pools: Vec<&PoolState> = self.pool_states
            .values()
            .filter(|pool| {
                (pool.token_a == pair_config.token_a && pool.token_b == pair_config.token_b) ||
                (pool.token_a == pair_config.token_b && pool.token_b == pair_config.token_a)
            })
            .collect();

        if pools.len() < 2 {
            return opportunities;
        }

        // Compare each pair of pools
        for (i, pool1) in pools.iter().enumerate() {
            for pool2 in pools.iter().skip(i + 1) {
                // Don't compare pools on same DEX (usually)
                if pool1.dex_type == pool2.dex_type {
                    continue;
                }

                // Try both directions
                if let Some(opp) = self.calculate_arbitrage(
                    pair_config,
                    pool1,
                    pool2,
                    true, // pool1 buy, pool2 sell
                ) {
                    if opp.ev_score >= self.min_ev_score &&
                       opp.net_profit_pct >= self.min_net_profit_pct &&
                       opp.combined_execution_prob >= self.min_execution_prob {
                        opportunities.push(opp);
                    }
                }
            }
        }

        opportunities
    }

    /// Calculate arbitrage opportunity between two pools
    fn calculate_arbitrage(
        &self,
        pair_config: &MonitoredPair,
        buy_pool: &PoolState,
        sell_pool: &PoolState,
        is_a_to_b: bool,
    ) -> Option<EnhancedArbitrageOpportunity> {
        let token_pair = TokenPair::new(pair_config.token_a, pair_config.token_b);

        // Find optimal trade size
        let mut best_opportunity: Option<EnhancedArbitrageOpportunity> = None;
        let mut best_ev = 0.0f64;

        // Test different trade sizes
        let step_count = 20;
        let step_size = (pair_config.max_trade_size - pair_config.min_trade_size) / step_count;

        for i in 0..=step_count {
            let trade_size = pair_config.min_trade_size + (i * step_size);

            if let Some(opp) = self.evaluate_trade_size(
                &token_pair,
                buy_pool,
                sell_pool,
                trade_size,
                is_a_to_b,
            ) {
                if opp.expected_value > best_ev {
                    best_ev = opp.expected_value;
                    best_opportunity = Some(opp);
                }
            }
        }

        best_opportunity
    }

    /// Evaluate a specific trade size for arbitrage
    fn evaluate_trade_size(
        &self,
        token_pair: &TokenPair,
        buy_pool: &PoolState,
        sell_pool: &PoolState,
        trade_size: u64,
        is_a_to_b: bool,
    ) -> Option<EnhancedArbitrageOpportunity> {
        // Calculate buy on pool1
        let (intermediate_amount, buy_impact) =
            buy_pool.calculate_price_impact(trade_size, is_a_to_b);

        if intermediate_amount == 0 {
            return None;
        }

        // Calculate sell on pool2
        let (final_amount, sell_impact) =
            sell_pool.calculate_price_impact(intermediate_amount, !is_a_to_b);

        if final_amount <= trade_size {
            return None; // No profit
        }

        // Calculate prices
        let buy_price = intermediate_amount as f64 / trade_size as f64;
        let sell_price = final_amount as f64 / intermediate_amount as f64;

        // Calculate gross profit
        let gross_profit = final_amount as i64 - trade_size as i64;
        let gross_profit_pct = (gross_profit as f64 / trade_size as f64) * 100.0;

        // Calculate fees
        let buy_fee = (trade_size as f64 * buy_pool.total_fee_bps as f64) / 10000.0;
        let sell_fee = (intermediate_amount as f64 * sell_pool.total_fee_bps as f64) / 10000.0;
        let total_fees = (buy_fee + sell_fee) as u64;
        let total_fee_pct = ((buy_fee + sell_fee) / trade_size as f64) * 100.0;

        // Estimate gas costs
        let estimated_gas = (self.base_gas_per_tx * 2) + self.jito_bundle_tip;

        // Calculate net profit
        let net_profit = gross_profit - total_fees as i64 - estimated_gas as i64;
        let net_profit_pct = (net_profit as f64 / trade_size as f64) * 100.0;

        if net_profit <= 0 {
            return None;
        }

        // Calculate execution probabilities
        let buy_prob = buy_pool.execution_probability(trade_size, is_a_to_b);
        let sell_prob = sell_pool.execution_probability(intermediate_amount, !is_a_to_b);
        let combined_prob = buy_prob * sell_prob;

        // Calculate expected value
        let expected_value = net_profit as f64 * combined_prob;

        // EV score: normalized to 0-100 scale
        let ev_score = (expected_value / 10_000.0).min(100.0);

        // Determine confidence level
        let confidence = if combined_prob > 0.8 && net_profit_pct > 1.0 {
            ConfidenceLevel::VeryHigh
        } else if combined_prob > 0.6 && net_profit_pct > 0.5 {
            ConfidenceLevel::High
        } else if combined_prob > 0.4 && net_profit_pct > 0.3 {
            ConfidenceLevel::Medium
        } else if combined_prob > 0.2 {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::VeryLow
        };

        Some(EnhancedArbitrageOpportunity {
            token_pair: token_pair.clone(),
            buy_pool: buy_pool.pool_address,
            sell_pool: sell_pool.pool_address,
            buy_dex: buy_pool.dex_type.clone(),
            sell_dex: sell_pool.dex_type.clone(),
            buy_price,
            sell_price,
            gross_profit_pct,
            optimal_trade_size: trade_size,
            expected_input: trade_size,
            expected_output: final_amount,
            expected_profit: gross_profit.max(0) as u64,
            total_fees,
            total_fee_pct,
            estimated_gas_lamports: estimated_gas,
            net_profit,
            net_profit_pct,
            buy_pool_impact_bps: buy_impact,
            sell_pool_impact_bps: sell_impact,
            buy_execution_prob: buy_prob,
            sell_execution_prob: sell_prob,
            combined_execution_prob: combined_prob,
            expected_value,
            ev_score,
            timestamp: current_timestamp(),
            confidence_level: confidence,
        })
    }

    /// Get current opportunities
    pub fn get_opportunities(&self) -> &[EnhancedArbitrageOpportunity] {
        &self.opportunities
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}