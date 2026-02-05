#!/bin/bash

# Monegle Quick Start Script
# This script helps you get started with Monegle quickly

set -e

echo "╔════════════════════════════════════════════════════════╗"
echo "║         Monegle - ASCII Video Streaming on Monad       ║"
echo "║                    Quick Start Guide                   ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check prerequisites
echo -e "${BLUE}[1/7] Checking prerequisites...${NC}"

if ! command -v rustc &> /dev/null; then
    echo -e "${RED}✗ Rust not found${NC}"
    echo "Install from: https://rustup.rs"
    exit 1
fi
echo -e "${GREEN}✓ Rust installed: $(rustc --version)${NC}"

if ! command -v forge &> /dev/null; then
    echo -e "${RED}✗ Foundry not found${NC}"
    echo "Install from: https://getfoundry.sh"
    exit 1
fi
echo -e "${GREEN}✓ Foundry installed: $(forge --version | head -1)${NC}"

echo ""
echo -e "${BLUE}[2/7] Building project...${NC}"
cargo build --release --workspace
echo -e "${GREEN}✓ Build complete${NC}"

echo ""
echo -e "${BLUE}[3/7] Setting up configuration...${NC}"
if [ ! -f config.toml ]; then
    cp config.example.toml config.toml
    echo -e "${GREEN}✓ Created config.toml from example${NC}"
else
    echo -e "${YELLOW}⚠ config.toml already exists, skipping${NC}"
fi

echo ""
echo -e "${BLUE}[4/7] Checking for private key...${NC}"
if [ -z "$PRIVATE_KEY" ] && [ -z "$MONEGLE_PRIVATE_KEY" ]; then
    echo -e "${YELLOW}⚠ No private key found in environment${NC}"
    echo ""
    echo "You need to set your private key for deployment:"
    echo "  export PRIVATE_KEY=\"0x...\""
    echo ""
    read -p "Do you want to enter it now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        read -sp "Enter private key (0x...): " PRIVATE_KEY
        echo ""
        export PRIVATE_KEY
        echo -e "${GREEN}✓ Private key set${NC}"
    else
        echo -e "${YELLOW}⚠ Skipping deployment. Set PRIVATE_KEY later to deploy.${NC}"
        SKIP_DEPLOY=true
    fi
else
    echo -e "${GREEN}✓ Private key found in environment${NC}"
fi

echo ""
if [ "$SKIP_DEPLOY" != "true" ]; then
    echo -e "${BLUE}[5/7] Deploying smart contract...${NC}"
    if ./scripts/deploy.sh; then
        echo -e "${GREEN}✓ Contract deployed successfully${NC}"

        # Read contract address
        if [ -f contract-address.txt ]; then
            CONTRACT_ADDR=$(cat contract-address.txt)
            echo ""
            echo -e "${YELLOW}Contract Address: $CONTRACT_ADDR${NC}"
            echo ""

            # Update config.toml
            if command -v sed &> /dev/null; then
                # Try to update config.toml automatically
                if grep -q 'contract_address = ""' config.toml; then
                    sed -i.bak "s/contract_address = \"\"/contract_address = \"$CONTRACT_ADDR\"/" config.toml
                    echo -e "${GREEN}✓ Updated config.toml with contract address${NC}"
                else
                    echo -e "${YELLOW}⚠ Please manually add to config.toml:${NC}"
                    echo "  contract_address = \"$CONTRACT_ADDR\""
                fi
            fi
        fi
    else
        echo -e "${RED}✗ Deployment failed${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}[5/7] Skipping deployment (no private key)${NC}"
fi

echo ""
echo -e "${BLUE}[6/7] Verifying setup...${NC}"
./scripts/check-setup.sh

echo ""
echo -e "${BLUE}[7/7] Setup complete!${NC}"
echo ""
echo "╔════════════════════════════════════════════════════════╗"
echo "║                   Next Steps                           ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""
echo -e "${GREEN}To start the sender (stream video):${NC}"
echo "  1. Make sure your webcam is connected"
echo "  2. Set private key: export MONEGLE_PRIVATE_KEY=\"0x...\""
echo "  3. Run: cargo run --release --bin monegle-sender"
echo ""
echo -e "${GREEN}To start the receiver (watch video):${NC}"
echo "  1. Get the stream ID from sender output"
echo "  2. Run: cargo run --release --bin monegle-receiver -- --stream-id 1"
echo ""
echo -e "${BLUE}Documentation:${NC}"
echo "  • README.md - Main documentation"
echo "  • BUILD.md - Detailed build guide"
echo "  • IMPLEMENTATION_SUMMARY.md - Technical overview"
echo ""
echo -e "${YELLOW}Need help?${NC}"
echo "  • Run: ./scripts/check-setup.sh"
echo "  • Read: docs/implementation-plan.md"
echo ""
echo "Happy streaming! 🎥"
