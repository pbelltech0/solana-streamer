# Command Reference

Complete reference of all available commands for running and managing the market-streaming service.

## Quick Command Comparison

| Task | Makefile | run.sh | just | Cargo |
|------|----------|--------|------|-------|
| Setup | `make setup` | Automatic | `just setup` | N/A |
| Run (debug) | `make run` | `./run.sh` | `just run` | `cargo run --bin market-streaming-service` |
| Run (release) | `make run-release` | `./run.sh -r` | `just run-release` | `cargo run --release --bin market-streaming-service` |
| Run (debug logs) | `make run-debug` | `./run.sh -d` | `just run-debug` | `RUST_LOG=debug cargo run --bin market-streaming-service` |
| Build | `make build` | N/A | `just build` | `cargo build --bin market-streaming-service` |
| Clean | `make clean` | N/A | `just clean` | `cargo clean` |
| Test | `make test` | N/A | `just test` | `cargo test` |
| Help | `make help` | `./run.sh -h` | `just` | N/A |

## Makefile Commands

All commands run from the `market-streaming/` directory.

### Setup & Configuration

```bash
make setup          # Create .env from template
make env-check      # Validate .env configuration
make status         # Show current configuration
make quick-start    # Interactive setup wizard
```

### Build

```bash
make build          # Build in debug mode
make build-release  # Build optimized binary
make check          # Fast compilation check
make clippy         # Run linter
make fmt            # Format code
```

### Run

```bash
make run            # Run debug build with .env
make run-release    # Run optimized build
make run-debug      # Run with debug logging
make run-quick      # Run using exported env vars (skip .env)
```

### Examples

```bash
make example        # Run pool monitoring example
make example-debug  # Run example with debug logs
```

### Testing

```bash
make test           # Run all tests
make test-verbose   # Run tests with output
```

### Installation

```bash
make install        # Install binary to ~/.cargo/bin
```

### Maintenance

```bash
make clean          # Remove build artifacts
make update         # Update dependencies
make doc            # Generate and open docs
```

### Development

```bash
make dev            # Run with auto-reload (requires cargo-watch)
make dev-setup      # Install dev tools
make tree           # Show dependency tree
make bloat          # Analyze binary size (requires cargo-bloat)
```

### All Available Makefile Targets

Run `make help` to see the complete list with descriptions.

## run.sh Script Commands

The `run.sh` script provides a simple way to run the service with various options.

### Usage

```bash
./run.sh [OPTIONS]
```

### Options

```bash
-r, --release       # Build and run in release mode
-s, --skip-build    # Skip building, run existing binary
-d, --debug         # Enable debug logging (RUST_LOG=debug)
-h, --help          # Show help message
```

### Examples

```bash
# Basic usage
./run.sh                    # Run in debug mode

# Optimized build
./run.sh --release          # Build and run release version

# Debug logging
./run.sh --debug            # Run with RUST_LOG=debug

# Fast restart (skip rebuilding)
./run.sh --skip-build       # Use existing binary

# Combined flags
./run.sh -r -s              # Run release build, skip rebuilding
./run.sh --release --debug  # Release build with debug logs
```

## Just Commands

Just is a modern command runner alternative to Make.

### Installation

```bash
# macOS
brew install just

# Linux
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash

# Cross-platform
cargo install just
```

### Available Recipes

```bash
just                # List all available recipes
just setup          # Create .env from template
just env-check      # Validate configuration
just build          # Build debug binary
just build-release  # Build release binary
just check          # Fast compile check
just clippy         # Run linter
just fmt            # Format code
just run            # Build and run (debug)
just run-release    # Build and run (release)
just run-debug      # Run with debug logging
just example        # Run example
just test           # Run tests
just test-verbose   # Run tests with output
just install        # Install to ~/.cargo/bin
just clean          # Clean build artifacts
just update         # Update dependencies
just doc            # Generate docs
just all            # Setup and build
just quick-start    # Interactive setup
just status         # Show configuration
just dev            # Auto-reload mode
just dev-setup      # Install dev tools
```

## Direct Cargo Commands

For advanced users who want direct control.

### Build

```bash
cargo build --bin market-streaming-service                    # Debug build
cargo build --release --bin market-streaming-service          # Release build
cargo check --bin market-streaming-service                    # Fast check
```

### Run

```bash
# Debug mode
cargo run --bin market-streaming-service

# Release mode
cargo run --release --bin market-streaming-service

# With environment variables
GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443 \
POOL_PUBKEYS=8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj \
cargo run --bin market-streaming-service

# With debug logging
RUST_LOG=debug cargo run --bin market-streaming-service

# With custom log filters
RUST_LOG=market_streaming=debug,yellowstone_grpc=info \
cargo run --bin market-streaming-service
```

### Run with Arguments

```bash
cargo run --bin market-streaming-service -- \
  --endpoint https://grpc.mainnet.solana.tools:443 \
  --pools 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj \
  --protocols raydium,orca \
  --commitment processed \
  --stats-interval 10
```

### Examples

```bash
cargo run --example monitor_pools                     # Run example
RUST_LOG=debug cargo run --example monitor_pools     # With debug logs
```

### Testing

```bash
cargo test                                            # Run all tests
cargo test --verbose                                  # Verbose output
cargo test -- --nocapture                             # Show print! output
cargo test --lib                                      # Only library tests
```

### Code Quality

```bash
cargo clippy                                          # Run linter
cargo clippy -- -D warnings                           # Fail on warnings
cargo fmt                                             # Format code
cargo fmt -- --check                                  # Check formatting
```

### Documentation

```bash
cargo doc --no-deps                                   # Generate docs
cargo doc --no-deps --open                            # Generate and open
```

### Installation

```bash
cargo install --path . --bin market-streaming-service # Install locally
cargo install --path . --force                        # Force reinstall
```

### Maintenance

```bash
cargo clean                                           # Clean build artifacts
cargo update                                          # Update dependencies
cargo tree                                            # Show dependency tree
```

## Environment Variables

All commands respect these environment variables:

### Required

```bash
GRPC_ENDPOINT       # Yellowstone gRPC endpoint URL
                    # Example: https://grpc.mainnet.solana.tools:443
```

### Optional

```bash
GRPC_AUTH_TOKEN     # Authentication token (if required by provider)
POOL_PUBKEYS        # Comma-separated pool addresses to monitor
DEX_PROTOCOLS       # Comma-separated protocols (raydium,orca,meteora)
COMMITMENT_LEVEL    # processed, confirmed, or finalized
STATS_INTERVAL      # Seconds between statistics output (default: 10)
CACHE_MAX_AGE       # Cache staleness threshold in ms (default: 5000)
RUST_LOG            # Log level (error, warn, info, debug, trace)
```

### Setting Environment Variables

#### Option 1: .env file (Recommended)

```bash
# Create .env file
make setup

# Edit .env
nano .env
```

#### Option 2: Export

```bash
export GRPC_ENDPOINT="https://grpc.mainnet.solana.tools:443"
export POOL_PUBKEYS="8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj"
export DEX_PROTOCOLS="raydium,orca"
export COMMITMENT_LEVEL="processed"
```

#### Option 3: Inline

```bash
GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443 \
POOL_PUBKEYS=8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj \
make run
```

## Common Workflows

### First-Time Setup

```bash
# Using Makefile
make setup
nano .env
make run

# Using run.sh
./run.sh           # Will create .env and prompt to edit

# Using just
just setup
nano .env
just run
```

### Development Workflow

```bash
# Check code compiles
make check

# Format and lint
make fmt
make clippy

# Run with debug logs
make run-debug

# Run with auto-reload
make dev
```

### Production Deployment

```bash
# Build optimized binary
make build-release

# Install system-wide
make install

# Or run directly
make run-release
```

### Testing Changes

```bash
# Quick compile check
make check

# Run tests
make test

# Run with changes
make run
```

### Troubleshooting

```bash
# Check configuration
make status

# Run with debug logging
make run-debug

# Or
./run.sh --debug

# Or
RUST_LOG=debug cargo run --bin market-streaming-service
```

### Clean Rebuild

```bash
# Clean and rebuild
make clean
make build

# Or in one command
make clean build
```

## Advanced Usage

### Custom Log Filters

```bash
# Only show errors from yellowstone-grpc
RUST_LOG=market_streaming=info,yellowstone_grpc=error make run

# Debug only specific modules
RUST_LOG=market_streaming::stream_client=debug make run
```

### Running Multiple Instances

```bash
# Terminal 1: Monitor Raydium pools
POOL_PUBKEYS=pool1,pool2 DEX_PROTOCOLS=raydium make run-quick

# Terminal 2: Monitor Orca pools
POOL_PUBKEYS=pool3,pool4 DEX_PROTOCOLS=orca make run-quick
```

### Performance Profiling

```bash
# Build with profiling enabled
cargo build --release --bin market-streaming-service

# Run with profiler
perf record -g cargo run --release --bin market-streaming-service

# Or use flamegraph
cargo install flamegraph
flamegraph cargo run --release --bin market-streaming-service
```

### Binary Size Analysis

```bash
# Install cargo-bloat
cargo install cargo-bloat

# Analyze release binary
make bloat
```

## Getting Help

### Command-Specific Help

```bash
make help           # Show all Makefile targets
./run.sh --help     # Show run.sh options
just                # List all just recipes
cargo run --bin market-streaming-service -- --help  # Show CLI options
```

### Documentation

- [README.md](README.md) - General overview and usage
- [INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md) - Integration instructions
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Common issues and solutions
- [COMMANDS.md](COMMANDS.md) - This file

### Support

For issues or questions, check the troubleshooting guide or open an issue on GitHub.
