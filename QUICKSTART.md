# Monegle Quick Start Guide

## Prerequisites

1. **Install Rust** (1.75+):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Get Monad testnet MON**:
   - Visit: https://testnet-faucet.monad.xyz/
   - Get some testnet MON tokens

3. **Set up private key**:
   ```bash
   export MONAD_PRIVATE_KEY="0x..."
   ```

## Build

```bash
cd /Users/goodlyrottenapple/git/monegle
cargo build --release
```

This will create:
- `target/release/monegle-sender` - Video streaming sender
- `target/release/monegle-receiver` - Video receiver

## Run

### Step 1: Start Sender

```bash
./target/release/monegle-sender --config config.toml
```

Output will show:
```
Sender ready! Receivers should monitor transactions FROM: 0xYourAddress...
```

Copy the sender address shown!

### Step 2: Start Receiver (Different Terminal)

```bash
./target/release/monegle-receiver \
  --sender-address 0xYourAddressFromStep1 \
  --ws-url wss://testnet-rpc.monad.xyz
```

You should see ASCII video playing after ~7 seconds!

## Configuration

Edit `config.toml` to adjust quality:

```toml
[sender]
fps = 15              # Lower = cheaper
resolution = [80, 60] # Smaller = cheaper
compression = "Auto"  # Keep this enabled!
```

## Cost Optimization

- **Demo mode** (cheap): fps=5, resolution=[60,40] → ~$200/hour
- **Low quality**: fps=10, resolution=[60,40] → ~$850/hour  
- **Medium quality**: fps=15, resolution=[80,60] → ~$5,100/hour

## Troubleshooting

### Camera not found
```bash
# List cameras
ls /dev/video*

# Try different camera
./target/release/monegle-sender --camera 1
```

### WebSocket connection failed
```bash
# Use HTTP polling instead
./target/release/monegle-receiver \
  --sender-address 0x... \
  --no-websocket
```

### High gas costs
Make sure `compression = "Auto"` in config.toml (not "None"!)

## Next Steps

- Read [GAS_COST_ANALYSIS.md](GAS_COST_ANALYSIS.md) for cost optimization
- Check [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for details
- See [config.example.toml](config.example.toml) for all options
