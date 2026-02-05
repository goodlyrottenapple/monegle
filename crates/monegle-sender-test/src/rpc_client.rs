use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, B256},
    providers::{Provider, ProviderBuilder, RootProvider},
    signers::local::PrivateKeySigner,
    transports::http::{Client, Http},
};
use alloy::rpc::types::TransactionReceipt;
use alloy::rpc::types::TransactionRequest;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMetrics {
    pub sequence: u64,
    #[serde(with = "serde_b256")]
    pub tx_hash: B256,
    pub submit_time_ms: u64,
    pub confirm_time_ms: Option<u64>,
    pub latency_ms: Option<u64>,
    pub gas_used: Option<u128>,
    pub success: bool,
    pub error: Option<String>,
    pub data_size: usize,
}

// Helper module for B256 serialization
mod serde_b256 {
    use alloy::primitives::B256;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &B256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<B256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

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
    RootProvider<Http<Client>>,
    Http<Client>,
    alloy::network::Ethereum,
>;

pub struct RpcClient {
    provider: FilledProvider,
    target_address: Address,
    metrics: Arc<Mutex<Vec<TxMetrics>>>,
    start_time: Instant,
}

impl RpcClient {
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        target_address: Address,
    ) -> Result<Self> {
        info!("Initializing RPC client");
        info!("RPC URL: {}", rpc_url);
        info!("Target: {}", target_address);

        let signer = PrivateKeySigner::from_str(private_key)
            .map_err(|e| anyhow!("Invalid private key: {}", e))?;

        let wallet = EthereumWallet::from(signer);

        let rpc_url_parsed = rpc_url.parse()?;
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(rpc_url_parsed);

        Ok(Self {
            provider,
            target_address,
            metrics: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        })
    }

    pub async fn submit_batch(&self, sequence: u64, calldata: Vec<u8>) -> Result<TxMetrics> {
        let submit_time = Instant::now();
        let submit_time_ms = submit_time.duration_since(self.start_time).as_millis() as u64;
        let data_size = calldata.len();

        debug!("Submitting batch {} ({} bytes)", sequence, data_size);

        // Create transaction (let GasFiller estimate gas automatically)
        let tx = TransactionRequest::default()
            .to(self.target_address)
            .with_input(Bytes::from(calldata));

        let mut metric = TxMetrics {
            sequence,
            tx_hash: B256::ZERO,
            submit_time_ms,
            confirm_time_ms: None,
            latency_ms: None,
            gas_used: None,
            success: false,
            error: None,
            data_size,
        };

        // Send transaction using the provider (which handles signing with wallet)
        match self.provider.send_transaction(tx).await {
            Ok(pending_tx) => {
                metric.tx_hash = *pending_tx.tx_hash();
                debug!("Transaction sent: {:?}", metric.tx_hash);

                // Wait for confirmation with timeout
                match tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    pending_tx.get_receipt()
                ).await {
                    Ok(Ok(receipt)) => {
                        let confirm_time = Instant::now();
                        let confirm_time_ms =
                            confirm_time.duration_since(self.start_time).as_millis() as u64;
                        let latency = confirm_time.duration_since(submit_time).as_millis() as u64;

                        metric.confirm_time_ms = Some(confirm_time_ms);
                        metric.latency_ms = Some(latency);
                        metric.gas_used = Some(receipt.gas_used);
                        metric.success = receipt.status();

                        if metric.success {
                            info!(
                                "Batch {} confirmed in {} ms, gas: {}",
                                sequence, latency, receipt.gas_used
                            );
                        } else {
                            warn!("Batch {} reverted", sequence);
                            metric.error = Some("Transaction reverted".to_string());
                        }
                    }
                    Ok(Err(e)) => {
                        error!("Confirmation failed for batch {}: {:?}", sequence, e);
                        metric.error = Some(format!("Confirmation failed: {:?}", e));
                    }
                    Err(_) => {
                        error!("Timeout waiting for batch {}", sequence);
                        metric.error = Some("Confirmation timeout (30s)".to_string());
                    }
                }
            }
                Err(e) => {
                    error!("Submit failed for batch {}: {:?}", sequence, e);
                    metric.error = Some(format!("Submit failed: {:?}", e));
                }
            }

        // Store metrics
        self.metrics.lock().await.push(metric.clone());

        Ok(metric)
    }

    async fn wait_for_receipt(&self, tx_hash: B256) -> Result<TransactionReceipt> {
        loop {
            match self.provider.get_transaction_receipt(tx_hash).await? {
                Some(receipt) => return Ok(receipt),
                None => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    pub async fn get_metrics(&self) -> Vec<TxMetrics> {
        self.metrics.lock().await.clone()
    }

    pub async fn print_summary(&self) {
        let metrics = self.get_metrics().await;
        let total = metrics.len();
        let successful = metrics.iter().filter(|m| m.success).count();
        let failed = total - successful;

        let latencies: Vec<u64> = metrics.iter().filter_map(|m| m.latency_ms).collect();
        let avg_latency = if !latencies.is_empty() {
            latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
        } else {
            0.0
        };

        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort();
        let p50 = if !sorted_latencies.is_empty() {
            sorted_latencies[sorted_latencies.len() / 2]
        } else {
            0
        };
        let p95 = if !sorted_latencies.is_empty() {
            sorted_latencies[(sorted_latencies.len() as f64 * 0.95) as usize]
        } else {
            0
        };
        let p99 = if !sorted_latencies.is_empty() {
            sorted_latencies[(sorted_latencies.len() as f64 * 0.99) as usize]
        } else {
            0
        };

        let gas_values: Vec<u128> = metrics.iter().filter_map(|m| m.gas_used).collect();
        let total_gas: u128 = gas_values.iter().sum();
        let avg_gas = if !gas_values.is_empty() {
            total_gas / gas_values.len() as u128
        } else {
            0
        };

        let total_data: usize = metrics.iter().map(|m| m.data_size).sum();

        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║           RPC CLIENT TEST SUMMARY                     ║");
        println!("╠═══════════════════════════════════════════════════════╣");
        println!("║ Transactions                                          ║");
        println!("║   Total:      {:>8}                                ║", total);
        println!(
            "║   Successful: {:>8} ({:>5.1}%)                      ║",
            successful,
            (successful as f64 / total as f64) * 100.0
        );
        println!(
            "║   Failed:     {:>8} ({:>5.1}%)                      ║",
            failed,
            (failed as f64 / total as f64) * 100.0
        );
        println!("║                                                       ║");
        println!("║ Latency (ms)                                          ║");
        println!("║   Average:    {:>8.0}                                ║", avg_latency);
        println!("║   P50:        {:>8}                                ║", p50);
        println!("║   P95:        {:>8}                                ║", p95);
        println!("║   P99:        {:>8}                                ║", p99);
        println!("║                                                       ║");
        println!("║ Gas Usage                                             ║");
        println!("║   Average:    {:>8}                                ║", avg_gas);
        println!("║   Total:      {:>8}                                ║", total_gas);
        println!("║                                                       ║");
        println!("║ Data                                                  ║");
        println!("║   Total sent: {:>8} KB                            ║", total_data / 1024);
        println!("╚═══════════════════════════════════════════════════════╝\n");

        // Print errors if any
        if failed > 0 {
            println!("Errors encountered:");
            for metric in metrics.iter() {
                if let Some(error) = &metric.error {
                    println!("  [Seq {}] {}", metric.sequence, error);
                }
            }
            println!();
        }
    }
}
