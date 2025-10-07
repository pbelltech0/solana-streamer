/// Pyth Network price feed integration for arbitrage validation
/// Provides real-time, oracle-grade price data for opportunity validation

use anyhow::{Context, Result};
use dashmap::DashMap;
use pyth_sdk::PriceFeed;
use pyth_sdk_solana::state::load_price_account;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::pubkey::Pubkey as SplPubkey;
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

    /// Get normalized EMA price
    pub fn normalized_ema_price(&self) -> f64 {
        self.ema_price * 10f64.powi(self.expo)
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

    /// Fetch price from Pyth oracle
    async fn fetch_price(&self, price_account: &Pubkey) -> Result<PriceFeed> {
        let account = self
            .rpc_client
            .get_account(price_account)
            .await
            .context("Failed to fetch Pyth price account")?;

        // Parse price account data directly
        // SolanaPriceAccount is GenericPriceAccount<32, ()>
        let price_account_data = load_price_account::<32, ()>(&account.data)
            .context("Failed to load Pyth price account")?;

        // Convert solana_sdk::Pubkey to spl_token's Pubkey for pyth-sdk-solana
        let spl_pubkey = SplPubkey::new_from_array(price_account.to_bytes());
        let price_feed = price_account_data.to_price_feed(&spl_pubkey);

        Ok(price_feed)
    }

    /// Update a single price feed
    async fn update_price_feed(&self, price_account: &Pubkey, config: &PythPriceFeedConfig) -> Result<()> {
        let price_feed = self.fetch_price(price_account).await?;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Get current price
        let maybe_price = price_feed.get_price_no_older_than(current_time, config.max_staleness_secs);
        let maybe_ema = price_feed.get_ema_price_no_older_than(current_time, config.max_staleness_secs);

        if let (Some(price), Some(ema)) = (maybe_price, maybe_ema) {
            let price_data = PythPriceData {
                symbol: config.symbol.clone(),
                price: price.price as f64,
                confidence: price.conf as f64,
                expo: price.expo,
                ema_price: ema.price as f64,
                ema_confidence: ema.conf as f64,
                publish_time: price.publish_time,
                last_updated: SystemTime::now(),
            };

            // Update cache
            if let Some(cached) = self.price_cache.get(price_account) {
                *cached.write().await = price_data;
            } else {
                self.price_cache.insert(*price_account, RwLock::new(price_data));
            }
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

    /// Get price by Pyth account address
    pub async fn get_price_by_account(&self, price_account: &Pubkey) -> Option<PythPriceData> {
        if let Some(cached) = self.price_cache.get(price_account) {
            Some(cached.read().await.clone())
        } else {
            None
        }
    }

    /// Get all current prices
    pub async fn get_all_prices(&self) -> Vec<PythPriceData> {
        let mut prices = vec![];
        for entry in self.price_cache.iter() {
            prices.push(entry.value().read().await.clone());
        }
        prices
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

    /// SOL/USD price feed on Pythnet
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

    /// USDC/USD price feed on Pythnet
    pub fn usdc_usd() -> PythPriceFeedConfig {
        PythPriceFeedConfig {
            symbol: "USDC/USD".to_string(),
            base_token: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            quote_token: Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap(), // USDT as proxy
            pyth_price_account: Pubkey::from_str("Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJy7eJotD").unwrap(),
            max_staleness_secs: 60,
            max_confidence_pct: 0.5, // 0.5% for stablecoins
        }
    }

    /// USDT/USD price feed on Pythnet
    pub fn usdt_usd() -> PythPriceFeedConfig {
        PythPriceFeedConfig {
            symbol: "USDT/USD".to_string(),
            base_token: Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap(),
            quote_token: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(), // USDC
            pyth_price_account: Pubkey::from_str("3vxLXJqLqF3JG5TCbYycbKWRBbCJQLxQmBGCkyqEEefL").unwrap(),
            max_staleness_secs: 60,
            max_confidence_pct: 0.5,
        }
    }

    /// Get all common price feeds
    pub fn all_common_feeds() -> Vec<PythPriceFeedConfig> {
        vec![sol_usd(), usdc_usd(), usdt_usd()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_data_confidence_pct() {
        let price_data = PythPriceData {
            symbol: "TEST/USD".to_string(),
            price: 100.0,
            confidence: 1.0,
            expo: -8,
            ema_price: 100.0,
            ema_confidence: 0.5,
            publish_time: 0,
            last_updated: SystemTime::now(),
        };

        assert_eq!(price_data.confidence_pct(), 1.0);
        assert!(price_data.has_acceptable_confidence(2.0));
        assert!(!price_data.has_acceptable_confidence(0.5));
    }

    #[test]
    fn test_pool_deviation() {
        let price_data = PythPriceData {
            symbol: "TEST/USD".to_string(),
            price: 100_000_000.0, // $1.00 with expo -8
            confidence: 100_000.0,
            expo: -8,
            ema_price: 100_000_000.0,
            ema_confidence: 50_000.0,
            publish_time: 0,
            last_updated: SystemTime::now(),
        };

        // Pool price is $1.05 - 5% deviation
        let pool_price = 1.05;
        let deviation = price_data.calculate_pool_deviation(pool_price);
        assert!((deviation - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_normalized_price() {
        let price_data = PythPriceData {
            symbol: "TEST/USD".to_string(),
            price: 100_000_000.0,
            confidence: 100_000.0,
            expo: -8,
            ema_price: 100_000_000.0,
            ema_confidence: 50_000.0,
            publish_time: 0,
            last_updated: SystemTime::now(),
        };

        assert!((price_data.normalized_price() - 1.0).abs() < 0.0001);
    }
}
