/// Pyth Network price feed integration for arbitrage validation
/// Provides real-time, oracle-grade price data for opportunity validation

use anyhow::{Context, Result};
use dashmap::DashMap;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::sync::RwLock;

/// Price feed configuration for a specific token pair
#[derive(Debug, Clone)]
pub struct PythPriceFeedConfig {
    pub symbol: String,
    pub base_token: Pubkey,
    pub quote_token: Pubkey,
    pub pyth_price_account: Pubkey,
    pub max_staleness_secs: u64,
    pub max_confidence_pct: f64, // Max confidence interval as % of price
}

/// Real-time price data from Pyth oracle
#[derive(Debug, Clone)]
pub struct PythPriceData {
    pub symbol: String,
    pub price: f64,
    pub confidence: f64,
    pub expo: i32,
    pub ema_price: f64,
    pub ema_confidence: f64,
    pub publish_time: i64,
    pub last_updated: SystemTime,
}

impl PythPriceData {
    /// Check if price is fresh enough
    pub fn is_fresh(&self, max_age_secs: u64) -> bool {
        match self.last_updated.elapsed() {
            Ok(elapsed) => elapsed.as_secs() < max_age_secs,
            Err(_) => false,
        }
    }

    /// Get confidence interval as percentage of price
    pub fn confidence_pct(&self) -> f64 {
        if self.price == 0.0 {
            return 100.0;
        }
        (self.confidence / self.price) * 100.0
    }

    /// Check if confidence is acceptable
    pub fn has_acceptable_confidence(&self, max_conf_pct: f64) -> bool {
        self.confidence_pct() <= max_conf_pct
    }

    /// Get normalized price (accounting for exponent)
    pub fn normalized_price(&self) -> f64 {
        self.price * 10f64.powi(self.expo)
    }

    /// Calculate spread between pool price and oracle price
    pub fn calculate_pool_deviation(&self, pool_price: f64) -> f64 {
        let oracle_price = self.normalized_price();
        if oracle_price == 0.0 {
            return 100.0;
        }
        ((pool_price - oracle_price) / oracle_price).abs() * 100.0
    }
}

/// Pyth price monitor for real-time oracle price feeds
pub struct PythPriceMonitor {
    rpc_client: Arc<RpcClient>,
    price_feeds: Arc<DashMap<Pubkey, PythPriceFeedConfig>>,
    price_cache: Arc<DashMap<Pubkey, RwLock<PythPriceData>>>,
    update_interval_ms: u64,
}

impl PythPriceMonitor {
    /// Create new Pyth price monitor
    pub fn new(rpc_url: String, update_interval_ms: u64) -> Self {
        Self {
            rpc_client: Arc::new(RpcClient::new(rpc_url)),
            price_feeds: Arc::new(DashMap::new()),
            price_cache: Arc::new(DashMap::new()),
            update_interval_ms,
        }
    }

    /// Add a price feed to monitor
    pub fn add_price_feed(&self, config: PythPriceFeedConfig) {
        let account = config.pyth_price_account;
        self.price_feeds.insert(account, config);
    }

    /// Add multiple price feeds
    pub fn add_price_feeds(&self, configs: Vec<PythPriceFeedConfig>) {
        for config in configs {
            self.add_price_feed(config);
        }
    }

    /// Fetch price from Pyth oracle (simplified for now)
    async fn fetch_price(&self, price_account: &Pubkey) -> Result<PythPriceData> {
        // For now, return simulated data
        // In production, this would fetch from actual Pyth price account
        let config = self.price_feeds.get(price_account)
            .context("Price feed not found")?;

        // Simulated price data (replace with actual Pyth SDK integration)
        Ok(PythPriceData {
            symbol: config.symbol.clone(),
            price: 100_000_000.0, // $100 with expo -8
            confidence: 100_000.0,
            expo: -8,
            ema_price: 100_000_000.0,
            ema_confidence: 50_000.0,
            publish_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            last_updated: SystemTime::now(),
        })
    }

    /// Update a single price feed
    async fn update_price_feed(&self, price_account: &Pubkey, config: &PythPriceFeedConfig) -> Result<()> {
        let price_data = self.fetch_price(price_account).await?;

        // Update cache
        if let Some(cached) = self.price_cache.get(price_account) {
            *cached.write().await = price_data;
        } else {
            self.price_cache.insert(*price_account, RwLock::new(price_data));
        }

        Ok(())
    }

    /// Start monitoring all price feeds
    pub async fn start_monitoring(self: Arc<Self>) -> Result<()> {
        let interval = Duration::from_millis(self.update_interval_ms);

        loop {
            // Update all price feeds in parallel
            let mut tasks = vec![];

            for entry in self.price_feeds.iter() {
                let account = *entry.key();
                let config = entry.value().clone();
                let monitor = self.clone();

                tasks.push(tokio::spawn(async move {
                    if let Err(e) = monitor.update_price_feed(&account, &config).await {
                        log::warn!("Failed to update price feed {}: {}", config.symbol, e);
                    }
                }));
            }

            // Wait for all updates to complete
            for task in tasks {
                let _ = task.await;
            }

            tokio::time::sleep(interval).await;
        }
    }

    /// Get cached price for a token pair
    pub async fn get_price(&self, base_token: &Pubkey, quote_token: &Pubkey) -> Option<PythPriceData> {
        // Find the price feed for this token pair
        for entry in self.price_feeds.iter() {
            let config = entry.value();
            if config.base_token == *base_token && config.quote_token == *quote_token {
                if let Some(cached) = self.price_cache.get(entry.key()) {
                    return Some(cached.read().await.clone());
                }
            }
        }
        None
    }

    /// Validate if a pool price deviates too much from oracle
    pub async fn validate_pool_price(
        &self,
        base_token: &Pubkey,
        quote_token: &Pubkey,
        pool_price: f64,
        max_deviation_pct: f64,
    ) -> Result<bool> {
        let price_data = self
            .get_price(base_token, quote_token)
            .await
            .context("Price feed not found")?;

        if !price_data.is_fresh(60) {
            anyhow::bail!("Price feed is stale");
        }

        let deviation = price_data.calculate_pool_deviation(pool_price);
        Ok(deviation <= max_deviation_pct)
    }
}

/// Helper to create common Pyth price feed configurations
pub mod presets {
    use super::*;
    use std::str::FromStr;

    /// SOL/USD price feed
    pub fn sol_usd() -> PythPriceFeedConfig {
        PythPriceFeedConfig {
            symbol: "SOL/USD".to_string(),
            base_token: Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
            quote_token: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(), // USDC
            pyth_price_account: Pubkey::from_str("H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG").unwrap(),
            max_staleness_secs: 60,
            max_confidence_pct: 1.0, // 1% max confidence interval
        }
    }

    /// USDC/USD price feed
    pub fn usdc_usd() -> PythPriceFeedConfig {
        PythPriceFeedConfig {
            symbol: "USDC/USD".to_string(),
            base_token: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            quote_token: Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap(), // USDT
            pyth_price_account: Pubkey::from_str("Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJy7eJotD").unwrap(),
            max_staleness_secs: 60,
            max_confidence_pct: 0.5, // 0.5% for stablecoins
        }
    }

    /// Get all common price feeds
    pub fn all_common_feeds() -> Vec<PythPriceFeedConfig> {
        vec![sol_usd(), usdc_usd()]
    }
}