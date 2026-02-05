# Implementation Plan

Step-by-step guide for building Monegle from scratch.

## Overview

This plan breaks down the implementation into 12 phases, each building on the previous. Estimated timeline: 4-6 weeks for a solo developer.

## Phase 0: Project Setup (Day 1)

### Goals
- Create Cargo workspace
- Set up git repository structure
- Add basic dependencies
- Configure tooling

### Tasks

**1. Initialize Workspace**

```bash
cd /Users/goodlyrottenapple/git/monegle
cargo init --lib crates/monegle-core
cargo init --bin crates/monegle-sender
cargo init --bin crates/monegle-receiver
cargo init --bin crates/monegle-relay
```

**2. Create Workspace Cargo.toml**

```toml
[workspace]
members = [
    "crates/monegle-core",
    "crates/monegle-sender",
    "crates/monegle-receiver",
    "crates/monegle-relay",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your.email@example.com>"]
license = "MIT"

[workspace.dependencies]
# Shared dependencies
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"

# Blockchain
alloy = { version = "1.0", features = ["full"] }

# Compression
flate2 = "1.0"

# Configuration
config = "0.14"
clap = { version = "4.5", features = ["derive"] }
```

**3. Add .gitignore**

```gitignore
/target/
**/*.rs.bk
*.swp
*.swo
*~
.DS_Store
Cargo.lock
config/*.toml
!config/*.example.toml
.env
/keys/
*.key
```

**4. Set up Development Tools**

```bash
# Install tools
rustup component add rustfmt clippy
cargo install cargo-watch

# Configure formatting
echo '[toolchain]
channel = "stable"' > rust-toolchain.toml
```

### Verification

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
```

## Phase 1: Core Types & Configuration (Days 2-3)

### Goals
- Define shared data structures
- Implement configuration management
- Create error types

### Files to Create

**crates/monegle-core/src/types.rs**

```rust
use serde::{Deserialize, Serialize};

/// Binary format header for frame batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBatchHeader {
    pub magic: u32,              // 0x4D4F4E47 ("MONG")
    pub version: u8,              // Protocol version
    pub frame_count: u8,          // Number of frames in batch
    pub compression_type: CompressionType,
    pub sequence_start: u64,      // Starting sequence number
}

impl FrameBatchHeader {
    pub const SIZE: usize = 16;

    pub fn to_bytes(&self) -> Vec<u8> {
        // Serialize to binary
        unimplemented!()
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        // Deserialize from binary
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompressionType {
    None = 0,
    DeltaRle = 1,
    Zlib = 2,
}

/// Single frame of ASCII art
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub timestamp_ms: u32,  // Relative to batch
    pub data: String,       // ASCII art (width × height chars)
}

/// Batch of frames ready for blockchain
#[derive(Debug, Clone)]
pub struct FrameBatch {
    pub header: FrameBatchHeader,
    pub frames: Vec<Frame>,
}

impl FrameBatch {
    pub fn encode_to_bytes(&self) -> Result<Vec<u8>, Error> {
        // Serialize header + frames
        unimplemented!()
    }

    pub fn decode_from_bytes(data: &[u8]) -> Result<Self, Error> {
        // Deserialize header + frames
        unimplemented!()
    }
}

/// Stream metadata (inferred from batches)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub sender: [u8; 20],     // Streamer address
    pub fps: u8,
    pub width: u16,
    pub height: u16,
    pub start_time: u64,      // Unix timestamp
}
```

**crates/monegle-core/src/config.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub sender: Option<SenderConfig>,
    pub receiver: Option<ReceiverConfig>,
    pub relay: Option<RelayConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub block_time_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SenderConfig {
    pub private_key_env: String,
    pub relay_address: String,
    pub device_index: usize,
    pub quality: QualityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QualityConfig {
    pub fps: u8,
    pub width: u16,
    pub height: u16,
    pub character_set: CharacterSet,
    pub compression: CompressionType,
}

#[derive(Debug, Clone, Deserialize)]
pub enum CharacterSet {
    Standard,
    Dense,
    Blocks,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReceiverConfig {
    pub relay_url: String,
    pub stream_address: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelayConfig {
    pub bind_address: String,
    pub target_address: String,
    pub event_ring_path: PathBuf,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, anyhow::Error> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("MONEGLE"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}
```

**crates/monegle-core/src/error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid magic number")]
    InvalidMagic,

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("Unknown compression type: {0}")]
    UnknownCompression(u8),

    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

**config/config.example.toml**

```toml
[network]
rpc_url = "https://testnet-rpc.monad.xyz"
chain_id = 10143
block_time_ms = 400

[sender]
private_key_env = "MONAD_PRIVATE_KEY"
relay_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
device_index = 0

[sender.quality]
fps = 15
width = 80
height = 60
character_set = "standard"
compression = "zlib"

[receiver]
relay_url = "ws://localhost:8080"
stream_address = "0x1234567890123456789012345678901234567890"

[relay]
bind_address = "0.0.0.0:8080"
target_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
event_ring_path = "/dev/shm/monad_events"
```

### Verification

```bash
cargo test --package monegle-core
```

## Phase 2: Compression & Encoding (Days 4-5)

### Goals
- Implement frame compression algorithms
- Binary serialization for blockchain
- Decompression and deserialization

### Files to Create

**crates/monegle-core/src/codec.rs**

```rust
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};

pub trait FrameEncoder {
    fn encode(&self, frames: &[Frame]) -> Result<Vec<u8>, Error>;
    fn decode(&self, data: &[u8]) -> Result<Vec<Frame>, Error>;
}

pub struct ZlibCodec {
    compression_level: Compression,
}

impl ZlibCodec {
    pub fn new(level: u8) -> Self {
        Self {
            compression_level: Compression::new(level.into()),
        }
    }
}

impl FrameEncoder for ZlibCodec {
    fn encode(&self, frames: &[Frame]) -> Result<Vec<u8>, Error> {
        // 1. Serialize frames to bytes
        let serialized = bincode::serialize(frames)
            .map_err(|e| Error::SerializationError(e.to_string()))?;

        // 2. Compress with zlib
        let mut encoder = ZlibEncoder::new(Vec::new(), self.compression_level);
        encoder.write_all(&serialized)?;
        Ok(encoder.finish()?)
    }

    fn decode(&self, data: &[u8]) -> Result<Vec<Frame>, Error> {
        // 1. Decompress
        let mut decoder = ZlibDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        // 2. Deserialize
        bincode::deserialize(&decompressed)
            .map_err(|e| Error::SerializationError(e.to_string()))
    }
}

// TODO: Implement DeltaRleCodec for better compression
```

### Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zlib_roundtrip() {
        let codec = ZlibCodec::new(6);
        let frames = vec![
            Frame {
                timestamp_ms: 0,
                data: "test frame".to_string(),
            },
        ];

        let encoded = codec.encode(&frames).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.len(), frames.len());
        assert_eq!(decoded[0].data, frames[0].data);
    }
}
```

### Verification

```bash
cargo test --package monegle-core -- codec
```

## Phase 3: Video Capture & ASCII Conversion (Days 6-8)

### Goals
- Capture video from camera
- Resize and convert to grayscale
- Map to ASCII characters

### Files to Create

**crates/monegle-sender/Cargo.toml**

Add dependencies:
```toml
[dependencies]
monegle-core = { path = "../monegle-core" }
tokio = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
nokhwa = { version = "0.10", features = ["input-native"] }
image = "0.24"
artem = "3.0"
```

**crates/monegle-sender/src/capture.rs**

```rust
use nokhwa::{Camera, pixel_format::RgbFormat, utils::{CameraIndex, RequestedFormat}};
use image::DynamicImage;
use tokio::sync::mpsc;

pub struct VideoCapture {
    camera: Camera,
    fps: u32,
}

impl VideoCapture {
    pub fn new(device_index: usize, fps: u32) -> Result<Self, anyhow::Error> {
        let index = CameraIndex::Index(device_index as u32);
        let format = RequestedFormat::new::<RgbFormat>(nokhwa::utils::RequestedFormatType::AbsoluteHighestFrameRate);

        let mut camera = Camera::new(index, format)?;
        camera.open_stream()?;

        Ok(Self { camera, fps })
    }

    pub async fn run(mut self, tx: mpsc::Sender<DynamicImage>) -> Result<(), anyhow::Error> {
        let interval = tokio::time::Duration::from_millis(1000 / self.fps as u64);
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            let frame = self.camera.frame()?;
            let img = DynamicImage::ImageRgb8(frame.decode_image::<RgbFormat>()?);

            if tx.send(img).await.is_err() {
                break; // Receiver dropped
            }
        }

        Ok(())
    }
}
```

**crates/monegle-sender/src/converter.rs**

```rust
use image::{DynamicImage, imageops::FilterType};
use artem::{convert, options::OptionBuilder};

pub struct AsciiConverter {
    target_width: u32,
    target_height: u32,
    charset: String,
}

impl AsciiConverter {
    pub fn new(width: u32, height: u32, charset: &str) -> Self {
        let charset = match charset {
            "dense" => " .:;+=xX$&#@",
            "blocks" => " ░▒▓█",
            _ => " .:-=+*#%@",  // standard
        };

        Self {
            target_width: width,
            target_height: height,
            charset: charset.to_string(),
        }
    }

    pub fn convert(&self, img: &DynamicImage) -> String {
        // 1. Resize
        let resized = img.resize_exact(
            self.target_width,
            self.target_height,
            FilterType::Lanczos3,
        );

        // 2. Convert to grayscale
        let gray = resized.to_luma8();

        // 3. Use artem for ASCII conversion
        let options = OptionBuilder::new()
            .target_size(self.target_width, self.target_height)
            .characters(self.charset.clone())
            .build();

        convert(gray, &options)
    }
}
```

### Verification

```bash
# Test with webcam
cargo run --bin monegle-sender -- --test-capture

# Should display ASCII frames in terminal
```

## Phase 4: Frame Batching & Blockchain Sender (Days 9-11)

### Goals
- Batch frames based on FPS and block time
- Connect to Monad testnet via alloy
- Submit transactions with frame data

### Files to Create

**crates/monegle-sender/src/batcher.rs**

```rust
pub struct FrameBatcher {
    frames_per_batch: usize,
    current_batch: Vec<Frame>,
    sequence_counter: u64,
}

impl FrameBatcher {
    pub fn new(fps: u8, block_time_ms: u64) -> Self {
        let frames_per_batch = ((block_time_ms as f32 / 1000.0) * fps as f32).ceil() as usize;

        Self {
            frames_per_batch,
            current_batch: Vec::new(),
            sequence_counter: 0,
        }
    }

    pub fn add_frame(&mut self, frame: Frame) -> Option<FrameBatch> {
        self.current_batch.push(frame);

        if self.current_batch.len() >= self.frames_per_batch {
            let batch = FrameBatch {
                header: FrameBatchHeader {
                    magic: 0x4D4F4E47,
                    version: 1,
                    frame_count: self.current_batch.len() as u8,
                    compression_type: CompressionType::Zlib,
                    sequence_start: self.sequence_counter,
                },
                frames: std::mem::take(&mut self.current_batch),
            };

            self.sequence_counter += 1;
            Some(batch)
        } else {
            None
        }
    }
}
```

**crates/monegle-sender/src/blockchain.rs**

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, Bytes, U256};
use alloy::rpc::types::TransactionRequest;

pub struct BlockchainSender {
    provider: RootProvider,
    relay_address: Address,
    wallet_address: Address,
}

impl BlockchainSender {
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        relay_address: Address,
    ) -> Result<Self, anyhow::Error> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet_address = signer.address();
        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url.parse()?);

        Ok(Self {
            provider,
            relay_address,
            wallet_address,
        })
    }

    pub async fn submit_batch(&self, batch: FrameBatch, payment: U256) -> Result<B256, anyhow::Error> {
        // Encode batch to calldata
        let calldata = batch.encode_to_bytes()?;

        // Create transaction
        let tx = TransactionRequest::default()
            .to(self.relay_address)
            .input(Bytes::from(calldata).into())
            .value(payment);

        // Submit
        let pending = self.provider.send_transaction(tx).await?;
        let receipt = pending.get_receipt().await?;

        Ok(receipt.transaction_hash)
    }
}
```

### Verification

```bash
# Set environment variable
export MONAD_PRIVATE_KEY="0x..."

# Send test transaction
cargo run --bin monegle-sender -- --test-send

# Check transaction on Monad testnet explorer
```

## Phase 5-7: Re-broadcast Node (Days 12-18)

See [rebroadcast-node.md](./rebroadcast-node.md) for detailed implementation.

**Summary**:
1. Integrate Execution Events SDK
2. Filter transactions by address
3. Extract and decompress frames
4. Set up WebSocket server
5. Broadcast to connected clients

## Phase 8-9: Receiver (Days 19-22)

### Goals
- Connect to relay via WebSocket
- Buffer and synchronize frames
- Display in terminal with ratatui

### Files to Create

**crates/monegle-receiver/src/websocket.rs**

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::StreamExt;

pub struct WebSocketClient {
    url: String,
}

impl WebSocketClient {
    pub async fn connect(&self, tx: mpsc::Sender<FrameBatch>) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (_, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    let batch: FrameBatch = serde_json::from_str(&text)?;
                    tx.send(batch).await?;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        Ok(())
    }
}
```

**crates/monegle-receiver/src/display.rs**

```rust
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Constraint, Direction},
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{enable_raw_mode, disable_raw_mode},
};

pub struct TerminalDisplay {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl TerminalDisplay {
    pub fn new() -> Result<Self, anyhow::Error> {
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(std::io::stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    pub fn render_frame(&mut self, frame_data: &str, metadata: &str) -> Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(f.size());

            // ASCII frame
            let frame = Paragraph::new(frame_data)
                .block(Block::default().borders(Borders::ALL).title("Stream"));
            f.render_widget(frame, chunks[0]);

            // Metadata
            let info = Paragraph::new(metadata)
                .block(Block::default().borders(Borders::ALL).title("Info"));
            f.render_widget(info, chunks[1]);
        })?;

        Ok(())
    }
}

impl Drop for TerminalDisplay {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}
```

### Verification

```bash
# Terminal 1: Run relay
cargo run --bin monegle-relay

# Terminal 2: Run sender
cargo run --bin monegle-sender

# Terminal 3: Run receiver
cargo run --bin monegle-receiver

# Should see ASCII video streaming
```

## Phase 10: Integration Testing (Days 23-25)

### Goals
- End-to-end testing
- Performance benchmarking
- Cost verification

### Tests to Run

**1. Latency Test**
```bash
# Measure time from camera capture to display
./scripts/test-latency.sh

# Target: < 1 second total latency
```

**2. Compression Ratio Test**
```bash
# Test different compression methods
cargo test --package monegle-core -- compression --nocapture

# Target: 60-80% compression ratio
```

**3. Cost Test**
```bash
# Stream for 10 minutes, measure actual costs
cargo run --bin monegle-sender -- --duration 600

# Check Monad explorer for gas costs
# Compare with estimates from cost-analysis.md
```

**4. Stress Test**
```bash
# Connect 100 receivers to single relay
./scripts/stress-test.sh

# Monitor: CPU, RAM, bandwidth
```

## Phase 11: Documentation & Polish (Days 26-28)

### Goals
- User-facing documentation
- CLI improvements
- Configuration helpers

### Tasks

**1. README.md**

```markdown
# Monegle

Live ASCII video streaming on Monad blockchain.

## Quick Start

[Installation instructions]
[Configuration guide]
[Running your first stream]
```

**2. CLI Enhancements**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "monegle")]
#[command(about = "ASCII video streaming on Monad", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start streaming
    Stream {
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Receive stream
    Receive {
        #[arg(short, long)]
        stream_address: String,
    },

    /// Run relay node
    Relay {
        #[arg(short, long)]
        config: Option<String>,
    },

    /// Estimate costs
    EstimateCost {
        #[arg(short, long)]
        fps: u8,

        #[arg(short, long)]
        width: u16,

        #[arg(short, long)]
        height: u16,
    },
}
```

**3. Configuration Wizard**

```bash
cargo run --bin monegle -- setup
# Interactive wizard to create config.toml
```

## Phase 12: Deployment & Demo (Days 29-30)

### Goals
- Deploy relay to VPS
- Create demo video
- Write blog post

### Deployment Checklist

- [ ] Set up VPS with Monad node
- [ ] Deploy monegle-relay
- [ ] Configure firewall (allow port 8080)
- [ ] Test from remote receiver
- [ ] Monitor for 24 hours
- [ ] Document any issues

### Demo Preparation

```bash
# 1. Record demo video
./scripts/record-demo.sh

# 2. Create animated GIF
./scripts/create-demo-gif.sh

# 3. Publish to GitHub
git tag v0.1.0
git push origin v0.1.0
gh release create v0.1.0 --notes "Initial release"
```

## Timeline Summary

| Phase | Days | Description |
|-------|------|-------------|
| 0 | 1 | Project setup |
| 1 | 2 | Core types & config |
| 2 | 2 | Compression & encoding |
| 3 | 3 | Video capture & ASCII |
| 4 | 3 | Batching & blockchain |
| 5-7 | 7 | Re-broadcast node |
| 8-9 | 4 | Receiver |
| 10 | 3 | Integration testing |
| 11 | 3 | Documentation |
| 12 | 2 | Deployment & demo |
| **Total** | **30 days** | |

## Development Tips

### 1. Iterative Approach

Start with simplest version:
- No compression (Phase 2 can use identity function)
- No delta encoding (Phase 2 can skip)
- Fixed quality (configure later)

### 2. Testing Strategy

Test each component independently:
```bash
# Unit tests
cargo test --package monegle-core

# Integration tests
cargo test --workspace -- --test-threads=1

# Manual testing
cargo run --bin monegle-sender -- --dry-run
```

### 3. Debugging Tools

```bash
# Watch logs
RUST_LOG=debug cargo run --bin monegle-sender

# Monitor network
tcpdump -i any port 8080

# Profile performance
cargo flamegraph --bin monegle-relay
```

### 4. Common Issues

**Camera access denied**:
```bash
# macOS
sudo tccutil reset Camera

# Linux
sudo usermod -a -G video $USER
```

**Monad node connection failed**:
```bash
# Check node status
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  https://testnet-rpc.monad.xyz
```

**WebSocket connection refused**:
```bash
# Check relay is listening
netstat -tlnp | grep 8080

# Test with websocat
websocat ws://localhost:8080
```

## Next Steps After MVP

1. **Optimize Compression**: Implement custom ASCII-optimized codec
2. **Add Color**: ANSI 256-color support
3. **Stream Discovery**: Registry contract for finding streams
4. **Mobile Client**: React Native receiver app
5. **Monetization**: Pay-per-view with on-chain payments
6. **Mainnet**: Deploy when costs are viable

## Resources

- [Monad Documentation](https://docs.monad.xyz/)
- [Execution Events SDK Guide](https://docs.monad.xyz/execution-events/getting-started/)
- [Alloy Book](https://alloy.rs/)
- [Ratatui Tutorial](https://ratatui.rs/tutorials/hello-world/)

## Getting Help

- GitHub Issues: https://github.com/yourusername/monegle/issues
- Monad Discord: https://discord.gg/monad
- Rust Discord: https://discord.gg/rust-lang
