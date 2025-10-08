use crate::pool_states::{DexPoolState, DexProtocol, OrcaWhirlpoolState, RaydiumClmmPoolState, MeteoraDlmmPoolState};
use crate::state_cache::PoolStateCache;
use anyhow::{Context, Result};
use borsh::BorshDeserialize;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// WebSocket message types for Helius Enhanced WebSocket
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscribeRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<SubscribeParams>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscribeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    account_include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    account_exclude: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commitment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction_details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    show_rewards: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_supported_transaction_version: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct WsResponse {
    jsonrpc: String,
    method: Option<String>,
    params: Option<Value>,
}

/// Configuration for WebSocket streaming
#[derive(Clone, Debug)]
pub struct WsStreamConfig {
    /// WebSocket endpoint URL
    pub wss_endpoint: String,
    /// RPC endpoint URL
    pub rpc_endpoint: String,
    /// List of pool pubkeys to monitor
    pub pool_pubkeys: Vec<Pubkey>,
    /// List of DEX protocols to monitor
    pub protocols: Vec<DexProtocol>,
    /// Commitment level
    pub commitment: String,
}

impl Default for WsStreamConfig {
    fn default() -> Self {
        Self {
            wss_endpoint: "wss://atlas-mainnet.helius-rpc.com/?api-key=YOUR_KEY".to_string(),
            rpc_endpoint: "https://mainnet.helius-rpc.com/?api-key=YOUR_KEY".to_string(),
            pool_pubkeys: Vec::new(),
            protocols: vec![
                DexProtocol::RaydiumClmm,
                DexProtocol::OrcaWhirlpool,
                DexProtocol::MeteoraDlmm,
            ],
            commitment: "confirmed".to_string(),
        }
    }
}

/// WebSocket client for monitoring DEX pool state changes
pub struct WsPoolStreamClient {
    pub config: WsStreamConfig,
    state_cache: Arc<PoolStateCache>,
}

impl WsPoolStreamClient {
    /// Create a new WebSocket stream client
    pub fn new(config: WsStreamConfig, state_cache: Arc<PoolStateCache>) -> Self {
        Self {
            config,
            state_cache,
        }
    }

    /// Start streaming pool account updates via WebSocket
    pub async fn start(&self) -> Result<()> {
        let url = &self.config.wss_endpoint;

        log::info!(
            "Connecting to WebSocket endpoint: {}",
            url.split("api-key=").next().unwrap_or(url)
        );

        // Connect to WebSocket
        let (ws_stream, response) = match connect_async(url).await {
            Ok(result) => result,
            Err(e) => {
                log::error!("WebSocket connection failed: {}", e);
                return Err(anyhow::anyhow!("Failed to connect to WebSocket endpoint: {}", e));
            }
        };

        log::info!("WebSocket connected successfully: {:?}", response.status());

        let (mut write, mut read) = ws_stream.split();

        // Build subscription for account updates
        let pool_addresses: Vec<String> = self.config.pool_pubkeys
            .iter()
            .map(|p| p.to_string())
            .collect();

        // Subscribe to account updates for all pools
        if !pool_addresses.is_empty() {
            let subscribe_msg = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "accountSubscribe",
                "params": [
                    pool_addresses[0], // Subscribe to first pool (WebSocket limitation)
                    {
                        "encoding": "base64",
                        "commitment": self.config.commitment
                    }
                ]
            });

            write.send(Message::Text(subscribe_msg.to_string())).await?;
            log::info!("Subscribed to account updates for {} pools", pool_addresses.len());
        }

        // Subscribe to logs for program updates (Helius uses logsSubscribe)
        let program_ids: Vec<String> = self.config.protocols
            .iter()
            .map(|p| p.program_id().to_string())
            .collect();

        // Subscribe to each program's logs separately
        for (idx, program_id) in program_ids.iter().enumerate() {
            let logs_msg = json!({
                "jsonrpc": "2.0",
                "id": 2 + idx as u64,
                "method": "logsSubscribe",
                "params": [
                    {
                        "mentions": [program_id]
                    },
                    {
                        "commitment": self.config.commitment
                    }
                ]
            });

            write.send(Message::Text(logs_msg.to_string())).await?;
        }

        log::info!("Subscribed to logs for {} DEX protocols", self.config.protocols.len());

        // Process incoming messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    self.process_message(&text).await;
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Message::Close(_)) => {
                    log::info!("WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket error: {:?}", e);
                    break;
                }
                _ => {}
            }
        }

        // Attempt to reconnect after a delay
        log::info!("WebSocket disconnected, attempting to reconnect in 5 seconds...");
        sleep(Duration::from_secs(5)).await;
        Box::pin(self.start()).await
    }

    /// Process a WebSocket message
    async fn process_message(&self, text: &str) {
        match serde_json::from_str::<Value>(text) {
            Ok(msg) => {
                // Log all messages for debugging
                if msg.get("id").is_some() {
                    log::debug!("Received subscription response: {:?}", msg);
                }

                // Handle account update notifications
                if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    log::info!("Received notification: {}", method);
                    match method {
                        "accountNotification" => {
                            if let Some(params) = msg.get("params") {
                                self.process_account_update(params).await;
                            }
                        }
                        "logsNotification" => {
                            if let Some(params) = msg.get("params") {
                                log::info!("Processing logs notification");
                                self.process_transaction_update(params).await;
                            }
                        }
                        _ => {
                            log::warn!("Unknown notification method: {}", method);
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to parse WebSocket message: {} - Message: {}", e, text);
            }
        }
    }

    /// Process account update notification
    async fn process_account_update(&self, params: &Value) {
        if let Some(result) = params.get("result") {
            if let Some(value) = result.get("value") {
                // Get the account data
                if let Some(data_str) = value.get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.get(0))
                    .and_then(|s| s.as_str())
                {
                    // Decode base64 data
                    match base64::decode(data_str) {
                        Ok(data) => {
                            // Get the slot
                            let slot = result.get("context")
                                .and_then(|c| c.get("slot"))
                                .and_then(|s| s.as_u64())
                                .unwrap_or(0);

                            // Try to identify which pool this is
                            for pubkey in &self.config.pool_pubkeys {
                                // Try to deserialize as each pool type
                                if let Some(pool_state) = self.try_deserialize_pool(&data) {
                                    self.state_cache.update(*pubkey, pool_state.clone(), slot);

                                    log::info!(
                                        "Updated pool {} - Price: {:.6}, Liquidity: {}",
                                        pubkey,
                                        pool_state.get_price(),
                                        pool_state.get_liquidity()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to decode base64 data: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Process transaction update notification
    async fn process_transaction_update(&self, _params: &Value) {
        // Transaction processing would go here
        // This is more complex with WebSockets and might need additional parsing
        log::debug!("Received transaction update");
    }

    /// Try to deserialize pool data as various pool types
    fn try_deserialize_pool(&self, data: &[u8]) -> Option<DexPoolState> {
        // Try Raydium CLMM
        if let Ok(state) = RaydiumClmmPoolState::try_from_slice(data) {
            return Some(DexPoolState::RaydiumClmm(state));
        }

        // Try Orca Whirlpool
        if let Ok(state) = OrcaWhirlpoolState::try_from_slice(data) {
            return Some(DexPoolState::OrcaWhirlpool(state));
        }

        // Try Meteora DLMM
        if let Ok(state) = MeteoraDlmmPoolState::try_from_slice(data) {
            return Some(DexPoolState::MeteoraDlmm(state));
        }

        None
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

// Add base64 module
mod base64 {
    pub fn decode(input: &str) -> Result<Vec<u8>, String> {
        use base64::{engine::general_purpose, Engine as _};
        general_purpose::STANDARD
            .decode(input)
            .map_err(|e| e.to_string())
    }
}