# File Structure and Documentation Index

Complete overview of all files in the market-streaming subcrate.

## 📁 Directory Structure

```
market-streaming/
├── src/
│   ├── lib.rs                      # Library entry point with exports
│   ├── pool_states.rs              # DEX pool state definitions
│   ├── state_cache.rs              # Thread-safe caching layer
│   ├── stream_client.rs            # gRPC streaming client
│   └── bin/
│       └── service.rs              # Standalone CLI service
│
├── examples/
│   └── monitor_pools.rs            # Example: Pool monitoring
│
├── Cargo.toml                      # Package manifest
│
├── README.md                       # Main documentation
├── INTEGRATION_GUIDE.md            # Integration with RPC providers
├── TROUBLESHOOTING.md              # Common issues and solutions
├── COMMANDS.md                     # Complete command reference
├── BUILD_TOOLS_SUMMARY.md          # Build tool comparison
├── FILES.md                        # This file
│
├── Makefile                        # Build automation (recommended)
├── justfile                        # Modern Make alternative
├── run.sh                          # Simple runner script
│
├── .env.example                    # Configuration template
└── .gitignore                      # Git ignore patterns
```

## 📄 Core Source Files

### `src/lib.rs`
**Purpose:** Library entry point

**Exports:**
- Pool state types (`DexPoolState`, `RaydiumClmmPoolState`, etc.)
- State cache (`PoolStateCache`, `CachedPoolState`)
- Stream client (`PoolStreamClient`, `StreamConfig`)
- Prelude module for convenient imports

**Use:** Import as a library in other Rust projects
```rust
use market_streaming::prelude::*;
```

---

### `src/pool_states.rs`
**Purpose:** DEX pool state definitions

**Contains:**
- `RaydiumClmmPoolState` - Raydium CLMM pool structure
- `OrcaWhirlpoolState` - Orca Whirlpool pool structure
- `MeteoraDlmmPoolState` - Meteora DLMM pool structure
- `DexPoolState` - Enum wrapping all pool types
- `DexProtocol` - Protocol identification and program IDs
- `PoolState` - Common trait for all pool types

**Key Methods:**
- `get_price()` - Get current pool price
- `get_liquidity()` - Get pool liquidity
- `get_token_pair()` - Get token mint addresses

---

### `src/state_cache.rs`
**Purpose:** Thread-safe pool state caching

**Contains:**
- `PoolStateCache` - DashMap-based concurrent cache
- `CachedPoolState` - Pool state with metadata (slot, timestamp)
- `CacheStats` - Cache statistics structure

**Key Features:**
- Automatic staleness detection
- Thread-safe concurrent access
- Efficient cleanup of stale entries

**API:**
```rust
let cache = PoolStateCache::new();
cache.update(pubkey, state, slot);
let fresh_pools = cache.get_all_fresh();
```

---

### `src/stream_client.rs`
**Purpose:** Yellowstone gRPC streaming client

**Contains:**
- `StreamConfig` - Client configuration
- `PoolStreamClient` - Main streaming client

**Key Methods:**
- `new()` - Create client with config
- `start()` - Start streaming (blocking)
- `add_pool()` - Add pool to monitor
- `state_cache()` - Access the cache

**Features:**
- TLS/SSL support
- Automatic reconnection handling
- Protocol-based filtering
- Commitment level configuration

---

### `src/bin/service.rs`
**Purpose:** Standalone CLI service

**Features:**
- Command-line argument parsing (clap)
- Environment variable support
- Configuration validation
- Auto-reloading statistics
- Structured logging

**CLI Arguments:**
- `--endpoint` - gRPC endpoint URL
- `--token` - Authentication token
- `--pools` - Pool addresses (comma-separated)
- `--protocols` - DEX protocols to monitor
- `--commitment` - Commitment level
- `--stats-interval` - Statistics interval
- `--cache-max-age` - Cache staleness threshold

---

## 📖 Documentation Files

### `README.md`
**Purpose:** Main documentation and overview

**Sections:**
- Quick start guide
- Supported protocols
- Features overview
- Installation instructions
- Usage examples (library and service)
- Configuration guide
- RPC provider setup
- Pool address discovery
- Architecture overview
- Troubleshooting basics

**Audience:** All users

---

### `INTEGRATION_GUIDE.md`
**Purpose:** Step-by-step RPC integration

**Sections:**
- Quick start with specific RPC providers
- Integration patterns (library, service, combined)
- Finding pool addresses
- RPC provider setup (Helius, QuickNode, Triton, self-hosted)
- Monitoring and debugging
- Production deployment (systemd, Docker)
- Performance tuning

**Audience:** Users integrating with existing projects

---

### `TROUBLESHOOTING.md`
**Purpose:** Solutions for common issues

**Sections:**
- Connection errors (DNS, TLS, transport)
- Authentication failures
- No pool updates received
- Performance issues
- RPC provider issues
- Testing and diagnostics
- Getting help

**Audience:** Users experiencing problems

---

### `COMMANDS.md`
**Purpose:** Complete command reference

**Sections:**
- Command comparison table
- Makefile commands (all targets)
- run.sh script usage
- just recipes
- Direct cargo commands
- Environment variables
- Common workflows
- Advanced usage

**Audience:** Power users, CI/CD setup

---

### `BUILD_TOOLS_SUMMARY.md`
**Purpose:** Build tool comparison and guide

**Sections:**
- Tool overview (Make, run.sh, just, cargo)
- Feature comparison matrix
- Recommendations by use case
- Installation requirements
- Quick start for each tool
- Common tasks
- Troubleshooting

**Audience:** First-time users choosing a tool

---

### `FILES.md`
**Purpose:** This file - documentation index

**Sections:**
- Directory structure
- File descriptions
- Documentation guide
- Quick reference

**Audience:** Contributors, maintainers

---

## 🛠 Build Tools

### `Makefile`
**Purpose:** GNU Make build automation

**Targets:** 40+ commands organized by category
- Setup: `setup`, `env-check`, `status`, `quick-start`
- Build: `build`, `build-release`, `check`, `clippy`, `fmt`
- Run: `run`, `run-release`, `run-debug`, `run-quick`
- Examples: `example`, `example-debug`
- Test: `test`, `test-verbose`
- Install: `install`
- Maintenance: `clean`, `update`, `doc`
- Development: `dev`, `dev-setup`, `tree`, `bloat`

**Usage:** `make <target>` or `make help`

---

### `justfile`
**Purpose:** Modern Make alternative

**Recipes:** Similar to Makefile but with simpler syntax

**Usage:** `just <recipe>` or `just` (shows list)

**Advantages:**
- Cross-platform (Windows, Mac, Linux)
- Cleaner syntax
- Better error messages

---

### `run.sh`
**Purpose:** Simple bash runner script

**Features:**
- Automatic .env creation
- Configuration validation
- Build and run in one command
- Debug/release mode switching
- Command-line flags

**Usage:** `./run.sh [--release] [--debug] [--skip-build]`

**Advantages:**
- No dependencies
- Simple to understand
- Good for quick starts

---

## ⚙️ Configuration Files

### `.env.example`
**Purpose:** Configuration template

**Contains:**
- All environment variables with descriptions
- Example values and endpoints
- Comments explaining each option

**Usage:** Copy to `.env` and edit

---

### `Cargo.toml`
**Purpose:** Rust package manifest

**Contains:**
- Package metadata (name, version, edition)
- Dependencies (yellowstone-grpc, tokio, etc.)
- Dev dependencies
- Binary definition (`[[bin]]`)
- Feature flags

---

## 📝 Example Files

### `examples/monitor_pools.rs`
**Purpose:** Example pool monitoring implementation

**Demonstrates:**
- Creating state cache
- Configuring stream client
- Spawning statistics task
- Handling pool updates
- Accessing cached data

**Run:** `make example` or `cargo run --example monitor_pools`

---

## 🔍 Quick Reference

### Finding Specific Information

| Need | File |
|------|------|
| Getting started | README.md → Quick Start |
| All commands | COMMANDS.md |
| Which tool to use | BUILD_TOOLS_SUMMARY.md |
| RPC setup | INTEGRATION_GUIDE.md |
| Connection errors | TROUBLESHOOTING.md |
| Library API | src/lib.rs, Rust docs |
| Pool structures | src/pool_states.rs |
| Caching | src/state_cache.rs |
| Configuration | .env.example |
| This overview | FILES.md |

### Running the Service

```bash
# Easiest: Use Makefile
make setup && make run

# Or: Use run script
./run.sh

# Or: Use just
just setup && just run
```

### Using as Library

```rust
// In your Cargo.toml
[dependencies]
market-streaming = { path = "path/to/market-streaming" }

// In your code
use market_streaming::prelude::*;
```

### Getting Help

1. **Quick reference:** `make help` or `./run.sh --help`
2. **Commands:** See COMMANDS.md
3. **Troubleshooting:** See TROUBLESHOOTING.md
4. **Integration:** See INTEGRATION_GUIDE.md
5. **API docs:** `make doc`

---

## 📦 Generated Files (Not in Git)

These files are created during build/run:

```
target/                             # Build artifacts
├── debug/
│   └── market-streaming-service    # Debug binary
└── release/
    └── market-streaming-service    # Release binary

.env                                # Your configuration (gitignored)
```

---

## 🎯 Next Steps

1. **Read:** README.md for overview
2. **Choose:** BUILD_TOOLS_SUMMARY.md to pick a tool
3. **Setup:** Run `make setup` or `./run.sh`
4. **Configure:** Edit `.env` file
5. **Run:** `make run` or `./run.sh`

For detailed integration: See INTEGRATION_GUIDE.md
For issues: See TROUBLESHOOTING.md
For all commands: See COMMANDS.md

---

**Last Updated:** October 2025
**Maintainer:** See main project README
