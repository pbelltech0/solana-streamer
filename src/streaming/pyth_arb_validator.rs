/// Pyth-enhanced arbitrage validator
/// Validates arbitrage opportunities against Pyth oracle prices to prevent:
/// - Stale price exploitation
/// - Oracle manipulation
/// - Pool price manipulation
/// - False arbitrage opportunities

use super::enhanced_arbitrage::EnhancedArbitrageOpportunity;
use super::pyth_price_monitor::{PythPriceMonitor, PythPriceData};
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;

/// Validation result with detailed reasoning
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub reason: String,
    pub oracle_price: Option<f64>,
    pub pool_price: Option<f64>,
    pub deviation_pct: Option<f64>,
    pub confidence_pct: Option<f64>,
}

impl ValidationResult {
    pub fn valid(reason: String) -> Self {
        Self {
            is_valid: true,
            reason,
            oracle_price: None,
            pool_price: None,
            deviation_pct: None,
            confidence_pct: None,
        }
    }

    pub fn invalid(reason: String) -> Self {
        Self {
            is_valid: false,
            reason,
            oracle_price: None,
            pool_price: None,
            deviation_pct: None,
            confidence_pct: None,
        }
    }

    pub fn with_metrics(
        is_valid: bool,
        reason: String,
        oracle_price: f64,
        pool_price: f64,
        deviation_pct: f64,
        confidence_pct: f64,
    ) -> Self {
        Self {
            is_valid,
            reason,
            oracle_price: Some(oracle_price),
            pool_price: Some(pool_price),
            deviation_pct: Some(deviation_pct),
            confidence_pct: Some(confidence_pct),
        }
    }
}

/// Configuration for oracle validation
#[derive(Debug, Clone)]
pub struct OracleValidationConfig {
    /// Maximum acceptable deviation from oracle price (%)
    pub max_price_deviation_pct: f64,
    /// Maximum acceptable oracle confidence interval (%)
    pub max_oracle_confidence_pct: f64,
    /// Maximum oracle staleness (seconds)
    pub max_staleness_secs: u64,
    /// Require both buy and sell pool validation
    pub require_both_pools: bool,
}

impl Default for OracleValidationConfig {
    fn default() -> Self {
        Self {
            max_price_deviation_pct: 5.0,     // 5% max deviation
            max_oracle_confidence_pct: 1.0,   // 1% max confidence interval
            max_staleness_secs: 60,            // 60 seconds max staleness
            require_both_pools: true,          // Validate both pools
        }
    }
}

impl OracleValidationConfig {
    /// Conservative settings - strictest validation
    pub fn conservative() -> Self {
        Self {
            max_price_deviation_pct: 2.0,
            max_oracle_confidence_pct: 0.5,
            max_staleness_secs: 30,
            require_both_pools: true,
        }
    }

    /// Balanced settings - recommended for most use cases
    pub fn balanced() -> Self {
        Self::default()
    }

    /// Aggressive settings - allows more opportunities
    pub fn aggressive() -> Self {
        Self {
            max_price_deviation_pct: 10.0,
            max_oracle_confidence_pct: 2.0,
            max_staleness_secs: 120,
            require_both_pools: false,
        }
    }
}

/// Pyth-enhanced arbitrage validator
pub struct PythArbValidator {
    pyth_monitor: Arc<PythPriceMonitor>,
    config: OracleValidationConfig,
}

impl PythArbValidator {
    /// Create new validator
    pub fn new(pyth_monitor: Arc<PythPriceMonitor>, config: OracleValidationConfig) -> Self {
        Self {
            pyth_monitor,
            config,
        }
    }

    /// Create with default config
    pub fn with_default_config(pyth_monitor: Arc<PythPriceMonitor>) -> Self {
        Self::new(pyth_monitor, OracleValidationConfig::default())
    }

    /// Validate an arbitrage opportunity against Pyth oracle
    pub async fn validate_opportunity(
        &self,
        opportunity: &EnhancedArbitrageOpportunity,
    ) -> Result<ValidationResult> {
        // Get oracle price for the token pair
        let oracle_price = self
            .pyth_monitor
            .get_price(&opportunity.token_pair.base, &opportunity.token_pair.quote)
            .await;

        let oracle_data = match oracle_price {
            Some(data) => data,
            None => {
                return Ok(ValidationResult::invalid(
                    "No Pyth oracle price available for this token pair".to_string(),
                ));
            }
        };

        // Check oracle freshness
        if !oracle_data.is_fresh(self.config.max_staleness_secs) {
            return Ok(ValidationResult::invalid(format!(
                "Oracle price is stale (max age: {}s)",
                self.config.max_staleness_secs
            )));
        }

        // Check oracle confidence
        let conf_pct = oracle_data.confidence_pct();
        if conf_pct > self.config.max_oracle_confidence_pct {
            return Ok(ValidationResult::with_metrics(
                false,
                format!(
                    "Oracle confidence interval too high: {:.2}% (max: {:.2}%)",
                    conf_pct, self.config.max_oracle_confidence_pct
                ),
                oracle_data.normalized_price(),
                0.0,
                0.0,
                conf_pct,
            ));
        }

        // Calculate average pool price
        let avg_pool_price = (opportunity.buy_price + opportunity.sell_price) / 2.0;

        // Calculate deviation from oracle
        let oracle_norm_price = oracle_data.normalized_price();
        let deviation_pct = ((avg_pool_price - oracle_norm_price) / oracle_norm_price).abs() * 100.0;

        // Check if deviation is acceptable
        if deviation_pct > self.config.max_price_deviation_pct {
            return Ok(ValidationResult::with_metrics(
                false,
                format!(
                    "Pool price deviates too much from oracle: {:.2}% (max: {:.2}%)",
                    deviation_pct, self.config.max_price_deviation_pct
                ),
                oracle_norm_price,
                avg_pool_price,
                deviation_pct,
                conf_pct,
            ));
        }

        // Additional check: validate buy and sell prices individually
        if self.config.require_both_pools {
            let buy_dev = ((opportunity.buy_price - oracle_norm_price) / oracle_norm_price).abs() * 100.0;
            let sell_dev = ((opportunity.sell_price - oracle_norm_price) / oracle_norm_price).abs() * 100.0;

            if buy_dev > self.config.max_price_deviation_pct {
                return Ok(ValidationResult::with_metrics(
                    false,
                    format!(
                        "Buy pool price deviates too much: {:.2}% (max: {:.2}%)",
                        buy_dev, self.config.max_price_deviation_pct
                    ),
                    oracle_norm_price,
                    opportunity.buy_price,
                    buy_dev,
                    conf_pct,
                ));
            }

            if sell_dev > self.config.max_price_deviation_pct {
                return Ok(ValidationResult::with_metrics(
                    false,
                    format!(
                        "Sell pool price deviates too much: {:.2}% (max: {:.2}%)",
                        sell_dev, self.config.max_price_deviation_pct
                    ),
                    oracle_norm_price,
                    opportunity.sell_price,
                    sell_dev,
                    conf_pct,
                ));
            }
        }

        // All checks passed
        Ok(ValidationResult::with_metrics(
            true,
            format!(
                "✅ Oracle validation passed (deviation: {:.2}%, confidence: {:.2}%)",
                deviation_pct, conf_pct
            ),
            oracle_norm_price,
            avg_pool_price,
            deviation_pct,
            conf_pct,
        ))
    }

    /// Validate multiple opportunities and filter valid ones
    pub async fn validate_opportunities(
        &self,
        opportunities: Vec<EnhancedArbitrageOpportunity>,
    ) -> Vec<(EnhancedArbitrageOpportunity, ValidationResult)> {
        let mut results = vec![];

        for opp in opportunities {
            match self.validate_opportunity(&opp).await {
                Ok(validation) => {
                    results.push((opp, validation));
                }
                Err(e) => {
                    log::warn!("Validation error: {}", e);
                    results.push((
                        opp,
                        ValidationResult::invalid(format!("Validation error: {}", e)),
                    ));
                }
            }
        }

        results
    }

    /// Filter opportunities to only valid ones
    pub async fn filter_valid_opportunities(
        &self,
        opportunities: Vec<EnhancedArbitrageOpportunity>,
    ) -> Vec<EnhancedArbitrageOpportunity> {
        let validated = self.validate_opportunities(opportunities).await;
        validated
            .into_iter()
            .filter_map(|(opp, result)| {
                if result.is_valid {
                    Some(opp)
                } else {
                    log::debug!("Filtered opportunity: {}", result.reason);
                    None
                }
            })
            .collect()
    }

    /// Get oracle price for debugging
    pub async fn get_oracle_price(
        &self,
        base: &Pubkey,
        quote: &Pubkey,
    ) -> Option<PythPriceData> {
        self.pyth_monitor.get_price(base, quote).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let valid = ValidationResult::valid("Test".to_string());
        assert!(valid.is_valid);

        let invalid = ValidationResult::invalid("Test".to_string());
        assert!(!invalid.is_valid);
    }

    #[test]
    fn test_oracle_config_presets() {
        let conservative = OracleValidationConfig::conservative();
        assert!(conservative.max_price_deviation_pct < 5.0);

        let balanced = OracleValidationConfig::balanced();
        assert_eq!(balanced.max_price_deviation_pct, 5.0);

        let aggressive = OracleValidationConfig::aggressive();
        assert!(aggressive.max_price_deviation_pct > 5.0);
    }
}
