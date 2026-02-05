use anyhow::Result;
use clap::Parser;
use monegle_core::{decode_frame, FrameBatch};
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::{info, warn};

mod listener {
    pub use monegle_receiver::listener::*;
}

use listener::TransactionListener;

#[derive(Parser, Debug)]
#[command(name = "frame-collector")]
#[command(about = "Collect frames from blockchain and save to files")]
struct Args {
    /// Sender address to monitor
    #[arg(short, long)]
    sender_address: String,

    /// WebSocket URL
    #[arg(long)]
    ws_url: String,

    /// Output directory for frames
    #[arg(short, long, default_value = "frames")]
    output_dir: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    let args = Args::parse();

    info!("Frame Collector starting...");
    info!("Monitoring: {}", args.sender_address);
    info!("WebSocket: {}", args.ws_url);
    info!("Output directory: {}", args.output_dir);

    // Create output directory
    let output_path = PathBuf::from(&args.output_dir);
    fs::create_dir_all(&output_path)?;
    info!("Created output directory: {:?}", output_path);

    // Initialize listener
    let listener = TransactionListener::new(&args.sender_address)?;
    let (batch_tx, mut batch_rx) = mpsc::channel::<FrameBatch>(100);

    // Spawn WebSocket listener
    tokio::spawn(async move {
        if let Err(e) = listener.start_websocket_loop(&args.ws_url, batch_tx).await {
            warn!("WebSocket loop error: {}", e);
        }
    });

    info!("Listening for frames...");

    let mut total_frames_saved = 0u64;
    let mut batch_count = 0u64;
    let mut previous_frame: Option<String> = None;
    let start_time = std::time::Instant::now();
    let mut last_log_time = start_time;

    // Receive and save frames
    while let Some(batch) = batch_rx.recv().await {
        batch_count += 1;
        let sequence = batch.sequence;
        let num_frames = batch.frames.len();

        info!("Received batch {} with {} frames", sequence, num_frames);

        // Decode and save each frame
        for (idx, compressed_frame) in batch.frames.iter().enumerate() {
            // Decode frame (with delta decoding support)
            let frame_text = match decode_frame(
                compressed_frame,
                if compressed_frame.is_keyframe {
                    None
                } else {
                    previous_frame.as_deref()
                },
            ) {
                Ok(text) => {
                    previous_frame = Some(text.clone());
                    text
                }
                Err(e) => {
                    warn!("Failed to decode frame {}.{}: {}", sequence, idx, e);
                    continue;
                }
            };

            // Calculate global frame number
            let frame_number = total_frames_saved;

            // Save to file
            let filename = format!("frame_{:06}_seq{}_idx{}.txt", frame_number, sequence, idx);
            let filepath = output_path.join(filename);

            match fs::write(&filepath, &frame_text) {
                Ok(_) => {
                    total_frames_saved += 1;

                    // Log every 10 frames or every 2 seconds
                    if frame_number % 10 == 0 || last_log_time.elapsed().as_secs() >= 2 {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let fps = total_frames_saved as f32 / elapsed;
                        info!("Saved frame {} (seq {}, idx {}) | Total: {} frames | FPS: {:.1} | Elapsed: {:.1}s",
                            frame_number, sequence, idx, total_frames_saved, fps, elapsed);
                        last_log_time = std::time::Instant::now();
                    }
                }
                Err(e) => {
                    warn!("Failed to save frame {}: {}", frame_number, e);
                }
            }
        }
    }

    info!("Collection ended: {} batches, {} frames saved", batch_count, total_frames_saved);

    Ok(())
}
