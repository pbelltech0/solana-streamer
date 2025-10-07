/// Jupiter Aggregator V6 integration for optimal routing
/// Compares direct DEX swaps vs Jupiter's multi-hop routes

use crate::streaming::enhanced_arbitrage::EnhancedArbitrageOpportunity;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

/// Jupiter quote response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuote {
    pub input_mint: String,
    pub in_amount: String,
    pub output_mint: String,
    pub out_amount: String,
    pub other_amount_threshold: String,
    pub swap_mode: String,
    pub slippage_bps: u16,
    pub price_impact_pct: f64,
    pub route_plan: Vec<RoutePlanStep>,
    pub context_slot: Option<u64>,
    pub time_taken: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanStep {
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    pub amm_key: String,
    pub label: Option<String>,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_amount: String,
    pub fee_mint: String,
}

/// Jupiter route with metadata
#[derive(Debug, Clone)]
pub struct JupiterRoute {
    pub quote: JupiterQuote,
    pub expected_out_amount: u64,
    pub expected_price_impact: f64,
    pub num_hops: usize,
    pub total_fees: u64,
}

impl JupiterRoute {
    /// Calculate net output after all fees
    pub fn net_output(&self) -> u64 {
        self.expected_out_amount.saturating_sub(self.total_fees)
    }

    /// Estimate execution probability (multi-hop is riskier)
    pub fn execution_probability(&self) -> f64 {
        // Each hop reduces probability slightly
        let base_prob: f64 = 0.95; // 95% per hop
        base_prob.powi(self.num_hops as i32)
    }
}

/// Jupiter API client
pub struct JupiterRouter {
    client: reqwest::Client,
    api_url: String,
}

impl JupiterRouter {
    /// Create new Jupiter router
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url: "https://quote-api.jup.ag/v6".to_string(),
        }
    }

    /// Create with custom API URL
    pub fn with_url(api_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
        }
    }

    /// Get quote from Jupiter
    pub async fn get_quote(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<JupiterQuote> {
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            self.api_url,
            input_mint,
            output_mint,
            amount,
            slippage_bps
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request to Jupiter API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Jupiter API error {}: {}", status, error_text);
        }

        let quote: JupiterQuote = response
            .json()
            .await
            .context("Failed to parse Jupiter quote response")?;

        Ok(quote)
    }

    /// Get route with additional metadata
    pub async fn get_route(
        &self,
        input_mint: &Pubkey,
        output_mint: &Pubkey,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<JupiterRoute> {
        let quote = self.get_quote(input_mint, output_mint, amount, slippage_bps).await?;

        let expected_out_amount = quote.out_amount.parse::<u64>()
            .context("Failed to parse output amount")?;

        // Calculate total fees from route plan
        let total_fees: u64 = quote.route_plan.iter()
            .map(|step| {
                step.swap_info.fee_amount.parse::<u64>().unwrap_or(0)
            })
            .sum();

        let num_hops = quote.route_plan.len();

        Ok(JupiterRoute {
            quote,
            expected_out_amount,
            expected_price_impact: 0.0, // Would parse from quote
            num_hops,
            total_fees,
        })
    }

    /// Compare Jupiter route vs direct swap
    pub async fn is_better_than_direct(
        &self,
        opportunity: &EnhancedArbitrageOpportunity,
        slippage_bps: u16,
    ) -> Result<Option<JupiterRoute>> {
        // Get Jupiter quote for the same trade
        let jupiter_route = self.get_route(
            &opportunity.token_pair.base,
            &opportunity.token_pair.quote,
            opportunity.optimal_trade_size,
            slippage_bps,
        ).await?;

        // Calculate Jupiter's net profit
        let jupiter_net_output = jupiter_route.net_output();
        let jupiter_gross_profit = jupiter_net_output as i64 - opportunity.optimal_trade_size as i64;

        // Apply execution probability discount
        let jupiter_execution_prob = jupiter_route.execution_probability();
        let jupiter_expected_value = jupiter_gross_profit as f64 * jupiter_execution_prob;

        // Compare to direct swap EV
        let direct_ev = opportunity.expected_value;

        // Jupiter is better if it has higher EV
        if jupiter_expected_value > direct_ev {
            Ok(Some(jupiter_route))
        } else {
            Ok(None)
        }
    }

    /// Get best route for arbitrage (try multiple slippage settings)
    pub async fn get_best_arb_route(
        &self,
        token_in: &Pubkey,
        token_out: &Pubkey,
        amount: u64,
    ) -> Result<JupiterRoute> {
        // Try different slippage settings and pick best
        let slippages = vec![10, 25, 50, 100]; // 0.1%, 0.25%, 0.5%, 1%

        let mut best_route: Option<JupiterRoute> = None;
        let mut best_ev = 0.0f64;

        for slippage in slippages {
            if let Ok(route) = self.get_route(token_in, token_out, amount, slippage).await {
                let ev = route.net_output() as f64 * route.execution_probability();
                if ev > best_ev {
                    best_ev = ev;
                    best_route = Some(route);
                }
            }
        }

        best_route.ok_or_else(|| anyhow::anyhow!("No viable Jupiter route found"))
    }
}

impl Default for JupiterRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to compare routes
pub struct RouteComparison {
    pub direct_ev: f64,
    pub jupiter_ev: f64,
    pub improvement_pct: f64,
    pub recommendation: String,
}

impl RouteComparison {
    pub fn compare(
        direct_opportunity: &EnhancedArbitrageOpportunity,
        jupiter_route: Option<&JupiterRoute>,
    ) -> Self {
        let direct_ev = direct_opportunity.expected_value;

        let (jupiter_ev, improvement_pct, recommendation) = if let Some(route) = jupiter_route {
            let jup_ev = route.net_output() as f64 * route.execution_probability();
            let improvement = ((jup_ev - direct_ev) / direct_ev) * 100.0;

            let rec = if improvement > 20.0 {
                "🟢 USE JUPITER - Significantly better".to_string()
            } else if improvement > 5.0 {
                "🟡 CONSIDER JUPITER - Moderately better".to_string()
            } else if improvement > 0.0 {
                "🟠 MARGINAL - Slightly better".to_string()
            } else {
                "🔴 USE DIRECT - Jupiter worse".to_string()
            };

            (jup_ev, improvement, rec)
        } else {
            (0.0, -100.0, "❌ NO JUPITER ROUTE".to_string())
        };

        Self {
            direct_ev,
            jupiter_ev,
            improvement_pct,
            recommendation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore] // Requires API access
    async fn test_get_jupiter_quote() {
        let router = JupiterRouter::new();

        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

        let amount = 100_000_000; // 0.1 SOL

        let quote = router.get_quote(&sol, &usdc, amount, 50).await;

        assert!(quote.is_ok());
        let quote = quote.unwrap();
        println!("Quote: {:?}", quote);
        println!("Out amount: {}", quote.out_amount);
        println!("Route plan: {} hops", quote.route_plan.len());
    }

    #[tokio::test]
    #[ignore] // Requires API access
    async fn test_get_route() {
        let router = JupiterRouter::new();

        let sol = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let usdc = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();

        let route = router.get_route(&sol, &usdc, 100_000_000, 50).await;

        assert!(route.is_ok());
        let route = route.unwrap();
        println!("Expected out: {}", route.expected_out_amount);
        println!("Num hops: {}", route.num_hops);
        println!("Execution prob: {:.2}%", route.execution_probability() * 100.0);
    }
}
