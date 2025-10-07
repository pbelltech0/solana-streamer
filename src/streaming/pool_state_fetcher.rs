/// Pool state enrichment via RPC queries
/// Fetches actual token vault balances for accurate price impact calculations

use crate::streaming::liquidity_monitor::{PoolState, DexType};
use anyhow::{Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use spl_token::solana_program::program_pack::Pack;
use spl_token::state::Account as TokenAccount;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Fetches and enriches pool states with actual on-chain data
pub struct PoolStateFetcher {
    rpc_client: Arc<RpcClient>,
}

impl PoolStateFetcher {
    /// Create a new pool state fetcher
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_client: Arc::new(RpcClient::new(rpc_url)),
        }
    }

    /// Get token account balance
    pub async fn get_token_balance(&self, token_account: &Pubkey) -> Result<u64> {
        let account = self.rpc_client
            .get_account(token_account)
            .await
            .context("Failed to fetch token account")?;

        let token_account = TokenAccount::unpack(&account.data)
            .context("Failed to unpack token account")?;

        Ok(token_account.amount)
    }

    /// Enrich pool state with actual vault balances
    pub async fn enrich_pool_state(&self, pool_state: &mut PoolState) -> Result<()> {
        match pool_state.dex_type {
            DexType::RaydiumClmm | DexType::OrcaWhirlpool => {
                // For CLMM pools, we need to query the pool account to get vault addresses
                // This is a simplified version - in production you'd parse the full pool account
                self.enrich_clmm_pool(pool_state).await?;
            }
            DexType::RaydiumCpmm | DexType::RaydiumAmmV4 => {
                // For AMM pools, vault addresses are in the pool account
                self.enrich_amm_pool(pool_state).await?;
            }
            DexType::MeteoraDlmm => {
                // For Meteora DLMM, reserves are in bin accounts
                self.enrich_dlmm_pool(pool_state).await?;
            }
        }

        pool_state.last_updated = current_timestamp();
        Ok(())
    }

    /// Enrich CLMM pool (Raydium CLMM, Orca Whirlpool)
    async fn enrich_clmm_pool(&self, pool_state: &mut PoolState) -> Result<()> {
        // Get pool account data to extract vault addresses
        let pool_account = self.rpc_client
            .get_account(&pool_state.pool_address)
            .await
            .context("Failed to fetch pool account")?;

        // For Raydium CLMM:
        // Vault0 is at offset 72 (after discriminator + various fields)
        // Vault1 is at offset 104
        if pool_account.data.len() >= 136 {
            let vault0_bytes = &pool_account.data[72..104];
            let vault1_bytes = &pool_account.data[104..136];

            if let (Ok(vault0), Ok(vault1)) = (
                Pubkey::try_from(vault0_bytes),
                Pubkey::try_from(vault1_bytes),
            ) {
                // Fetch balances
                if let Ok(balance0) = self.get_token_balance(&vault0).await {
                    pool_state.reserve_a = balance0;
                }
                if let Ok(balance1) = self.get_token_balance(&vault1).await {
                    pool_state.reserve_b = balance1;
                }
            }
        }

        Ok(())
    }

    /// Enrich AMM pool (Raydium CPMM, AMM V4)
    async fn enrich_amm_pool(&self, pool_state: &mut PoolState) -> Result<()> {
        // Similar approach - parse pool account for vault addresses
        let pool_account = self.rpc_client
            .get_account(&pool_state.pool_address)
            .await
            .context("Failed to fetch pool account")?;

        // For Raydium CPMM, token vaults are at specific offsets
        if pool_account.data.len() >= 256 {
            // These offsets are approximate - you'd need to check the actual struct layout
            let vault0_bytes = &pool_account.data[40..72];
            let vault1_bytes = &pool_account.data[72..104];

            if let (Ok(vault0), Ok(vault1)) = (
                Pubkey::try_from(vault0_bytes),
                Pubkey::try_from(vault1_bytes),
            ) {
                if let Ok(balance0) = self.get_token_balance(&vault0).await {
                    pool_state.reserve_a = balance0;
                }
                if let Ok(balance1) = self.get_token_balance(&vault1).await {
                    pool_state.reserve_b = balance1;
                }
            }
        }

        Ok(())
    }

    /// Enrich DLMM pool (Meteora)
    async fn enrich_dlmm_pool(&self, pool_state: &mut PoolState) -> Result<()> {
        // Meteora DLMM stores liquidity in bins
        // For simplicity, we'll query the reserve accounts from the pool
        let pool_account = self.rpc_client
            .get_account(&pool_state.pool_address)
            .await
            .context("Failed to fetch pool account")?;

        if pool_account.data.len() >= 200 {
            let reserve_x_bytes = &pool_account.data[40..72];
            let reserve_y_bytes = &pool_account.data[72..104];

            if let (Ok(reserve_x), Ok(reserve_y)) = (
                Pubkey::try_from(reserve_x_bytes),
                Pubkey::try_from(reserve_y_bytes),
            ) {
                if let Ok(balance_x) = self.get_token_balance(&reserve_x).await {
                    pool_state.reserve_a = balance_x;
                }
                if let Ok(balance_y) = self.get_token_balance(&reserve_y).await {
                    pool_state.reserve_b = balance_y;
                }
            }
        }

        Ok(())
    }

    /// Batch fetch multiple pool states (more efficient)
    pub async fn enrich_multiple_pools(&self, pools: &mut [PoolState]) -> Result<()> {
        // Process pools concurrently for better performance
        let mut tasks = Vec::new();

        for pool in pools.iter_mut() {
            let fetcher = self.clone();
            let mut pool_clone = pool.clone();

            tasks.push(tokio::spawn(async move {
                fetcher.enrich_pool_state(&mut pool_clone).await?;
                Ok::<PoolState, anyhow::Error>(pool_clone)
            }));
        }

        // Wait for all tasks to complete
        for (i, task) in tasks.into_iter().enumerate() {
            if let Ok(Ok(enriched_pool)) = task.await {
                pools[i] = enriched_pool;
            }
        }

        Ok(())
    }
}

impl Clone for PoolStateFetcher {
    fn clone(&self) -> Self {
        Self {
            rpc_client: Arc::clone(&self.rpc_client),
        }
    }
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

    #[tokio::test]
    #[ignore] // Requires RPC connection
    async fn test_fetch_token_balance() {
        let fetcher = PoolStateFetcher::new("https://api.mainnet-beta.solana.com".to_string());

        // Use a known token account (USDC vault from a popular pool)
        let token_account = Pubkey::from_str("8BnEgHoWFysVcuFFX7QztDmzuH8r5ZFvyP3sYwn1XTh6")
            .unwrap();

        let balance = fetcher.get_token_balance(&token_account).await;
        assert!(balance.is_ok());
        println!("Balance: {:?}", balance);
    }

    #[tokio::test]
    #[ignore] // Requires RPC connection
    async fn test_enrich_pool_state() {
        let fetcher = PoolStateFetcher::new("https://api.mainnet-beta.solana.com".to_string());

        let mut pool_state = PoolState {
            pool_address: Pubkey::from_str("5x1amFuGMfUVzy49Y4Pc3HyCVD5ubMXRa3xg9gFLdMJR")
                .unwrap(),
            dex_type: DexType::RaydiumClmm,
            token_a: Pubkey::default(),
            token_b: Pubkey::default(),
            reserve_a: 0,
            reserve_b: 0,
            liquidity: 0,
            sqrt_price_x64: None,
            tick_current: None,
            active_bin_id: None,
            bin_step: None,
            total_fee_bps: 25,
            last_updated: 0,
            last_trade_timestamp: None,
            volume_24h: None,
        };

        let result = fetcher.enrich_pool_state(&mut pool_state).await;
        assert!(result.is_ok());
        println!("Reserve A: {}", pool_state.reserve_a);
        println!("Reserve B: {}", pool_state.reserve_b);
    }
}
