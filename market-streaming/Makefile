# Market Streaming Service Makefile
# Convenient commands for building, running, and managing the service

.PHONY: help setup build run run-release example clean check test install env-check

# Default target
.DEFAULT_GOAL := help

# Colors for output
CYAN := \033[0;36m
GREEN := \033[0;32m
YELLOW := \033[1;33m
RED := \033[0;31m
NC := \033[0m # No Color

##@ General

help: ## Display this help message
	@echo "$(CYAN)Market Streaming Service - Available Commands$(NC)\n"
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make $(CYAN)<target>$(NC)\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  $(CYAN)%-15s$(NC) %s\n", $$1, $$2 } /^##@/ { printf "\n$(YELLOW)%s$(NC)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

##@ Setup

setup: ## Initial setup - create .env from example
	@echo "$(GREEN)Setting up environment...$(NC)"
	@if [ ! -f .env ]; then \
		cp .env.example .env; \
		echo "$(GREEN)✓ Created .env file from .env.example$(NC)"; \
		echo "$(YELLOW)⚠ Please edit .env with your configuration before running$(NC)"; \
		echo "$(YELLOW)  Required: POOL_PUBKEYS, GRPC_ENDPOINT$(NC)"; \
	else \
		echo "$(YELLOW).env already exists, skipping...$(NC)"; \
	fi

env-check: ## Check if environment is properly configured
	@echo "$(CYAN)Checking environment configuration...$(NC)"
	@if [ ! -f .env ]; then \
		echo "$(RED)✗ .env file not found!$(NC)"; \
		echo "$(YELLOW)  Run: make setup$(NC)"; \
		exit 1; \
	fi
	@. ./.env && \
	if [ -z "$$GRPC_ENDPOINT" ]; then \
		echo "$(RED)✗ GRPC_ENDPOINT not set in .env$(NC)"; \
		exit 1; \
	else \
		echo "$(GREEN)✓ GRPC_ENDPOINT: $$GRPC_ENDPOINT$(NC)"; \
	fi
	@. ./.env && \
	if [ -z "$$POOL_PUBKEYS" ]; then \
		echo "$(YELLOW)⚠ POOL_PUBKEYS not set in .env$(NC)"; \
		echo "$(YELLOW)  Service will run but won't monitor any pools$(NC)"; \
	else \
		POOL_COUNT=$$(echo $$POOL_PUBKEYS | tr ',' '\n' | wc -l | tr -d ' '); \
		echo "$(GREEN)✓ Monitoring $$POOL_COUNT pool(s)$(NC)"; \
	fi
	@echo "$(GREEN)✓ Environment configuration OK$(NC)"

##@ Build

build: ## Build the service in debug mode
	@echo "$(CYAN)Building market-streaming service (debug)...$(NC)"
	@cargo build --bin market-streaming-service
	@echo "$(GREEN)✓ Build complete!$(NC)"

build-release: ## Build the service in release mode (optimized)
	@echo "$(CYAN)Building market-streaming service (release)...$(NC)"
	@cargo build --release --bin market-streaming-service
	@echo "$(GREEN)✓ Release build complete!$(NC)"
	@echo "$(GREEN)Binary: ../target/release/market-streaming-service$(NC)"

check: ## Run cargo check (fast compile check)
	@echo "$(CYAN)Checking code...$(NC)"
	@cargo check --bin market-streaming-service
	@echo "$(GREEN)✓ Check passed!$(NC)"

clippy: ## Run clippy linter
	@echo "$(CYAN)Running clippy...$(NC)"
	@cargo clippy --bin market-streaming-service -- -D warnings
	@echo "$(GREEN)✓ Clippy passed!$(NC)"

fmt: ## Format code with rustfmt
	@echo "$(CYAN)Formatting code...$(NC)"
	@cargo fmt
	@echo "$(GREEN)✓ Code formatted!$(NC)"

##@ Run

run: env-check build ## Run the service in debug mode (with .env)
	@echo "$(CYAN)Starting market-streaming service (debug)...$(NC)"
	@echo "$(YELLOW)Press Ctrl+C to stop$(NC)\n"
	@set -a && . ./.env && set +a && \
	RUST_LOG=$${RUST_LOG:-info} cargo run --bin market-streaming-service

run-release: env-check build-release ## Run the service in release mode (optimized)
	@echo "$(CYAN)Starting market-streaming service (release)...$(NC)"
	@echo "$(YELLOW)Press Ctrl+C to stop$(NC)\n"
	@set -a && . ./.env && set +a && \
	RUST_LOG=$${RUST_LOG:-info} cargo run --release --bin market-streaming-service

run-debug: env-check build ## Run with debug logging enabled
	@echo "$(CYAN)Starting market-streaming service (debug logging)...$(NC)"
	@echo "$(YELLOW)Press Ctrl+C to stop$(NC)\n"
	@set -a && . ./.env && set +a && \
	RUST_LOG=debug cargo run --bin market-streaming-service

run-quick: ## Run without env check (use exported env vars)
	@echo "$(CYAN)Starting market-streaming service (using exported env vars)...$(NC)"
	@RUST_LOG=$${RUST_LOG:-info} cargo run --bin market-streaming-service

##@ Examples

example: ## Run the pool monitoring example
	@echo "$(CYAN)Running pool monitoring example...$(NC)"
	@set -a && [ -f .env ] && . ./.env && set +a; \
	RUST_LOG=$${RUST_LOG:-info} cargo run --example monitor_pools

example-debug: ## Run example with debug logging
	@echo "$(CYAN)Running pool monitoring example (debug)...$(NC)"
	@set -a && [ -f .env ] && . ./.env && set +a; \
	RUST_LOG=debug cargo run --example monitor_pools

##@ Test

test: ## Run tests
	@echo "$(CYAN)Running tests...$(NC)"
	@cargo test
	@echo "$(GREEN)✓ Tests passed!$(NC)"

test-verbose: ## Run tests with verbose output
	@echo "$(CYAN)Running tests (verbose)...$(NC)"
	@cargo test -- --nocapture --test-threads=1

##@ Install

install: build-release ## Install the binary to ~/.cargo/bin
	@echo "$(CYAN)Installing market-streaming-service...$(NC)"
	@cargo install --path . --bin market-streaming-service
	@echo "$(GREEN)✓ Installed to ~/.cargo/bin/market-streaming-service$(NC)"
	@echo "$(GREEN)You can now run: market-streaming-service$(NC)"

##@ Maintenance

clean: ## Clean build artifacts
	@echo "$(CYAN)Cleaning build artifacts...$(NC)"
	@cargo clean
	@echo "$(GREEN)✓ Clean complete!$(NC)"

update: ## Update dependencies
	@echo "$(CYAN)Updating dependencies...$(NC)"
	@cargo update
	@echo "$(GREEN)✓ Dependencies updated!$(NC)"

doc: ## Generate and open documentation
	@echo "$(CYAN)Generating documentation...$(NC)"
	@cargo doc --no-deps --open

##@ Convenience

all: setup build ## Setup environment and build
	@echo "$(GREEN)✓ Setup and build complete!$(NC)"
	@echo "$(CYAN)Next steps:$(NC)"
	@echo "  1. Edit .env with your configuration"
	@echo "  2. Run: make run"

quick-start: setup ## Interactive quick start guide
	@echo "$(CYAN)=== Market Streaming Quick Start ===$(NC)\n"
	@echo "$(YELLOW)Step 1: Edit your .env file$(NC)"
	@echo "  Required settings:"
	@echo "    - GRPC_ENDPOINT: Your Yellowstone gRPC endpoint"
	@echo "    - POOL_PUBKEYS: Pool addresses to monitor (comma-separated)"
	@echo ""
	@echo "$(YELLOW)Step 2: Build and run$(NC)"
	@echo "  Run: make run"
	@echo ""
	@echo "$(CYAN)Common endpoints:$(NC)"
	@echo "  - Public: https://grpc.mainnet.solana.tools:443"
	@echo "  - Helius: https://mainnet.helius-rpc.com (requires API key)"
	@echo ""
	@echo "$(CYAN)Example pools to monitor:$(NC)"
	@echo "  - Raydium SOL/USDC: 8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj"
	@echo "  - Orca SOL/USDC: HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ"
	@echo ""
	@read -p "$(YELLOW)Open .env in editor now? (y/n) $(NC)" -n 1 -r; \
	echo; \
	if [[ $$REPLY =~ ^[Yy]$$ ]]; then \
		$${EDITOR:-nano} .env; \
	fi

status: env-check ## Show current configuration
	@echo "$(CYAN)=== Current Configuration ===$(NC)"
	@. ./.env && \
	echo "  Endpoint: $$GRPC_ENDPOINT"; \
	echo "  Auth Token: $${GRPC_AUTH_TOKEN:+***SET***}$${GRPC_AUTH_TOKEN:-Not set}"; \
	echo "  Protocols: $${DEX_PROTOCOLS:-raydium,orca,meteora}"; \
	echo "  Commitment: $${COMMITMENT_LEVEL:-processed}"; \
	echo "  Stats Interval: $${STATS_INTERVAL:-10}s"; \
	echo "  Cache Max Age: $${CACHE_MAX_AGE:-5000}ms"
	@. ./.env && \
	if [ -n "$$POOL_PUBKEYS" ]; then \
		echo "$(CYAN)\nMonitored Pools:$(NC)"; \
		echo "$$POOL_PUBKEYS" | tr ',' '\n' | nl; \
	else \
		echo "$(YELLOW)\n⚠ No pools configured$(NC)"; \
	fi

logs: ## Show systemd logs (if running as service)
	@journalctl -u market-streaming -f

##@ Development

dev: ## Run in development mode with auto-reload (requires cargo-watch)
	@which cargo-watch > /dev/null || (echo "$(RED)cargo-watch not installed$(NC)" && echo "Install with: cargo install cargo-watch" && exit 1)
	@echo "$(CYAN)Starting development mode with auto-reload...$(NC)"
	@set -a && . ./.env && set +a && \
	cargo watch -x 'run --bin market-streaming-service'

dev-setup: ## Install development tools
	@echo "$(CYAN)Installing development tools...$(NC)"
	@cargo install cargo-watch
	@cargo install cargo-edit
	@echo "$(GREEN)✓ Development tools installed!$(NC)"

benchmark: ## Run benchmarks (if available)
	@echo "$(CYAN)Running benchmarks...$(NC)"
	@cargo bench

tree: ## Show dependency tree
	@cargo tree

bloat: ## Analyze binary size (requires cargo-bloat)
	@which cargo-bloat > /dev/null || (echo "$(RED)cargo-bloat not installed$(NC)" && echo "Install with: cargo install cargo-bloat" && exit 1)
	@cargo bloat --release --bin market-streaming-service

##@ Docker (if you add Dockerfile later)

docker-build: ## Build Docker image
	@echo "$(CYAN)Building Docker image...$(NC)"
	@cd .. && docker build -t market-streaming -f market-streaming/Dockerfile .
	@echo "$(GREEN)✓ Docker image built!$(NC)"

docker-run: ## Run in Docker container
	@echo "$(CYAN)Running in Docker...$(NC)"
	@docker run -it --env-file .env market-streaming
