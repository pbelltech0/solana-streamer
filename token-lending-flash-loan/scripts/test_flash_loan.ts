/**
 * Test Flash Loan Client Script
 *
 * This script demonstrates how to execute a flash loan transaction
 * on devnet/testnet/localnet.
 *
 * Prerequisites:
 * 1. Run setup_devnet.sh to deploy programs
 * 2. Have .env.[network] file with configuration
 * 3. Install dependencies: npm install @solana/web3.js @solana/spl-token dotenv
 *
 * Usage:
 *   ts-node scripts/test_flash_loan.ts [devnet|testnet|localnet]
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import * as fs from 'fs';
import * as dotenv from 'dotenv';

// Network configuration
const network = process.argv[2] || 'devnet';
const envFile = `.env.${network}`;

if (!fs.existsSync(envFile)) {
  console.error(`Configuration file ${envFile} not found!`);
  console.error('Run ./scripts/setup_devnet.sh first');
  process.exit(1);
}

dotenv.config({ path: envFile });

// Load configuration
const NETWORK = process.env.NETWORK!;
const LENDING_PROGRAM = new PublicKey(process.env.LENDING_PROGRAM!);
const RECEIVER_PROGRAM = new PublicKey(process.env.RECEIVER_PROGRAM!);
const TEST_MINT = new PublicKey(process.env.TEST_MINT!);
const SUPPLY_ACCOUNT = new PublicKey(process.env.SUPPLY_ACCOUNT!);
const BORROWER_ACCOUNT = new PublicKey(process.env.BORROWER_ACCOUNT!);
const FEE_RECEIVER = new PublicKey(process.env.FEE_RECEIVER!);

// Network RPC endpoint
const RPC_ENDPOINTS: { [key: string]: string } = {
  devnet: 'https://api.devnet.solana.com',
  testnet: 'https://api.testnet.solana.com',
  localnet: 'http://localhost:8899',
};

const connection = new Connection(RPC_ENDPOINTS[NETWORK], 'confirmed');

// Load wallet
const wallet = Keypair.fromSecretKey(
  Uint8Array.from(
    JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, 'utf-8'))
  )
);

console.log('=================================');
console.log('Flash Loan Test Client');
console.log('=================================');
console.log('');
console.log(`Network:         ${NETWORK}`);
console.log(`Wallet:          ${wallet.publicKey.toBase58()}`);
console.log(`Lending Program: ${LENDING_PROGRAM.toBase58()}`);
console.log(`Receiver:        ${RECEIVER_PROGRAM.toBase58()}`);
console.log('');

/**
 * Create Flash Loan instruction
 */
function createFlashLoanInstruction(
  amount: bigint,
  lendingProgram: PublicKey,
  sourceLiquidity: PublicKey,
  destinationLiquidity: PublicKey,
  reserve: PublicKey,
  lendingMarket: PublicKey,
  lendingMarketAuthority: PublicKey,
  receiverProgram: PublicKey,
  feeReceiver: PublicKey,
  receiverAccounts: Array<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>
): TransactionInstruction {
  // Instruction data: [tag(12), amount(u64)]
  const data = Buffer.alloc(9);
  data.writeUInt8(12, 0); // FlashLoan instruction tag
  data.writeBigUInt64LE(amount, 1);

  const keys = [
    { pubkey: sourceLiquidity, isSigner: false, isWritable: true },
    { pubkey: destinationLiquidity, isSigner: false, isWritable: true },
    { pubkey: reserve, isSigner: false, isWritable: true },
    { pubkey: lendingMarket, isSigner: false, isWritable: false },
    { pubkey: lendingMarketAuthority, isSigner: false, isWritable: false },
    { pubkey: receiverProgram, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: feeReceiver, isSigner: false, isWritable: true },
    ...receiverAccounts,
  ];

  return new TransactionInstruction({
    keys,
    programId: lendingProgram,
    data,
  });
}

/**
 * Execute flash loan test
 */
async function testFlashLoan() {
  try {
    console.log('Step 1: Check balances...');
    const balance = await connection.getBalance(wallet.publicKey);
    console.log(`Wallet SOL balance: ${balance / LAMPORTS_PER_SOL}`);

    // Note: You'll need to create and initialize these accounts first
    // This is a placeholder showing the structure
    const lendingMarket = Keypair.generate(); // Replace with actual initialized account
    const reserve = Keypair.generate(); // Replace with actual initialized account

    // Derive lending market authority
    const [lendingMarketAuthority, bump] = await PublicKey.findProgramAddress(
      [lendingMarket.publicKey.toBuffer()],
      LENDING_PROGRAM
    );

    console.log(`Lending Market Authority: ${lendingMarketAuthority.toBase58()} (bump: ${bump})`);
    console.log('');

    console.log('Step 2: Building flash loan transaction...');

    // Flash loan amount: 100 tokens (6 decimals)
    const flashLoanAmount = BigInt(100_000_000);

    // Receiver program accounts
    const receiverAccounts = [
      { pubkey: BORROWER_ACCOUNT, isSigner: false, isWritable: true },
      { pubkey: SUPPLY_ACCOUNT, isSigner: false, isWritable: true },
      { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ];

    const flashLoanIx = createFlashLoanInstruction(
      flashLoanAmount,
      LENDING_PROGRAM,
      SUPPLY_ACCOUNT,
      BORROWER_ACCOUNT,
      reserve.publicKey,
      lendingMarket.publicKey,
      lendingMarketAuthority,
      RECEIVER_PROGRAM,
      FEE_RECEIVER,
      receiverAccounts
    );

    const transaction = new Transaction().add(flashLoanIx);

    console.log('Step 3: Sending transaction...');
    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [wallet],
      {
        commitment: 'confirmed',
      }
    );

    console.log('');
    console.log('✓ Flash loan executed successfully!');
    console.log(`Transaction signature: ${signature}`);
    console.log('');

    if (NETWORK === 'devnet') {
      console.log(`View on explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);
    } else if (NETWORK === 'testnet') {
      console.log(`View on explorer: https://explorer.solana.com/tx/${signature}?cluster=testnet`);
    }

    console.log('');
    console.log('Step 4: Verifying results...');

    // Check logs
    const txDetails = await connection.getTransaction(signature, {
      commitment: 'confirmed',
    });

    if (txDetails?.meta?.logMessages) {
      console.log('Transaction logs:');
      txDetails.meta.logMessages.forEach(log => console.log(`  ${log}`));
    }

  } catch (error) {
    console.error('');
    console.error('❌ Flash loan failed!');
    console.error(error);

    if (error instanceof Error) {
      console.error('Error details:', error.message);
    }

    process.exit(1);
  }
}

// Main execution
console.log('⚠️  Note: This script requires initialized lending market and reserve accounts');
console.log('See TESTING.md for setup instructions');
console.log('');

// Uncomment to run:
// testFlashLoan();

console.log('Script loaded. To execute, uncomment the testFlashLoan() call at the end of the file.');
console.log('Make sure you have initialized the lending market and reserve first!');