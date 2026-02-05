# Cost Analysis

Detailed cost breakdown for running Monegle on Monad testnet and mainnet.

## Executive Summary

| Quality Level | FPS | Resolution | Blockchain Cost/Hour | Relay Cost/Hour | Total/Hour |
|--------------|-----|------------|---------------------|----------------|------------|
| Low | 10 | 40×30 | $45 | $5 | **$50** |
| Medium | 15 | 80×60 | $90 | $5 | **$95** |
| High | 24 | 120×80 | $270 | $5 | **$275** |

*Assumes Monad testnet gas prices (~$0.004/tx) and relay operator fee of $0.002/tx*

## Cost Components

### 1. Blockchain Costs

#### Gas Cost Breakdown

**Per Transaction**:
```
Base transaction cost:     21,000 gas
Calldata (non-zero bytes): data_size × 16 gas/byte
Total gas:                 21,000 + (data_size × 16)
```

**Example** (15 FPS, 80×60, 1,000 bytes compressed):
```
Gas = 21,000 + (1,000 × 16) = 37,000 gas
```

#### Monad Testnet Pricing

**Current Rates** (as of Feb 2026):
- Gas price: ~72 Gwei
- MON price: ~$0.02 USD
- **Effective cost**: ~$0.004-0.007 per transaction

**Calculation**:
```
Cost per tx = gas_used × gas_price × MON_price_usd
            = 37,000 × 72e-9 MON × $0.02
            = ~$0.005
```

#### Transaction Frequency by Quality

With 400ms block times (2.5 blocks/second):

| Quality | FPS | Frames/Tx | Tx/Second | Tx/Hour | Blockchain Cost/Hour |
|---------|-----|-----------|-----------|---------|---------------------|
| Low | 10 | 4 | 2.5 | 9,000 | $45 |
| Medium | 15 | 6 | 2.5 | 9,000 | $45-90* |
| High | 24 | 10 | 2.5 | 9,000 | $90-270* |

*Higher quality = more calldata per tx = more gas*

### 2. Data Size Analysis

#### ASCII Frame Sizes (Uncompressed)

| Resolution | Characters | Bytes | Frames/400ms | Batch Size |
|-----------|-----------|-------|--------------|-----------|
| 40×30 | 1,200 | 1,200 | 4 | 4,800 |
| 80×60 | 4,800 | 4,800 | 6 | 28,800 |
| 120×80 | 9,600 | 9,600 | 10 | 96,000 |

#### Compression Ratios

**Delta Encoding** (only changed characters):
- Static scene: 90% reduction
- Moderate motion: 70% reduction
- High motion: 50% reduction

**Average**: ~60% reduction

**After Delta + Zlib**:
- 40×30 batch: 4,800 → 1,920 → **600-900 bytes**
- 80×60 batch: 28,800 → 11,520 → **1,000-1,500 bytes**
- 120×80 batch: 96,000 → 38,400 → **3,000-5,000 bytes**

#### Gas Cost by Data Size

| Compressed Size | Gas (Calldata) | Total Gas | Cost @ $0.005/tx |
|----------------|----------------|-----------|-----------------|
| 600 bytes | 9,600 | 30,600 | $0.004 |
| 1,000 bytes | 16,000 | 37,000 | $0.005 |
| 3,000 bytes | 48,000 | 69,000 | $0.009 |
| 5,000 bytes | 80,000 | 101,000 | $0.013 |

### 3. Relay Operator Costs

#### Infrastructure Costs (Monthly)

**VPS with Monad Node**:
- CPU: 8 cores
- RAM: 32 GB
- Storage: 500 GB SSD
- Bandwidth: 10 TB

**Providers**:
- Hetzner: ~$50/month
- DigitalOcean: ~$80/month
- AWS: ~$150/month

**Amortized Hourly**: ~$0.07-0.20/hour

#### Operating Costs

- Electricity: Negligible (VPS)
- Maintenance: ~10 hours/month @ $50/hour = $500/month → ~$0.70/hour
- **Total Operator Cost**: ~$1/hour

#### Profit Margin

**Revenue** (per stream):
- Streamers pay $0.002/tx
- 2.5 tx/second × 3,600 seconds = 9,000 tx/hour
- **Revenue**: $18/hour

**Profit**:
- Revenue: $18/hour
- Cost: $1/hour
- **Net Profit**: $17/hour per stream

**Break-even**: 1 stream
**Sustainable**: 5+ concurrent streams → $85/hour profit

### 4. Total Cost Summary

#### Per Hour

| Component | Low Quality | Medium Quality | High Quality |
|-----------|------------|----------------|--------------|
| Blockchain | $45 | $90 | $270 |
| Relay Fee | $18 | $18 | $18 |
| **Total** | **$63** | **$108** | **$288** |

#### Per Minute

| Component | Low | Medium | High |
|-----------|-----|--------|------|
| Blockchain | $0.75 | $1.50 | $4.50 |
| Relay Fee | $0.30 | $0.30 | $0.30 |
| **Total** | **$1.05** | **$1.80** | **$4.80** |

#### Per Day (24 hours)

| Quality | Total Cost |
|---------|-----------|
| Low | $1,512 |
| Medium | $2,592 |
| High | $6,912 |

## Optimization Strategies

### 1. Reduce Frame Rate

**Impact**: Linear cost reduction

| FPS | Tx/Hour | Cost Multiplier |
|-----|---------|----------------|
| 5 | 4,500 | 0.5× |
| 10 | 9,000 | 1.0× |
| 15 | 9,000 | 1.0×* |
| 20 | 9,000 | 1.0×* |
| 24 | 9,000 | 1.0×* |

*Same tx/hour due to batching, but more data per tx*

**Recommendation**: Lower FPS = lower data per tx = lower gas

Optimal: **10 FPS** with 4 frames/tx (600-900 bytes)

### 2. Aggressive Compression

**Current**: Delta + RLE + Zlib level 6

**Options**:
- Zlib level 9: +5% compression, +20% CPU
- Brotli: +10% compression, +50% CPU
- Custom ASCII-optimized codec: +15% compression

**Impact**: 10-15% cost reduction

**Example**:
- 1,000 bytes → 850 bytes with Brotli
- Gas: 37,000 → 30,600 (-17%)
- **Savings**: ~$15/hour @ medium quality

### 3. Adaptive Quality

**Strategy**: Adjust quality based on content

- Static scenes: Send keyframes every 5 seconds, deltas in between
- High motion: Reduce resolution temporarily
- Dark/light scenes: Use smaller character set

**Impact**: 20-30% cost reduction on average

### 4. Scheduled Streaming

**Strategy**: Stream only during peak viewer times

- Stream 8 hours/day instead of 24/7
- **Savings**: 67% reduction in daily costs

**Example** (Medium Quality):
- 24/7: $2,592/day
- 8 hours: $864/day
- **Savings**: $1,728/day

## Mainnet Cost Projections

### Assumptions

- Mainnet gas prices: 10× testnet (conservative)
- MON price on mainnet: $0.50 (speculative)
- Transaction costs: ~$0.05/tx (10× testnet)

### Mainnet Costs

| Quality | Blockchain/Hour | Relay Fee/Hour | Total/Hour |
|---------|----------------|----------------|------------|
| Low | $450 | $180 | **$630** |
| Medium | $900 | $180 | **$1,080** |
| High | $2,700 | $180 | **$2,880** |

### Mitigation Strategies for Mainnet

1. **Lower Quality Default**: Use 5-10 FPS on mainnet
2. **Premium Tiers**: Charge viewers for high-quality streams
3. **Sponsorships**: Brands pay for stream costs
4. **Layer 2**: Wait for Monad L2 or use another L2

## Comparison with Traditional Streaming

### YouTube Live

**Cost**: Free for streamer, YouTube absorbs costs

**Equivalent Monegle Cost**: $0 (but centralized, censorship risk)

### Twitch

**Cost**: Free for streamer, Twitch takes 50% of revenue

**Equivalent**: If streamer makes $100/hour, pays $50 to Twitch

**Monegle**: $108/hour, but fully decentralized

### Livepeer (Decentralized)

**Cost**: ~$0.50/hour for 720p video

**Equivalent Monegle**: $63/hour for much lower quality (ASCII)

**Verdict**: Monegle is 100× more expensive for worse quality

## Business Model Viability

### Target Market

**Who would pay $100/hour for ASCII streaming?**

1. **NFT/Crypto Art Projects**: Novelty/artistic value
2. **Educational Demos**: Blockchain tech demonstrations
3. **Marketing Stunts**: "First live ASCII stream on Monad"
4. **Hackathon Demos**: Proof-of-concept projects

**Not viable for**: General video streaming (too expensive)

### Pricing Model

**Option 1: Free to View, Streamer Pays**
- Streamer: Pays $108/hour
- Viewers: Free
- Use case: Marketing, education

**Option 2: Pay-per-View**
- Streamer: Pays $108/hour
- Viewers: Pay $1/hour each
- Break-even: 108 viewers
- Use case: Exclusive content

**Option 3: Subscription**
- Streamer: Pays $108/hour
- Viewers: $10/month subscription
- Break-even: 11 subscribers (for 1 hour/day stream)
- Use case: Regular content creators

### Revenue Scenarios

**Scenario 1: Niche Art Project**
- Stream: 1 hour/week
- Viewers: 50 (free)
- Cost: $108/week = $468/month
- Revenue: $0 (free viewers)
- **Net**: -$468/month

**Scenario 2: Educational Series**
- Stream: 8 hours/month
- Viewers: 100 paying $5/month
- Cost: $864/month
- Revenue: $500/month
- **Net**: -$364/month (need sponsorship)

**Scenario 3: Premium Tech Demo**
- Stream: Daily 1-hour show (30 days)
- Viewers: 200 paying $15/month
- Cost: $3,240/month
- Revenue: $3,000/month
- **Net**: -$240/month (close to break-even)

## Recommendations

### For MVP Testing

- **Quality**: Low (10 FPS, 40×30)
- **Duration**: 5-10 minutes per test
- **Cost**: ~$10 per session
- **Purpose**: Prove technical feasibility

### For Demo/Marketing

- **Quality**: Medium (15 FPS, 80×60)
- **Duration**: 1 hour event
- **Cost**: $108
- **Purpose**: Showcase to investors/community

### For Production (If Pursuing)

- **Wait for mainnet cost data**: Testnet prices != mainnet
- **Implement adaptive quality**: Reduce costs 20-30%
- **Explore sponsorships**: Offset blockchain costs
- **Consider Layer 2**: If/when available

## Cost Monitoring

### Metrics to Track

1. **Gas per transaction**: Actual gas used vs. estimated
2. **Compression ratio**: Bytes saved vs. uncompressed
3. **Effective cost per frame**: Total cost / frames delivered
4. **Viewer minutes**: Total viewing time across all viewers

### Alerts

- Gas price spike > 2× normal → Pause stream
- Compression ratio < 50% → Investigate content type
- Cost per viewer-hour > $2 → Re-evaluate quality

### Dashboard (Future)

```
┌─────────────────────────────────────┐
│  Monegle Cost Dashboard             │
├─────────────────────────────────────┤
│  Current Stream: medium-quality     │
│  Uptime: 45 minutes                 │
│                                     │
│  Blockchain Costs:                  │
│    - Txs sent: 6,750                │
│    - Gas used: 249,750,000          │
│    - Cost: $67.50                   │
│                                     │
│  Relay Costs:                       │
│    - Fee paid: $13.50               │
│                                     │
│  Total Cost: $81.00                 │
│  Cost per minute: $1.80             │
│                                     │
│  Viewers: 12                        │
│  Cost per viewer-hour: $6.75        │
└─────────────────────────────────────┘
```

## Conclusion

**Key Findings**:

1. **Testnet is affordable** for short demos (~$100/hour)
2. **Mainnet will be 10-100× more expensive** (wait for data)
3. **Monegle is 100× more expensive than Livepeer** for video quality
4. **Not viable for general streaming**, but suitable for:
   - Hackathon demos
   - NFT/art projects
   - Educational content
   - Marketing stunts

**Strategic Recommendation**:

Build on testnet as a **technical proof-of-concept** and **learning project**, not as a commercial product. Focus on demonstrating innovative use of Execution Events SDK and blockchain streaming architecture.

If pursuing production:
1. Wait for mainnet cost data
2. Explore Layer 2 solutions
3. Target niche markets willing to pay premium
4. Seek sponsors/grants to subsidize costs

## References

- [Monad Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing)
- [MON Token Price - CoinMarketCap](https://coinmarketcap.com/currencies/monad/)
- [Livepeer Pricing](https://livepeer.org/pricing)
