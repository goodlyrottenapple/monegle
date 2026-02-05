#!/bin/bash

# Monegle Setup Verification Script

echo "=== Monegle Setup Verification ==="
echo ""

# Track if all checks pass
ALL_PASSED=true

# Check Rust
echo "Checking Rust installation..."
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    echo "✅ Rust: $RUST_VERSION"
else
    echo "❌ Rust not found. Install from https://rustup.rs"
    ALL_PASSED=false
fi

# Check Cargo
if command -v cargo &> /dev/null; then
    CARGO_VERSION=$(cargo --version)
    echo "✅ Cargo: $CARGO_VERSION"
else
    echo "❌ Cargo not found. Install Rust from https://rustup.rs"
    ALL_PASSED=false
fi

echo ""

# Check Foundry
echo "Checking Foundry installation..."
if command -v forge &> /dev/null; then
    FORGE_VERSION=$(forge --version | head -1)
    echo "✅ Foundry: $FORGE_VERSION"
else
    echo "❌ Foundry not found. Install from https://getfoundry.sh"
    ALL_PASSED=false
fi

echo ""

# Check for config file
echo "Checking configuration..."
if [ -f "config.toml" ]; then
    echo "✅ config.toml exists"

    # Check if contract address is set
    if grep -q 'contract_address = ""' config.toml || ! grep -q 'contract_address' config.toml; then
        echo "⚠️  Warning: contract_address not set in config.toml"
        echo "   Deploy contract first: ./scripts/deploy.sh"
    else
        echo "✅ Contract address configured"
    fi
else
    echo "⚠️  config.toml not found"
    echo "   Copy from example: cp config.example.toml config.toml"
fi

echo ""

# Check environment variables
echo "Checking environment variables..."
if [ -n "$PRIVATE_KEY" ] || [ -n "$MONEGLE_PRIVATE_KEY" ]; then
    echo "✅ Private key environment variable set"
else
    echo "⚠️  Private key not set"
    echo "   Set with: export MONEGLE_PRIVATE_KEY=0x..."
fi

echo ""

# Check project structure
echo "Checking project structure..."
REQUIRED_DIRS=(
    "crates/monegle-core/src"
    "crates/monegle-sender/src"
    "crates/monegle-receiver/src"
    "contracts/src"
)

for dir in "${REQUIRED_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        echo "✅ $dir exists"
    else
        echo "❌ $dir missing"
        ALL_PASSED=false
    fi
done

echo ""

# Try to compile
echo "Checking if project compiles..."
if command -v cargo &> /dev/null; then
    if cargo check --workspace --quiet 2>/dev/null; then
        echo "✅ Project compiles successfully"
    else
        echo "⚠️  Project has compilation errors"
        echo "   Run 'cargo check --workspace' for details"
    fi
else
    echo "⏭️  Skipping compilation check (cargo not found)"
fi

echo ""
echo "=== Summary ==="
if [ "$ALL_PASSED" = true ]; then
    echo "✅ All checks passed! Ready to build."
    echo ""
    echo "Next steps:"
    echo "  1. Deploy contract: ./scripts/deploy.sh"
    echo "  2. Update config.toml with contract address"
    echo "  3. Build project: cargo build --release --workspace"
    echo "  4. Run sender: cargo run --release --bin monegle-sender"
    exit 0
else
    echo "❌ Some checks failed. Please install missing dependencies."
    exit 1
fi
