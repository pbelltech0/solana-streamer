use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::{Pubkey, PUBKEY_BYTES},
};

/// Lending market state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LendingMarket {
    /// Version of the struct
    pub version: u8,
    /// Bump seed for derived authority address
    pub bump_seed: u8,
    /// Owner authority which can add new reserves
    pub owner: Pubkey,
    /// Quote currency
    pub quote_currency: [u8; 32],
}

impl LendingMarket {
    /// Create a new lending market
    pub fn new(params: InitLendingMarketParams) -> Self {
        Self {
            version: 1,
            bump_seed: params.bump_seed,
            owner: params.owner,
            quote_currency: params.quote_currency,
        }
    }
}

/// Initialize lending market params
pub struct InitLendingMarketParams {
    /// Bump seed for derived authority address
    pub bump_seed: u8,
    /// Owner authority
    pub owner: Pubkey,
    /// Quote currency
    pub quote_currency: [u8; 32],
}

impl Sealed for LendingMarket {}

impl IsInitialized for LendingMarket {
    fn is_initialized(&self) -> bool {
        self.version != 0
    }
}

const LENDING_MARKET_LEN: usize = 66; // 1 + 1 + 32 + 32

impl Pack for LendingMarket {
    const LEN: usize = LENDING_MARKET_LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, LENDING_MARKET_LEN];
        let (version, bump_seed, owner, quote_currency) = mut_array_refs![output, 1, 1, 32, 32];

        version[0] = self.version;
        bump_seed[0] = self.bump_seed;
        owner.copy_from_slice(self.owner.as_ref());
        quote_currency.copy_from_slice(&self.quote_currency);
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, LENDING_MARKET_LEN];
        let (version, bump_seed, owner, quote_currency) = array_refs![input, 1, 1, 32, 32];

        Ok(Self {
            version: version[0],
            bump_seed: bump_seed[0],
            owner: Pubkey::new_from_array(*owner),
            quote_currency: *quote_currency,
        })
    }
}

/// Reserve state
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reserve {
    /// Version of the struct
    pub version: u8,
    /// Lending market address
    pub lending_market: Pubkey,
    /// Reserve liquidity
    pub liquidity: ReserveLiquidity,
    /// Reserve configuration
    pub config: ReserveConfig,
}

/// Reserve liquidity
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveLiquidity {
    /// Reserve liquidity mint address
    pub mint_pubkey: Pubkey,
    /// Reserve liquidity supply address
    pub supply_pubkey: Pubkey,
    /// Reserve liquidity available
    pub available_amount: u64,
}

/// Reserve configuration values
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReserveConfig {
    /// Flash loan fee rate (bps)
    pub flash_loan_fee_bps: u64,
    /// Protocol fee (percentage of flash loan fee)
    pub protocol_flash_loan_fee_bps: u64,
}

impl Reserve {
    /// Calculate flash loan fees
    pub fn calculate_flash_loan_fees(&self, amount: u64) -> Result<FlashLoanFees, ProgramError> {
        let total_fee = amount
            .checked_mul(self.config.flash_loan_fee_bps)
            .and_then(|v| v.checked_div(10000))
            .ok_or(ProgramError::InvalidArgument)?;

        let protocol_fee = total_fee
            .checked_mul(self.config.protocol_flash_loan_fee_bps)
            .and_then(|v| v.checked_div(10000))
            .ok_or(ProgramError::InvalidArgument)?;

        let host_fee = total_fee
            .checked_sub(protocol_fee)
            .ok_or(ProgramError::InvalidArgument)?;

        Ok(FlashLoanFees {
            total_fee,
            protocol_fee,
            host_fee,
        })
    }
}

/// Flash loan fees breakdown
pub struct FlashLoanFees {
    /// Total fee
    pub total_fee: u64,
    /// Protocol fee
    pub protocol_fee: u64,
    /// Host fee
    pub host_fee: u64,
}

impl Sealed for Reserve {}

impl IsInitialized for Reserve {
    fn is_initialized(&self) -> bool {
        self.version != 0
    }
}

const RESERVE_LEN: usize = 233; // 1 + 32 + (32 + 32 + 8) + (8 + 8) + padding

impl Pack for Reserve {
    const LEN: usize = RESERVE_LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let output = array_mut_ref![dst, 0, RESERVE_LEN];
        let (
            version,
            lending_market,
            liquidity_mint,
            liquidity_supply,
            liquidity_available,
            flash_loan_fee_bps,
            protocol_flash_loan_fee_bps,
            _padding,
        ) = mut_array_refs![output, 1, 32, 32, 32, 8, 8, 8, 112];

        version[0] = self.version;
        lending_market.copy_from_slice(self.lending_market.as_ref());
        liquidity_mint.copy_from_slice(self.liquidity.mint_pubkey.as_ref());
        liquidity_supply.copy_from_slice(self.liquidity.supply_pubkey.as_ref());
        *liquidity_available = self.liquidity.available_amount.to_le_bytes();
        *flash_loan_fee_bps = self.config.flash_loan_fee_bps.to_le_bytes();
        *protocol_flash_loan_fee_bps = self.config.protocol_flash_loan_fee_bps.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let input = array_ref![src, 0, RESERVE_LEN];
        let (
            version,
            lending_market,
            liquidity_mint,
            liquidity_supply,
            liquidity_available,
            flash_loan_fee_bps,
            protocol_flash_loan_fee_bps,
            _padding,
        ) = array_refs![input, 1, 32, 32, 32, 8, 8, 8, 112];

        Ok(Self {
            version: version[0],
            lending_market: Pubkey::new_from_array(*lending_market),
            liquidity: ReserveLiquidity {
                mint_pubkey: Pubkey::new_from_array(*liquidity_mint),
                supply_pubkey: Pubkey::new_from_array(*liquidity_supply),
                available_amount: u64::from_le_bytes(*liquidity_available),
            },
            config: ReserveConfig {
                flash_loan_fee_bps: u64::from_le_bytes(*flash_loan_fee_bps),
                protocol_flash_loan_fee_bps: u64::from_le_bytes(*protocol_flash_loan_fee_bps),
            },
        })
    }
}