/// Flash loan integration modules for automated arbitrage
///
/// This module provides infrastructure for:
/// - Detecting arbitrage opportunities from streaming events
/// - Building and submitting flash loan transactions
/// - Executing profitable trades atomically

pub mod opportunity_detector;
pub mod transaction_builder;

pub use opportunity_detector::{OpportunityDetector, ArbitrageOpportunity, PoolProtocol};
pub use transaction_builder::{FlashLoanTxBuilder, SimulationResult};