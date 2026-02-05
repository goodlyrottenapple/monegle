# Monegle RPC Feasibility Test - Execution Guide

## Overview

This guide walks through executing the RPC throughput feasibility tests to determine if Monad testnet can support real-time video streaming at 2.5 transactions/second.

## Test Components Created

✅ **monegle-sender-test** - Test binary for RPC throughput testing
✅ **SyntheticFrameGenerator** - Generates fake frames without camera
✅ **RpcClient** - Tracks detailed metrics for each transaction
✅ **test-all-rpcs.sh** - Multi-RPC testing script
✅ **analyze-test-results.py** - Results analysis and recommendations

## Prerequisites

### 1. Rust Environment
```bash
# Verify Rust installation
rustc --version
cargo --version

# If not installed, get from https://rustup.rs
```

### 2. Monad Testnet Setup
```bash
# Get testnet MON tokens from faucet
# https://faucet.chainstack.com/monad-testnet-faucet

# Set private key
export MONAD_PRIVATE_KEY="0x..."
```

### 3. Build Test Binary
```bash
cd /Users/goodlyrottenapple/git/monegle

# Build the test binary
cargo build --release --bin monegle-sender-test

# Verify it works
./target/release/monegle-sender-test --help
```

## Test Execution

### Quick Test (60 seconds)

Test a single RPC endpoint quickly:

```bash
export MONAD_PRIVATE_KEY="0x..."

cargo run --release --bin monegle-sender-test -- \
  --rpc-url "https://testnet-rpc.monad.xyz" \
  --target-address "0x0000000000000000000000000000000000000001" \
  --fps 15 \
  --width 80 \
  --height 60 \
  --duration 60 \
  --output test-results/quick-test.json
```

**Expected output:**
```
╔═══════════════════════════════════════════════════════╗
║     MONEGLE RPC THROUGHPUT FEASIBILITY TEST          ║
╠═══════════════════════════════════════════════════════╣
║ RPC URL:      https://testnet-rpc.monad.xyz          ║
║ Target:       0x0000000000000000000000000000000001   ║
║ Quality:      15 FPS, 80×60 chars                    ║
║ Duration:     60 seconds                              ║
╚═══════════════════════════════════════════════════════╝

Starting test...

[INFO] [Seq 0] Generated batch: 6 frames, 8432 bytes
[INFO] Batch 0 confirmed in 456 ms, gas: 89234
...
```

### Full Test Suite (5 minutes per RPC)

Test multiple RPC endpoints:

```bash
# 1. Edit scripts/test-all-rpcs.sh to add your RPC endpoints
nano scripts/test-all-rpcs.sh

# Update this section:
RPCS=(
    "https://testnet-rpc.monad.xyz"
    "https://monad-testnet.chainstack.com/YOUR_KEY_HERE"
    # Add more endpoints...
)

# 2. Run the test suite
./scripts/test-all-rpcs.sh
```

This will:
- Test each RPC endpoint for 5 minutes
- Generate JSON metrics for each test
- Save results to `test-results/`

### Analyze Results

After tests complete:

```bash
# Run analysis
python3 scripts/analyze-test-results.py
```

**Expected output:**
```
╔═══════════════════════════════════════════════════════════════════════════════╗
║              MONEGLE RPC TEST RESULTS ANALYSIS                                ║
╚═══════════════════════════════════════════════════════════════════════════════╝

╔═══════════════════════════════════════════════════════════════════════════════╗
║                         RPC ENDPOINT COMPARISON                               ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║ Endpoint          Success Rate   Avg Lat   P95 Lat   P99 Lat   Rate Limited  ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║ rpc-0-...         96.5%      524ms  1245ms   1876ms        0       ║
║ rpc-1-...         78.2%      892ms  2134ms   3456ms       12       ║
╚═══════════════════════════════════════════════════════════════════════════════╝

╔═══════════════════════════════════════════════════════════════════════════════╗
║                            RECOMMENDATIONS                                    ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║ Best Endpoint: rpc-0-20260205-143022.json                                    ║
║   Success Rate: 96.5%                                                         ║
║   Avg Latency:  524ms                                                         ║
║                                                                               ║
║ ✅ EXCELLENT RESULTS                                                          ║
║                                                                               ║
║ This RPC endpoint is highly suitable for production use.                     ║
║                                                                               ║
║ Next Steps:                                                                   ║
║   1. Proceed with full Monegle implementation                                ║
║   2. Deploy relay and receiver components                                    ║
║   3. Run end-to-end integration tests                                        ║
╚═══════════════════════════════════════════════════════════════════════════════╝
```

## Test Scenarios

### Scenario 1: Baseline (Recommended First Test)
```bash
--fps 10 --width 40 --height 30 --duration 60
```
- **Purpose**: Verify basic throughput
- **Load**: Light (4 frames/batch, ~2KB/tx)
- **Expected**: >95% success rate

### Scenario 2: Medium Quality (Production Target)
```bash
--fps 15 --width 80 --height 60 --duration 300
```
- **Purpose**: Realistic load test
- **Load**: Medium (6 frames/batch, ~8KB/tx)
- **Expected**: >90% success rate

### Scenario 3: High Quality (Maximum)
```bash
--fps 24 --width 120 --height 80 --duration 300
```
- **Purpose**: Maximum data size
- **Load**: High (10 frames/batch, ~15KB/tx)
- **Expected**: >85% success rate

### Scenario 4: Stress Test (Long Duration)
```bash
--fps 15 --width 80 --height 60 --duration 1800
```
- **Purpose**: Detect rate limiting over time
- **Load**: Medium for 30 minutes
- **Expected**: Sustained >90% success rate

### Scenario 5: Burst Test (Rate Limit Detection)
```bash
# Modify batch_interval in main.rs to 200ms (5 tx/s)
--fps 15 --width 80 --height 60 --duration 300
```
- **Purpose**: Find rate limit threshold
- **Load**: Double rate (5 tx/s)
- **Expected**: Will trigger rate limiting

## Interpreting Results

### Success Rate Thresholds

| Success Rate | Assessment | Action |
|--------------|------------|--------|
| ≥95% | ✅ Excellent | Proceed with full implementation |
| 80-94% | ⚠️ Moderate | Implement RPC rotation + retry |
| 70-79% | ⚠️ Poor | Use paid RPC or reduce FPS |
| <70% | ❌ Critical | Reconsider viability |

### Latency Thresholds

| Avg Latency | P95 Latency | Assessment |
|-------------|-------------|------------|
| <1000ms | <2000ms | ✅ Excellent |
| 1000-2000ms | 2000-3000ms | ⚠️ Acceptable |
| >2000ms | >3000ms | ❌ Too high |

### Rate Limiting Indicators

- **429 errors**: RPC is rate limiting
- **Increasing failure rate over time**: Gradual rate limit
- **Connection drops**: Hard rate limit or ban

## Common Issues & Solutions

### Issue: "Private key not set"
```bash
export MONAD_PRIVATE_KEY="0x..."
# Or add to ~/.bashrc or ~/.zshrc
```

### Issue: "Insufficient funds"
Get more testnet MON from faucet:
- https://faucet.chainstack.com/monad-testnet-faucet
- https://www.alchemy.com/faucets/monad-testnet

### Issue: "Connection refused"
Check RPC URL is correct:
```bash
curl -X POST https://testnet-rpc.monad.xyz \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

### Issue: "All transactions failing"
- Check gas price (may need EIP-1559 parameters)
- Verify target address is valid
- Check RPC endpoint is for Monad testnet (chain ID 10143)

### Issue: "Test takes too long"
Reduce duration for quick testing:
```bash
--duration 60  # 1 minute test
```

## Decision Matrix

### ✅ GO (Proceed with Full Implementation)

**Criteria:**
- At least one RPC: >90% success rate
- Average latency: <1.5 seconds
- No permanent rate limiting

**Next Steps:**
1. Complete relay component implementation
2. Build receiver with buffering
3. Run end-to-end tests
4. Deploy to testnet

### ⚠️ CONDITIONAL (Implement Mitigations)

**Criteria:**
- Best RPC: 70-90% success rate
- Some rate limiting detected
- Latency: 1.5-2.5 seconds

**Required Mitigations:**
1. RPC rotation (3+ endpoints)
2. Transaction retry logic
3. Adaptive quality reduction
4. Rate limit backoff

**Timeline:**
- Implement mitigations: 3-5 days
- Retest: 1-2 days
- Reevaluate results

### ❌ NO-GO (Reconsider Approach)

**Criteria:**
- All RPCs: <70% success rate
- Frequent permanent bans
- Latency: >3 seconds consistently

**Alternative Options:**
1. Use paid RPC service ($50-79/month)
2. Reduce quality significantly (5-8 FPS)
3. Alternative architecture (IPFS + pointers)
4. Reconsider project viability

## Metrics Export

All test runs export detailed JSON metrics:

```json
[
  {
    "sequence": 0,
    "tx_hash": "0xabcd...",
    "submit_time_ms": 1234,
    "confirm_time_ms": 1690,
    "latency_ms": 456,
    "gas_used": 89234,
    "success": true,
    "error": null,
    "data_size": 8432
  },
  ...
]
```

Use these for:
- Custom analysis
- Graphing (Excel, matplotlib)
- Cost calculations
- Performance tuning

## Post-Test Actions

### If Results are Good (>90%)

1. **Document findings**
   ```bash
   cp test-results/analysis-summary.json docs/rpc-test-results.json
   ```

2. **Update cost estimates**
   - Use actual gas costs from tests
   - Calculate real $/hour

3. **Proceed with full implementation**
   - Build relay component
   - Complete receiver with UI
   - Integration testing

### If Results Need Improvement (70-90%)

1. **Implement RPC rotation**
   ```rust
   // See docs/feasibility-test-plan.md Strategy 1
   pub struct RpcPool {
       clients: Vec<RpcClient>,
       ...
   }
   ```

2. **Add retry logic**
   - Max 3 retries per transaction
   - Exponential backoff
   - Switch RPC on failure

3. **Test with mitigations**
   - Run tests again
   - Verify improvement

### If Results are Poor (<70%)

1. **Try paid RPC service**
   - Alchemy: $50/month
   - Chainstack: $79/month
   - Retest with dedicated endpoint

2. **Reduce quality**
   - Test with 10 FPS
   - Test with 8 FPS
   - Test with 5 FPS
   - Find minimum viable quality

3. **Alternative architecture**
   - IPFS for frame storage
   - On-chain pointers only
   - Hybrid approach

## Timeline

**Total: 1-2 days for initial testing**

### Day 1
- Morning: Build and verify test binary
- Afternoon: Run baseline tests (60s each)
- Evening: Run medium quality test (5 min)

### Day 2
- Morning: Run stress test (30 min)
- Afternoon: Analyze results
- Evening: Make GO/NO-GO decision

**If mitigations needed: +3-5 days**

## Success Criteria

### Must Have
- [ ] At least one RPC endpoint: >90% success rate
- [ ] Average confirmation latency: <2 seconds
- [ ] No permanent bans from RPC providers
- [ ] Test data exported and analyzed

### Nice to Have
- [ ] Multiple RPC endpoints work reliably
- [ ] P95 latency: <3 seconds
- [ ] Gas costs match estimates (±20%)
- [ ] No rate limiting detected

## Contact & Support

**Issues encountered?**
- Check logs: `RUST_LOG=debug cargo run ...`
- Review test results JSON files
- Consult `docs/feasibility-test-plan.md`

**Ready to proceed?**
- See `docs/implementation-plan.md`
- Continue with Phase 7+ of implementation

---

**Test Version**: 1.0
**Last Updated**: 2026-02-05
**Status**: Ready for Execution
