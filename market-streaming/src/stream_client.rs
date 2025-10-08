use crate::pool_states::{DexPoolState, DexProtocol, OrcaWhirlpoolState, RaydiumClmmPoolState, MeteoraDlmmPoolState};
use crate::state_cache::PoolStateCache;
use anyhow::{Context, Result};
use borsh::BorshDeserialize;
use futures::{SinkExt, StreamExt};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::*;
use yellowstone_grpc_proto::tonic::transport::ClientTlsConfig;
/// Configuration for pool streaming
#[derive(Clone, Debug)]
pub struct StreamConfig {
    /// Yellowstone gRPC endpoint
    pub grpc_endpoint: String,
    /// Optional auth token
    pub auth_token: Option<String>,
    /// List of pool pubkeys to monitor
    pub pool_pubkeys: Vec<Pubkey>,
    /// List of DEX protocols to monitor
    pub protocols: Vec<DexProtocol>,
    /// Commitment level
    pub commitment: CommitmentLevel,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: "https://grpc.mainnet.solana.tools:443".to_string(),
            auth_token: None,
            pool_pubkeys: Vec::new(),
            protocols: vec![
                DexProtocol::RaydiumClmm,
                DexProtocol::OrcaWhirlpool,
                DexProtocol::MeteoraDlmm,
            ],
            commitment: CommitmentLevel::Processed,
        }
    }
}

/// Pool stream client for monitoring DEX pool state changes
pub struct PoolStreamClient {
    config: StreamConfig,
    state_cache: Arc<PoolStateCache>,
}

impl PoolStreamClient {
    /// Create a new pool stream client
    pub fn new(config: StreamConfig, state_cache: Arc<PoolStateCache>) -> Self {
        Self {
            config,
            state_cache,
        }
    }

    /// Start streaming pool account updates
    pub async fn start(&self) -> Result<()> {
        // Build gRPC client with proper TLS and timeout configuration
        let builder = GeyserGrpcClient::build_from_shared(self.config.grpc_endpoint.clone())
            .context("Failed to build gRPC client")?
            .x_token(self.config.auth_token.clone())?
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .max_decoding_message_size(128 * 1024 * 1024) // 128 MB
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30));

        // Connect
        let mut client = builder.connect().await
            .context("Failed to connect to gRPC endpoint")?;

        // Build program IDs for filtering
        let program_ids: Vec<String> = self
            .config
            .protocols
            .iter()
            .map(|p| p.program_id().to_string())
            .collect();

        log::info!(
            "Starting pool stream with {} pools and {} protocols",
            self.config.pool_pubkeys.len(),
            self.config.protocols.len()
        );

        // Build subscription request
        let mut accounts_filter = std::collections::HashMap::new();
        accounts_filter.insert(
            "dex_pools".to_string(),
            SubscribeRequestFilterAccounts {
                account: self
                    .config
                    .pool_pubkeys
                    .iter()
                    .map(|p| p.to_string())
                    .collect(),
                owner: program_ids,
                ..Default::default()
            },
        );

        let request = SubscribeRequest {
            accounts: accounts_filter,
            commitment: Some(self.config.commitment as i32),
            ..Default::default()
        };

        // Subscribe to updates
        let (mut subscribe_tx, mut stream) = client.subscribe().await?;
        subscribe_tx.send(request).await?;

        // Process updates
        while let Some(msg) = stream.next().await {
            match msg {
                Ok(update) => {
                    if let Some(update_msg) = update.update_oneof {
                        match update_msg {
                            subscribe_update::UpdateOneof::Account(account_update) => {
                                self.process_account_update(account_update).await;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    log::error!("Stream error: {:?}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process a single account update
    async fn process_account_update(&self, update: SubscribeUpdateAccount) {
        let Some(account_info) = update.account else {
            return;
        };

        // Parse pubkey
        let Ok(pubkey) = Pubkey::try_from(account_info.pubkey.as_slice()) else {
            log::warn!("Failed to parse account pubkey");
            return;
        };

        // Parse owner (program ID)
        let Ok(owner) = Pubkey::try_from(account_info.owner.as_slice()) else {
            log::warn!("Failed to parse owner pubkey");
            return;
        };

        // Determine protocol
        let protocol = self
            .config
            .protocols
            .iter()
            .find(|p| p.program_id() == owner.to_string())
            .copied();

        let Some(protocol) = protocol else {
            log::warn!("Unknown protocol for owner: {}", owner);
            return;
        };

        // Deserialize based on protocol
        let pool_state = match protocol {
            DexProtocol::RaydiumClmm => {
                match RaydiumClmmPoolState::try_from_slice(&account_info.data) {
                    Ok(state) => DexPoolState::RaydiumClmm(state),
                    Err(e) => {
                        log::error!("Failed to deserialize Raydium CLMM pool state: {}", e);
                        return;
                    }
                }
            }
            DexProtocol::OrcaWhirlpool => {
                match OrcaWhirlpoolState::try_from_slice(&account_info.data) {
                    Ok(state) => DexPoolState::OrcaWhirlpool(state),
                    Err(e) => {
                        log::error!("Failed to deserialize Orca Whirlpool state: {}", e);
                        return;
                    }
                }
            }
            DexProtocol::MeteoraDlmm => {
                match MeteoraDlmmPoolState::try_from_slice(&account_info.data) {
                    Ok(state) => DexPoolState::MeteoraDlmm(state),
                    Err(e) => {
                        log::error!("Failed to deserialize Meteora DLMM state: {}", e);
                        return;
                    }
                }
            }
            _ => {
                log::warn!("Pool state deserialization not implemented for {:?}", protocol);
                return;
            }
        };

        // Update cache
        self.state_cache.update(pubkey, pool_state.clone(), update.slot);

        log::info!(
            "Updated pool {} ({}) - Price: {:.6}, Liquidity: {}",
            pubkey,
            protocol.name(),
            pool_state.get_price(),
            pool_state.get_liquidity()
        );
    }

    /// Add a pool to monitor
    pub fn add_pool(&mut self, pubkey: Pubkey) {
        if !self.config.pool_pubkeys.contains(&pubkey) {
            self.config.pool_pubkeys.push(pubkey);
        }
    }

    /// Get the state cache
    pub fn state_cache(&self) -> Arc<PoolStateCache> {
        self.state_cache.clone()
    }
}
