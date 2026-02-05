# Monegle Documentation

ASCII Video Streaming on Monad Blockchain using Execution Events SDK

## Quick Links

- [Architecture Overview](./architecture.md) - System design and component interaction
- [Execution Events Integration](./execution-events-integration.md) - How to use Monad's Execution Events SDK
- [Cost Analysis](./cost-analysis.md) - Cost estimates for different quality settings
- [Implementation Plan](./implementation-plan.md) - Step-by-step development guide
- [Re-broadcast Node Design](./rebroadcast-node.md) - Design of the relay server component

## Project Overview

Monegle enables live video streaming from a laptop camera to remote viewers using Monad blockchain as the transport layer, with video displayed as ASCII art in the terminal.

### Key Features

- **Configurable Quality**: 5-30 FPS, 40×30 to 120×80 character resolution
- **Cost-Optimized**: Uses Monad's Execution Events SDK to minimize on-chain costs
- **Low Latency**: Leverages Monad's 400ms block times and pre-consensus event access
- **Off-chain Relay**: Re-broadcast node processes events and streams via WebSocket
- **Payment Integration**: Streamers can pay relay operators via transaction value

## Components

1. **Sender (`monegle-sender`)** - Captures video, converts to ASCII, compresses, sends to blockchain
2. **Re-broadcast Node (`monegle-relay`)** - Runs Execution Events SDK, extracts frames, re-broadcasts via WebSocket
3. **Receiver (`monegle-receiver`)** - Connects to relay via WebSocket, displays ASCII video in terminal
4. **Core Library (`monegle-core`)** - Shared types, compression codecs, utilities

## Technology Stack

- **Language**: Rust
- **Blockchain**: Monad Testnet (EVM-compatible, 400ms blocks)
- **Execution Events SDK**: `monad-exec-events` (Rust crate from Monad Labs)
- **Video Capture**: `nokhwa` (cross-platform camera access)
- **ASCII Conversion**: `artem` + `image` crates
- **Terminal UI**: `ratatui` + `crossterm`
- **Streaming Protocol**: WebSocket (for relay → receiver)
- **Compression**: Multi-stage (delta encoding + RLE + zlib)

## Architecture Diagram

```
┌─────────────────┐
│  Laptop Camera  │
└────────┬────────┘
         │ Video Frames
         ▼
┌─────────────────┐
│  monegle-sender │
│  - Capture      │
│  - ASCII Conv   │
│  - Compress     │
└────────┬────────┘
         │ Tx (calldata = compressed frames)
         ▼
┌─────────────────┐
│ Monad Blockchain│
│  (Testnet)      │
└────────┬────────┘
         │ Execution Events (pre-consensus)
         ▼
┌─────────────────┐
│ monegle-relay   │
│ - Events SDK    │
│ - Extract Data  │
│ - WebSocket Srv │
└────────┬────────┘
         │ WebSocket Stream
         ▼
┌─────────────────┐
│ monegle-receiver│
│ - WS Client     │
│ - Decompress    │
│ - Terminal UI   │
└─────────────────┘
```

## Cost Estimates

Based on Monad's near-zero testnet fees (~$0.003-0.005 per tx):

| Quality | FPS | Resolution | Cost/Hour | Notes |
|---------|-----|------------|-----------|-------|
| Low | 10 | 40×30 | ~$45 | Minimal bandwidth |
| Medium | 15 | 80×60 | ~$90 | Recommended |
| High | 24 | 120×80 | ~$270 | High quality |

*Note: Plus relay operator fee (negotiable, e.g., +$0.002/tx)*

## Quick Start

### Prerequisites

- Rust 1.75+
- Linux with Monad node (for relay) or macOS/Windows (for sender/receiver)
- Camera/webcam
- Monad testnet MON tokens

### 1. Clone and Build

```bash
git clone https://github.com/yourusername/monegle.git
cd monegle
cargo build --release --workspace
```

### 2. Configure

```bash
cp config.example.toml config.toml
# Edit config.toml with your settings
```

### 3. Run Components

**Terminal 1 - Sender:**
```bash
export MONAD_PRIVATE_KEY="0x..."
cargo run --release --bin monegle-sender
```

**Terminal 2 - Relay (requires Monad node):**
```bash
cargo run --release --bin monegle-relay -- --bind 0.0.0.0:8080
```

**Terminal 3 - Receiver:**
```bash
cargo run --release --bin monegle-receiver -- --relay ws://localhost:8080
```

## Development Status

- [ ] Phase 1: Core types and configuration
- [ ] Phase 2: Video capture and ASCII conversion
- [ ] Phase 3: Compression and encoding
- [ ] Phase 4: Blockchain sender
- [ ] Phase 5: Execution Events SDK integration
- [ ] Phase 6: Re-broadcast node (relay server)
- [ ] Phase 7: WebSocket receiver
- [ ] Phase 8: Terminal UI
- [ ] Phase 9: Testing and optimization

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development guidelines.

## License

See [LICENSE](../LICENSE) file.

## Resources

- [Monad Execution Events Documentation](https://docs.monad.xyz/execution-events/)
- [Monad Developer Docs](https://docs.monad.xyz/)
- [Monad Testnet Faucet](https://chainstack.com/how-to-get-monad-testnet-tokens/)
