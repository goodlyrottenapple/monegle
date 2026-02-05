# Execution Events SDK Integration

Guide for integrating Monad's Execution Events SDK into the re-broadcast node.

## Overview

The Execution Events SDK allows applications to observe EVM actions in real-time via shared memory IPC, providing the lowest-latency access to transaction data **before** finalization on-chain.

### Key Characteristics

- **Language Support**: C and Rust
- **Performance**: Shared memory IPC (microsecond latency)
- **Platform**: Linux required (partial macOS support for historical data only)
- **Deployment**: Must run on same host as Monad node
- **Access Pattern**: Pre-consensus event streaming

## SDK Architecture

### Event Structure

Every execution event consists of two parts:

1. **Event Descriptor** (64 bytes, fixed size):
   - Sequence number (for gap detection)
   - Event type code
   - Timestamp (nanoseconds)
   - Payload size and location
   - Content extensions

2. **Event Payload** (variable size):
   - Event-specific data
   - For transaction data: tx hash, sender, calldata, etc.

### Key Event Types for Monegle

| Event Type | Description | Relevance |
|------------|-------------|-----------|
| `TXN_HEADER_START` | Transaction begins processing | Filter for target address |
| `TXN_DATA` | Transaction calldata | **Contains video frames** |
| `TXN_HEADER_END` | Transaction processing complete | Signal to process batch |
| `LOG_ENTRY` | Contract log event | Not needed (no contract) |

## Rust API Reference

### Crate: `monad-exec-events`

*Note: This crate is distributed with Monad node software, not on crates.io*

```toml
# Add to monegle-relay/Cargo.toml
[dependencies]
monad-exec-events = { path = "/path/to/monad/sdk/monad-exec-events" }
monad-event-ring = { path = "/path/to/monad/sdk/monad-event-ring" }
```

### Core Types

```rust
use monad_exec_events::{EventRing, EventDescriptor, EventType};

// Event ring handle
pub struct EventRing { /* ... */ }

// Event descriptor (64 bytes)
pub struct EventDescriptor {
    pub sequence: u64,
    pub event_type: EventType,
    pub timestamp_ns: u64,
    pub payload_size: u32,
    // ...
}

// Event types
pub enum EventType {
    TxnHeaderStart,
    TxnData,
    TxnHeaderEnd,
    // ...
}

// Transaction header event
pub struct TxnHeaderStart {
    pub tx_hash: [u8; 32],      // Keccak hash
    pub sender: [u8; 20],        // Address
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee: u128,
    pub value: [u8; 32],         // U256
    pub to: Option<[u8; 20]>,    // Recipient address
    pub data_len: u32,           // Length of tx.data
    // ...
}

// Transaction data event
pub struct TxnData {
    pub offset: u32,             // Offset in tx.data
    pub data: Vec<u8>,           // Chunk of calldata
}
```

## Integration Pattern for Monegle

### 1. Initialize Event Ring

```rust
use monad_exec_events::EventRing;

pub struct EventsListener {
    ring: EventRing,
    target_address: [u8; 20],  // Address to filter for
}

impl EventsListener {
    pub fn new(target_address: [u8; 20]) -> Result<Self> {
        // Open shared memory event ring
        let ring = EventRing::open("/dev/shm/monad_events")?;

        Ok(Self {
            ring,
            target_address,
        })
    }
}
```

### 2. Event Loop

```rust
use monad_exec_events::{EventDescriptor, EventType};

impl EventsListener {
    pub async fn process_events(&mut self) -> Result<()> {
        let mut current_tx: Option<TransactionContext> = None;

        loop {
            // Read next event from ring buffer
            let descriptor = self.ring.read_event()?;

            match descriptor.event_type {
                EventType::TxnHeaderStart => {
                    let header = self.ring.read_payload::<TxnHeaderStart>(&descriptor)?;

                    // Check if this tx is to our target address
                    if header.to == Some(self.target_address) {
                        current_tx = Some(TransactionContext {
                            tx_hash: header.tx_hash,
                            sender: header.sender,
                            value: header.value,
                            data: Vec::with_capacity(header.data_len as usize),
                        });
                    }
                }

                EventType::TxnData => {
                    if let Some(ref mut tx) = current_tx {
                        let data = self.ring.read_payload::<TxnData>(&descriptor)?;
                        tx.data.extend_from_slice(&data.data);
                    }
                }

                EventType::TxnHeaderEnd => {
                    if let Some(tx) = current_tx.take() {
                        // Process complete transaction
                        self.handle_stream_transaction(tx).await?;
                    }
                }

                _ => {
                    // Ignore other event types
                }
            }
        }
    }

    async fn handle_stream_transaction(&self, tx: TransactionContext) -> Result<()> {
        // tx.data contains the compressed video frames
        // Extract and forward to WebSocket clients
        println!("Received stream tx: {:?}", hex::encode(tx.tx_hash));
        println!("  From: {:?}", hex::encode(tx.sender));
        println!("  Data size: {} bytes", tx.data.len());

        // Decompress and broadcast (see rebroadcast-node.md)
        // ...

        Ok(())
    }
}

struct TransactionContext {
    tx_hash: [u8; 32],
    sender: [u8; 20],
    value: [u8; 32],
    data: Vec<u8>,
}
```

### 3. Address Filtering

Since there's no contract, we filter by the recipient address (`tx.to`):

```rust
// Configuration
pub struct RelayConfig {
    pub target_address: [u8; 20],  // Address streamers send to
    pub operator_address: [u8; 20], // Relay operator's address (same as target)
}

impl RelayConfig {
    pub fn from_operator_keypair(keypair: &Keypair) -> Self {
        let address = keypair.address().to_fixed_bytes();
        Self {
            target_address: address,
            operator_address: address,
        }
    }
}
```

### 4. Error Handling

```rust
use monad_exec_events::EventRingError;

impl EventsListener {
    pub async fn run_with_recovery(&mut self) -> ! {
        loop {
            match self.process_events().await {
                Ok(_) => unreachable!("event loop should never exit normally"),
                Err(EventRingError::SequenceGap { expected, got }) => {
                    tracing::warn!("Detected sequence gap: expected {}, got {}", expected, got);
                    // Log warning but continue - we can tolerate missing frames
                    continue;
                }
                Err(EventRingError::RingClosed) => {
                    tracing::error!("Event ring closed, attempting reconnect...");
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    self.reconnect().await;
                }
                Err(e) => {
                    tracing::error!("Fatal error in event loop: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    async fn reconnect(&mut self) {
        for attempt in 1..=5 {
            match EventRing::open("/dev/shm/monad_events") {
                Ok(ring) => {
                    self.ring = ring;
                    tracing::info!("Reconnected to event ring");
                    return;
                }
                Err(e) => {
                    tracing::warn!("Reconnect attempt {} failed: {:?}", attempt, e);
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
            }
        }
        panic!("Failed to reconnect after 5 attempts");
    }
}
```

## System Requirements

### 1. Monad Node Setup

The relay node must run alongside a Monad node with execution events enabled:

```toml
# monad.toml (node configuration)
[execution]
exec-event-ring = true

[rpc]
ws-enabled = true  # Optional, for fallback
```

### 2. Shared Memory Configuration

Execution events use `hugetlbfs` for performance:

```bash
# Mount hugetlbfs
sudo mkdir -p /mnt/huge
sudo mount -t hugetlbfs hugetlbfs /mnt/huge

# Install dependencies
sudo apt-get install libhugetlbfs-dev libhugetlbfs0 libzstd-dev
```

### 3. Permissions

The relay process must have read access to the event ring:

```bash
# Add user to monad group
sudo usermod -a -G monad $(whoami)

# Verify permissions
ls -l /dev/shm/monad_events*
```

## Performance Considerations

### Latency Characteristics

- **Event availability**: Pre-consensus (before block finalization)
- **IPC latency**: < 1 microsecond (shared memory)
- **Processing overhead**: ~10-100 microseconds (decompression)
- **Total relay latency**: < 1 millisecond (event → WebSocket)

This is **significantly faster** than polling via RPC (400ms+ block time).

### Throughput

- Event ring capacity: ~1GB (configurable)
- Can handle 10,000+ TPS on modern hardware
- Monegle stream: ~2.5 transactions/second (not a bottleneck)

### CPU Usage

- Event loop: Single thread, < 5% CPU idle
- Decompression: Parallel per transaction, ~10% CPU under load
- WebSocket broadcasting: Scales with viewer count

## Testing Without Monad Node

For development without access to a Monad node:

### Option 1: Historical Data Replay

```rust
use monad_exec_events::HistoricalEventReader;

// Read from file (works on macOS)
let reader = HistoricalEventReader::open("testdata/events.bin")?;

for event in reader.iter() {
    // Process event as normal
}
```

### Option 2: Mock Event Generator

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_filtering() {
        let mut listener = MockEventsListener::new();

        // Inject mock transaction
        listener.inject_transaction(TransactionContext {
            tx_hash: [1u8; 32],
            sender: [2u8; 20],
            value: [0u8; 32],
            data: create_test_frame_batch(),
        });

        // Verify processing
        let result = listener.handle_stream_transaction(/* ... */).await;
        assert!(result.is_ok());
    }
}
```

## Deployment Architecture

```
┌─────────────────────────────────────┐
│  Host Machine (Linux)               │
│                                     │
│  ┌───────────────┐                 │
│  │  Monad Node   │                 │
│  │  (Category)   │                 │
│  └───────┬───────┘                 │
│          │                          │
│          │ Shared Memory            │
│          │ /dev/shm/monad_events    │
│          ▼                          │
│  ┌───────────────┐                 │
│  │ monegle-relay │                 │
│  │ - Events SDK  │                 │
│  │ - WS Server   │                 │
│  └───────┬───────┘                 │
└──────────┼─────────────────────────┘
           │
           │ TCP/IP
           │ Port 8080 (WebSocket)
           ▼
    ┌──────────────┐
    │  Receivers   │
    │  (anywhere)  │
    └──────────────┘
```

## Troubleshooting

### Event Ring Not Found

```
Error: Failed to open event ring: No such file or directory
```

**Solution**: Verify Monad node is running with `--exec-event-ring` flag

### Permission Denied

```
Error: Failed to open event ring: Permission denied
```

**Solution**: Add user to `monad` group and restart shell

### Sequence Gaps

```
Warning: Detected sequence gap: expected 12345, got 12347
```

**Solution**: Normal under high load; relay will continue with minor frame loss

### Ring Buffer Full

```
Error: Event ring full, reader too slow
```

**Solution**: Increase processing speed or reduce stream quality

## References

- [Monad Execution Events Documentation](https://docs.monad.xyz/execution-events/)
- [Getting Started Guide](https://docs.monad.xyz/execution-events/getting-started/)
- [Node Operations - Events Setup](https://docs.monad.xyz/node-ops/events-and-websockets)
- [Rust API Reference](https://docs.monad.xyz/execution-events/rust-api/)
