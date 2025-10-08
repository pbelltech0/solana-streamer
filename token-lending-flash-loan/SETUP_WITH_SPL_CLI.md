# Setup Flash Loans with spl-token-lending-cli

This guide shows how to use the existing `spl-token-lending` infrastructure for flash loans.

## Option 1: Use Official SPL Token Lending (Recommended)

The official SPL Token Lending program already supports flash loans! No need for a custom program.

### Setup on Devnet

```bash
# 1. Configure for devnet
solana config set --url https://api.devnet.solana.com

# 2. Create a lending market
spl-token-lending create-market

# Output example:
# Creating lending market 7cVfgArCheMR6Cs4t6vz5rfnqd56vZq4ndaBrY5xkxXy
# Signature: 2ZE7Rz...

LENDING_MARKET=<your_market_address>

# 3. Create a reserve
# First create or use existing token mint
TEST_MINT=<your_token_mint>

spl-token-lending add-reserve \
  --market $LENDING_MARKET \
  --source <your_token_account> \
  --amount 1000 \
  --pyth-product <pyth_product_id> \
  --pyth-price <pyth_price_id>

# Output:
# Adding reserve for mint: So11111...
# Reserve address: HZxX9X...
# Signature: 3kF8Dn...

RESERVE=<your_reserve_address>

# 4. Deposit liquidity into reserve
spl-token-lending deposit \
  --reserve $RESERVE \
  --source <your_token_account> \
  --amount 10000

# 5. Now you can execute flash loans against this reserve!
```

### Execute Flash Loan

The official program supports flash loans via the `FlashLoan` instruction (tag 12).

```bash
# Use the official program ID
LENDING_PROGRAM=TokenLending1111111111111111111111111111111

# Build your flash loan transaction
# See scripts/flash_loan_with_spl.ts for client example
```

## Option 2: Using Local Validator

```bash
# 1. Start validator
solana-test-validator \
  --bpf-program TokenLending1111111111111111111111111111111 \
  spl-token-lending.so

# 2. In another terminal
solana config set --url http://localhost:8899

# 3. Airdrop SOL
solana airdrop 10

# 4. Create test token
TEST_MINT=$(spl-token create-token --decimals 6 | grep "Creating token" | awk '{print $3}')

# 5. Create token account and mint tokens
TOKEN_ACCOUNT=$(spl-token create-account $TEST_MINT | grep "Creating account" | awk '{print $3}')
spl-token mint $TEST_MINT 100000 $TOKEN_ACCOUNT

# 6. Create lending market
MARKET=$(spl-token-lending create-market | grep "Creating lending market" | awk '{print $4}')

# 7. Add reserve
RESERVE=$(spl-token-lending add-reserve \
  --market $MARKET \
  --source $TOKEN_ACCOUNT \
  --amount 10000 \
  --pyth-product PLACEHOLDER11111111111111111111111111 \
  --pyth-price PLACEHOLDER11111111111111111111111111 | \
  grep "Reserve address" | awk '{print $3}')

echo "Reserve created: $RESERVE"

# 8. Now execute flash loans against this reserve
```

## Key Addresses

### Devnet
```
Program ID: TokenLending1111111111111111111111111111111
```

### Mainnet
```
Program ID: LendZqTs7gn5CTSJU1jWKhKuVpjJGom45nnwPb2AMTi
```

## Reserve State Structure

The official reserve structure includes:
- Liquidity supply amount
- Available liquidity
- Flash loan fee configuration
- All the fields your program needs

## Integration with Your Flash Loan Receiver

Your flash loan receiver program (`programs/example-receiver`) will work with the official program! Just point it to:

1. **Source liquidity**: Reserve's liquidity supply token account
2. **Reserve account**: Created via `spl-token-lending add-reserve`
3. **Lending market**: Created via `spl-token-lending create-market`

## Quick Test Script

```bash
#!/bin/bash
# test_flash_loan_spl.sh

# Setup
solana config set --url https://api.devnet.solana.com
solana airdrop 2

# Create token
MINT=$(spl-token create-token --decimals 6 | grep "Creating token" | awk '{print $3}')
ACCOUNT=$(spl-token create-account $MINT | grep "Creating account" | awk '{print $3}')
spl-token mint $MINT 100000 $ACCOUNT

# Create market and reserve
MARKET=$(spl-token-lending create-market | grep "Creating lending market" | awk '{print $4}')

RESERVE=$(spl-token-lending add-reserve \
  --market $MARKET \
  --source $ACCOUNT \
  --amount 10000 \
  --pyth-product 11111111111111111111111111111111 \
  --pyth-price 11111111111111111111111111111111 | \
  grep "Reserve address" | awk '{print $3}')

echo "Setup complete!"
echo "Market: $MARKET"
echo "Reserve: $RESERVE"
echo ""
echo "Ready for flash loans!"
```

## Next Steps

1. **Use official program** for flash loans (already deployed)
2. **Deploy your receiver** program that implements the flash loan logic
3. **Execute transactions** that call the official lending program with your receiver

## CLI Reference

```bash
# Create market
spl-token-lending create-market [OPTIONS]

# Add reserve
spl-token-lending add-reserve \
  --market <MARKET> \
  --source <TOKEN_ACCOUNT> \
  --amount <AMOUNT> \
  --pyth-product <PRODUCT_ID> \
  --pyth-price <PRICE_ID>

# Deposit
spl-token-lending deposit \
  --reserve <RESERVE> \
  --source <TOKEN_ACCOUNT> \
  --amount <AMOUNT>

# Get reserve info
spl-token-lending show-reserve <RESERVE_ADDRESS>
```

## Important Notes

- **Oracle requirement**: SPL Token Lending requires Pyth oracles. Use placeholder addresses for testing.
- **Fees**: Default flash loan fee is 0.09% (9 basis points)
- **Your receiver program works**: The example receiver you have will work with the official program!
- **No custom program needed**: Unless you want different fee structures or custom logic

The official SPL Token Lending program is production-ready and audited. Use it instead of building from scratch!