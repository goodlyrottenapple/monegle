#!/bin/bash

# Monegle Contract Deployment Script for Monad Testnet

set -e

echo "=== Monegle Contract Deployment ==="

# Check if Foundry is installed
if ! command -v forge &> /dev/null; then
    echo "Error: Foundry not installed. Install from https://getfoundry.sh"
    exit 1
fi

# Check for required environment variables
if [ -z "$PRIVATE_KEY" ]; then
    echo "Error: PRIVATE_KEY environment variable not set"
    exit 1
fi

# Configuration
RPC_URL="${RPC_URL:-https://testnet-rpc.monad.xyz}"
CHAIN_ID="${CHAIN_ID:-10143}"

echo "RPC URL: $RPC_URL"
echo "Chain ID: $CHAIN_ID"

# Navigate to contracts directory
cd "$(dirname "$0")/../contracts"

# Install dependencies if needed
if [ ! -d "lib" ]; then
    echo "Installing Foundry dependencies..."
    forge install
fi

# Deploy contract
echo ""
echo "Deploying MonadStreamer contract..."

DEPLOY_OUTPUT=$(forge create \
    --rpc-url "$RPC_URL" \
    --private-key "$PRIVATE_KEY" \
    --legacy \
    src/MonadStreamer.sol:MonadStreamer)

echo "$DEPLOY_OUTPUT"

# Extract contract address
CONTRACT_ADDRESS=$(echo "$DEPLOY_OUTPUT" | grep "Deployed to:" | awk '{print $3}')

if [ -z "$CONTRACT_ADDRESS" ]; then
    echo "Error: Failed to extract contract address"
    exit 1
fi

echo ""
echo "=== Deployment Successful ==="
echo "Contract Address: $CONTRACT_ADDRESS"
echo ""
echo "Update your config.toml with:"
echo "[network]"
echo "contract_address = \"$CONTRACT_ADDRESS\""
echo ""

# Save to file
echo "$CONTRACT_ADDRESS" > ../contract-address.txt
echo "Contract address saved to contract-address.txt"
