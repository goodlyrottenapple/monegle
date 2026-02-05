#!/bin/bash

# Monad Testnet RPC Throughput Testing Script
# Tests multiple RPC endpoints to determine viability for video streaming

set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔═══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     MONEGLE RPC THROUGHPUT TESTING SUITE             ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════════════╝${NC}"
echo ""

# Monad testnet RPC endpoints
# Add your API keys where needed
RPCS=(
    "https://testnet-rpc.monad.xyz"
    # "https://monad-testnet.chainstack.com/YOUR_KEY_HERE"
    # "https://monad-testnet.blockpi.network/v1/rpc/YOUR_KEY_HERE"
)

# Test configuration
TARGET_ADDRESS="0x0000000000000000000000000000000000000001"  # Burn address for testing
DURATION=300  # 5 minutes
FPS=15
WIDTH=80
HEIGHT=60

# Check for private key
if [ -z "$MONAD_PRIVATE_KEY" ]; then
    echo -e "${RED}Error: MONAD_PRIVATE_KEY environment variable not set${NC}"
    echo "Set it with: export MONAD_PRIVATE_KEY=0x..."
    exit 1
fi

# Check if test binary exists
if ! cargo build --release --bin monegle-sender-test 2>/dev/null; then
    echo -e "${YELLOW}Building test binary...${NC}"
    cargo build --release --bin monegle-sender-test
fi

# Create results directory
mkdir -p test-results

echo -e "${GREEN}Testing ${#RPCS[@]} RPC endpoint(s)...${NC}"
echo ""
echo "Configuration:"
echo "  • FPS: $FPS"
echo "  • Resolution: ${WIDTH}x${HEIGHT}"
echo "  • Duration: $DURATION seconds"
echo "  • Target rate: 2.5 tx/second"
echo ""

# Test each RPC
for i in "${!RPCS[@]}"; do
    RPC="${RPCS[$i]}"
    OUTPUT="test-results/rpc-$i-$(date +%Y%m%d-%H%M%S).json"

    echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}Testing RPC #$i${NC}"
    echo -e "${BLUE}URL: $RPC${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
    echo ""

    # Run test
    if ./target/release/monegle-sender-test \
        --rpc-url "$RPC" \
        --target-address "$TARGET_ADDRESS" \
        --fps "$FPS" \
        --width "$WIDTH" \
        --height "$HEIGHT" \
        --duration "$DURATION" \
        --output "$OUTPUT"; then

        echo ""
        echo -e "${GREEN}✓ Test completed successfully${NC}"
        echo -e "${GREEN}  Results saved to: $OUTPUT${NC}"
        echo ""
    else
        echo ""
        echo -e "${RED}✗ Test failed${NC}"
        echo ""
    fi

    # Brief pause between tests
    if [ $i -lt $((${#RPCS[@]} - 1)) ]; then
        echo -e "${YELLOW}Waiting 30 seconds before next test...${NC}"
        sleep 30
    fi
done

echo -e "${BLUE}═══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}All tests complete!${NC}"
echo ""
echo "Results directory: test-results/"
echo ""
echo "Next steps:"
echo "  1. Run analysis: python3 scripts/analyze-test-results.py"
echo "  2. Review recommendations"
echo "  3. Decide on RPC strategy"
echo ""
