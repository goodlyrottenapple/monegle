use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes},
    providers::{Provider, ProviderBuilder},
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::local::PrivateKeySigner,
    transports::http::reqwest::Url,
};
use anyhow::{anyhow, Result};
use monegle_core::FrameBatch;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::{debug, info, warn, error};

type FilledProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::fillers::JoinFill<
            alloy::providers::Identity,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::GasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::BlobGasFiller,
                    alloy::providers::fillers::JoinFill<
                        alloy::providers::fillers::NonceFiller,
                        alloy::providers::fillers::ChainIdFiller,
                    >,
                >,
            >,
        >,
        alloy::providers::fillers::WalletFiller<EthereumWallet>,
    >,
    alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>,
    alloy::transports::http::Http<alloy::transports::http::Client>,
    alloy::network::Ethereum,
>;

/// Blockchain sender component
/// Uses raw transaction approach: sends frame data in calldata to target address
/// No smart contract needed! Receiver monitors transactions via WebSocket RPC
pub struct BlockchainSender {
    provider: FilledProvider,
    target_address: Address,
    sender_address: Address,
}

impl BlockchainSender {
    /// Initialize blockchain sender with raw transactions (no contract needed!)
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        target_address: &str,
    ) -> Result<Self> {
        info!("Initializing blockchain sender with raw transaction approach");
        info!("RPC URL: {}", rpc_url);
        info!("Target: {}", target_address);

        // Parse private key
        let signer = PrivateKeySigner::from_str(private_key)
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;

        let sender_address = signer.address();
        info!("Sender address: {}", sender_address);

        let wallet = EthereumWallet::from(signer);

        // Setup provider with HTTP transport
        let url = Url::parse(rpc_url)?;
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(url);

        // Parse target address (dummy address for frame transactions)
        let target_addr = Address::from_str(target_address)
            .map_err(|e| anyhow!("Invalid target address: {}", e))?;

        info!("Ready to stream! Receiver should monitor transactions to: {}", sender_address);

        Ok(Self {
            provider,
            target_address: target_addr,
            sender_address,
        })
    }

    /// Get the sender address (this is what receivers should monitor)
    pub fn sender_address(&self) -> Address {
        self.sender_address
    }

    /// Submit a batch of frames via raw transaction (FAST - doesn't wait for confirmation)
    /// This sends the frame data directly in calldata to the target address
    /// Receiver monitors transactions via WebSocket RPC subscriptions
    pub async fn submit_batch_fast(&self, batch: &FrameBatch) -> Result<String> {
        debug!("Submitting batch {} via raw transaction", batch.sequence);

        // Encode frame batch to bytes
        let encoded = batch.encode_to_bytes()?;
        let calldata = Bytes::from(encoded);
        let calldata_len = calldata.len();

        debug!("Batch {} encoded: {} bytes", batch.sequence, calldata_len);

        // Create raw transaction to target address with frame data in calldata
        let tx = TransactionRequest::default()
            .to(self.target_address)
            .with_input(calldata);
        // Note: Gas is estimated automatically by GasFiller

        let pending_tx = self.provider.send_transaction(tx)
            .await
            .map_err(|e| {
                error!("Failed to send transaction: {}", e);
                anyhow!("Transaction send failed: {}", e)
            })?;

        let tx_hash = format!("{:?}", pending_tx.tx_hash());

        info!(
            "Batch {} submitted: tx={}, size={}KB (not waiting for confirmation)",
            batch.sequence,
            tx_hash,
            calldata_len / 1024
        );

        Ok(tx_hash)
    }

    /// Start submission loop (RATE-LIMITED MODE - submits with small delay between txs)
    pub async fn start_submission_loop(
        self,
        mut rx: mpsc::Receiver<FrameBatch>,
    ) -> Result<()> {
        info!("Starting rate-limited submission loop");
        info!("Receivers should monitor transactions FROM: {}", self.sender_address);
        info!("Transactions will be submitted with 1.5s delay between each (max ~0.67 tx/sec)");

        let mut submitted_count = 0u64;
        let mut total_bytes = 0usize;
        let start_time = std::time::Instant::now();
        let mut last_submit_time = std::time::Instant::now();

        // Rate limiting: minimum time between transactions
        // Increased to 1.5 seconds to give RPC node time to update nonces
        let min_interval = std::time::Duration::from_millis(1500); // 1.5s between txs

        while let Some(batch) = rx.recv().await {
            // Wait if we're submitting too fast
            let elapsed_since_last = last_submit_time.elapsed();
            if elapsed_since_last < min_interval {
                let wait_time = min_interval - elapsed_since_last;
                debug!("Rate limiting: waiting {:?} before next submission", wait_time);
                tokio::time::sleep(wait_time).await;
            }

            match self.submit_batch_fast(&batch).await {
                Ok(tx_hash) => {
                    submitted_count += 1;
                    total_bytes += batch.size_bytes();
                    last_submit_time = std::time::Instant::now();

                    if submitted_count % 10 == 0 {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let rate = submitted_count as f32 / elapsed;
                        let avg_bytes = total_bytes / submitted_count as usize;
                        info!(
                            "Submitted {} batches in {:.1}s ({:.1} tx/sec), avg size: {}KB",
                            submitted_count, elapsed, rate, avg_bytes / 1024
                        );
                    }

                    debug!("Batch {} submitted: {}", batch.sequence, tx_hash);
                }
                Err(e) => {
                    error!("Failed to submit batch {}: {}", batch.sequence, e);

                    // If it's a nonce error, wait longer for mempool to clear
                    if e.to_string().contains("higher priority") || e.to_string().contains("nonce") {
                        warn!("Nonce collision detected - waiting 3 seconds for mempool to clear...");
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    } else {
                        warn!("Retrying after 2 second delay...");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }

                    // Try one more time
                    match self.submit_batch_fast(&batch).await {
                        Ok(tx_hash) => {
                            submitted_count += 1;
                            total_bytes += batch.size_bytes();
                            last_submit_time = std::time::Instant::now();
                            info!("✓ Retry successful for batch {}: {}", batch.sequence, tx_hash);
                        }
                        Err(e2) => {
                            error!("✗ Retry failed for batch {}: {}", batch.sequence, e2);
                            warn!("Skipping batch {} and continuing...", batch.sequence);
                        }
                    }
                }
            }
        }

        let elapsed = start_time.elapsed().as_secs_f32();
        let rate = submitted_count as f32 / elapsed;

        info!("Submission loop stopped gracefully");
        info!(
            "Total: {} batches in {:.1}s ({:.1} tx/sec), {}KB total data",
            submitted_count, elapsed, rate, total_bytes / 1024
        );

        Ok(())
    }
}
