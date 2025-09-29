# Flash Loan Arbitrage Guide for Raydium CLMM

## Table of Contents
1. [Flash Loan Protocols on Solana](#flash-loan-protocols-on-solana)
2. [Flash Loan Arbitrage Strategy for CLMM](#flash-loan-arbitrage-strategy-for-clmm)
3. [CLMM-Specific Opportunities](#clmm-specific-flash-loan-opportunities)
4. [Optimal Flash Loan Size Calculation](#optimal-flash-loan-size-calculation)
5. [Implementation Examples](#implementation-with-kamino-flash-loans)
6. [Profitability Analysis](#key-constraints-for-profitability)
7. [Risk Management](#risk-management)
8. [Monitoring Strategy](#monitoring-strategy)

---

## Flash Loan Protocols on Solana

### Primary Providers

#### 1. Kamino Finance (K-Lend) ⭐ Recommended
- **Fee**: 0.001% (1 basis point)
- **Supported Assets**: All major tokens
- **Documentation**: Best documented API
- **SDK**: `@kamino-finance/klend-sdk`
- **Website**: https://kamino.finance

#### 2. Marginfi (mrgnlend)
- **Fee**: Variable
- **Features**: Flash loan capability built-in
- **SDK**: `@mrgnlabs/marginfi-client-v2`
- **Documentation**: Good TypeScript SDK
- **Use Case**: Supports leveraged positions

#### 3. Solend
- **Status**: Legacy option
- **Development**: Less active development
- **SDK**: `@solendprotocol/solend-sdk`

---

## Flash Loan Arbitrage Strategy for CLMM

### Core Concept
Execute atomic transactions that **borrow → arbitrage → repay** in a single transaction with zero upfront capital.

### Strategy Workflow

```rust
// Pseudo-code for flash loan arbitrage
Transaction {
    1. Begin Flash Loan (borrow 100 SOL from Kamino)
    2. Swap on Raydium CLMM Pool A (100 SOL → USDC at price X)
    3. Swap on Orca/Jupiter (USDC → SOL at price Y where Y > X)
    4. Repay Flash Loan (100 SOL + 0.001% fee)
    5. Keep profit (if any)
}
// All instructions must succeed atomically or entire tx reverts
```

### Key Principle
**Atomicity**: If any step fails, the entire transaction reverts. You never lose the borrowed capital.

---

## CLMM-Specific Flash Loan Opportunities

### 1. Cross-Pool Arbitrage

Monitor tick array data to detect:
- **Price discrepancies** between Raydium CLMM fee tiers (0.01% vs 0.25% vs 1%)
- **Liquidity gaps** where large swaps move price significantly

#### Example from Your Stream Data

```json
{
  "pool_id": "AQAGYQsdU853WAKhXM79CgNdoyhrRwXvYHX6qrDyC1FS",
  "ticks": [
    {
      "tick": -10740,
      "liquidity_gross": 5966659855
    },
    {
      "tick": -10200,
      "liquidity_gross": 224521837412
    }
  ]
}
```

**Gap Detection Logic:**
```javascript
// Detect liquidity gaps
const liquidityGap = Math.abs(tick1.liquidity_gross - tick2.liquidity_gross);
const averageLiquidity = (tick1.liquidity_gross + tick2.liquidity_gross) / 2;

if (liquidityGap / averageLiquidity > 0.5) {
    // Large gap detected - potential arbitrage opportunity
    const optimalFlashLoanSize = calculateOptimalSize(liquidityDistribution);
}
```

### 2. Tick Boundary Arbitrage

**Opportunity**: When price crosses tick boundaries
- LPs going out-of-range stop earning fees
- Creates temporary liquidity vacuum
- **Strategy**: Flash loan to provide JIT (Just-In-Time) liquidity for large swaps

**Detection Method:**
```typescript
// Monitor price approaching tick boundaries
if (Math.abs(currentPrice - tickBoundary) < threshold) {
    // Price about to cross boundary
    // LPs will go out of range
    // Flash loan opportunity for JIT liquidity
}
```

### 3. Fee Tier Arbitrage

Same token pair across different fee tiers:

```
SOL/USDC 0.05% pool: Price = $160.00
SOL/USDC 0.25% pool: Price = $160.15
Spread: $0.15 per SOL
```

**Flash Loan Calculation:**
```typescript
// Borrow 1000 SOL (0.001% fee = 0.01 SOL)
// Buy on 0.05% pool: -1000 SOL, +160,000 USDC (fee: 80 USDC = $80)
// Sell on 0.25% pool: +1001.01 SOL, -160,150 USDC (fee: 400 USDC = $400)
// Repay 1000.01 SOL
// Net: $150 spread - $480 fees = -$330 (loss)
// Conclusion: Only profitable if spread > 0.3%
```

### 4. Oracle vs Pool Price Arbitrage

**From Your Stream:**
```json
{
  "protocol": "RaydiumClmm",
  "program_id": "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK"
}
```

**Strategy:**
1. Compare pool price with Pyth/Switchboard oracles
2. Flash loan when: `|pool_price - oracle_price| > flash_loan_fee + swap_fees`
3. Arbitrage the difference before market corrects

**Oracle Integration:**
```typescript
import { PythSolanaReceiver } from "@pythnetwork/pyth-solana-receiver";

async function detectOracleArbitrage(poolPrice: number, assetMint: PublicKey) {
    const pythPrice = await pythClient.getPrice(assetMint);
    const priceDivergence = Math.abs(poolPrice - pythPrice.price);
    const divergencePercent = (priceDivergence / pythPrice.price) * 100;

    // Minimum 0.5% divergence to cover all fees
    if (divergencePercent > 0.5) {
        return {
            opportunity: true,
            expectedProfit: calculateProfit(priceDivergence),
            flashLoanSize: calculateOptimalSize(priceDivergence)
        };
    }
    return { opportunity: false };
}
```

---

## Optimal Flash Loan Size Calculation

### Formula Based on Tick Array Liquidity Data

```typescript
function calculateOptimalFlashLoanSize(tickArrayData) {
    // Extract liquidity from your stream
    const activeLiquidity = tickArrayData.ticks
        .filter(t => t.liquidity_gross > 0)
        .reduce((sum, t) => sum + t.liquidity_gross, 0);

    // Optimal size = size that maximizes (profit - slippage - fees)
    // Typically 1-5% of active liquidity to minimize price impact
    const optimalSize = activeLiquidity * 0.03;

    // Factor in flash loan fee (0.001% for Kamino)
    const flashLoanFee = optimalSize * 0.00001;

    // Calculate slippage based on liquidity distribution
    const expectedSlippage = calculateSlippage(optimalSize, tickArrayData);

    // Must exceed minimum profit threshold
    const swapFees = optimalSize * 0.0025; // 0.25% typical fee
    const networkFees = 0.000005 * 4; // ~4 signatures
    const minProfitThreshold = flashLoanFee + swapFees + networkFees + expectedSlippage;

    return {
        optimalSize,
        minProfitThreshold,
        expectedSlippage,
        fees: {
            flashLoan: flashLoanFee,
            swap: swapFees,
            network: networkFees
        }
    };
}

function calculateSlippage(amount, tickArrayData) {
    // Price impact calculation across ticks
    let remainingAmount = amount;
    let totalSlippage = 0;

    for (const tick of tickArrayData.ticks.filter(t => t.liquidity_gross > 0)) {
        const tickLiquidity = tick.liquidity_gross;
        const amountInTick = Math.min(remainingAmount, tickLiquidity);

        // Simplified price impact formula
        const priceImpact = (amountInTick / tickLiquidity) * 0.01; // 1% per full liquidity usage
        totalSlippage += priceImpact * amountInTick;

        remainingAmount -= amountInTick;
        if (remainingAmount <= 0) break;
    }

    return totalSlippage;
}
```

### Example Calculation

From your stream data:
```
Pool ID: AQAGYQsdU853WAKhXM79CgNdoyhrRwXvYHX6qrDyC1FS
Total Active Liquidity: 230,488,497,267 (5.97B + 224.5B)

Optimal Flash Loan Size: 230,488,497,267 * 0.03 = 6,914,654,918
Flash Loan Fee (0.001%): 69,147
Expected Slippage (~0.1%): 6,914,655
Swap Fees (0.25%): 17,286,637
Network Fees: 0.00002 SOL

Minimum Profitable Spread: 24,270,439 units (~0.35%)
```

---

## Implementation with Kamino Flash Loans

### TypeScript Example

```typescript
import { MarginfiAccountWrapper, MarginfiClient } from "@mrgnlabs/marginfi-client-v2";
import { Connection, PublicKey, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { AnchorProvider } from "@coral-xyz/anchor";

async function executeClmmArbitrage(
    marginfiClient: MarginfiClient,
    marginfiAccount: MarginfiAccountWrapper,
    poolA: PublicKey,
    poolB: PublicKey,
    flashLoanAmount: number,
    wallet: any
) {
    try {
        // 1. Calculate end index for flash loan
        const endIndex = 4; // Number of instructions before repayment

        // 2. Create flash loan begin instruction
        const beginFlashLoan = await marginfiAccount.makeBeginFlashLoanIx(endIndex);

        // 3. Build Raydium CLMM swap instruction (Pool A)
        const swapIxA = await buildRaydiumClmmSwapIx({
            pool: poolA,
            amount: flashLoanAmount,
            direction: "SOL_to_USDC",
            minimumAmountOut: calculateMinimumOut(flashLoanAmount, 0.005), // 0.5% slippage
            wallet: wallet.publicKey
        });

        // 4. Build reverse swap instruction (Pool B or different DEX)
        const swapIxB = await buildSwapIx({
            pool: poolB,
            amount: "all", // Use all USDC from previous swap
            direction: "USDC_to_SOL",
            minimumAmountOut: flashLoanAmount * 1.003, // Must cover loan + profit
            wallet: wallet.publicKey
        });

        // 5. Get projected active balances for flash loan end
        const projectedBalances = await marginfiAccount.getActiveBalances();

        // 6. Create flash loan end instruction (repayment)
        const endFlashLoan = await marginfiAccount.makeEndFlashLoanIx(
            projectedBalances
        );

        // 7. Build atomic transaction
        const transaction = new Transaction();

        // Add compute budget (important for complex transactions)
        transaction.add(
            ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
            ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 50_000 })
        );

        // Add all instructions in order
        transaction.add(...beginFlashLoan.instructions);
        transaction.add(swapIxA);
        transaction.add(swapIxB);
        transaction.add(...endFlashLoan.instructions);

        // 8. Send transaction with high priority
        console.log("Executing flash loan arbitrage...");
        const signature = await sendAndConfirmTransaction(
            marginfiClient.provider.connection,
            transaction,
            [wallet],
            {
                commitment: 'confirmed',
                skipPreflight: false, // Set to true in production for speed
                preflightCommitment: 'confirmed'
            }
        );

        console.log("✅ Arbitrage successful!");
        console.log("Transaction signature:", signature);

        return { success: true, signature };

    } catch (error) {
        console.error("❌ Arbitrage failed:", error);
        // Transaction will automatically revert on failure
        return { success: false, error: error.message };
    }
}

// Helper function to build Raydium CLMM swap
async function buildRaydiumClmmSwapIx(params: {
    pool: PublicKey,
    amount: number,
    direction: string,
    minimumAmountOut: number,
    wallet: PublicKey
}) {
    // Use Raydium SDK to build swap instruction
    // This is a simplified version - use actual Raydium SDK

    const { SwapUtils } = require("@raydium-io/raydium-sdk-v2");

    return await SwapUtils.makeSwapInstruction({
        poolAddress: params.pool,
        userSourceToken: /* ... */,
        userDestinationToken: /* ... */,
        amountIn: params.amount,
        minimumAmountOut: params.minimumAmountOut,
        owner: params.wallet
    });
}

function calculateMinimumOut(amountIn: number, slippageTolerance: number): number {
    return amountIn * (1 - slippageTolerance);
}
```

### Complete Setup Example

```typescript
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { MarginfiClient } from "@mrgnlabs/marginfi-client-v2";

async function setupFlashLoanArbitrage() {
    // 1. Setup connection
    const connection = new Connection("https://mainnet.helius-rpc.com/?api-key=YOUR_KEY", 'confirmed');

    // 2. Setup wallet
    const wallet = new Wallet(Keypair.fromSecretKey(/* your secret key */));

    // 3. Create provider
    const provider = new AnchorProvider(connection, wallet, {
        commitment: 'confirmed',
        preflightCommitment: 'confirmed'
    });

    // 4. Initialize Marginfi client
    const marginfiClient = await MarginfiClient.fetch(
        { environment: "production" },
        wallet,
        connection
    );

    // 5. Get or create Marginfi account
    let marginfiAccount = await marginfiClient.getMarginfiAccount();

    if (!marginfiAccount) {
        console.log("Creating new Marginfi account...");
        marginfiAccount = await marginfiClient.createMarginfiAccount();
    }

    // 6. Define pools for arbitrage
    const raydiumPool = new PublicKey("AQAGYQsdU853WAKhXM79CgNdoyhrRwXvYHX6qrDyC1FS");
    const orcaPool = new PublicKey("ORCA_POOL_ADDRESS");

    // 7. Execute arbitrage
    const result = await executeClmmArbitrage(
        marginfiClient,
        marginfiAccount,
        raydiumPool,
        orcaPool,
        1000, // Flash loan 1000 SOL
        wallet
    );

    return result;
}

// Run the arbitrage
setupFlashLoanArbitrage()
    .then(result => console.log("Result:", result))
    .catch(err => console.error("Error:", err));
```

---

## Advanced: Multi-Hop Flash Loan Arbitrage

### Triangle Arbitrage Strategy

```typescript
async function triangleArbitrage(
    marginfiAccount: MarginfiAccountWrapper,
    amount: number
) {
    const transaction = new Transaction();

    // Begin flash loan
    const beginFlash = await marginfiAccount.makeBeginFlashLoanIx(6);
    transaction.add(...beginFlash.instructions);

    // Hop 1: SOL → USDC (Raydium CLMM)
    const hop1 = await buildRaydiumClmmSwapIx({
        inputMint: SOL_MINT,
        outputMint: USDC_MINT,
        amount: amount
    });
    transaction.add(hop1);

    // Hop 2: USDC → BTC (Orca Whirlpool)
    const hop2 = await buildOrcaSwapIx({
        inputMint: USDC_MINT,
        outputMint: BTC_MINT,
        amount: "all"
    });
    transaction.add(hop2);

    // Hop 3: BTC → SOL (Jupiter aggregator for best price)
    const hop3 = await buildJupiterSwapIx({
        inputMint: BTC_MINT,
        outputMint: SOL_MINT,
        amount: "all",
        minimumAmountOut: amount * 1.005 // Must profit 0.5%
    });
    transaction.add(hop3);

    // End flash loan (repay)
    const endFlash = await marginfiAccount.makeEndFlashLoanIx(
        await marginfiAccount.getActiveBalances()
    );
    transaction.add(...endFlash.instructions);

    return transaction;
}
```

---

## Key Constraints for Profitability

### Profitability Formula

```
Profit > Flash_Loan_Fee + Swap_Fees + Network_Fees + Slippage + MEV_Cost

Where:
- Flash_Loan_Fee = 0.001% (Kamino)
- Swap_Fees = 0.05% to 2% (depends on CLMM fee tier)
- Network_Fees = ~0.00001 SOL per signature (~$0.002)
- Slippage = f(loan_size, liquidity_distribution)
- MEV_Cost = Jito bundle tip (optional, 0.0001-0.001 SOL)

Minimum Spread Needed ≈ 0.3% for most opportunities
```

### Detailed Cost Breakdown

| Component | Typical Cost | Impact |
|-----------|-------------|--------|
| Flash Loan Fee (Kamino) | 0.001% | $1 per $1M borrowed |
| CLMM Swap Fee (0.25%) | 0.25% | $2,500 per $1M swap |
| Slippage | 0.05-0.5% | $500-$5,000 per $1M |
| Network Fees | 0.00002 SOL | ~$0.004 per tx |
| Priority Fees | 0.0001 SOL | ~$0.02 per tx |
| Jito Bundle (optional) | 0.0001 SOL | ~$0.02 per tx |
| **Total** | **~0.35-0.8%** | **$3,500-$8,000 per $1M** |

### Break-Even Analysis

For a **1000 SOL ($160,000) flash loan**:

```
Revenue needed to break even:
- Flash loan fee: 0.01 SOL ($1.60)
- Swap fees (two swaps @ 0.25%): 5 SOL ($800)
- Slippage (0.1%): 1 SOL ($160)
- Network fees: 0.00002 SOL ($0.003)
- Priority fees: 0.0001 SOL ($0.016)

Total cost: 6.01002 SOL ($961.60)
Minimum price spread needed: 0.6% to break even
Comfortable profit margin: 1%+ spread
```

---

## Risk Management

### 1. Transaction Failure Risk
**Risk**: Any instruction in the atomic transaction fails
**Mitigation**:
- ✅ Entire transaction reverts automatically
- ✅ No loss of capital (borrowed funds are returned)
- ✅ Only lose network fees (~$0.004)

**Best Practices**:
```typescript
// Always use simulation before sending
const simulation = await connection.simulateTransaction(transaction);
if (simulation.value.err) {
    console.log("Simulation failed, skipping execution");
    return;
}
```

### 2. MEV Competition Risk
**Risk**: Other bots front-run your arbitrage
**Mitigation**:
- Use Jito bundles for MEV protection
- Submit transactions directly to validators
- Increase priority fees during high competition

**Jito Integration**:
```typescript
import { searcherClient } from "jito-ts-sdk";

async function sendWithJito(transaction: Transaction) {
    const jitoClient = searcherClient("mainnet");

    // Add tip instruction
    const tipAccount = getRandomTipAccount();
    transaction.add(
        SystemProgram.transfer({
            fromPubkey: wallet.publicKey,
            toPubkey: tipAccount,
            lamports: 10_000 // 0.00001 SOL tip
        })
    );

    // Send as bundle
    const bundle = await jitoClient.sendBundle([transaction]);
    return bundle;
}
```

### 3. Slippage Risk
**Risk**: Price moves unfavorably during execution
**Mitigation**:
- Set `minimumAmountOut` on all swaps
- Use tight slippage tolerance (0.5-1%)
- Monitor liquidity depth before executing

```typescript
const minAmountOut = expectedAmount * (1 - SLIPPAGE_TOLERANCE);

const swapIx = await buildSwapIx({
    amount: inputAmount,
    minimumAmountOut: minAmountOut, // Transaction fails if not met
    slippageBps: 50 // 0.5%
});
```

### 4. Network Congestion Risk
**Risk**: Transaction times out or gets dropped
**Mitigation**:
- Use dynamic priority fees
- Monitor network congestion
- Retry with higher fees if needed

```typescript
import { getRecentPrioritizationFees } from "@solana/web3.js";

async function getDynamicPriorityFee(connection: Connection): Promise<number> {
    const fees = await connection.getRecentPrioritizationFees();
    const avgFee = fees.reduce((a, b) => a + b.prioritizationFee, 0) / fees.length;

    // Use 2x average during congestion
    return Math.max(avgFee * 2, 10_000); // Minimum 10k micro-lamports
}
```

### 5. Oracle Lag Risk
**Risk**: Oracle prices lag behind actual pool prices
**Mitigation**:
- Use multiple oracle sources
- Compare Pyth + Switchboard
- Add safety buffer to price checks

```typescript
async function getConsensusPri(assetMint: PublicKey) {
    const pythPrice = await pythClient.getPrice(assetMint);
    const switchboardPrice = await switchboardClient.getPrice(assetMint);

    // Use more conservative price
    const consensusPrice = Math.min(pythPrice, switchboardPrice);

    // Add 0.1% safety buffer
    return consensusPrice * 0.999;
}
```

---

## Monitoring Strategy

### Real-Time Opportunity Detection

#### 1. Enhance Your Yellowstone gRPC Stream

```rust
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::*;

async fn subscribe_to_arbitrage_opportunities() -> Result<()> {
    let mut client = GeyserGrpcClient::connect(endpoint, None, None).await?;

    let mut accounts_filter = HashMap::new();

    // Subscribe to CLMM pool state accounts
    accounts_filter.insert(
        "raydium_clmm_pools".to_string(),
        SubscribeRequestFilterAccounts {
            account: vec![],
            owner: vec![RAYDIUM_CLMM_PROGRAM_ID.to_string()],
            filters: vec![
                SubscribeRequestFilterAccountsFilter {
                    memcmp: Some(SubscribeRequestFilterAccountsFilterMemcmp {
                        offset: 0,
                        data: Some(SubscribeRequestFilterAccountsDataSize::DataSize(1544)),
                    }),
                }
            ],
        },
    );

    // Subscribe to swap transactions
    let mut transactions_filter = HashMap::new();
    transactions_filter.insert(
        "clmm_swaps".to_string(),
        SubscribeRequestFilterTransactions {
            vote: Some(false),
            failed: Some(false),
            account_include: vec![RAYDIUM_CLMM_PROGRAM_ID.to_string()],
            account_exclude: vec![],
            account_required: vec![],
        },
    );

    let request = SubscribeRequest {
        accounts: accounts_filter,
        slots: HashMap::new(),
        transactions: transactions_filter,
        blocks: HashMap::new(),
        blocks_meta: HashMap::new(),
        entry: HashMap::new(),
        commitment: Some(CommitmentLevel::Confirmed as i32),
        accounts_data_slice: vec![],
        ping: None,
    };

    let mut stream = client.subscribe(request).await?;

    while let Some(message) = stream.next().await {
        match message {
            Ok(update) => process_update_for_arbitrage(update).await?,
            Err(e) => eprintln!("Stream error: {:?}", e),
        }
    }

    Ok(())
}
```

#### 2. Process Updates for Arbitrage Detection

```typescript
import { EventEmitter } from 'events';

class ArbitrageMonitor extends EventEmitter {
    private pools: Map<string, PoolState> = new Map();
    private priceOracles: Map<string, OraclePrice> = new Map();

    async processAccountUpdate(update: AccountUpdate) {
        const poolId = update.pubkey;
        const poolState = this.decodePoolState(update.data);

        // Update internal pool state
        this.pools.set(poolId, poolState);

        // Calculate current pool price
        const poolPrice = this.calculatePrice(poolState);

        // Get oracle price
        const oraclePrice = await this.getOraclePrice(poolState.token_mint);

        // Check for arbitrage opportunity
        const opportunity = this.detectArbitrage(poolPrice, oraclePrice, poolState);

        if (opportunity) {
            this.emit('opportunity', {
                poolId,
                poolPrice,
                oraclePrice,
                spread: opportunity.spread,
                expectedProfit: opportunity.profit,
                flashLoanSize: opportunity.optimalSize
            });
        }
    }

    detectArbitrage(poolPrice: number, oraclePrice: number, poolState: PoolState) {
        const spread = Math.abs(poolPrice - oraclePrice) / oraclePrice;

        // Minimum 0.5% spread to cover costs
        if (spread < 0.005) return null;

        // Calculate optimal flash loan size
        const optimalSize = this.calculateOptimalSize(poolState, spread);

        // Calculate expected profit
        const costs = this.calculateCosts(optimalSize);
        const revenue = optimalSize * spread;
        const profit = revenue - costs;

        if (profit > 0) {
            return {
                spread: spread * 100, // Convert to percentage
                profit,
                optimalSize,
                costs
            };
        }

        return null;
    }

    calculatePrice(poolState: PoolState): number {
        // Convert sqrt_price_x64 to actual price
        const Q64 = 2n ** 64n;
        const sqrtPrice = BigInt(poolState.sqrt_price_x64);
        const price = (sqrtPrice * sqrtPrice) / Q64;
        return Number(price) / (10 ** poolState.token_decimals);
    }

    calculateOptimalSize(poolState: PoolState, spread: number): number {
        // Use your liquidity data to calculate optimal size
        const totalLiquidity = poolState.liquidity;

        // Optimal is typically 2-5% of liquidity
        const baseSize = totalLiquidity * 0.03;

        // Adjust based on spread (higher spread = larger size)
        const spreadMultiplier = Math.min(spread / 0.01, 2);

        return baseSize * spreadMultiplier;
    }

    calculateCosts(flashLoanSize: number): number {
        const flashLoanFee = flashLoanSize * 0.00001; // 0.001%
        const swapFees = flashLoanSize * 0.005; // 0.5% total (two swaps)
        const slippage = flashLoanSize * 0.001; // 0.1% estimated
        const networkFees = 0.00002; // ~0.00002 SOL

        return flashLoanFee + swapFees + slippage + networkFees;
    }
}

// Usage
const monitor = new ArbitrageMonitor();

monitor.on('opportunity', async (opp) => {
    console.log('🎯 Arbitrage Opportunity Detected!');
    console.log(`Pool: ${opp.poolId}`);
    console.log(`Spread: ${opp.spread.toFixed(2)}%`);
    console.log(`Expected Profit: ${opp.expectedProfit.toFixed(4)} SOL`);
    console.log(`Optimal Flash Loan Size: ${opp.flashLoanSize.toFixed(2)} SOL`);

    // Execute flash loan arbitrage
    await executeFlashLoanArbitrage(opp);
});
```

#### 3. Multi-Pool Price Comparison

```typescript
class CrossPoolMonitor {
    private pools: Map<string, PoolData> = new Map();

    async monitorCrossPoolArbitrage() {
        // Get all pools for same token pair
        const solUsdcPools = this.getPoolsByPair('SOL', 'USDC');

        for (let i = 0; i < solUsdcPools.length; i++) {
            for (let j = i + 1; j < solUsdcPools.length; j++) {
                const poolA = solUsdcPools[i];
                const poolB = solUsdcPools[j];

                const priceA = poolA.price;
                const priceB = poolB.price;

                const spread = Math.abs(priceA - priceB) / Math.min(priceA, priceB);

                if (spread > 0.003) { // 0.3% minimum
                    const opportunity = {
                        poolA: poolA.id,
                        poolB: poolB.id,
                        priceA,
                        priceB,
                        spread: spread * 100,
                        buyPool: priceA < priceB ? poolA : poolB,
                        sellPool: priceA < priceB ? poolB : poolA
                    };

                    console.log('Cross-pool arbitrage:', opportunity);
                    await this.executeCrossPoolArbitrage(opportunity);
                }
            }
        }
    }

    getPoolsByPair(tokenA: string, tokenB: string): PoolData[] {
        return Array.from(this.pools.values()).filter(pool =>
            (pool.tokenA === tokenA && pool.tokenB === tokenB) ||
            (pool.tokenA === tokenB && pool.tokenB === tokenA)
        );
    }
}
```

### Performance Metrics to Track

```typescript
interface ArbitrageMetrics {
    // Opportunity metrics
    opportunitiesDetected: number;
    opportunitiesExecuted: number;
    opportunitiesMissed: number;
    averageSpread: number;

    // Execution metrics
    successRate: number;
    averageExecutionTime: number;
    averageProfit: number;
    totalProfit: number;

    // Cost metrics
    totalGasCost: number;
    totalFlashLoanFees: number;
    totalSlippage: number;

    // Competition metrics
    frontRunAttempts: number;
    mevProtectionCost: number;
}

class MetricsTracker {
    private metrics: ArbitrageMetrics;

    trackOpportunity(result: ArbitrageResult) {
        this.metrics.opportunitiesDetected++;

        if (result.executed) {
            this.metrics.opportunitiesExecuted++;
            this.metrics.totalProfit += result.profit;
            this.metrics.totalGasCost += result.gasCost;

            // Update averages
            this.updateAverages();
        } else {
            this.metrics.opportunitiesMissed++;
        }
    }

    printDailyReport() {
        console.log('📊 Daily Arbitrage Report');
        console.log('========================');
        console.log(`Opportunities Detected: ${this.metrics.opportunitiesDetected}`);
        console.log(`Opportunities Executed: ${this.metrics.opportunitiesExecuted}`);
        console.log(`Success Rate: ${this.metrics.successRate.toFixed(2)}%`);
        console.log(`Total Profit: ${this.metrics.totalProfit.toFixed(4)} SOL`);
        console.log(`Average Profit: ${this.metrics.averageProfit.toFixed(4)} SOL`);
        console.log(`Total Costs: ${this.metrics.totalGasCost.toFixed(4)} SOL`);
        console.log(`Net Profit: ${(this.metrics.totalProfit - this.metrics.totalGasCost).toFixed(4)} SOL`);
    }
}
```

---

## Profitability Reality Check

### Expected Returns

#### Capital Requirements (Even with Flash Loans)
- **Gas fees**: $0.01-0.10 per attempt
- **Failed transactions**: 50-80% failure rate initially
- **Daily costs**: $10-50 in gas fees alone

#### Competitive Landscape
- **Speed requirement**: Sub-100ms execution
- **Infrastructure**: Co-located nodes ($500-2000/month)
- **Jito bundles**: Additional $0.01-0.05 per successful arb
- **MEV competition**: 10-100+ bots competing for same opportunities

#### Realistic Profit Estimates

**Conservative Scenario** (Part-time bot, standard RPC):
- Opportunities detected: 20-50/day
- Successful executions: 2-5/day
- Average profit per execution: $5-20
- Daily costs: $20-30
- **Net daily profit: $-10 to $50**
- **Monthly: Break-even to $1,500**

**Optimistic Scenario** (Optimized bot, dedicated node, Jito):
- Opportunities detected: 100-200/day
- Successful executions: 20-40/day
- Average profit per execution: $10-30
- Daily costs: $50-100
- **Net daily profit: $150-500**
- **Monthly: $4,500-$15,000**

**Professional Scenario** (Multiple bots, validator connections, advanced strategies):
- Opportunities detected: 500+/day
- Successful executions: 100-200/day
- Average profit per execution: $15-50
- Daily costs: $200-500
- **Net daily profit: $1,000-3,000**
- **Monthly: $30,000-$90,000**

### Time to Profitability

| Stage | Timeline | Investment | Expected Return |
|-------|----------|-----------|-----------------|
| Development | 2-4 weeks | Time only | $0 |
| Testing (devnet) | 1-2 weeks | $0 | $0 |
| Initial deployment | 1-2 weeks | $100-500 | -$50 to $200 |
| Optimization | 1-3 months | $500-2000 | $500-3000/month |
| Professional operation | 3-6 months | $5000+ | $5000-20000/month |

### Success Factors

✅ **Required for profitability:**
1. Sub-100ms latency to Solana validators
2. Automated opportunity detection
3. Dynamic fee calculation
4. MEV protection (Jito bundles)
5. Multi-pool monitoring
6. Continuous optimization

❌ **Common failure points:**
1. High latency RPC endpoints
2. Static fee calculations
3. Single-pool monitoring
4. No MEV protection
5. Poor error handling
6. Lack of monitoring

---

## Next Steps

### 1. Start with Simulation
```bash
# Test on devnet first
npm install @solana/web3.js @mrgnlabs/marginfi-client-v2
node test-flash-loan-simulation.js
```

### 2. Monitor Opportunities (No Execution)
```bash
# Run passive monitoring to collect data
node monitor-arbitrage-opportunities.js --dry-run
```

### 3. Execute Small Test Trades
```bash
# Start with minimal amounts to test infrastructure
node execute-flash-loan.js --amount 0.1 --testnet
```

### 4. Scale Gradually
- Increase trade sizes as confidence grows
- Add more pools to monitoring
- Optimize execution speed
- Implement advanced strategies

---

## Resources

### Documentation
- Kamino Finance: https://docs.kamino.finance
- Marginfi: https://docs.marginfi.com
- Raydium SDK: https://github.com/raydium-io/raydium-sdk-v2
- Yellowstone gRPC: https://docs.helius.dev/grpc

### SDKs
```bash
npm install @kamino-finance/klend-sdk
npm install @mrgnlabs/marginfi-client-v2
npm install @raydium-io/raydium-sdk-v2
npm install @orca-so/whirlpools-sdk
npm install @jup-ag/api
```

### Community
- Raydium Discord: https://discord.gg/raydium
- Solana Stack Exchange: https://solana.stackexchange.com
- Yellowstone gRPC Examples: https://github.com/rpcpool/yellowstone-grpc

---

## Conclusion

Flash loan arbitrage on Raydium CLMM pools is **technically feasible but highly competitive**. Success requires:

1. ✅ **Low latency infrastructure** (co-located nodes)
2. ✅ **Real-time monitoring** (your Yellowstone gRPC stream)
3. ✅ **Atomic execution** (flash loans ensure no capital risk)
4. ✅ **MEV protection** (Jito bundles)
5. ✅ **Continuous optimization** (adapt to changing market)

**Recommended Starting Point:**
1. Use this guide to build a **monitoring system first**
2. Collect data on opportunities for 1-2 weeks
3. Analyze profitability with real data
4. Start with **small test executions** ($10-100)
5. Scale gradually based on success rate

Good luck with your arbitrage bot! 🚀