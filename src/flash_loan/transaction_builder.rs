use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use anyhow::Result;

use crate::flash_loan::opportunity_detector::ArbitrageOpportunity;

/// Simulation result showing what would happen in a flash loan
#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub would_succeed: bool,
    pub loan_amount: u64,
    pub expected_profit: u64,
    pub flash_loan_fee: u64,
    pub swap_fees: u64,
    pub total_fees: u64,
    pub net_profit: u64,
    pub pool_a: Pubkey,
    pub pool_b: Pubkey,
    pub reason: String,
}

/// Builds and submits flash loan transactions
pub struct FlashLoanTxBuilder {
    client: RpcClient,
    payer: Keypair,
    flash_loan_receiver_program: Pubkey,
    simulation_mode: bool,
}

impl FlashLoanTxBuilder {
    pub fn new(
        rpc_url: String,
        payer: Keypair,
        flash_loan_receiver_program: Pubkey,
    ) -> Self {
        Self {
            client: RpcClient::new(rpc_url),
            payer,
            flash_loan_receiver_program,
            simulation_mode: false,
        }
    }

    /// Create a builder in simulation mode (no actual transactions)
    pub fn new_simulation_mode(
        rpc_url: String,
        payer: Keypair,
        flash_loan_receiver_program: Pubkey,
    ) -> Self {
        Self {
            client: RpcClient::new(rpc_url),
            payer,
            flash_loan_receiver_program,
            simulation_mode: true,
        }
    }

    /// Enable or disable simulation mode
    pub fn set_simulation_mode(&mut self, enabled: bool) {
        self.simulation_mode = enabled;
    }

    /// Check if simulation mode is enabled
    pub fn is_simulation_mode(&self) -> bool {
        self.simulation_mode
    }

    /// Simulate flash loan execution without submitting transaction
    ///
    /// Arbitrage flow (borrowing quote token / token1):
    /// 1. Borrow loan_amount of token1 (flash loan fee: 0.09%)
    /// 2. Buy token0 at Pool A (low price): spend token1 → get token0 (swap fee: 0.25%)
    /// 3. Sell token0 at Pool B (high price): sell token0 → get token1 (swap fee: 0.25%)
    /// 4. Net received: loan * (1 - 0.0025)² * (price_b / price_a)
    /// 5. Must repay: loan * (1 + 0.0009)
    /// 6. Profit: received - repayment
    pub fn simulate_flash_loan_detailed(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> SimulationResult {
        let loan_amount = opportunity.loan_amount;

        // Fee constants
        const FLASH_LOAN_FEE_RATE: f64 = 0.0009; // 0.09% Solend
        const SWAP_FEE_RATE: f64 = 0.0025;       // 0.25% per swap

        // Calculate individual fees for reporting
        let flash_loan_fee = (loan_amount as f64 * FLASH_LOAN_FEE_RATE) as u64;

        // Calculate net amount after swap fees
        // After two swaps: (1 - 0.0025)² = 0.99500625
        let swap_fee_multiplier = (1.0 - SWAP_FEE_RATE) * (1.0 - SWAP_FEE_RATE);

        // Total swap fees (implicit in the calculation)
        // = loan - loan * 0.99500625 * (price_b / price_a) when converted back
        let swap_fee_a = (loan_amount as f64 * SWAP_FEE_RATE) as u64;
        let token0_amount = (loan_amount as f64 * (1.0 - SWAP_FEE_RATE)) / opportunity.price_a;
        let swap_fee_b = (token0_amount * SWAP_FEE_RATE) as u64;
        let swap_fees = swap_fee_a + swap_fee_b;
        let total_fees = flash_loan_fee + swap_fees;

        // Price spread
        let price_spread = opportunity.price_b - opportunity.price_a;
        let price_spread_pct = price_spread / opportunity.price_a;

        // Price multiplier for arbitrage
        let price_multiplier = opportunity.price_b / opportunity.price_a;

        // Net token1 received after both swaps
        let net_received = loan_amount as f64 * swap_fee_multiplier * price_multiplier;

        // Amount to repay (loan + flash loan fee)
        let repayment = loan_amount as f64 * (1.0 + FLASH_LOAN_FEE_RATE);

        // Gross profit (before subtracting repayment)
        let gross_profit = net_received;

        // Net profit after all fees
        let net_profit_f64 = net_received - repayment;

        let (net_profit, would_succeed, reason) = if net_profit_f64 > 0.0 {
            (
                net_profit_f64 as u64,
                true,
                format!(
                    "Profitable! Spread: {:.2}%, Received: {:.0} lamports, Repay: {:.0} lamports, Net: {:.0} lamports",
                    price_spread_pct * 100.0,
                    net_received,
                    repayment,
                    net_profit_f64
                ),
            )
        } else {
            (
                0,
                false,
                format!(
                    "Not profitable. Received {:.0} < Repayment {:.0}",
                    net_received, repayment
                ),
            )
        };

        SimulationResult {
            would_succeed,
            loan_amount,
            expected_profit: gross_profit as u64,
            flash_loan_fee,
            swap_fees,
            total_fees,
            net_profit,
            pool_a: opportunity.pool_a,
            pool_b: opportunity.pool_b,
            reason,
        }
    }

    /// Build and submit flash loan transaction (or simulate if in simulation mode)
    pub async fn execute_flash_loan(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<Signature> {
        if self.simulation_mode {
            log::info!("🧪 SIMULATION MODE - No transaction will be submitted");
            let sim = self.simulate_flash_loan_detailed(opportunity);
            log::info!("Simulation result: {:?}", sim);

            return Err(anyhow::anyhow!(
                "Simulation mode enabled. To execute real transactions, disable simulation mode."
            ));
        }

        // 1. Build flash loan instruction (from Solend)
        let flash_loan_ix = self.build_solend_flash_loan_instruction(opportunity)?;

        // 2. Get recent blockhash
        let recent_blockhash = self.client.get_latest_blockhash()?;

        // 3. Create transaction
        let tx = Transaction::new_signed_with_payer(
            &[flash_loan_ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        // 4. Submit transaction
        let signature = self.client.send_and_confirm_transaction(&tx)?;

        Ok(signature)
    }

    /// Build Solend flash loan instruction
    ///
    /// Note: This is a placeholder implementation. The actual Solend flash loan
    /// instruction requires:
    /// 1. Proper account ordering (source liquidity, destination, receiver program, etc.)
    /// 2. Correct instruction data encoding
    /// 3. All required Solend program accounts
    ///
    /// Reference: https://github.com/solendprotocol/solana-program-library
    fn build_solend_flash_loan_instruction(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<Instruction> {
        // Solend flash loan instruction format
        let solend_program_id = solana_sdk::pubkey!("So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo");

        // TODO: Build actual Solend flash loan instruction
        // This requires:
        // 1. Solend reserve account (liquidity source)
        // 2. Your receiver program ID
        // 3. Loan amount
        // 4. All required accounts
        //
        // The instruction data typically includes:
        // - Instruction discriminator (flash loan variant)
        // - Amount to borrow
        // - Optional parameters

        log::warn!("⚠️  Solend flash loan instruction builder is a placeholder");
        log::info!(
            "Opportunity details: pool_a={}, pool_b={}, loan_amount={}, expected_profit={}",
            opportunity.pool_a,
            opportunity.pool_b,
            opportunity.loan_amount,
            opportunity.expected_profit
        );

        // Placeholder instruction structure
        Ok(Instruction {
            program_id: solend_program_id,
            accounts: vec![
                // TODO: Add Solend accounts:
                // - Lending market
                // - Reserve
                // - Reserve liquidity supply
                // - Reserve collateral mint
                // - Receiver token account
                // - Flash loan receiver program (yours)
                // - Host fee receiver
                // - Token program
            ],
            data: vec![
                // TODO: Encode flash loan instruction data
                // Typically includes:
                // - Instruction tag
                // - Amount to borrow
            ],
        })
    }

    /// Simulate transaction before submission
    ///
    /// This is crucial for flash loans to ensure the arbitrage will be profitable
    /// before consuming gas fees
    pub async fn simulate_flash_loan(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<bool> {
        let flash_loan_ix = self.build_solend_flash_loan_instruction(opportunity)?;
        let recent_blockhash = self.client.get_latest_blockhash()?;

        let tx = Transaction::new_signed_with_payer(
            &[flash_loan_ix],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            recent_blockhash,
        );

        // Simulate the transaction
        match self.client.simulate_transaction(&tx) {
            Ok(response) => {
                if response.value.err.is_none() {
                    log::info!("✅ Simulation successful");
                    Ok(true)
                } else {
                    log::warn!("❌ Simulation failed: {:?}", response.value.err);
                    Ok(false)
                }
            }
            Err(e) => {
                log::error!("Simulation error: {}", e);
                Err(e.into())
            }
        }
    }

    /// Get the payer's public key
    pub fn payer_pubkey(&self) -> Pubkey {
        self.payer.pubkey()
    }

    /// Get the flash loan receiver program ID
    pub fn receiver_program_id(&self) -> Pubkey {
        self.flash_loan_receiver_program
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_builder_creation() {
        let keypair = Keypair::new();
        let receiver_program = Pubkey::new_unique();

        let builder = FlashLoanTxBuilder::new(
            "https://api.mainnet-beta.solana.com".to_string(),
            keypair,
            receiver_program,
        );

        assert_eq!(builder.receiver_program_id(), receiver_program);
    }
}