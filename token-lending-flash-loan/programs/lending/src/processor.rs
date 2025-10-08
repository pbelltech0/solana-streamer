use crate::{
    error::LendingError,
    instruction::LendingInstruction,
    state::{LendingMarket, Reserve},
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
};
use spl_token::instruction as token_instruction;

/// Instruction processor
pub struct Processor;

impl Processor {
    /// Process an instruction
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = LendingInstruction::unpack(instruction_data)?;

        match instruction {
            LendingInstruction::FlashLoan { amount } => {
                msg!("Instruction: FlashLoan");
                Self::process_flash_loan(program_id, amount, accounts)
            }
        }
    }

    /// Process FlashLoan instruction
    fn process_flash_loan(
        program_id: &Pubkey,
        amount: u64,
        accounts: &[AccountInfo],
    ) -> ProgramResult {
        if amount == 0 {
            return Err(LendingError::InvalidAmount.into());
        }

        let account_info_iter = &mut accounts.iter();

        // Account 0: Source liquidity (reserve supply)
        let source_liquidity_info = next_account_info(account_info_iter)?;
        // Account 1: Destination liquidity (borrower account)
        let destination_liquidity_info = next_account_info(account_info_iter)?;
        // Account 2: Reserve
        let reserve_info = next_account_info(account_info_iter)?;
        // Account 3: Lending market
        let lending_market_info = next_account_info(account_info_iter)?;
        // Account 4: Lending market authority
        let lending_market_authority_info = next_account_info(account_info_iter)?;
        // Account 5: Flash loan receiver program
        let flash_loan_receiver_program_info = next_account_info(account_info_iter)?;
        // Account 6: Token program
        let token_program_info = next_account_info(account_info_iter)?;
        // Account 7: Flash loan fee receiver
        let flash_loan_fee_receiver_info = next_account_info(account_info_iter)?;
        // Account 8: Host fee receiver (optional)
        let host_fee_receiver_info = next_account_info(account_info_iter).ok();

        // Validate accounts
        if reserve_info.owner != program_id {
            return Err(LendingError::InvalidAccountOwner.into());
        }

        if lending_market_info.owner != program_id {
            return Err(LendingError::InvalidAccountOwner.into());
        }

        // Load and validate reserve
        let mut reserve = Reserve::unpack(&reserve_info.data.borrow())?;
        if reserve.lending_market != *lending_market_info.key {
            return Err(LendingError::InvalidReserve.into());
        }

        // Load lending market to get authority bump seed
        let lending_market = LendingMarket::unpack(&lending_market_info.data.borrow())?;

        // Verify lending market authority is correct
        let lending_market_authority_seeds = &[
            lending_market_info.key.as_ref(),
            &[lending_market.bump_seed],
        ];
        let expected_authority = Pubkey::create_program_address(
            lending_market_authority_seeds,
            program_id,
        )?;

        if expected_authority != *lending_market_authority_info.key {
            return Err(LendingError::InvalidLendingMarket.into());
        }

        // Check available liquidity
        if reserve.liquidity.available_amount < amount {
            return Err(LendingError::InsufficientLiquidity.into());
        }

        // Calculate fees
        let fees = reserve.calculate_flash_loan_fees(amount)?;
        let repay_amount = amount
            .checked_add(fees.total_fee)
            .ok_or(LendingError::MathOverflow)?;

        msg!("Flash loan: amount={}, fee={}, repay={}", amount, fees.total_fee, repay_amount);

        // Get initial balance of source liquidity
        let source_liquidity_account = spl_token::state::Account::unpack(
            &source_liquidity_info.data.borrow()
        )?;
        let initial_balance = source_liquidity_account.amount;

        // Step 1: Transfer loan amount to destination
        msg!("Transferring {} tokens to borrower", amount);
        invoke_signed(
            &token_instruction::transfer(
                token_program_info.key,
                source_liquidity_info.key,
                destination_liquidity_info.key,
                lending_market_authority_info.key,
                &[],
                amount,
            )?,
            &[
                source_liquidity_info.clone(),
                destination_liquidity_info.clone(),
                lending_market_authority_info.clone(),
                token_program_info.clone(),
            ],
            &[lending_market_authority_seeds],
        )?;

        // Update reserve liquidity
        reserve.liquidity.available_amount = reserve
            .liquidity
            .available_amount
            .checked_sub(amount)
            .ok_or(LendingError::MathOverflow)?;

        // Step 2: Call receiver program's ReceiveFlashLoan instruction
        msg!("Calling flash loan receiver program");

        // Build instruction data for receiver: [0, amount_bytes]
        let mut receiver_instruction_data = vec![0u8]; // Tag 0 for ReceiveFlashLoan
        receiver_instruction_data.extend_from_slice(&amount.to_le_bytes());

        // Build receiver instruction accounts (pass through remaining accounts)
        let mut receiver_accounts = vec![
            // First account should be the destination liquidity account
            destination_liquidity_info.clone(),
        ];

        // Add all remaining accounts for receiver program
        for account_info in account_info_iter {
            receiver_accounts.push(account_info.clone());
        }

        invoke(
            &solana_program::instruction::Instruction {
                program_id: *flash_loan_receiver_program_info.key,
                accounts: receiver_accounts
                    .iter()
                    .map(|acc| solana_program::instruction::AccountMeta {
                        pubkey: *acc.key,
                        is_signer: acc.is_signer,
                        is_writable: acc.is_writable,
                    })
                    .collect(),
                data: receiver_instruction_data,
            },
            &receiver_accounts,
        )?;

        msg!("Flash loan receiver returned");

        // Step 3: Verify repayment
        // Reload source liquidity account to check balance
        source_liquidity_info.data.borrow_mut();
        let final_source_account = spl_token::state::Account::unpack(
            &source_liquidity_info.data.borrow()
        )?;
        let final_balance = final_source_account.amount;

        let expected_balance = initial_balance
            .checked_add(fees.total_fee)
            .ok_or(LendingError::MathOverflow)?;

        if final_balance < expected_balance {
            msg!(
                "Flash loan not repaid! Expected: {}, Got: {}",
                expected_balance,
                final_balance
            );
            return Err(LendingError::FlashLoanNotRepaid.into());
        }

        // Update reserve liquidity with repaid amount + fees
        reserve.liquidity.available_amount = reserve
            .liquidity
            .available_amount
            .checked_add(amount)
            .and_then(|v| v.checked_add(fees.total_fee))
            .ok_or(LendingError::MathOverflow)?;

        // Step 4: Distribute fees
        // Transfer protocol fee to fee receiver
        if fees.protocol_fee > 0 {
            msg!("Transferring protocol fee: {}", fees.protocol_fee);
            invoke_signed(
                &token_instruction::transfer(
                    token_program_info.key,
                    source_liquidity_info.key,
                    flash_loan_fee_receiver_info.key,
                    lending_market_authority_info.key,
                    &[],
                    fees.protocol_fee,
                )?,
                &[
                    source_liquidity_info.clone(),
                    flash_loan_fee_receiver_info.clone(),
                    lending_market_authority_info.clone(),
                    token_program_info.clone(),
                ],
                &[lending_market_authority_seeds],
            )?;

            // Reduce available liquidity by protocol fee
            reserve.liquidity.available_amount = reserve
                .liquidity
                .available_amount
                .checked_sub(fees.protocol_fee)
                .ok_or(LendingError::MathOverflow)?;
        }

        // Transfer host fee if host fee receiver is provided
        if let Some(host_fee_receiver) = host_fee_receiver_info {
            if fees.host_fee > 0 {
                msg!("Transferring host fee: {}", fees.host_fee);
                invoke_signed(
                    &token_instruction::transfer(
                        token_program_info.key,
                        source_liquidity_info.key,
                        host_fee_receiver.key,
                        lending_market_authority_info.key,
                        &[],
                        fees.host_fee,
                    )?,
                    &[
                        source_liquidity_info.clone(),
                        host_fee_receiver.clone(),
                        lending_market_authority_info.clone(),
                        token_program_info.clone(),
                    ],
                    &[lending_market_authority_seeds],
                )?;

                // Reduce available liquidity by host fee
                reserve.liquidity.available_amount = reserve
                    .liquidity
                    .available_amount
                    .checked_sub(fees.host_fee)
                    .ok_or(LendingError::MathOverflow)?;
            }
        }

        // Save updated reserve
        Reserve::pack(reserve, &mut reserve_info.data.borrow_mut())?;

        msg!("Flash loan completed successfully");
        Ok(())
    }
}