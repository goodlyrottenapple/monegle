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

    /// Submit a batch of frames via raw transaction
    /// This sends the frame data directly in calldata to the target address
    /// Receiver monitors transactions via WebSocket RPC subscriptions
    pub async fn submit_batch(&self, batch: &FrameBatch) -> Result<TransactionReceipt> {
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

        debug!("Transaction sent: {:?}", pending_tx.tx_hash());

        let receipt = pending_tx.get_receipt().await.map_err(|e| {
            error!("Failed to get receipt: {}", e);
            anyhow!("Failed to get receipt: {}", e)
        })?;

        info!(
            "Batch {} confirmed: tx={:?}, gas={}, size={}KB",
            batch.sequence,
            receipt.transaction_hash,
            receipt.gas_used,
            calldata_len / 1024
        );

        Ok(receipt)
    }

    /// Start submission loop
    pub async fn start_submission_loop(
        self,
        mut rx: mpsc::Receiver<FrameBatch>,
    ) -> Result<()> {
        info!("Starting submission loop");
        info!("Receivers should monitor transactions FROM: {}", self.sender_address);

        let mut submitted_count = 0u64;
        let mut total_gas = 0u128;
        let mut total_bytes = 0usize;

        while let Some(batch) = rx.recv().await {
            match self.submit_batch(&batch).await {
                Ok(receipt) => {
                    submitted_count += 1;
                    total_gas += receipt.gas_used;
                    total_bytes += batch.size_bytes();

                    if submitted_count % 10 == 0 {
                        let avg_gas = total_gas / submitted_count as u128;
                        let avg_bytes = total_bytes / submitted_count as usize;
                        info!(
                            "Submitted {} batches, avg gas: {}, avg size: {}KB",
                            submitted_count, avg_gas, avg_bytes / 1024
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to submit batch {}: {}", batch.sequence, e);
                    warn!("Continuing with next batch...");
                }
            }
        }

        info!("Submission loop stopped gracefully");

        info!(
            "Total batches submitted: {}, total gas: {}, total data: {}KB",
            submitted_count, total_gas, total_bytes / 1024
        );

        Ok(())
    }
}
