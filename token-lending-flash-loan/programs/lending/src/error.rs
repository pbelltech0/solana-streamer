use solana_program::program_error::ProgramError;
use thiserror::Error;

/// Errors that may be returned by the token lending program
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LendingError {
    /// Invalid instruction
    #[error("Invalid instruction")]
    InvalidInstruction,

    /// Insufficient liquidity
    #[error("Insufficient liquidity available")]
    InsufficientLiquidity,

    /// Invalid amount
    #[error("Invalid amount")]
    InvalidAmount,

    /// Flash loan not repaid
    #[error("Flash loan was not fully repaid")]
    FlashLoanNotRepaid,

    /// Invalid account owner
    #[error("Invalid account owner")]
    InvalidAccountOwner,

    /// Invalid account data
    #[error("Invalid account data")]
    InvalidAccountData,

    /// Math overflow
    #[error("Math overflow")]
    MathOverflow,

    /// Invalid reserve
    #[error("Invalid reserve")]
    InvalidReserve,

    /// Invalid lending market
    #[error("Invalid lending market")]
    InvalidLendingMarket,
}

impl From<LendingError> for ProgramError {
    fn from(e: LendingError) -> Self {
        ProgramError::Custom(e as u32)
    }
}