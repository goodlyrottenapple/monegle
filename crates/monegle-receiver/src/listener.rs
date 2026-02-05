use alloy::{
    consensus::Transaction,
    eips::BlockNumberOrTag,
    primitives::Address,
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::BlockTransactionsKind,
};
use anyhow::{anyhow, Result};
use futures::StreamExt;
use monegle_core::FrameBatch;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::{debug, info, warn, error};

/// Transaction listener component
/// Uses WebSocket RPC subscriptions to monitor blockchain transactions in real-time
/// Fallback to HTTP polling if WebSocket is unavailable
pub struct TransactionListener {
    sender_address: Address,
}

impl TransactionListener {
    /// Initialize transaction listener
    /// Monitors transactions FROM the specified sender address
    pub fn new(sender_address: &str) -> Result<Self> {
        info!("Initializing transaction listener (WebSocket RPC approach)");

        let sender_addr = Address::from_str(sender_address)
            .map_err(|e| anyhow!("Invalid sender address: {}", e))?;

        info!("Monitoring transactions FROM: {}", sender_addr);
        info!("Will extract frame data from transaction calldata");

        Ok(Self {
            sender_address: sender_addr,
        })
    }

    /// Start WebSocket subscription loop
    /// Subscribes to new blocks and extracts frame batches from transactions
    pub async fn start_websocket_loop(
        self,
        ws_url: &str,
        batch_tx: mpsc::Sender<FrameBatch>,
    ) -> Result<()> {
        info!("Starting WebSocket subscription loop");
        info!("WebSocket URL: {}", ws_url);

        // Connect via WebSocket
        let ws = WsConnect::new(ws_url);
        let provider = ProviderBuilder::new()
            .on_ws(ws)
            .await
            .map_err(|e| anyhow!("Failed to connect to WebSocket: {}", e))?;

        info!("WebSocket connected successfully");

        // Subscribe to new block headers
        let sub = provider.subscribe_blocks().await
            .map_err(|e| anyhow!("Failed to subscribe to blocks: {}", e))?;

        let mut stream = sub.into_stream();

        info!("Subscribed to new blocks via WebSocket");

        let mut block_count = 0u64;
        let mut last_heartbeat = std::time::Instant::now();

        while let Some(block_header) = stream.next().await {
            let block_number = block_header.number;
            block_count += 1;

            // Heartbeat every 10 blocks or every 10 seconds
            if block_count % 10 == 0 || last_heartbeat.elapsed().as_secs() >= 10 {
                info!("💓 WebSocket heartbeat: processed {} blocks (current: {})", block_count, block_number);
                last_heartbeat = std::time::Instant::now();
            }

            debug!("New block: {}", block_number);

            // Fetch full block with transactions
            match provider.get_block_by_number(
                BlockNumberOrTag::Number(block_number),
                BlockTransactionsKind::Full
            ).await {
                Ok(Some(full_block)) => {
                    // Extract transactions
                    if let Some(txs) = full_block.transactions.as_transactions() {
                        for tx in txs {
                            // Filter: transaction must be FROM our sender address
                            if tx.from == self.sender_address {
                                debug!(
                                    "Found transaction from sender: {:?}, to: {:?}, size: {} bytes",
                                    tx.inner.tx_hash(),
                                    tx.inner.to(),
                                    tx.inner.input().len()
                                );

                                // Extract calldata
                                let calldata = tx.inner.input().to_vec();

                                if !calldata.is_empty() {
                                    // Decode FrameBatch from calldata
                                    match FrameBatch::decode_from_bytes(&calldata) {
                                        Ok(batch) => {
                                            info!(
                                                "Received batch {} with {} frames from tx {:?}",
                                                batch.sequence,
                                                batch.frames.len(),
                                                tx.inner.tx_hash()
                                            );

                                            if batch_tx.send(batch).await.is_err() {
                                                warn!("WebSocket channel closed, stopping loop");
                                                return Ok(());
                                            }
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to decode batch from tx {:?}: {}",
                                                tx.inner.tx_hash(), e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(None) => {
                    warn!("Block {} not found", block_number);
                }
                Err(e) => {
                    error!("Failed to fetch block {}: {}", block_number, e);
                }
            }
        }

        warn!("WebSocket stream ended unexpectedly");
        Ok(())
    }

    /// Start HTTP polling loop (fallback)
    /// Polls for new blocks at regular intervals
    pub async fn start_polling_loop(
        self,
        rpc_url: &str,
        batch_tx: mpsc::Sender<FrameBatch>,
        poll_interval_ms: u64,
    ) -> Result<()> {
        info!("Starting HTTP polling loop (fallback mode)");
        info!("HTTP RPC URL: {}", rpc_url);
        info!("Poll interval: {}ms", poll_interval_ms);

        // Setup HTTP provider
        let provider = ProviderBuilder::new()
            .on_http(rpc_url.parse()?)
;

        let mut last_block = provider.get_block_number().await?;
        info!("Starting from block: {}", last_block);

        let mut interval = tokio::time::interval(
            std::time::Duration::from_millis(poll_interval_ms)
        );

        loop {
            interval.tick().await;

            // Get current block
            match provider.get_block_number().await {
                Ok(current_block) => {
                    if current_block <= last_block {
                        continue;
                    }

                    debug!(
                        "Polling blocks {} to {} for transactions from {}",
                        last_block + 1,
                        current_block,
                        self.sender_address
                    );

                    // Process new blocks
                    for block_num in (last_block + 1)..=current_block {
                        match provider.get_block_by_number(
                            BlockNumberOrTag::Number(block_num),
                            BlockTransactionsKind::Full
                        ).await {
                            Ok(Some(block)) => {
                                if let Some(txs) = block.transactions.as_transactions() {
                                    for tx in txs {
                                        // Filter: transaction FROM sender address
                                        if tx.from == self.sender_address {
                                            let calldata = tx.inner.input().to_vec();

                                            if !calldata.is_empty() {
                                                match FrameBatch::decode_from_bytes(&calldata) {
                                                    Ok(batch) => {
                                                        info!(
                                                            "Received batch {} with {} frames",
                                                            batch.sequence,
                                                            batch.frames.len()
                                                        );

                                                        if batch_tx.send(batch).await.is_err() {
                                                            warn!("Polling channel closed");
                                                            return Ok(());
                                                        }
                                                    }
                                                    Err(e) => {
                                                        warn!("Failed to decode batch: {}", e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                warn!("Block {} not found", block_num);
                            }
                            Err(e) => {
                                error!("Failed to fetch block {}: {}", block_num, e);
                            }
                        }
                    }

                    last_block = current_block;
                }
                Err(e) => {
                    warn!("Failed to get block number: {}", e);
                }
            }
        }
    }
}
