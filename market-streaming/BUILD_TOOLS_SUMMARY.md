# Build Tools Summary

This document provides an overview of all the build and run tools available for the market-streaming service.

## Available Tools

### 1. **Makefile** ✅ Recommended
**Best for:** Unix/Linux/macOS users, traditional build automation

```bash
make help           # Show all commands
make setup          # First-time setup
make run            # Build and run
make run-release    # Optimized build
```

**Pros:**
- ✅ Available on all Unix systems
- ✅ Industry standard
- ✅ Comprehensive command set
- ✅ Colored output with help text

**Cons:**
- ❌ Not natively available on Windows
- ❌ Syntax can be complex

### 2. **run.sh Script** ✅ Recommended
**Best for:** Quick starts, simple workflows, portable scripts

```bash
./run.sh            # Run with defaults
./run.sh --release  # Optimized build
./run.sh --debug    # Debug logging
./run.sh --help     # Show options
```

**Pros:**
- ✅ Simple and portable
- ✅ No additional tools needed
- ✅ Auto-creates .env if missing
- ✅ Command-line flags for options
- ✅ Validates configuration before running

**Cons:**
- ❌ Fewer features than Make/Just
- ❌ Bash only (not Windows PowerShell)

### 3. **Justfile**
**Best for:** Modern alternative to Make, cross-platform

```bash
just                # List all recipes
just setup          # First-time setup
just run            # Build and run
just status         # Show config
```

**Pros:**
- ✅ Modern, simpler syntax than Make
- ✅ Cross-platform (Windows, Mac, Linux)
- ✅ Better error messages
- ✅ Cleaner recipe definitions

**Cons:**
- ❌ Requires separate installation
- ❌ Less common than Make

### 4. **Cargo Commands**
**Best for:** Advanced users, CI/CD, precise control

```bash
cargo run --bin market-streaming-service
cargo build --release --bin market-streaming-service
```

**Pros:**
- ✅ No wrapper tools needed
- ✅ Maximum control
- ✅ Works everywhere Rust works

**Cons:**
- ❌ Verbose commands
- ❌ No automatic .env loading
- ❌ More typing required

## Feature Comparison

| Feature | Makefile | run.sh | just | cargo |
|---------|----------|--------|------|-------|
| Setup automation | ✅ | ✅ | ✅ | ❌ |
| .env loading | ✅ | ✅ | ✅ | ❌ |
| Config validation | ✅ | ✅ | ✅ | ❌ |
| Debug/Release modes | ✅ | ✅ | ✅ | ✅ |
| Tests | ✅ | ❌ | ✅ | ✅ |
| Examples | ✅ | ❌ | ✅ | ✅ |
| Code formatting | ✅ | ❌ | ✅ | ✅ |
| Linting | ✅ | ❌ | ✅ | ✅ |
| Installation | ✅ | ❌ | ✅ | ✅ |
| Help text | ✅ | ✅ | ✅ | ✅ |
| Cross-platform | ⚠️ | ⚠️ | ✅ | ✅ |

## Quick Start Comparison

### Using Makefile

```bash
cd market-streaming
make setup          # Create .env
nano .env           # Edit configuration
make run            # Build and run
```

### Using run.sh

```bash
cd market-streaming
./run.sh            # Auto-creates .env and prompts to edit
# Edit .env when prompted
./run.sh            # Run again
```

### Using just

```bash
cd market-streaming
just setup          # Create .env
nano .env           # Edit configuration
just run            # Build and run
```

### Using cargo

```bash
cd market-streaming
cp .env.example .env
nano .env           # Edit configuration
# Load .env manually
export $(cat .env | grep -v '^#' | xargs)
cargo run --bin market-streaming-service
```

## Recommendation by Use Case

### For Most Users
**Use: Makefile or run.sh**

Both are available without installation and provide the best balance of features and ease of use.

```bash
# Quick one-liner to get started
make setup && nano .env && make run
```

### For Beginners
**Use: run.sh**

Simplest to understand and use. Automatically handles common issues.

```bash
./run.sh --help     # See all options
./run.sh            # Just run it
```

### For Windows Users
**Use: just or cargo**

Both work well on Windows. Just provides make-like convenience.

```bash
# Install just
cargo install just

# Use it
just run
```

### For CI/CD
**Use: cargo directly**

Most reliable in automated environments.

```bash
cargo build --release --bin market-streaming-service
./target/release/market-streaming-service \
  --endpoint "$GRPC_ENDPOINT" \
  --pools "$POOL_PUBKEYS"
```

### For Development
**Use: Makefile with make dev**

Provides auto-reload on file changes.

```bash
make dev-setup      # Install cargo-watch
make dev            # Auto-reload on changes
```

## Complete Command Reference

See [COMMANDS.md](COMMANDS.md) for the complete command reference covering all tools.

## Installation Requirements

### Makefile
- **Preinstalled on:** macOS, Linux, most Unix systems
- **Install on Windows:** via WSL, Cygwin, or MinGW

### run.sh
- **Preinstalled on:** macOS, Linux, Unix (bash)
- **Install on Windows:** via WSL or Git Bash

### just
- **Install:**
  ```bash
  # macOS
  brew install just

  # Linux
  curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash

  # Cross-platform
  cargo install just
  ```

### cargo
- **Preinstalled with:** Rust toolchain
- **Install:** https://rustup.rs

## Environment Configuration

All tools use the same `.env` file format:

```bash
GRPC_ENDPOINT=https://grpc.mainnet.solana.tools:443
GRPC_AUTH_TOKEN=your-token-here
POOL_PUBKEYS=pool1,pool2,pool3
DEX_PROTOCOLS=raydium,orca,meteora
COMMITMENT_LEVEL=processed
STATS_INTERVAL=10
CACHE_MAX_AGE=5000
```

Create this file using:
- `make setup`
- `just setup`
- `./run.sh` (auto-creates on first run)
- `cp .env.example .env` (manual)

## Common Tasks

### First-Time Setup

```bash
# Using any tool
make setup          # or: just setup, or: ./run.sh
nano .env
make run            # or: just run, or: ./run.sh
```

### Running the Service

```bash
# Development (fast compile, with debug symbols)
make run            # or: just run, or: ./run.sh

# Production (optimized, faster runtime)
make run-release    # or: just run-release, or: ./run.sh --release

# With debug logging
make run-debug      # or: just run-debug, or: ./run.sh --debug
```

### Development Workflow

```bash
# Format code
make fmt            # or: just fmt

# Check for errors
make check          # or: just check

# Run linter
make clippy         # or: just clippy

# Run tests
make test           # or: just test

# Auto-reload on changes
make dev            # or: just dev
```

### Building for Production

```bash
# Build optimized binary
make build-release  # or: just build-release

# Install to ~/.cargo/bin
make install        # or: just install

# Run installed binary
market-streaming-service --help
```

## Troubleshooting

### Makefile: "command not found"
Install make on your system or use `./run.sh` instead.

### run.sh: "Permission denied"
Make executable: `chmod +x run.sh`

### just: "command not found"
Install just: `cargo install just` or use Makefile instead.

### .env: "not found"
Run setup: `make setup` or `just setup` or `./run.sh`

### Environment variables not loading
Makefile and just load .env automatically. For cargo, use:
```bash
set -a; source .env; set +a
cargo run --bin market-streaming-service
```

## Getting Help

Each tool has built-in help:

```bash
make help           # Show all Makefile targets
./run.sh --help     # Show run.sh options
just                # List all just recipes
cargo run --bin market-streaming-service -- --help  # CLI help
```

## Documentation

- **README.md** - Overview and usage
- **INTEGRATION_GUIDE.md** - Integration instructions
- **TROUBLESHOOTING.md** - Common issues
- **COMMANDS.md** - Complete command reference
- **BUILD_TOOLS_SUMMARY.md** - This file

## Next Steps

1. **Choose a tool** based on your preference (Makefile recommended)
2. **Run setup** to create `.env`
3. **Configure** your RPC endpoint and pools
4. **Run** the service

```bash
make setup && nano .env && make run
```

That's it! You're now streaming DEX pool data in real-time. 🚀
