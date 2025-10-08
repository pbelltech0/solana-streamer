#!/bin/bash

set -e

echo "Building SPL Token Lending Flash Loan Programs..."
echo ""

# Build lending program
echo "Building lending program..."
cargo build-bpf --manifest-path programs/lending/Cargo.toml

echo ""
echo "Building example receiver program..."
cargo build-bpf --manifest-path programs/example-receiver/Cargo.toml

echo ""
echo "✓ Build complete!"
echo ""
echo "Programs:"
echo "  - Lending: target/deploy/token_lending_flash_loan.so"
echo "  - Receiver: target/deploy/flash_loan_example_receiver.so"
echo ""
echo "To deploy:"
echo "  solana program deploy target/deploy/token_lending_flash_loan.so"
echo "  solana program deploy target/deploy/flash_loan_example_receiver.so"