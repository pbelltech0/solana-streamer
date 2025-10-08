#![deny(missing_docs)]
#![forbid(unsafe_code)]

//! Example Flash Loan Receiver Program
//!
//! This program demonstrates how to implement the ReceiveFlashLoan interface
//! according to the SPL Token Lending flash loan specification.
//!
//! The receiver program must:
//! 1. Implement an instruction with tag 0 (ReceiveFlashLoan)
//! 2. Accept the loan amount as a parameter
//! 3. Perform user-defined operations with the borrowed funds
//! 4. Ensure the full loan amount plus fees is returned to the reserve

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::invoke,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use spl_token::instruction as token_instruction;

solana_program::declare_id!("F1ashReceiver1111111111111111111111111111111");

entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse instruction
    let (&instruction_tag, rest) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    // Instruction tag must be 0 for ReceiveFlashLoan
    if instruction_tag != 0 {
        msg!("Error: Invalid instruction tag. Expected 0, got {}", instruction_tag);
        return Err(ProgramError::InvalidInstructionData);
    }

    // Parse amount (u64 = 8 bytes)
    let amount_bytes: [u8; 8] = rest
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(ProgramError::InvalidInstructionData)?;
    let amount = u64::from_le_bytes(amount_bytes);

    msg!("ReceiveFlashLoan called with amount: {}", amount);

    // Process the flash loan
    process_receive_flash_loan(program_id, accounts, amount)
}

/// Process ReceiveFlashLoan instruction
///
/// This is where you implement your custom logic with the borrowed funds.
/// In this example, we simply verify we received the tokens and prepare
/// them for repayment. In a real implementation, you would:
/// - Execute arbitrage trades
/// - Perform liquidations
/// - Refinance positions
/// - Or any other profitable operation
fn process_receive_flash_loan(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    // Account 0: Token account that received the flash loan
    let token_account_info = next_account_info(account_info_iter)?;

    // Account 1: Source liquidity (reserve) - needed for repayment
    let source_liquidity_info = next_account_info(account_info_iter)?;

    // Account 2: Authority for token account
    let authority_info = next_account_info(account_info_iter)?;

    // Account 3: Token program
    let token_program_info = next_account_info(account_info_iter)?;

    // Verify we received the tokens
    let token_account = spl_token::state::Account::unpack(&token_account_info.data.borrow())?;
    msg!("Token account balance: {}", token_account.amount);

    if token_account.amount < amount {
        msg!("Error: Insufficient tokens received");
        return Err(ProgramError::InsufficientFunds);
    }

    // ========================================
    // YOUR CUSTOM LOGIC GOES HERE
    // ========================================
    //
    // This is where you would:
    // 1. Execute trades, arbitrage, liquidations, etc.
    // 2. Use the borrowed funds to make profit
    // 3. Ensure you end up with enough tokens to repay the loan + fees
    //
    // Example operations:
    // - Call DEX programs to swap tokens
    // - Call lending programs to liquidate positions
    // - Call other DeFi protocols
    //
    // For this example, we'll just log that we received the funds
    msg!("Executing custom flash loan logic...");
    msg!("In a real implementation, perform profitable operations here");

    // Example: Calculate expected repayment (loan + 0.09% fee)
    let fee = amount
        .checked_mul(9)
        .and_then(|v| v.checked_div(10000))
        .ok_or(ProgramError::InvalidArgument)?;
    let repay_amount = amount
        .checked_add(fee)
        .ok_or(ProgramError::InvalidArgument)?;

    msg!("Expected repayment: {} (amount: {}, fee: {})", repay_amount, amount, fee);

    // ========================================
    // REPAYMENT
    // ========================================
    //
    // CRITICAL: You must repay the loan + fees back to the source liquidity account
    // The lending program will verify this after we return
    //
    // In this example, we're just returning the borrowed amount.
    // In a real implementation, your custom logic above must generate enough
    // profit to cover the fees, so you'll have repay_amount in your token account.

    msg!("Repaying flash loan: {} tokens", repay_amount);

    // Transfer tokens back to reserve
    invoke(
        &token_instruction::transfer(
            token_program_info.key,
            token_account_info.key,
            source_liquidity_info.key,
            authority_info.key,
            &[],
            repay_amount,
        )?,
        &[
            token_account_info.clone(),
            source_liquidity_info.clone(),
            authority_info.clone(),
            token_program_info.clone(),
        ],
    )?;

    msg!("Flash loan repaid successfully");
    Ok(())
}