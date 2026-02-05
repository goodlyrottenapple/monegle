# Documentation Summary

All planning documents for Monegle - ASCII Video Streaming on Monad Blockchain

## Quick Reference

| Document | Purpose | Read Time |
|----------|---------|-----------|
| [README.md](./README.md) | Project overview and quick start | 5 min |
| [architecture.md](./architecture.md) | Complete system architecture | 15 min |
| [execution-events-integration.md](./execution-events-integration.md) | Execution Events SDK guide | 10 min |
| [rebroadcast-node.md](./rebroadcast-node.md) | Re-broadcast relay design | 12 min |
| [cost-analysis.md](./cost-analysis.md) | Detailed cost breakdown | 10 min |
| [implementation-plan.md](./implementation-plan.md) | Step-by-step build guide | 20 min |

## Key Architectural Decision: Execution Events SDK

The critical innovation in this design is using **Monad's Execution Events SDK** instead of traditional smart contract events:

### Traditional Approach (Rejected)
```
Sender → Smart Contract → Emit Event → Blockchain → RPC Poll → Receiver
         ├─ Store in logs (expensive)
         └─ High latency (400ms+ block time)
```

### Monegle Approach (Adopted)
```
Sender → Raw Transaction → Blockchain
              ↓
         [Execution Events SDK]
         (Shared Memory IPC)
              ↓
         Re-broadcast Node → WebSocket → Receiver
         ├─ Pre-consensus access
         ├─ Sub-millisecond latency
         └─ No on-chain storage cost
```

### Benefits
1. **67% lower on-chain costs** - No event emission gas
2. **400× lower latency** - Pre-consensus vs finalized blocks
3. **Off-chain flexibility** - Relay can transcode, buffer, etc.
4. **Scalability** - One relay serves 1000+ viewers

### Trade-offs
- Requires running Monad node (infrastructure cost)
- Linux-only (shared memory requirement)
- No on-chain persistence (unless relay stores)
- Centralization risk (relay is single point of failure)

## Components Overview

### 1. Sender (`monegle-sender`)

**Tech Stack**: Rust, nokhwa (camera), artem (ASCII), alloy (blockchain)

**Flow**:
```
Camera (15 FPS) → Resize (80×60) → Grayscale → ASCII Map → Compress (60-80%) → Batch (6 frames) → Blockchain (2.5 tx/s)
```

**Output**: Transactions to relay operator's address with frame data in calldata

### 2. Re-broadcast Node (`monegle-relay`)

**Tech Stack**: Rust, monad-exec-events (SDK), tokio-tungstenite (WebSocket)

**Flow**:
```
Shared Memory → Filter by Address → Extract Calldata → Decompress → WebSocket Broadcast
(< 1 ms latency)
```

**Requirements**: Co-located with Monad node, Linux, 8 cores, 32 GB RAM

### 3. Receiver (`monegle-receiver`)

**Tech Stack**: Rust, tokio-tungstenite, ratatui (terminal UI)

**Flow**:
```
WebSocket → Frame Buffer (100 frames) → Playback Timer (15 FPS) → Terminal Render
```

**Platform**: Cross-platform (Linux, macOS, Windows)

## Cost Analysis Summary

### Testnet Costs (per hour)

| Quality | Resolution | FPS | Blockchain | Relay Fee | Total |
|---------|-----------|-----|-----------|-----------|-------|
| Low | 40×30 | 10 | $45 | $18 | **$63** |
| Medium | 80×60 | 15 | $90 | $18 | **$108** |
| High | 120×80 | 24 | $270 | $18 | **$288** |

### Mainnet Projections (10× gas price)

| Quality | Total/Hour |
|---------|-----------|
| Low | $630 |
| Medium | $1,080 |
| High | $2,880 |

**Recommendation**: Use testnet for MVP. Mainnet is not cost-effective for general streaming.

## Implementation Timeline

**Total: 30 days (solo developer)**

- **Week 1** (Days 1-7): Core infrastructure, compression, video capture
- **Week 2** (Days 8-14): Blockchain integration, start relay node
- **Week 3** (Days 15-21): Complete relay, build receiver
- **Week 4** (Days 22-28): Testing, documentation, polish
- **Week 5** (Days 29-30): Deployment and demo

## Technical Challenges

### 1. Execution Events SDK Integration ⚠️

**Challenge**: SDK not on crates.io, requires Monad node access

**Solution**:
- Use historical data for development (macOS compatible)
- Deploy to VPS with Monad node for production
- Mock SDK for unit tests

### 2. Real-time Compression ⚠️

**Challenge**: Must compress 6 frames in < 400ms to maintain throughput

**Solution**:
- Parallel compression with rayon
- Delta encoding (only changed characters)
- Adaptive quality based on content

### 3. WebSocket Scaling ⚠️

**Challenge**: Single relay must handle 1000+ concurrent viewers

**Solution**:
- Tokio async I/O (efficient)
- Message broadcasting (one decode, many sends)
- Future: Multiple relays with load balancing

### 4. Blockchain Cost ⚠️

**Challenge**: Even optimized, still expensive for continuous streaming

**Solution**:
- Target short-duration demos (< 1 hour)
- Explore sponsorships for longer streams
- Wait for Layer 2 solutions

## Risks & Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|------------|------------|
| Monad SDK unavailable | **High** | Low | Use historical data, mock for tests |
| Mainnet costs too high | **High** | High | Stay on testnet, target niche markets |
| Relay node complexity | Medium | Medium | Provide Docker deployment, docs |
| Video quality poor | Low | Medium | Configurable quality, user tuning |
| Latency too high | Medium | Low | Execution Events ensure < 1s latency |

## Success Criteria

### MVP (Minimum Viable Product)

- [ ] Capture video from webcam at 10-15 FPS
- [ ] Convert to ASCII art (80×60 or smaller)
- [ ] Compress to < 2 KB per batch
- [ ] Submit to Monad testnet successfully
- [ ] Relay extracts frames via Execution Events SDK
- [ ] Receiver displays in terminal with < 2s latency
- [ ] Total cost < $100/hour on testnet
- [ ] System runs for 10+ minutes without crashes

### Stretch Goals

- [ ] Support multiple concurrent streams
- [ ] Implement recording/replay
- [ ] Add color support (ANSI 256)
- [ ] Mobile receiver app
- [ ] Stream discovery UI
- [ ] Mainnet deployment

## Use Cases

### Viable on Testnet

1. **Hackathon Demos** - Showcase technical innovation
2. **Educational Content** - Teach blockchain streaming
3. **Art Projects** - ASCII art as NFT/crypto art
4. **Marketing Stunts** - "First ASCII stream on Monad"
5. **Protocol Testing** - Stress test Execution Events SDK

### Not Viable (Yet)

- General purpose video streaming (too expensive)
- 24/7 live streams (cost prohibitive)
- High-quality video (ASCII limitation)
- Mobile-first apps (requires desktop for sender)

## Research Questions Answered

### Q1: How does Execution Events SDK work?

**A**: Shared memory IPC between Monad node and external applications. Events published to `/dev/shm/monad_events`, read via `EventRing::read_event()`. Provides pre-consensus access to transaction data.

**References**:
- [Execution Events Overview](https://docs.monad.xyz/execution-events/overview)
- [Getting Started Guide](https://docs.monad.xyz/execution-events/getting-started/)

### Q2: What are actual Monad gas costs?

**A**: Testnet averages $0.004-0.007 per transaction. With calldata optimization, video streaming costs ~$90-270/hour depending on quality.

**References**:
- [Monad Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing)
- [Cost Analysis Document](./cost-analysis.md)

### Q3: Can Execution Events replace smart contracts entirely?

**A**: For this use case, yes. We don't need on-chain storage or logic - just transport. Execution Events provide lower cost and latency.

**However**: No on-chain persistence means streams can't be replayed unless relay stores them off-chain.

### Q4: Is Monad fast enough for real-time video?

**A**: Yes. 400ms block times + sub-millisecond Execution Events = < 1 second total latency. Acceptable for ASCII streaming.

**But**: Not comparable to traditional streaming (< 100ms). Still an order of magnitude slower.

## Next Steps

### Immediate (This Week)

1. Review all documentation with user
2. Clarify any questions or concerns
3. Get approval to proceed with implementation
4. Set up development environment

### Short-term (Month 1)

1. Implement Phase 0-4 (sender pipeline)
2. Test on Monad testnet
3. Verify cost estimates
4. Document learnings

### Medium-term (Month 2-3)

1. Implement relay node
2. Deploy to VPS with Monad node
3. Implement receiver
4. Conduct end-to-end testing

### Long-term (Month 4+)

1. Optimize compression
2. Add features (color, recording)
3. Explore mainnet deployment
4. Consider productization

## Resources

### Official Documentation

- [Monad Developer Docs](https://docs.monad.xyz/)
- [Execution Events Documentation](https://docs.monad.xyz/execution-events/)
- [Monad Testnet Information](https://docs.monad.xyz/node-ops/run-a-node)

### Tools & Libraries

- [Alloy (Rust Ethereum Library)](https://alloy.rs/)
- [Nokhwa (Camera Capture)](https://docs.rs/nokhwa/)
- [Ratatui (Terminal UI)](https://ratatui.rs/)
- [Artem (ASCII Art)](https://docs.rs/artem/)

### Community

- [Monad Discord](https://discord.gg/monad)
- [Monad Twitter](https://twitter.com/monad_xyz)
- [Rust Discord](https://discord.gg/rust-lang)

## Conclusion

Monegle demonstrates innovative use of Monad's Execution Events SDK for real-time data streaming on blockchain. While not commercially viable at current costs, it serves as an excellent technical proof-of-concept and learning project.

**Key Innovation**: Using pre-consensus execution events eliminates the need for expensive on-chain event logs while achieving sub-second latency.

**Primary Value**: Educational and demonstrative. Shows what's possible with blockchain streaming architecture.

**Future Potential**: If Layer 2 solutions reduce costs 10-100×, blockchain-based streaming could become viable for niche applications.

---

**Status**: Planning complete, ready for implementation.

**Last Updated**: 2026-02-05
