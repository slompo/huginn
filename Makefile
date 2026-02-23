# Huginn CLI Makefile

# Binary paths
CARGO ?= cargo
TARGET_DIR := target
BINARY := $(TARGET_DIR)/release/huginn
INSTALL_DIR := /usr/local/bin

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
CYAN := \033[0;36m
RESET := \033[0m

.PHONY: all build release run dev clean test fmt lint help install uninstall

all: build

## build: Build debug version
build:
	@echo "$(CYAN)Building debug version...$(RESET)"
	$(CARGO) build

## release: Build optimized release version
release:
	@echo "$(CYAN)Building release version...$(RESET)"
	$(CARGO) build --release
	@echo "$(GREEN)Binary: $(BINARY)$(RESET)"
	@ls -lh $(BINARY)

## install: Build and install to /usr/local/bin (requires sudo)
install: release
	@echo "$(CYAN)Installing to $(INSTALL_DIR)...$(RESET)"
	sudo cp $(BINARY) $(INSTALL_DIR)/huginn
	@echo "$(GREEN)Installed! Run with: huginn$(RESET)"

## uninstall: Remove from /usr/local/bin
uninstall:
	@echo "$(YELLOW)Uninstalling from $(INSTALL_DIR)...$(RESET)"
	sudo rm -f $(INSTALL_DIR)/huginn
	@echo "$(GREEN)Uninstalled.$(RESET)"

## run: Run installed version from PATH
run:
	@echo "$(CYAN)Running Huginn...$(RESET)"
	huginn

## dev: Run debug version with backtrace
dev: build
	@echo "$(CYAN)Running Huginn (debug)...$(RESET)"
	RUST_BACKTRACE=1 $(CARGO) run

## test: Run tests
test:
	@echo "$(CYAN)Running tests...$(RESET)"
	$(CARGO) test

## fmt: Format code
fmt:
	@echo "$(CYAN)Formatting code...$(RESET)"
	$(CARGO) fmt

## lint: Run clippy linter
lint:
	@echo "$(CYAN)Running clippy...$(RESET)"
	$(CARGO) clippy -- -D warnings

## clean: Remove build artifacts
clean:
	@echo "$(YELLOW)Cleaning build artifacts...$(RESET)"
	$(CARGO) clean

## check: Quick check for compilation errors
check:
	@echo "$(CYAN)Checking for errors...$(RESET)"
	$(CARGO) check

## watch: Watch for changes and rebuild
watch:
	@echo "$(CYAN)Watching for changes...$(RESET)"
	$(CARGO) watch -x build

## release-build: Build with all optimizations (smaller binary)
release-build:
	@echo "$(CYAN)Building optimized release...$(RESET)"
	$(CARGO) build --release
	@echo "$(GREEN)Done! Binary size:$(RESET)"
	@ls -lh $(BINARY)

## help: Show this help message
help:
	@echo "$(CYAN)Huginn CLI - Available commands:$(RESET)"
	@echo ""
	@grep -E '^## ' $(MAKEFILE_LIST) | sed 's/## /  /' | column -t -s ':'
	@echo ""
	@echo "$(YELLOW)Examples:$(RESET)"
	@echo "  make install    - Build and install to /usr/local/bin"
	@echo "  make run        - Run installed version"
	@echo "  make uninstall  - Remove from system"
