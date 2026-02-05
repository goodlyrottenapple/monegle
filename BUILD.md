# Build and Verification Guide

This guide walks through building and verifying the Monegle project.

## Prerequisites

### 1. Install Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Source the cargo environment
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### 2. Install Foundry (for smart contracts)

```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash

# Run foundryup to install forge, cast, anvil
foundryup

# Verify installation
forge --version
```

### 3. Get Monad Testnet Tokens

- Visit [Chainstack Monad Faucet](https://faucet.chainstack.com/monad-testnet-faucet)
- Or use [Alchemy Faucet](https://www.alchemy.com/faucets/monad-testnet)
- You'll need testnet MON for gas fees

## Building the Project

### Step 1: Verify Project Structure

```bash
cd /Users/goodlyrottenapple/git/monegle

# Check workspace structure
ls -R crates/

# Should show:
# crates/monegle-core/
# crates/monegle-sender/
# crates/monegle-receiver/
```

### Step 2: Build All Crates

```bash
# Check compilation (doesn't build binaries)
cargo check --workspace

# Build in debug mode
cargo build --workspace

# Build optimized release binaries
cargo build --release --workspace
```

Expected output:
```
   Compiling monegle-core v0.1.0
   Compiling monegle-sender v0.1.0
   Compiling monegle-receiver v0.1.0
    Finished release [optimized] target(s) in X.XXs
```

Binaries will be in:
- `target/release/monegle-sender`
- `target/release/monegle-receiver`

### Step 3: Run Tests

```bash
# Run all tests
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific crate tests
cargo test -p monegle-core
cargo test -p monegle-sender
cargo test -p monegle-receiver
```

### Step 4: Check for Warnings

```bash
# Run clippy (Rust linter)
cargo clippy --workspace

# Fix common issues automatically
cargo clippy --workspace --fix
```

## Deploying the Smart Contract

### Step 1: Setup Environment

```bash
# Set your private key (NEVER commit this!)
export PRIVATE_KEY="0x..."

# Optional: customize RPC
export RPC_URL="https://testnet-rpc.monad.xyz"
```

### Step 2: Deploy Contract

```bash
# Make deployment script executable
chmod +x scripts/deploy.sh

# Run deployment
./scripts/deploy.sh
```

Expected output:
```
=== Monegle Contract Deployment ===
RPC URL: https://testnet-rpc.monad.xyz
Chain ID: 10143

Deploying MonadStreamer contract...
Deployed to: 0x1234567890abcdef...

=== Deployment Successful ===
Contract Address: 0x1234567890abcdef...

Update your config.toml with:
[network]
contract_address = "0x1234567890abcdef..."
```

### Step 3: Update Configuration

```bash
# Copy example config
cp config.example.toml config.toml

# Edit config.toml and set:
# - contract_address (from deployment)
# - Adjust fps, resolution as needed

# Edit with your favorite editor
vim config.toml
# or
nano config.toml
```

## Running the Application

### Sender (Terminal 1)

```bash
# Set private key
export MONEGLE_PRIVATE_KEY="0x..."

# Enable debug logging (optional)
export RUST_LOG=info

# Run sender
cargo run --release --bin monegle-sender -- --config config.toml

# Or use the built binary
./target/release/monegle-sender --config config.toml
```

Expected startup output:
```
[INFO] Monegle Sender starting...
[INFO] Initializing blockchain sender
[INFO] Starting stream: 80x60 @ 15 FPS
[INFO] Stream started! Transaction: 0x..., Gas used: 85000
[INFO] Stream ID: 1
[INFO] Initializing camera 0 at 640x480 @ 15 FPS
[INFO] Camera initialized successfully
[INFO] All components initialized, starting pipeline
[INFO] Pipeline started! Press Ctrl+C to stop.
[INFO] Captured 150 frames
[INFO] Batch 0 ready: 6 frames, 8542 bytes
[INFO] Batch 0 confirmed: tx=0x..., gas=95000
```

### Receiver (Terminal 2)

```bash
# Use the stream ID from sender output
cargo run --release --bin monegle-receiver -- \
  --config config.toml \
  --stream-id 1

# Or use the built binary
./target/release/monegle-receiver --config config.toml --stream-id 1
```

Expected startup output:
```
[INFO] Monegle Receiver starting...
[INFO] Initializing event listener
[INFO] Stream metadata: 80x60 @ 15 FPS, compression: Delta
[INFO] All components initialized, starting pipeline
[INFO] Starting polling loop (interval: 400ms)
[INFO] Starting decoding loop
[INFO] Starting buffering loop (target FPS: 15)
[INFO] Buffering initial frames...
[INFO] Buffer ready, starting playback
[INFO] Starting terminal display
```

You should now see ASCII video in the terminal!

## Verification Checklist

### Build Verification

- [ ] `cargo check --workspace` succeeds
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` has no errors
- [ ] Binaries exist in `target/release/`

### Contract Verification

- [ ] Foundry installed (`forge --version` works)
- [ ] Contract deploys successfully
- [ ] Contract address saved to config.toml
- [ ] Can query contract: `cast call <address> "nextStreamId()" --rpc-url <rpc>`

### Runtime Verification

- [ ] Sender starts without errors
- [ ] Stream transaction confirms
- [ ] Camera captures frames
- [ ] Batches submitted to blockchain
- [ ] Receiver connects to stream
- [ ] Frames decoded successfully
- [ ] ASCII video displays in terminal
- [ ] No dropped frames under normal conditions

## Troubleshooting

### Build Errors

**Error: `failed to resolve: use of undeclared crate`**
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --workspace
```

**Error: `linker 'cc' not found`**
```bash
# Install C compiler (macOS)
xcode-select --install

# Install C compiler (Linux)
sudo apt-get install build-essential
```

### Camera Errors

**Error: `Failed to open camera`**
```bash
# List available cameras
ls -la /dev/video*

# Try different camera index
cargo run --bin monegle-sender -- --camera 1

# Check permissions (Linux)
sudo usermod -a -G video $USER
```

### Blockchain Errors

**Error: `insufficient funds for gas`**
- Get more testnet MON from faucet
- Check balance: `cast balance <your-address> --rpc-url <rpc>`

**Error: `connection refused`**
- Verify RPC URL is correct
- Test connection: `curl -X POST <rpc-url> -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'`

**Error: `nonce too low`**
- Transaction stuck, wait for confirmation
- Or manually increase nonce in transaction

### Performance Issues

**High gas costs:**
- Reduce FPS: `fps = 10`
- Reduce resolution: `resolution = [40, 30]`
- Use Delta compression: `compression = "Delta"`

**Frame drops:**
- Increase buffer: `buffer_blocks = 10`
- Reduce FPS
- Check network latency

**High CPU usage:**
- Reduce FPS
- Use smaller resolution
- Disable debug logging: `RUST_LOG=warn`

## Performance Benchmarks

Expected performance on modern hardware:

| Component | CPU Usage | Memory | Network |
|-----------|-----------|---------|---------|
| Sender    | 10-20%    | 50MB    | 5-10 KB/s upload |
| Receiver  | 5-10%     | 30MB    | 5-10 KB/s download |

## Next Steps

Once everything is working:

1. Experiment with different quality settings
2. Try different compression strategies
3. Measure actual costs on testnet
4. Test with network interruptions
5. Try multiple concurrent viewers (requires modifications)

## Support

If you encounter issues not covered here:

1. Check logs with `RUST_LOG=debug`
2. Review [docs/architecture.md](docs/architecture.md)
3. Open an issue on GitHub
4. Check Monad documentation for blockchain-specific issues
