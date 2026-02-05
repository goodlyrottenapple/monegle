# Feasibility Test Plan: RPC Throughput Testing

## Objective

Test whether Monad testnet RPC endpoints can handle continuous transaction submission at 2.5 tx/second (required for video streaming) without rate limiting or bans.

## Research Questions

1. **Can a single RPC endpoint handle 2.5 tx/s sustained load?**
2. **What are the actual rate limits of public RPC endpoints?**
3. **Do we need RPC rotation to avoid rate limiting?**
4. **What is the actual transaction confirmation time under load?**
5. **What is the failure rate (dropped/reverted transactions)?**

## Test Approach

### Phase 1: Minimal Sender Implementation (Week 1)

Build **only** the sender components needed for testing:

```
monegle/
├── crates/
│   ├── monegle-core/          # Types & codec only
│   └── monegle-sender-test/   # Minimal sender for testing
└── scripts/
    └── analyze-test-results.py
```

**What to build:**
- Frame batch encoding (no real video, use synthetic frames)
- Transaction creation and submission
- RPC client with metrics tracking
- Test harness to run experiments

**What to skip:**
- Video capture (use fake frames)
- ASCII conversion (generate dummy ASCII)
- Receiver/relay (not needed for RPC testing)

### Phase 2: Testbench Setup (Day 8)

Create test scenarios with varying parameters:

| Test | FPS | Resolution | Tx/s | Duration | Purpose |
|------|-----|------------|------|----------|---------|
| Baseline | 10 | 40×30 | 2.5 | 5 min | Verify basic throughput |
| Medium | 15 | 80×60 | 2.5 | 10 min | Realistic load test |
| High | 24 | 120×80 | 2.5 | 10 min | Maximum data size |
| Stress | 30 | 120×80 | 2.5 | 30 min | Long-duration test |
| Burst | 15 | 80×60 | 5.0 | 5 min | Test rate limit response |

### Phase 3: RPC Testing (Week 2)

Test multiple RPC providers:

**Public Endpoints:**
1. Official Monad testnet: `https://testnet-rpc.monad.xyz`
2. Chainstack: `https://monad-testnet.chainstack.com/...`
3. BlockPI: `https://monad-testnet.blockpi.network/...`
4. Alchemy (if available)

**Test each endpoint:**
- Sustained 2.5 tx/s for 10 minutes
- Measure success rate, latency, errors
- Detect rate limiting (429 errors, connection drops)

### Phase 4: Analysis & Mitigation (Week 2)

Based on results, design mitigation strategy:

**Option A: RPC Rotation**
- Round-robin across multiple endpoints
- Fallback on failure
- Track per-endpoint metrics

**Option B: Quality Adjustment**
- Reduce FPS if rate limited
- Adaptive batch size
- Dynamic compression level

**Option C: Paid RPC Service**
- Use dedicated Alchemy/Chainstack plan
- Worth it if public endpoints fail

## Implementation Details

### Component 1: Synthetic Frame Generator

**Purpose**: Generate fake ASCII frames without needing a camera

**File**: `crates/monegle-core/src/synthetic.rs`

```rust
use crate::types::{Frame, FrameBatch, FrameBatchHeader, CompressionType};
use rand::Rng;

pub struct SyntheticFrameGenerator {
    width: u16,
    height: u16,
    frame_count: u64,
}

impl SyntheticFrameGenerator {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
        }
    }

    /// Generate a random ASCII frame (for testing compression)
    pub fn generate_frame(&mut self) -> Frame {
        let size = (self.width * self.height) as usize;
        let charset = " .:-=+*#%@";

        let mut rng = rand::thread_rng();
        let data: String = (0..size)
            .map(|_| charset.chars().nth(rng.gen_range(0..charset.len())).unwrap())
            .collect();

        self.frame_count += 1;

        Frame {
            timestamp_ms: (self.frame_count * 66) as u32, // ~15 FPS
            data,
        }
    }

    /// Generate a batch of frames
    pub fn generate_batch(&mut self, count: usize, sequence: u64) -> FrameBatch {
        let frames: Vec<Frame> = (0..count).map(|_| self.generate_frame()).collect();

        FrameBatch {
            header: FrameBatchHeader {
                magic: 0x4D4F4E47,
                version: 1,
                frame_count: frames.len() as u8,
                compression_type: CompressionType::Zlib,
                sequence_start: sequence,
            },
            frames,
        }
    }

    /// Generate a batch with mostly static content (high compression ratio)
    pub fn generate_static_batch(&mut self, count: usize, sequence: u64) -> FrameBatch {
        let base_frame = self.generate_frame();
        let frames: Vec<Frame> = (0..count)
            .map(|i| Frame {
                timestamp_ms: base_frame.timestamp_ms + (i as u32 * 66),
                data: base_frame.data.clone(), // Same content
            })
            .collect();

        FrameBatch {
            header: FrameBatchHeader {
                magic: 0x4D4F4E47,
                version: 1,
                frame_count: frames.len() as u8,
                compression_type: CompressionType::Zlib,
                sequence_start: sequence,
            },
            frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_generation() {
        let mut gen = SyntheticFrameGenerator::new(80, 60);
        let frame = gen.generate_frame();
        assert_eq!(frame.data.len(), 4800);
    }

    #[test]
    fn test_batch_generation() {
        let mut gen = SyntheticFrameGenerator::new(80, 60);
        let batch = gen.generate_batch(6, 0);
        assert_eq!(batch.frames.len(), 6);
        assert_eq!(batch.header.sequence_start, 0);
    }
}
```

### Component 2: RPC Client with Metrics

**Purpose**: Track detailed metrics for each transaction attempt

**File**: `crates/monegle-sender-test/src/rpc_client.rs`

```rust
use alloy::providers::{Provider, ProviderBuilder};
use alloy::signers::local::PrivateKeySigner;
use alloy::network::EthereumWallet;
use alloy::primitives::{Address, Bytes, U256, B256};
use alloy::rpc::types::TransactionRequest;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct TxMetrics {
    pub sequence: u64,
    pub tx_hash: B256,
    pub submit_time: Instant,
    pub confirm_time: Option<Instant>,
    pub latency_ms: Option<u64>,
    pub gas_used: Option<u64>,
    pub success: bool,
    pub error: Option<String>,
    pub data_size: usize,
}

pub struct RpcClient {
    provider: RootProvider,
    target_address: Address,
    metrics: Arc<Mutex<Vec<TxMetrics>>>,
}

impl RpcClient {
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        target_address: Address,
    ) -> Result<Self, anyhow::Error> {
        let signer: PrivateKeySigner = private_key.parse()?;
        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url.parse()?);

        Ok(Self {
            provider,
            target_address,
            metrics: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn submit_batch(
        &self,
        sequence: u64,
        calldata: Vec<u8>,
    ) -> Result<TxMetrics, anyhow::Error> {
        let submit_time = Instant::now();
        let data_size = calldata.len();

        // Create transaction
        let tx = TransactionRequest::default()
            .to(self.target_address)
            .input(Bytes::from(calldata).into());

        // Submit
        let result = self.provider.send_transaction(tx).await;

        let mut metric = TxMetrics {
            sequence,
            tx_hash: B256::ZERO,
            submit_time,
            confirm_time: None,
            latency_ms: None,
            gas_used: None,
            success: false,
            error: None,
            data_size,
        };

        match result {
            Ok(pending) => {
                metric.tx_hash = *pending.tx_hash();

                // Wait for confirmation
                match pending.get_receipt().await {
                    Ok(receipt) => {
                        let confirm_time = Instant::now();
                        metric.confirm_time = Some(confirm_time);
                        metric.latency_ms = Some(confirm_time.duration_since(submit_time).as_millis() as u64);
                        metric.gas_used = Some(receipt.gas_used);
                        metric.success = receipt.status();
                    }
                    Err(e) => {
                        metric.error = Some(format!("Confirmation failed: {:?}", e));
                    }
                }
            }
            Err(e) => {
                metric.error = Some(format!("Submit failed: {:?}", e));
            }
        }

        // Store metrics
        self.metrics.lock().await.push(metric.clone());

        Ok(metric)
    }

    pub async fn get_metrics(&self) -> Vec<TxMetrics> {
        self.metrics.lock().await.clone()
    }

    pub async fn print_summary(&self) {
        let metrics = self.get_metrics().await;
        let total = metrics.len();
        let successful = metrics.iter().filter(|m| m.success).count();
        let failed = total - successful;

        let avg_latency: f64 = metrics
            .iter()
            .filter_map(|m| m.latency_ms)
            .map(|l| l as f64)
            .sum::<f64>()
            / successful.max(1) as f64;

        let total_gas: u64 = metrics.iter().filter_map(|m| m.gas_used).sum();
        let avg_gas = total_gas / successful.max(1) as u64;

        let total_data: usize = metrics.iter().map(|m| m.data_size).sum();

        println!("\n=== RPC Client Summary ===");
        println!("Total transactions: {}", total);
        println!("Successful: {} ({:.1}%)", successful, (successful as f64 / total as f64) * 100.0);
        println!("Failed: {} ({:.1}%)", failed, (failed as f64 / total as f64) * 100.0);
        println!("Average latency: {:.0} ms", avg_latency);
        println!("Average gas used: {}", avg_gas);
        println!("Total data sent: {} KB", total_data / 1024);
        println!("=========================\n");

        // Print errors
        if failed > 0 {
            println!("Errors encountered:");
            for (i, metric) in metrics.iter().enumerate() {
                if let Some(error) = &metric.error {
                    println!("  [{}] {}", i, error);
                }
            }
        }
    }
}
```

### Component 3: Test Harness

**Purpose**: Run automated tests with different configurations

**File**: `crates/monegle-sender-test/src/main.rs`

```rust
use monegle_core::synthetic::SyntheticFrameGenerator;
use monegle_core::codec::{ZlibCodec, FrameEncoder};
use clap::Parser;
use std::time::Duration;

mod rpc_client;
use rpc_client::RpcClient;

#[derive(Parser)]
struct Args {
    /// RPC endpoint URL
    #[arg(short, long)]
    rpc_url: String,

    /// Private key (or use MONAD_PRIVATE_KEY env var)
    #[arg(short, long, env = "MONAD_PRIVATE_KEY")]
    private_key: String,

    /// Target address (dummy contract or EOA)
    #[arg(short, long)]
    target_address: String,

    /// Frames per second
    #[arg(short, long, default_value = "15")]
    fps: u8,

    /// Frame width (characters)
    #[arg(long, default_value = "80")]
    width: u16,

    /// Frame height (characters)
    #[arg(long, default_value = "60")]
    height: u16,

    /// Test duration (seconds)
    #[arg(short, long, default_value = "60")]
    duration: u64,

    /// Output metrics to file
    #[arg(short, long)]
    output: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    println!("=== Monegle RPC Feasibility Test ===");
    println!("RPC URL: {}", args.rpc_url);
    println!("Target: {}", args.target_address);
    println!("Quality: {} FPS, {}x{}", args.fps, args.width, args.height);
    println!("Duration: {} seconds", args.duration);
    println!("====================================\n");

    // Initialize components
    let client = RpcClient::new(
        &args.rpc_url,
        &args.private_key,
        args.target_address.parse()?,
    )
    .await?;

    let mut generator = SyntheticFrameGenerator::new(args.width, args.height);
    let codec = ZlibCodec::new(6);

    // Calculate frames per batch (based on 400ms block time)
    let frames_per_batch = ((0.4 * args.fps as f32).ceil() as usize).max(1);
    let batch_interval = Duration::from_millis(400);

    println!("Batching: {} frames per batch, every 400ms\n", frames_per_batch);

    // Run test
    let start_time = std::time::Instant::now();
    let mut sequence = 0u64;
    let mut ticker = tokio::time::interval(batch_interval);

    while start_time.elapsed().as_secs() < args.duration {
        ticker.tick().await;

        // Generate batch
        let batch = generator.generate_batch(frames_per_batch, sequence);

        // Encode
        let encoded = batch.encode_to_bytes()?;
        let compressed = codec.encode(&batch.frames)?;

        println!(
            "[Seq {}] Generated batch: {} frames, {} bytes (unencoded), {} bytes (compressed)",
            sequence,
            batch.frames.len(),
            encoded.len(),
            compressed.len()
        );

        // Submit to blockchain
        match client.submit_batch(sequence, compressed).await {
            Ok(metric) => {
                if metric.success {
                    println!(
                        "  ✓ Confirmed in {} ms, gas: {}, tx: {}",
                        metric.latency_ms.unwrap_or(0),
                        metric.gas_used.unwrap_or(0),
                        metric.tx_hash
                    );
                } else {
                    println!("  ✗ Failed: {:?}", metric.error);
                }
            }
            Err(e) => {
                println!("  ✗ Error: {:?}", e);
            }
        }

        sequence += 1;
    }

    // Print summary
    client.print_summary().await;

    // Export metrics if requested
    if let Some(output_path) = args.output {
        let metrics = client.get_metrics().await;
        let json = serde_json::to_string_pretty(&metrics)?;
        std::fs::write(&output_path, json)?;
        println!("Metrics exported to: {}", output_path);
    }

    Ok(())
}
```

### Component 4: Multi-RPC Testing Script

**Purpose**: Test multiple RPC endpoints in parallel

**File**: `scripts/test-all-rpcs.sh`

```bash
#!/bin/bash

# Monad testnet RPC endpoints
RPCS=(
    "https://testnet-rpc.monad.xyz"
    "https://monad-testnet.chainstack.com/YOUR_KEY_HERE"
    "https://monad-testnet.blockpi.network/v1/rpc/YOUR_KEY_HERE"
)

TARGET_ADDRESS="0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
DURATION=300  # 5 minutes
FPS=15
WIDTH=80
HEIGHT=60

mkdir -p test-results

echo "Testing ${#RPCS[@]} RPC endpoints..."

for i in "${!RPCS[@]}"; do
    RPC="${RPCS[$i]}"
    OUTPUT="test-results/rpc-$i-$(date +%s).json"

    echo ""
    echo "=== Testing RPC $i: $RPC ==="

    cargo run --release --bin monegle-sender-test -- \
        --rpc-url "$RPC" \
        --target-address "$TARGET_ADDRESS" \
        --fps "$FPS" \
        --width "$WIDTH" \
        --height "$HEIGHT" \
        --duration "$DURATION" \
        --output "$OUTPUT"

    if [ $? -eq 0 ]; then
        echo "✓ Test completed successfully"
    else
        echo "✗ Test failed"
    fi
done

echo ""
echo "All tests complete. Results in test-results/"
```

### Component 5: Results Analysis Script

**Purpose**: Analyze test results and generate recommendations

**File**: `scripts/analyze-test-results.py`

```python
#!/usr/bin/env python3

import json
import sys
from pathlib import Path
from statistics import mean, median, stdev

def analyze_metrics(file_path):
    with open(file_path) as f:
        metrics = json.load(f)

    total = len(metrics)
    successful = sum(1 for m in metrics if m['success'])
    failed = total - successful

    latencies = [m['latency_ms'] for m in metrics if m['latency_ms'] is not None]
    gas_used = [m['gas_used'] for m in metrics if m['gas_used'] is not None]

    return {
        'file': file_path.name,
        'total_tx': total,
        'successful': successful,
        'failed': failed,
        'success_rate': (successful / total) * 100 if total > 0 else 0,
        'avg_latency': mean(latencies) if latencies else 0,
        'median_latency': median(latencies) if latencies else 0,
        'p95_latency': sorted(latencies)[int(len(latencies) * 0.95)] if latencies else 0,
        'avg_gas': mean(gas_used) if gas_used else 0,
        'total_data_kb': sum(m['data_size'] for m in metrics) / 1024,
    }

def main():
    results_dir = Path('test-results')

    if not results_dir.exists():
        print("No test-results/ directory found")
        return

    results = []
    for json_file in results_dir.glob('*.json'):
        result = analyze_metrics(json_file)
        results.append(result)

    # Sort by success rate
    results.sort(key=lambda r: r['success_rate'], reverse=True)

    print("\n=== RPC Endpoint Comparison ===\n")
    print(f"{'Endpoint':<40} {'Success Rate':<15} {'Avg Latency':<15} {'P95 Latency':<15}")
    print("-" * 85)

    for r in results:
        print(f"{r['file']:<40} {r['success_rate']:>6.1f}% "
              f"{r['avg_latency']:>10.0f} ms {r['p95_latency']:>10.0f} ms")

    print("\n=== Recommendations ===\n")

    best = results[0] if results else None
    if best and best['success_rate'] >= 95:
        print(f"✓ Use {best['file']} - High success rate ({best['success_rate']:.1f}%)")
    elif best and best['success_rate'] >= 80:
        print(f"⚠ {best['file']} works but has some failures ({best['success_rate']:.1f}%)")
        print("  Consider implementing RPC rotation for better reliability")
    else:
        print("✗ No RPC endpoint achieved >80% success rate")
        print("  Recommendations:")
        print("  1. Reduce transaction rate (lower FPS)")
        print("  2. Use paid RPC service (Alchemy, Chainstack)")
        print("  3. Implement RPC rotation with fallback")

    if any(r['avg_latency'] > 2000 for r in results):
        print("\n⚠ High latency detected (>2s)")
        print("  This may cause buffering issues for viewers")

if __name__ == '__main__':
    main()
```

## Test Execution Plan

### Week 1: Implementation

**Day 1-2**: Core types and codec
- `monegle-core/src/types.rs`
- `monegle-core/src/codec.rs`
- `monegle-core/src/synthetic.rs`

**Day 3-4**: RPC client and metrics
- `monegle-sender-test/src/rpc_client.rs`

**Day 5-6**: Test harness
- `monegle-sender-test/src/main.rs`
- `scripts/test-all-rpcs.sh`
- `scripts/analyze-test-results.py`

**Day 7**: Testing and debugging
- Run initial tests
- Fix bugs
- Verify metrics collection

### Week 2: Testing & Analysis

**Day 8**: Baseline tests
- Test each RPC endpoint individually
- 5-minute duration each
- Medium quality (15 FPS, 80×60)

**Day 9**: Stress tests
- Long duration (30 minutes)
- High quality (24 FPS, 120×80)
- Monitor for rate limiting

**Day 10**: Burst tests
- 2× transaction rate (5 tx/s)
- Detect rate limit thresholds
- Test recovery behavior

**Day 11-12**: Analysis
- Run analysis script
- Compare RPC endpoints
- Design mitigation strategy

**Day 13-14**: Mitigation implementation
- Implement RPC rotation if needed
- Add adaptive quality adjustment
- Test with mitigation in place

## Success Criteria

### Must Have
- [ ] At least one RPC endpoint achieves >90% success rate
- [ ] Average confirmation latency < 2 seconds
- [ ] No permanent bans from RPC providers

### Nice to Have
- [ ] Multiple RPC endpoints work reliably
- [ ] P95 latency < 3 seconds
- [ ] Gas costs match estimates (±20%)

### Red Flags
- ❌ All RPC endpoints rate limit after < 5 minutes
- ❌ Success rate < 70% on any endpoint
- ❌ Account/IP banned from RPC provider

## Mitigation Strategies

### Strategy 1: RPC Rotation

**Implementation**:
```rust
pub struct RpcPool {
    clients: Vec<RpcClient>,
    current_index: Arc<Mutex<usize>>,
}

impl RpcPool {
    pub async fn submit_with_rotation(&self, batch: FrameBatch) -> Result<()> {
        let mut attempts = 0;
        let max_attempts = self.clients.len() * 2;

        while attempts < max_attempts {
            let index = {
                let mut idx = self.current_index.lock().await;
                let current = *idx;
                *idx = (*idx + 1) % self.clients.len();
                current
            };

            match self.clients[index].submit_batch(batch.clone()).await {
                Ok(_) => return Ok(()),
                Err(e) if e.is_rate_limited() => {
                    // Try next RPC
                    attempts += 1;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(anyhow::anyhow!("All RPCs failed"))
    }
}
```

### Strategy 2: Adaptive Quality

**Implementation**:
```rust
pub struct AdaptiveQualityController {
    target_success_rate: f64,
    current_fps: u8,
    min_fps: u8,
    max_fps: u8,
}

impl AdaptiveQualityController {
    pub fn adjust_based_on_metrics(&mut self, metrics: &[TxMetrics]) {
        let recent = &metrics[metrics.len().saturating_sub(10)..];
        let success_rate = recent.iter().filter(|m| m.success).count() as f64 / recent.len() as f64;

        if success_rate < self.target_success_rate && self.current_fps > self.min_fps {
            self.current_fps -= 1;
            println!("⚠ Reducing FPS to {} due to low success rate", self.current_fps);
        } else if success_rate > 0.95 && self.current_fps < self.max_fps {
            self.current_fps += 1;
            println!("✓ Increasing FPS to {}", self.current_fps);
        }
    }
}
```

### Strategy 3: Paid RPC Service

**Cost Analysis**:
- Alchemy: ~$50/month for 150M compute units
- Chainstack: ~$79/month for dedicated endpoint
- Our needs: 2.5 tx/s × 3600s × 24h = 216K tx/day

**Recommendation**: Only if free endpoints fail

## Deliverables

1. **Working test binary** (`monegle-sender-test`)
2. **Test results** (JSON files for each RPC)
3. **Analysis report** (generated by Python script)
4. **Mitigation strategy** (based on results)
5. **Go/No-Go decision** for full implementation

## Timeline

**Total: 2 weeks**

- Week 1: Implementation
- Week 2: Testing & Analysis

**Decision Point**: End of Week 2
- ✅ **GO**: If success rate >90%, proceed with full implementation
- ⚠️ **CONDITIONAL**: If 70-90%, implement mitigation and retest
- ❌ **NO-GO**: If <70%, reconsider approach or reduce quality

## Next Steps After Testing

### If Results are Good (>90% success)
→ Proceed with full implementation (relay + receiver)

### If Results are Moderate (70-90% success)
→ Implement RPC rotation, test for another week

### If Results are Poor (<70% success)
→ Options:
1. Use paid RPC service
2. Reduce quality (lower FPS)
3. Explore alternative transport (IPFS + on-chain pointers)
4. Reconsider project viability

## References

- [Monad RPC Endpoints](https://docs.monad.xyz/node-ops/rpc-providers)
- [Alloy Provider Documentation](https://alloy.rs/providers/overview.html)
- [Rate Limiting Best Practices](https://docs.alchemy.com/reference/rate-limits)
