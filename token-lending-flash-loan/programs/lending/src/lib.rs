#![deny(missing_docs)]
#![forbid(unsafe_code)]

//! SPL Token Lending Flash Loan Program
//!
//! This program implements flash loans according to the SPL Token Lending specification.
//! Flash loans allow users to borrow assets without collateral as long as the loan is
//! repaid within the same transaction.

/// Program error types
pub mod error;
/// Instruction types and builders
pub mod instruction;
/// Instruction processing logic
pub mod processor;
/// State account structures
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
pub mod entrypoint;

// Export current SDK types for downstream users
pub use solana_program;

solana_program::declare_id!("F1ashLending11111111111111111111111111111111");