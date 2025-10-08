use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::convert::TryInto;
use std::mem::size_of;

/// Instructions supported by the token lending program
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum LendingInstruction {
    /// Flash Loan
    ///
    /// Takes a flash loan from the reserve liquidity supply. The loan must be repaid
    /// with fees in the same transaction, or the entire transaction will fail.
    ///
    /// Accounts expected by this instruction:
    ///
    /// 0. `[writable]` Source liquidity token account - liquidity supply
    /// 1. `[writable]` Destination liquidity token account - receiver's account
    /// 2. `[writable]` Reserve account
    /// 3. `[]` Lending market account
    /// 4. `[]` Derived lending market authority
    /// 5. `[]` Flash loan receiver program account
    /// 6. `[]` Token program id
    /// 7. `[writable]` Flash loan fee receiver account
    /// 8. `[writable]` Host fee receiver account (optional)
    /// 9+ `[]` Additional accounts expected by the receiver program
    FlashLoan {
        /// The amount to borrow
        amount: u64,
    },
}

impl LendingInstruction {
    /// Unpacks a byte buffer into a LendingInstruction
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        Ok(match tag {
            12 => {
                let amount = rest
                    .get(..8)
                    .and_then(|slice| slice.try_into().ok())
                    .map(u64::from_le_bytes)
                    .ok_or(ProgramError::InvalidInstructionData)?;
                Self::FlashLoan { amount }
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    /// Packs a LendingInstruction into a byte buffer
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match self {
            Self::FlashLoan { amount } => {
                buf.push(12);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates a FlashLoan instruction
#[allow(clippy::too_many_arguments)]
pub fn flash_loan(
    program_id: Pubkey,
    amount: u64,
    source_liquidity: Pubkey,
    destination_liquidity: Pubkey,
    reserve: Pubkey,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    flash_loan_receiver_program: Pubkey,
    token_program_id: Pubkey,
    flash_loan_fee_receiver: Pubkey,
    host_fee_receiver: Option<Pubkey>,
    receiver_program_accounts: Vec<AccountMeta>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(source_liquidity, false),
        AccountMeta::new(destination_liquidity, false),
        AccountMeta::new(reserve, false),
        AccountMeta::new_readonly(lending_market, false),
        AccountMeta::new_readonly(lending_market_authority, false),
        AccountMeta::new_readonly(flash_loan_receiver_program, false),
        AccountMeta::new_readonly(token_program_id, false),
        AccountMeta::new(flash_loan_fee_receiver, false),
    ];

    if let Some(host_fee_receiver) = host_fee_receiver {
        accounts.push(AccountMeta::new(host_fee_receiver, false));
    }

    accounts.extend(receiver_program_accounts);

    let data = LendingInstruction::FlashLoan { amount }.pack();

    Instruction {
        program_id,
        accounts,
        data,
    }
}