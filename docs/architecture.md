# Monegle Architecture

Detailed system architecture for ASCII video streaming on Monad blockchain.

## System Overview

Monegle consists of four main components communicating through two distinct channels:

1. **Monad Blockchain** - Transport layer for video data
2. **Shared Memory IPC** - High-speed event delivery (Execution Events SDK)
3. **WebSocket Protocol** - Relay to receiver streaming
4. **Terminal UI** - ASCII video display

```
┌──────────────┐
│ Video Camera │
└──────┬───────┘
       │ Raw Frames (15 FPS @ 640x480)
       ▼
┌──────────────────────────────────────────┐
│         monegle-sender (Rust)            │
│  ┌─────────────────────────────────┐    │
│  │  1. Video Capture (nokhwa)      │    │
│  │     - Open camera device         │    │
│  │     - Capture at configured FPS  │    │
│  └──────────┬──────────────────────┘    │
│             │                             │
│             ▼                             │
│  ┌─────────────────────────────────┐    │
│  │  2. ASCII Conversion            │    │
│  │     - Resize to 80x60           │    │
│  │     - Grayscale conversion      │    │
│  │     - Map brightness → chars    │    │
│  └──────────┬──────────────────────┘    │
│             │                             │
│             ▼                             │
│  ┌─────────────────────────────────┐    │
│  │  3. Compression Pipeline        │    │
│  │     - Delta encoding (vs prev)  │    │
│  │     - RLE for repeated chars    │    │
│  │     - Zlib compression          │    │
│  └──────────┬──────────────────────┘    │
│             │                             │
│             ▼                             │
│  ┌─────────────────────────────────┐    │
│  │  4. Frame Batching              │    │
│  │     - Buffer 6 frames (400ms)   │    │
│  │     - Sequence numbering        │    │
│  │     - Binary serialization      │    │
│  └──────────┬──────────────────────┘    │
│             │                             │
│             ▼                             │
│  ┌─────────────────────────────────┐    │
│  │  5. Transaction Creation        │    │
│  │     - Encode batch to calldata  │    │
│  │     - Add payment (tx.value)    │    │
│  │     - Sign with private key     │    │
│  │     - Submit to RPC endpoint    │    │
│  └──────────┬──────────────────────┘    │
└─────────────┼──────────────────────────┘
              │
              │ Transaction
              │ - to: relay_operator_address
              │ - data: compressed_frames (2-4 KB)
              │ - value: 0.001 MON (payment)
              │
              ▼
┌─────────────────────────────────────────┐
│       Monad Blockchain (Testnet)        │
│  ┌─────────────────────────────────┐   │
│  │  Transaction Pool               │   │
│  │  - Pending transactions         │   │
│  │  - Nonce ordering              │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  Consensus & Ordering           │   │
│  │  - 400ms block time             │   │
│  │  - Transaction ordering         │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  Parallel Execution             │   │
│  │  - Process transactions         │   │
│  │  - Generate execution events    │   │
│  └──────────┬──────────────────────┘   │
└─────────────┼──────────────────────────┘
              │
              │ Execution Events (shared memory)
              │ - TXN_HEADER_START
              │ - TXN_DATA (calldata)
              │ - TXN_HEADER_END
              │
              ▼
┌─────────────────────────────────────────┐
│    monegle-relay (Co-located with node) │
│  ┌─────────────────────────────────┐   │
│  │  1. Execution Events Listener   │   │
│  │     - Read from shared memory    │   │
│  │     - Filter by target address   │   │
│  │     - Accumulate tx.data         │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  2. Frame Extraction            │   │
│  │     - Deserialize frame batch    │   │
│  │     - Verify sequence numbers    │   │
│  │     - Track sender address       │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  3. Decompression               │   │
│  │     - Zlib decompress            │   │
│  │     - Decode RLE                 │   │
│  │     - Reconstruct delta frames   │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  4. Payment Processing          │   │
│  │     - Track tx.value received    │   │
│  │     - Log earnings per stream    │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  5. WebSocket Server            │   │
│  │     - Manage client connections  │   │
│  │     - Broadcast frames           │   │
│  │     - Handle subscriptions       │   │
│  └──────────┬──────────────────────┘   │
└─────────────┼──────────────────────────┘
              │
              │ WebSocket Protocol
              │ - JSON messages
              │ - Frame broadcasts
              │ - Stream metadata
              │
              ▼
┌─────────────────────────────────────────┐
│        monegle-receiver (Rust)          │
│  ┌─────────────────────────────────┐   │
│  │  1. WebSocket Client            │   │
│  │     - Connect to relay           │   │
│  │     - Subscribe to stream        │   │
│  │     - Receive frame messages     │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  2. Frame Buffering             │   │
│  │     - Circular buffer (100 fr)   │   │
│  │     - Sequence ordering          │   │
│  │     - Gap detection              │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  3. Playback Timing             │   │
│  │     - Calculate frame timing     │   │
│  │     - Jitter compensation        │   │
│  │     - Skip/repeat for sync       │   │
│  └──────────┬──────────────────────┘   │
│             │                            │
│             ▼                            │
│  ┌─────────────────────────────────┐   │
│  │  4. Terminal Rendering          │   │
│  │     - Ratatui UI framework       │   │
│  │     - ASCII frame display        │   │
│  │     - Metadata overlay           │   │
│  └──────────┬──────────────────────┘   │
└─────────────┼──────────────────────────┘
              │
              ▼
       ┌──────────────┐
       │   Terminal   │
       │   Display    │
       └──────────────┘
```

## Component Details

### 1. Sender (monegle-sender)

**Purpose**: Capture video, convert to ASCII, compress, and submit to blockchain

**Key Modules**:
- `capture.rs` - Video capture using nokhwa
- `converter.rs` - ASCII conversion pipeline
- `compressor.rs` - Multi-stage compression
- `batcher.rs` - Frame batching logic
- `blockchain.rs` - Transaction submission

**Threading Model**:
```rust
tokio::spawn(capture_loop());      // Captures frames at 15 FPS
tokio::spawn(conversion_loop());   // Converts to ASCII
tokio::spawn(compression_loop());  // Compresses frames
tokio::spawn(batching_loop());     // Batches and submits txs
```

**Data Flow**:
```
Camera → FrameQueue(10) → AsciiQueue(10) → CompressedQueue(5) → Blockchain
```

### 2. Re-broadcast Node (monegle-relay)

**Purpose**: Listen for execution events, extract frames, re-broadcast via WebSocket

**Key Modules**:
- `events.rs` - Execution Events SDK integration
- `extractor.rs` - Frame extraction and deserialization
- `decompressor.rs` - Frame decompression
- `websocket.rs` - WebSocket server implementation
- `payment.rs` - Payment tracking

**Threading Model**:
```rust
tokio::spawn(events_listener());    // Reads from shared memory
tokio::spawn(frame_processor());    // Decompresses frames
tokio::spawn(websocket_server());   // Handles WS connections
```

**Requirements**:
- **Must** run on same host as Monad node (shared memory access)
- Linux required (partial macOS support for testing with historical data)
- Needs read access to `/dev/shm/monad_events`

### 3. Receiver (monegle-receiver)

**Purpose**: Connect to relay, buffer frames, display as ASCII video

**Key Modules**:
- `websocket.rs` - WebSocket client
- `buffer.rs` - Circular frame buffer
- `player.rs` - Playback timing logic
- `display.rs` - Terminal UI (ratatui)

**Threading Model**:
```rust
tokio::spawn(websocket_receiver()); // Receives frames from relay
tokio::spawn(playback_loop());      // Displays at target FPS
```

**Platform Support**: Linux, macOS, Windows (anywhere with terminal)

## Data Formats

### Frame Batch Encoding (Sender → Blockchain)

**Binary Format** (optimized for calldata):

```
[Header: 8 bytes]
  - magic: u32 = 0x4D4F4E47 ("MONG")
  - version: u8 = 1
  - frame_count: u8
  - compression_type: u8 (0=None, 1=Delta+RLE, 2=Zlib)
  - reserved: u8

[Sequence: 8 bytes]
  - sequence_start: u64

[Frames: Variable]
  For each frame:
    [Frame Header: 4 bytes]
      - timestamp_ms: u32 (relative to batch start)

    [Frame Data: Variable]
      - delta_count: u16 (if delta encoding)
      - deltas: [(pos: u16, char: u8), ...]
      OR
      - full_data: [u8; width * height]
```

**Example** (15 FPS, 80×60, 6 frames per batch):
- Batch header: 16 bytes
- 6 frames with delta encoding (~300 bytes each): 1,800 bytes
- **Total before zlib**: ~1,816 bytes
- **Total after zlib (level 6)**: ~800-1,200 bytes

### WebSocket Protocol (Relay → Receiver)

**JSON Messages**:

```json
// Stream metadata (on connection)
{
  "type": "stream_info",
  "stream_id": "0x1234...",
  "sender": "0xabcd...",
  "fps": 15,
  "width": 80,
  "height": 60,
  "start_time": 1704067200000
}

// Frame batch
{
  "type": "frame_batch",
  "sequence": 12345,
  "frames": [
    {
      "timestamp": 1704067201234,
      "data": "    ████    \n  ██    ██  \n..."  // ASCII art
    },
    // ... 5 more frames
  ]
}

// Stream ended
{
  "type": "stream_end",
  "reason": "sender_disconnect"
}
```

**Binary Alternative** (future optimization):
- Use MessagePack or Protocol Buffers instead of JSON
- Reduces bandwidth by ~30%

## Network Architecture

### Sender → Blockchain

**Connection**: HTTPS RPC (e.g., `https://testnet-rpc.monad.xyz`)

**Transaction Pattern**:
- **Frequency**: 2.5 transactions/second (every 400ms block)
- **Size**: 800-1,200 bytes calldata per tx
- **Gas**: ~50,000 gas per tx (mostly calldata)
- **Cost**: ~$0.003-0.005 per tx

**Error Handling**:
- Exponential backoff on RPC errors
- Nonce management with local tracking
- Max 10 pending transactions

### Relay ↔ Blockchain

**Connection**: Shared memory IPC (local)

**Event Flow**:
1. Monad node writes events to `/dev/shm/monad_events`
2. Relay reads via `EventRing::read_event()`
3. **Latency**: < 1 millisecond

**Advantages over RPC**:
- 400x lower latency (1ms vs 400ms)
- Pre-consensus access (events before block finalization)
- No polling overhead

### Relay → Receiver(s)

**Connection**: WebSocket (TCP)

**Protocol**: `wss://relay-host:8080/stream`

**Scaling**:
- Single relay can handle 1,000+ concurrent viewers
- Each viewer: ~10 KB/s bandwidth (15 FPS, 80×60)
- Total: ~10 MB/s for 1,000 viewers

**Load Balancing** (future):
- Multiple relay nodes reading same events
- Receivers connect to nearest relay
- No blockchain overhead (all read from shared memory)

## Security Considerations

### Sender

**Threats**:
- Private key exposure → Use environment variables, never commit
- Camera hijacking → OS-level permissions, user consent

**Mitigations**:
- Store keys in secure keystore (future: hardware wallet)
- Require explicit camera permission
- Log all transactions for audit

### Relay Node

**Threats**:
- Malicious transactions (DoS with large calldata)
- Payment theft (operator steals extra MON)
- Stream hijacking (operator modifies frames)

**Mitigations**:
- Rate limiting per sender address (max 10 tx/second)
- Payment automatically received via tx.value (trustless)
- Read-only access to blockchain (cannot modify)
- Optional: Signatures on frame batches for authenticity

### Receiver

**Threats**:
- Malicious relay (sends corrupted frames)
- MitM attack on WebSocket connection

**Mitigations**:
- Use WSS (WebSocket Secure) with TLS
- Verify frame sequence numbers (detect tampering)
- Optional: Verify sender signatures on frames

## Scalability Analysis

### Current Design (MVP)

**Capacity**:
- 1 sender → 1 relay → 1,000 receivers
- Bottleneck: Relay WebSocket broadcasting

**Cost per Hour** (15 FPS, 80×60):
- Blockchain: ~$90 (2.5 tx/s × $0.01/tx × 3600s)
- Relay hosting: ~$5 (VPS with Monad node)
- **Total**: ~$95/hour

### Scaling to Multiple Streams

**Option 1: Shared Relay**
- One relay watches all transactions to its address
- Differentiates streams by sender address
- Receivers subscribe to specific sender

**Capacity**: 100+ concurrent streams on one relay

**Option 2: Dedicated Relays**
- Each streamer runs their own relay
- More decentralized, no single point of failure

**Cost**: +$5/hour per stream (relay hosting)

### Scaling to Mainnet

**Considerations**:
- Mainnet gas prices likely higher than testnet
- Execution Events SDK should work identically
- May need more efficient compression (reduce calldata)

**Estimated Cost** (if mainnet gas = 10x testnet):
- ~$900/hour for 15 FPS, 80×60
- Consider lower FPS (10 FPS → $600/hour)
- Or use Layer 2 (future)

## Alternative Architectures Considered

### 1. Pure On-chain (Rejected)

**Design**: Store frames in contract storage or events

**Pros**: Permanent storage, easy to query
**Cons**:
- Much higher gas costs (storage = 20,000 gas/word)
- Slower (must wait for finality)
- Doesn't leverage Execution Events

### 2. IPFS + On-chain Pointers (Rejected)

**Design**: Store frames on IPFS, put CIDs on-chain

**Pros**: Lower on-chain cost, permanent storage
**Cons**:
- IPFS retrieval latency (100ms-1s)
- Too slow for real-time streaming
- Adds complexity

### 3. P2P Streaming (Future Consideration)

**Design**: Receivers connect directly to sender via WebRTC

**Pros**: No relay needed, fully decentralized
**Cons**:
- Sender must be online continuously
- NAT traversal complexity
- Blockchain only used for signaling

**Verdict**: Consider for future version

## Deployment Models

### Model 1: Hobbyist (MVP)

```
┌────────────────┐      ┌──────────────────┐
│  Home Laptop   │ ───→ │  VPS (relay)     │
│  - Sender      │      │  - Monad node    │
│                │      │  - monegle-relay │
└────────────────┘      └─────────┬────────┘
                                  │
                        ┌─────────┴─────────┐
                        │                   │
                   ┌────▼────┐         ┌───▼────┐
                   │ Viewer  │         │ Viewer │
                   └─────────┘         └────────┘
```

**Cost**: ~$100/month (VPS + blockchain)

### Model 2: Service Provider

```
┌──────────────┐     ┌─────────────────────┐
│  Streamer 1  │─┐   │  Relay Infrastructure│
├──────────────┤ ├──→│  - Load balancer     │
│  Streamer 2  │─┤   │  - Multiple relays   │
├──────────────┤ │   │  - Shared Monad node │
│  Streamer N  │─┘   └─────────┬───────────┘
└──────────────┘               │
                     ┌─────────┴─────────┐
                     │                   │
                ┌────▼────┐         ┌───▼────┐
                │ Viewers │         │ Viewers│
                │ (1000s) │         │ (1000s)│
                └─────────┘         └────────┘
```

**Revenue Model**:
- Streamer pays $0.01/tx to relay operator
- Operator provides infrastructure
- Viewers watch for free

**Cost**: $500-1000/month (dedicated infrastructure)
**Revenue**: $0.01/tx × 2.5 tx/s × N streamers

## Future Enhancements

1. **Multi-quality Streams**: Relay transcodes to multiple resolutions
2. **Recording**: Relay stores frames to S3 for replay
3. **Authentication**: Paid streams with JWT tokens
4. **Color Support**: ANSI 256-color terminals
5. **Mobile Clients**: React Native terminal emulator
6. **Mesh Relays**: Relays sync via gossip protocol
7. **Mainnet Deployment**: Optimize for mainnet gas costs

## References

- [Monad Architecture Documentation](https://docs.monad.xyz/monad-arch/)
- [Execution Events Overview](https://docs.monad.xyz/execution-events/overview)
- [Real-Time Data Sources](https://docs.monad.xyz/monad-arch/realtime-data/data-sources)
