//! Market Streaming - DEX Pool State Monitoring via Yellowstone gRPC
//!
//! This library provides real-time monitoring of DEX pool states across multiple
//! protocols using Yellowstone gRPC streaming.
//!
//! # Supported DEX Protocols
//! - Raydium CLMM
//! - Orca Whirlpool
//! - Meteora DLMM
//!
//! # Example Usage
//!
//! ```no_run
//! use market_streaming::prelude::*;
//! use solana_sdk::pubkey::Pubkey;
//! use std::str::FromStr;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let state_cache = Arc::new(PoolStateCache::new());
//!
//!     let config = StreamConfig {
//!         grpc_endpoint: "https://grpc.mainnet.solana.tools:443".to_string(),
//!         auth_token: None,
//!         pool_pubkeys: vec![
//!             Pubkey::from_str("...")?,
//!         ],
//!         protocols: vec![
//!             DexProtocol::RaydiumClmm,
//!             DexProtocol::OrcaWhirlpool,
//!         ],
//!         commitment: yellowstone_grpc_proto::prelude::CommitmentLevel::Processed,
//!     };
//!
//!     let client = PoolStreamClient::new(config, state_cache.clone());
//!     client.start().await?;
//!
//!     Ok(())
//! }
//! ```

pub mod pool_states;
pub mod state_cache;
pub mod stream_client;
pub mod ws_client;

// Re-export commonly used types
pub use pool_states::{
    DexPoolState, DexProtocol, MeteoraDlmmPoolState, OrcaWhirlpoolState, RaydiumClmmPoolState,
};
pub use state_cache::{CachedPoolState, CacheStats, PoolStateCache};
pub use stream_client::{PoolStreamClient, StreamConfig};
pub use ws_client::{WsPoolStreamClient, WsStreamConfig};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::pool_states::{
        DexPoolState, DexProtocol, MeteoraDlmmPoolState, OrcaWhirlpoolState,
        RaydiumClmmPoolState,
    };
    pub use crate::state_cache::{CachedPoolState, CacheStats, PoolStateCache};
    pub use crate::stream_client::{PoolStreamClient, StreamConfig};
    pub use crate::ws_client::{WsPoolStreamClient, WsStreamConfig};
}
