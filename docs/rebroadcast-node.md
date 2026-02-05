# Re-broadcast Node Design

Detailed design for the `monegle-relay` component that bridges Monad Execution Events to WebSocket streaming.

## Overview

The re-broadcast node (relay) is the critical middleware that:
1. Reads execution events from Monad node via shared memory
2. Filters for video stream transactions
3. Extracts and decompresses frame data
4. Re-broadcasts to multiple WebSocket clients
5. Tracks payments received from streamers

## Architecture

```
┌─────────────────────────────────────────────────┐
│         monegle-relay Process                   │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │  Event Listener Thread                   │  │
│  │  - Reads from shared memory              │  │
│  │  - Filters by target address             │  │
│  │  - Accumulates transaction data          │  │
│  └─────────────┬────────────────────────────┘  │
│                │ TxComplete events              │
│                ▼                                │
│  ┌──────────────────────────────────────────┐  │
│  │  Frame Processor Thread Pool (4 threads) │  │
│  │  - Deserialize frame batches             │  │
│  │  - Decompress (zlib, RLE, delta)         │  │
│  │  - Validate sequence numbers             │  │
│  └─────────────┬────────────────────────────┘  │
│                │ Decoded frames                 │
│                ▼                                │
│  ┌──────────────────────────────────────────┐  │
│  │  Stream Manager                          │  │
│  │  - Track active streams by sender        │  │
│  │  - Maintain per-stream state             │  │
│  │  - Handle stream lifecycle               │  │
│  └─────────────┬────────────────────────────┘  │
│                │ Frame batches                  │
│                ▼                                │
│  ┌──────────────────────────────────────────┐  │
│  │  WebSocket Server (Tokio + tungstenite)  │  │
│  │  - Accept client connections             │  │
│  │  - Handle subscriptions                  │  │
│  │  - Broadcast frames to subscribers       │  │
│  └─────────────┬────────────────────────────┘  │
└────────────────┼──────────────────────────────┘
                 │ WebSocket connections
                 ▼
         ┌───────────────┐
         │  Receivers    │
         └───────────────┘
```

## Core Components

### 1. Event Listener

**Purpose**: Read execution events from shared memory and filter for video transactions

**Implementation**:

```rust
use monad_exec_events::{EventRing, EventType, TxnHeaderStart, TxnData};

pub struct EventListener {
    ring: EventRing,
    target_address: [u8; 20],
    tx_channel: mpsc::Sender<CompletedTransaction>,
}

impl EventListener {
    pub async fn run(&mut self) -> Result<()> {
        let mut pending_txs: HashMap<[u8; 32], PendingTx> = HashMap::new();

        loop {
            let descriptor = self.ring.read_event()?;

            match descriptor.event_type {
                EventType::TxnHeaderStart => {
                    let header = self.ring.read_payload::<TxnHeaderStart>(&descriptor)?;

                    // Filter: only process txs to our address
                    if header.to != Some(self.target_address) {
                        continue;
                    }

                    pending_txs.insert(header.tx_hash, PendingTx {
                        sender: header.sender,
                        value: u256_from_bytes(&header.value),
                        data: Vec::with_capacity(header.data_len as usize),
                        timestamp_ns: descriptor.timestamp_ns,
                    });
                }

                EventType::TxnData => {
                    let data = self.ring.read_payload::<TxnData>(&descriptor)?;

                    // Find corresponding pending tx
                    // Note: SDK provides tx_hash context
                    if let Some(tx) = pending_txs.get_mut(&current_tx_hash) {
                        tx.data.extend_from_slice(&data.data);
                    }
                }

                EventType::TxnHeaderEnd => {
                    if let Some(tx) = pending_txs.remove(&current_tx_hash) {
                        // Transaction complete, send for processing
                        self.tx_channel.send(CompletedTransaction {
                            hash: current_tx_hash,
                            sender: tx.sender,
                            value: tx.value,
                            calldata: tx.data,
                            timestamp_ns: tx.timestamp_ns,
                        }).await?;
                    }
                }

                _ => {} // Ignore other events
            }
        }
    }
}

struct PendingTx {
    sender: [u8; 20],
    value: U256,
    data: Vec<u8>,
    timestamp_ns: u64,
}
```

**Configuration**:
```toml
[event_listener]
target_address = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
ring_path = "/dev/shm/monad_events"
max_pending = 100  # Max concurrent pending transactions
```

**Error Handling**:
- Sequence gaps: Log warning, continue
- Ring closed: Attempt reconnect
- Memory full: Drop oldest pending tx

### 2. Frame Processor

**Purpose**: Deserialize and decompress frame batches in parallel

**Implementation**:

```rust
use tokio::sync::mpsc;
use rayon::prelude::*;

pub struct FrameProcessor {
    workers: usize,
}

impl FrameProcessor {
    pub async fn process_transactions(
        &self,
        rx: mpsc::Receiver<CompletedTransaction>,
        tx: mpsc::Sender<DecodedFrameBatch>,
    ) {
        // Spawn worker pool
        let (work_tx, work_rx) = crossbeam::channel::unbounded();

        for _ in 0..self.workers {
            let work_rx = work_rx.clone();
            let tx = tx.clone();

            tokio::task::spawn_blocking(move || {
                for completed_tx in work_rx {
                    match Self::decode_transaction(completed_tx) {
                        Ok(batch) => {
                            let _ = tx.blocking_send(batch);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to decode tx: {:?}", e);
                        }
                    }
                }
            });
        }

        // Feed work to pool
        while let Some(tx) = rx.recv().await {
            work_tx.send(tx).unwrap();
        }
    }

    fn decode_transaction(tx: CompletedTransaction) -> Result<DecodedFrameBatch> {
        // 1. Parse binary format
        let header = FrameBatchHeader::from_bytes(&tx.calldata[0..16])?;

        if header.magic != 0x4D4F4E47 {
            return Err(Error::InvalidMagic);
        }

        // 2. Decompress based on type
        let decompressed = match header.compression_type {
            0 => tx.calldata[16..].to_vec(), // No compression
            1 => Self::decompress_delta_rle(&tx.calldata[16..], &header)?,
            2 => Self::decompress_zlib(&tx.calldata[16..])?,
            _ => return Err(Error::UnknownCompression),
        };

        // 3. Parse frames
        let frames = Self::parse_frames(&decompressed, &header)?;

        Ok(DecodedFrameBatch {
            sender: tx.sender,
            sequence_start: header.sequence_start,
            frames,
            timestamp_ns: tx.timestamp_ns,
            payment: tx.value,
        })
    }

    fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let mut decoder = ZlibDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }

    fn decompress_delta_rle(data: &[u8], header: &FrameBatchHeader) -> Result<Vec<u8>> {
        // Custom delta + RLE decompression
        // Implementation depends on compression format from sender
        unimplemented!("Delta RLE decompression")
    }

    fn parse_frames(data: &[u8], header: &FrameBatchHeader) -> Result<Vec<Frame>> {
        let mut frames = Vec::with_capacity(header.frame_count as usize);
        let mut offset = 0;

        for _ in 0..header.frame_count {
            let timestamp_ms = u32::from_le_bytes(data[offset..offset+4].try_into()?);
            offset += 4;

            // Read frame data (format depends on compression)
            // For delta encoding: read deltas and reconstruct
            // For full frames: read directly

            let frame = Frame {
                timestamp_ms,
                data: /* extracted frame data */,
            };

            frames.push(frame);
        }

        Ok(frames)
    }
}
```

**Configuration**:
```toml
[frame_processor]
workers = 4  # Number of parallel decompression workers
max_queue_size = 50  # Max queued transactions
```

**Performance**:
- Decompression: ~100 µs per batch (zlib level 6)
- Throughput: 10,000 batches/second per core
- Latency: < 1 ms end-to-end

### 3. Stream Manager

**Purpose**: Track active streams and maintain per-stream state

**Implementation**:

```rust
use std::collections::HashMap;

pub struct StreamManager {
    streams: HashMap<Address, StreamState>,
}

#[derive(Debug)]
pub struct StreamState {
    pub sender: Address,
    pub start_time: Instant,
    pub last_sequence: u64,
    pub fps: u8,
    pub width: u16,
    pub height: u16,
    pub total_frames: u64,
    pub total_payment: U256,
    pub subscriber_count: usize,
}

impl StreamManager {
    pub fn handle_frame_batch(&mut self, batch: DecodedFrameBatch) -> Result<()> {
        let stream = self.streams.entry(batch.sender).or_insert_with(|| {
            StreamState {
                sender: batch.sender,
                start_time: Instant::now(),
                last_sequence: 0,
                fps: Self::infer_fps(&batch),
                width: Self::infer_width(&batch),
                height: Self::infer_height(&batch),
                total_frames: 0,
                total_payment: U256::ZERO,
                subscriber_count: 0,
            }
        });

        // Validate sequence
        if batch.sequence_start != stream.last_sequence + 1 {
            tracing::warn!(
                "Sequence gap for stream {:?}: expected {}, got {}",
                batch.sender,
                stream.last_sequence + 1,
                batch.sequence_start
            );
        }

        stream.last_sequence = batch.sequence_start;
        stream.total_frames += batch.frames.len() as u64;
        stream.total_payment += batch.payment;

        tracing::debug!(
            "Stream {:?}: sequence {}, frames {}, payment {:?}",
            batch.sender,
            batch.sequence_start,
            batch.frames.len(),
            batch.payment
        );

        Ok(())
    }

    pub fn get_stream_info(&self, sender: &Address) -> Option<StreamInfo> {
        self.streams.get(sender).map(|s| StreamInfo {
            sender: s.sender,
            uptime_secs: s.start_time.elapsed().as_secs(),
            fps: s.fps,
            width: s.width,
            height: s.height,
            total_frames: s.total_frames,
            total_payment_mon: s.total_payment,
            viewers: s.subscriber_count,
        })
    }

    pub fn list_active_streams(&self) -> Vec<StreamInfo> {
        self.streams.values().map(|s| /* ... */).collect()
    }

    fn infer_fps(batch: &DecodedFrameBatch) -> u8 {
        // Infer from frames per batch and 400ms block time
        (batch.frames.len() as f32 / 0.4 * 1.0) as u8
    }
}
```

**Configuration**:
```toml
[stream_manager]
max_streams = 100  # Max concurrent streams
inactive_timeout_secs = 300  # Remove stream after 5 min inactivity
```

### 4. WebSocket Server

**Purpose**: Accept client connections and broadcast frames

**Implementation**:

```rust
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};

pub struct WebSocketServer {
    bind_addr: SocketAddr,
    clients: Arc<RwLock<HashMap<Uuid, Client>>>,
    stream_manager: Arc<Mutex<StreamManager>>,
}

struct Client {
    id: Uuid,
    sender: futures::channel::mpsc::UnboundedSender<Message>,
    subscribed_stream: Option<Address>,
}

impl WebSocketServer {
    pub async fn run(
        &mut self,
        mut frame_rx: mpsc::Receiver<DecodedFrameBatch>,
    ) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        tracing::info!("WebSocket server listening on {}", self.bind_addr);

        // Spawn frame broadcaster
        let clients = self.clients.clone();
        tokio::spawn(async move {
            while let Some(batch) = frame_rx.recv().await {
                Self::broadcast_frames(clients.clone(), batch).await;
            }
        });

        // Accept connections
        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::info!("New connection from {}", addr);

            let clients = self.clients.clone();
            let stream_manager = self.stream_manager.clone();

            tokio::spawn(async move {
                Self::handle_client(stream, clients, stream_manager).await;
            });
        }
    }

    async fn handle_client(
        stream: TcpStream,
        clients: Arc<RwLock<HashMap<Uuid, Client>>>,
        stream_manager: Arc<Mutex<StreamManager>>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                tracing::error!("WebSocket handshake failed: {:?}", e);
                return;
            }
        };

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let client_id = Uuid::new_v4();

        let (tx, mut rx) = futures::channel::mpsc::unbounded();

        // Add client
        clients.write().await.insert(client_id, Client {
            id: client_id,
            sender: tx,
            subscribed_stream: None,
        });

        // Send initial stream list
        let streams = stream_manager.lock().await.list_active_streams();
        let msg = serde_json::to_string(&StreamListMessage { streams }).unwrap();
        let _ = ws_sender.send(Message::Text(msg)).await;

        // Handle incoming messages and outgoing broadcasts
        loop {
            tokio::select! {
                // Receive from client
                Some(msg) = ws_receiver.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            Self::handle_client_message(
                                client_id,
                                &text,
                                &clients,
                                &stream_manager,
                            ).await;
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }

                // Send to client
                Some(msg) = rx.next() => {
                    if ws_sender.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }

        // Remove client
        clients.write().await.remove(&client_id);
        tracing::info!("Client {} disconnected", client_id);
    }

    async fn handle_client_message(
        client_id: Uuid,
        text: &str,
        clients: &Arc<RwLock<HashMap<Uuid, Client>>>,
        stream_manager: &Arc<Mutex<StreamManager>>,
    ) {
        let msg: ClientMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(_) => return,
        };

        match msg {
            ClientMessage::Subscribe { stream_address } => {
                let mut clients = clients.write().await;
                if let Some(client) = clients.get_mut(&client_id) {
                    client.subscribed_stream = Some(stream_address);

                    // Send stream info
                    if let Some(info) = stream_manager.lock().await.get_stream_info(&stream_address) {
                        let response = StreamInfoMessage { info };
                        let json = serde_json::to_string(&response).unwrap();
                        let _ = client.sender.unbounded_send(Message::Text(json));
                    }

                    tracing::info!("Client {} subscribed to {:?}", client_id, stream_address);
                }
            }

            ClientMessage::Unsubscribe => {
                let mut clients = clients.write().await;
                if let Some(client) = clients.get_mut(&client_id) {
                    client.subscribed_stream = None;
                }
            }

            ClientMessage::ListStreams => {
                let streams = stream_manager.lock().await.list_active_streams();
                let response = StreamListMessage { streams };
                let json = serde_json::to_string(&response).unwrap();

                if let Some(client) = clients.read().await.get(&client_id) {
                    let _ = client.sender.unbounded_send(Message::Text(json));
                }
            }
        }
    }

    async fn broadcast_frames(
        clients: Arc<RwLock<HashMap<Uuid, Client>>>,
        batch: DecodedFrameBatch,
    ) {
        let msg = FrameBatchMessage {
            sender: batch.sender,
            sequence: batch.sequence_start,
            frames: batch.frames,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let ws_msg = Message::Text(json);

        let clients = clients.read().await;
        for client in clients.values() {
            // Only send to subscribed clients
            if client.subscribed_stream == Some(batch.sender) {
                let _ = client.sender.unbounded_send(ws_msg.clone());
            }
        }
    }
}
```

**WebSocket Protocol**:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    Subscribe { stream_address: Address },
    Unsubscribe,
    ListStreams,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ServerMessage {
    StreamList { streams: Vec<StreamInfo> },
    StreamInfo { info: StreamInfo },
    FrameBatch { sender: Address, sequence: u64, frames: Vec<Frame> },
}
```

**Configuration**:
```toml
[websocket]
bind_address = "0.0.0.0:8080"
max_connections = 1000
ping_interval_secs = 30
```

## Deployment

### System Requirements

**Hardware**:
- CPU: 8 cores (4 for node, 4 for relay)
- RAM: 32 GB (24 GB for node, 8 GB for relay)
- Storage: 500 GB SSD
- Network: 1 Gbps

**Software**:
- OS: Linux (Ubuntu 22.04 or Debian 12)
- Monad node v1.0+
- Rust 1.75+

### Installation

```bash
# 1. Install dependencies
sudo apt-get update
sudo apt-get install build-essential libhugetlbfs-dev libzstd-dev

# 2. Install Monad node (follow official docs)
# ...

# 3. Build monegle-relay
cd monegle
cargo build --release --bin monegle-relay

# 4. Configure
cp config/relay.example.toml config/relay.toml
# Edit relay.toml with your operator address

# 5. Run
./target/release/monegle-relay --config config/relay.toml
```

### Docker Deployment

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin monegle-relay

FROM ubuntu:22.04
RUN apt-get update && apt-get install -y libhugetlbfs0 libzstd1
COPY --from=builder /app/target/release/monegle-relay /usr/local/bin/
ENTRYPOINT ["monegle-relay"]
```

```bash
docker build -t monegle-relay .
docker run -d \
  --name relay \
  -v /dev/shm:/dev/shm \
  -v ./config:/etc/monegle \
  -p 8080:8080 \
  monegle-relay --config /etc/monegle/relay.toml
```

### Monitoring

**Metrics to Track**:
- Events processed per second
- Transactions decoded per second
- WebSocket connections
- Frames broadcast per second
- Latency (event → broadcast)
- Payment received (total MON)

**Prometheus Metrics**:
```rust
use prometheus::{IntCounter, Histogram, register_int_counter, register_histogram};

lazy_static! {
    static ref EVENTS_PROCESSED: IntCounter =
        register_int_counter!("relay_events_processed_total", "Total events processed").unwrap();

    static ref FRAMES_DECODED: IntCounter =
        register_int_counter!("relay_frames_decoded_total", "Total frames decoded").unwrap();

    static ref WS_CONNECTIONS: IntGauge =
        register_int_gauge!("relay_websocket_connections", "Active WebSocket connections").unwrap();

    static ref LATENCY: Histogram =
        register_histogram!("relay_latency_seconds", "Event to broadcast latency").unwrap();
}
```

**Grafana Dashboard**: See `docs/grafana-dashboard.json`

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_decompression() {
        let compressed = create_test_frame_batch();
        let decoded = FrameProcessor::decode_transaction(compressed).unwrap();
        assert_eq!(decoded.frames.len(), 6);
    }

    #[tokio::test]
    async fn test_websocket_broadcast() {
        let server = WebSocketServer::new("127.0.0.1:0".parse().unwrap());
        // Test broadcast to multiple clients
    }
}
```

### Integration Tests

```bash
# Terminal 1: Start mock Monad node
./scripts/mock-monad-node.sh

# Terminal 2: Start relay
cargo run --bin monegle-relay

# Terminal 3: Send test transactions
./scripts/send-test-tx.sh

# Terminal 4: Connect receiver
cargo run --bin monegle-receiver -- --relay ws://localhost:8080
```

## References

- [Monad Execution Events Documentation](https://docs.monad.xyz/execution-events/)
- [tokio-tungstenite WebSocket Library](https://docs.rs/tokio-tungstenite/)
- [Rayon Parallel Processing](https://docs.rs/rayon/)
