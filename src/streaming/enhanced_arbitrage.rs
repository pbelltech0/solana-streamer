/// Enhanced arbitrage detection with liquidity-aware probability scoring
/// Focuses on maximizing expected value: profit * execution_probability

use super::liquidity_monitor::{LiquidityMonitor, PoolState, DexType};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
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

impl EnhancedArbitrageOpportunity {
    /// Check if this opportunity is worth executing
    pub fn is_executable(&self, min_ev_score: f64, min_net_profit_pct: f64) -> bool {
        self.ev_score >= min_ev_score
            && self.net_profit_pct >= min_net_profit_pct
            && self.combined_execution_prob > 0.3 // Minimum 30% chance
    }

    /// Get recommended action
    pub fn recommendation(&self) -> String {
        match self.confidence_level {
            ConfidenceLevel::VeryHigh => {
                format!("🟢 EXECUTE: High confidence, EV={:.2}, Net={:.2}%",
                    self.ev_score, self.net_profit_pct)
            }
            ConfidenceLevel::High => {
                format!("🟡 CONSIDER: Good opportunity, EV={:.2}, Net={:.2}%",
                    self.ev_score, self.net_profit_pct)
            }
            ConfidenceLevel::Medium => {
                format!("🟠 MONITOR: Moderate risk, EV={:.2}, Net={:.2}%",
                    self.ev_score, self.net_profit_pct)
            }
            ConfidenceLevel::Low => {
                format!("🔴 SKIP: Low confidence, EV={:.2}, Net={:.2}%",
                    self.ev_score, self.net_profit_pct)
            }
            ConfidenceLevel::VeryLow => {
                format!("⛔ AVOID: Very risky, EV={:.2}, Net={:.2}%",
                    self.ev_score, self.net_profit_pct)
            }
        }
    }
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
    liquidity_monitor: LiquidityMonitor,
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
            liquidity_monitor: LiquidityMonitor::new(60), // 60 second max pool age
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

    /// Update pool state from liquidity event
    pub fn update_pool_state(&mut self, pool_state: PoolState) {
        self.liquidity_monitor.update_pool(pool_state);
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
        let pools = self.liquidity_monitor.get_pools_for_pair(
            pair_config.token_a,
            pair_config.token_b,
        );

        if pools.len() < 2 {
            return opportunities; // Need at least 2 pools for arbitrage
        }

        // Compare each pair of pools
        for (i, pool1) in pools.iter().enumerate() {
            for pool2 in pools.iter().skip(i + 1) {
                // Don't compare pools on same DEX
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
                    if opp.is_executable(self.min_ev_score, self.min_net_profit_pct) {
                        opportunities.push(opp);
                    }
                }

                if let Some(opp) = self.calculate_arbitrage(
                    pair_config,
                    pool2,
                    pool1,
                    true, // pool2 buy, pool1 sell
                ) {
                    if opp.is_executable(self.min_ev_score, self.min_net_profit_pct) {
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

        // Find optimal trade size by testing different amounts
        let mut best_opportunity: Option<EnhancedArbitrageOpportunity> = None;
        let mut best_ev = 0.0f64;

        // Test trade sizes from min to max in steps
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

        if final_amount == 0 {
            return None;
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

        // Estimate gas costs (2 swaps + 1 Jito bundle)
        let estimated_gas = (self.base_gas_per_tx * 2) + self.jito_bundle_tip;

        // Calculate net profit
        let net_profit = gross_profit - total_fees as i64 - estimated_gas as i64;
        let net_profit_pct = (net_profit as f64 / trade_size as f64) * 100.0;

        // Calculate execution probabilities
        let buy_prob = buy_pool.execution_probability(trade_size, is_a_to_b);
        let sell_prob = sell_pool.execution_probability(intermediate_amount, !is_a_to_b);
        let combined_prob = buy_prob * sell_prob; // Both must succeed

        // Calculate expected value
        let expected_value = net_profit as f64 * combined_prob;

        // EV score: normalized to 0-100 scale
        // We consider both absolute EV and percentage return
        let ev_score = (expected_value / 10_000.0) * 10.0 + (net_profit_pct * combined_prob);

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

    /// Get top N opportunities by EV score
    pub fn get_top_opportunities(&self, n: usize) -> Vec<&EnhancedArbitrageOpportunity> {
        self.opportunities.iter().take(n).collect()
    }

    /// Get statistics
    pub fn stats(&self) -> DetectorStats {
        DetectorStats {
            monitored_pairs: self.monitored_pairs.len(),
            active_opportunities: self.opportunities.len(),
            liquidity_stats: self.liquidity_monitor.stats(),
            high_confidence_count: self.opportunities.iter()
                .filter(|o| matches!(o.confidence_level, ConfidenceLevel::High | ConfidenceLevel::VeryHigh))
                .count(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStats {
    pub monitored_pairs: usize,
    pub active_opportunities: usize,
    pub liquidity_stats: super::liquidity_monitor::LiquidityStats,
    pub high_confidence_count: usize,
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
    use std::str::FromStr;

    #[test]
    fn test_opportunity_evaluation() {
        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

        let pair = MonitoredPair {
            name: "SOL/USDC".to_string(),
            token_a: sol,
            token_b: usdc,
            min_trade_size: 100_000_000,  // 0.1 SOL
            max_trade_size: 10_000_000_000, // 10 SOL
            target_pools: vec![],
        };

        let detector = EnhancedArbitrageDetector::new(
            vec![pair],
            0.3, // 0.3% min profit
            0.5, // 50% min execution prob
        );

        let stats = detector.stats();
        assert_eq!(stats.monitored_pairs, 1);
    }
}
