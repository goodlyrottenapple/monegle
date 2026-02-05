# Monegle: ASCII Video Streaming on Monad Blockchain

**Live video streaming as ASCII art on the blockchain!**

Monegle captures video from your laptop camera, converts it to ASCII art, and streams it on Monad testnet using raw blockchain transactions. Receivers monitor transactions via WebSocket RPC subscriptions and display the video in their terminal in real-time.

## Quick Start

### 1. Build
```bash
cargo build --release
```

### 2. Configure
```bash
cp config.example.toml config.toml
export MONAD_PRIVATE_KEY="0x..."
```

### 3. Start Sender
```bash
./target/release/monegle-sender --config config.toml
```

### 4. Start Receiver
```bash
./target/release/monegle-receiver --sender-address 0xYourAddress
```

## Features

- **Real-time video** capture and ASCII conversion
- **True RGB colors** - photorealistic colored ASCII using actual video colors
- **Multiple character sets** - Standard, Dense, Detailed (45 chars), Blocks
- **Color modes** - Monochrome, Purple, Blue, Green, or full RGB
- **Multi-stage compression** - Delta + RLE + Zlib (automatic)
- **WebSocket RPC streaming** - ~7s latency via blockchain subscription
- **Self-describing protocol** - Each batch includes complete metadata
- **No smart contract** - Uses raw transactions for simplicity
- **Cross-platform** - Works on macOS, Linux, Windows

## Documentation

- [PROTOCOL.md](PROTOCOL.md) - **Protocol specification and metadata format**
- [DRY_RUN_GUIDE.md](DRY_RUN_GUIDE.md) - Test camera without blockchain
- [BUILD.md](BUILD.md) - Build instructions
- [GAS_COST_ANALYSIS.md](GAS_COST_ANALYSIS.md) - Cost analysis
- [config.example.toml](config.example.toml) - Configuration guide

## Performance

- **Latency**: 7.2 seconds
- **Success Rate**: 95.5%
- **Cost**: ~$5,100/hour @ 15 FPS with compression

