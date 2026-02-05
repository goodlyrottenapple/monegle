mod rpc_client;

use anyhow::Result;
use clap::Parser;
use monegle_core::{CompressionType, StreamId, SyntheticFrameGenerator};
use rpc_client::RpcClient;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "monegle-sender-test")]
#[command(about = "RPC throughput test for Monegle", long_about = None)]
struct Args {
    /// RPC endpoint URL
    #[arg(short, long)]
    rpc_url: String,

    /// Private key (or use MONAD_PRIVATE_KEY env var)
    #[arg(short, long)]
    private_key: Option<String>,

    /// Target address (dummy address for frame transactions)
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

    /// Output metrics to JSON file
    #[arg(short, long)]
    output: Option<String>,

    /// Use static frames (better compression)
    #[arg(long)]
    static_frames: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Get private key from args or environment
    let private_key = args.private_key
        .or_else(|| std::env::var("MONAD_PRIVATE_KEY").ok())
        .or_else(|| std::env::var("PRIVATE_KEY").ok())
        .expect("Private key must be provided via --private-key or MONAD_PRIVATE_KEY environment variable");

    println!("\n╔═══════════════════════════════════════════════════════╗");
    println!("║     MONEGLE RPC THROUGHPUT FEASIBILITY TEST          ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║ RPC URL:      {:<40}║", truncate(&args.rpc_url, 40));
    println!("║ Target:       {:<40}║", &args.target_address);
    println!("║ Quality:      {} FPS, {}×{} chars{:<20}║",
        args.fps, args.width, args.height, "");
    println!("║ Duration:     {} seconds{:<33}║", args.duration, "");
    println!("║ Frame type:   {:<40}║",
        if args.static_frames { "Static (high compression)" } else { "Random (realistic)" });
    println!("╚═══════════════════════════════════════════════════════╝\n");

    // Initialize RPC client
    let client = RpcClient::new(
        &args.rpc_url,
        &private_key,
        args.target_address.parse()?,
    )
    .await?;

    // Initialize synthetic frame generator
    let mut generator = SyntheticFrameGenerator::new(args.width, args.height);

    // Calculate frames per batch (based on 400ms block time)
    // 2.5 blocks/sec × frame_interval = frames_per_batch
    let frames_per_batch = ((0.4 * args.fps as f32).ceil() as usize).max(1);
    let batch_interval = Duration::from_millis(400);

    let stream_id: StreamId = [0u8; 32]; // Dummy stream ID for testing

    info!(
        "Batching: {} frames per batch, every {} ms",
        frames_per_batch,
        batch_interval.as_millis()
    );
    info!("Target rate: 2.5 transactions/second");

    println!("Starting test...\n");

    // Run test
    let start_time = std::time::Instant::now();
    let mut sequence = 0u64;
    let mut ticker = tokio::time::interval(batch_interval);

    while start_time.elapsed().as_secs() < args.duration {
        ticker.tick().await;

        // Generate batch
        let batch = if args.static_frames {
            generator.generate_static_batch(
                frames_per_batch,
                sequence,
                stream_id,
                CompressionType::None,
            )
        } else {
            generator.generate_batch(
                frames_per_batch,
                sequence,
                stream_id,
                CompressionType::None,
            )
        };

        // Encode to bytes
        let encoded = batch.encode_to_bytes()?;

        info!(
            "[Seq {}] Generated batch: {} frames, {} bytes",
            sequence,
            batch.frames.len(),
            encoded.len()
        );

        // Submit to blockchain
        match client.submit_batch(sequence, encoded).await {
            Ok(metric) => {
                if !metric.success {
                    warn!("Batch {} failed: {:?}", sequence, metric.error);
                }
            }
            Err(e) => {
                warn!("Batch {} error: {:?}", sequence, e);
            }
        }

        sequence += 1;
    }

    println!("\nTest complete!\n");

    // Print summary
    client.print_summary().await;

    // Export metrics if requested
    if let Some(output_path) = args.output {
        let metrics = client.get_metrics().await;
        let json = serde_json::to_string_pretty(&metrics)?;
        std::fs::write(&output_path, json)?;
        println!("✓ Metrics exported to: {}\n", output_path);
    }

    // Provide recommendations
    let metrics = client.get_metrics().await;
    let total = metrics.len();
    let successful = metrics.iter().filter(|m| m.success).count();
    let success_rate = (successful as f64 / total as f64) * 100.0;

    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║                   RECOMMENDATIONS                     ║");
    println!("╠═══════════════════════════════════════════════════════╣");

    if success_rate >= 95.0 {
        println!("║ ✅ EXCELLENT: {}% success rate                     ║", success_rate as u32);
        println!("║                                                       ║");
        println!("║ This RPC endpoint is suitable for production use.    ║");
        println!("║ Proceed with full Monegle implementation.            ║");
    } else if success_rate >= 80.0 {
        println!("║ ⚠️  MODERATE: {}% success rate                     ║", success_rate as u32);
        println!("║                                                       ║");
        println!("║ Recommendations:                                      ║");
        println!("║   • Implement RPC rotation (multiple endpoints)      ║");
        println!("║   • Add retry logic for failed transactions          ║");
        println!("║   • Consider reducing FPS slightly                   ║");
    } else {
        println!("║ ❌ POOR: {}% success rate                          ║", success_rate as u32);
        println!("║                                                       ║");
        println!("║ Critical issues detected. Options:                   ║");
        println!("║   1. Use paid RPC service (Alchemy, Chainstack)     ║");
        println!("║   2. Reduce FPS (try 10 FPS instead of 15)          ║");
        println!("║   3. Test alternative RPC endpoints                  ║");
        println!("║   4. Reconsider project viability                    ║");
    }

    println!("╚═══════════════════════════════════════════════════════╝\n");

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[0..max_len - 3])
    }
}
