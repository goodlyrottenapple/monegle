mod capture;
mod converter;
mod batcher;
mod blockchain;
mod counter_mode;

use anyhow::{anyhow, Result};
use clap::Parser;
use monegle_core::Config;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use capture::VideoCapture;
use converter::AsciiConverter;
use batcher::FrameBatcher;
use blockchain::BlockchainSender;

#[derive(Parser, Debug)]
#[command(name = "monegle-sender")]
#[command(about = "Monegle ASCII Video Streaming Sender", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Stream name (optional, for identification)
    #[arg(short, long)]
    stream: Option<String>,

    /// Camera device index (overrides config)
    #[arg(long)]
    camera: Option<u32>,

    /// Target address for frame transactions (overrides config)
    #[arg(long)]
    target: Option<String>,

    /// Dry run mode: display ASCII video in terminal without sending to blockchain
    #[arg(long)]
    dry_run: bool,

    /// Test counter mode: send frames with incrementing counter instead of camera
    #[arg(long)]
    counter: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    info!("Monegle Sender starting...");

    // Load configuration
    let config = Config::from_file(&args.config)?;
    config.validate()?;

    let sender_config = config.sender
        .ok_or_else(|| anyhow!("Sender configuration not found"))?;

    let network_config = config.network;

    // Get private key from environment (only needed if not dry-run)
    let private_key = if !args.dry_run {
        std::env::var("MONAD_PRIVATE_KEY")
            .or_else(|_| std::env::var("MONEGLE_PRIVATE_KEY"))
            .or_else(|_| std::env::var("PRIVATE_KEY"))
            .map_err(|_| anyhow!("MONAD_PRIVATE_KEY environment variable not set"))?
    } else {
        String::new() // Not needed for dry-run
    };

    // Initialize blockchain sender (only if not dry-run)
    let (_sender_address, stream_id) = if !args.dry_run {
        let target_address = args.target.clone()
            .unwrap_or_else(|| sender_config.target_address.clone());

        let blockchain_sender = BlockchainSender::new(
            &network_config.rpc_url,
            &private_key,
            &target_address,
        ).await?;

        let sender_address = blockchain_sender.sender_address();
        info!("Sender ready! Receivers should monitor transactions FROM: {}", sender_address);

        let mut stream_id = [0u8; 32];
        stream_id[..20].copy_from_slice(sender_address.as_slice());
        info!("Stream ID: {}", hex::encode(&stream_id));

        (sender_address, stream_id)
    } else {
        info!("=== DRY RUN MODE ===");
        info!("Will display ASCII video in terminal (no blockchain submission)");
        (Default::default(), [0u8; 32])
    };

    // Initialize components
    let camera_device = args.camera.unwrap_or(sender_config.camera_device);
    let fps = sender_config.fps as u32;
    let width = sender_config.resolution[0] as u32;
    let height = sender_config.resolution[1] as u32;

    let ascii_converter = AsciiConverter::new(
        width,
        height,
        sender_config.character_set,
        sender_config.color_mode,
    );

    info!("Components initialized, starting pipeline");
    info!("Press Ctrl+C to stop");

    if args.counter {
        // COUNTER TEST MODE: Generate test frames with incrementing counter
        if args.dry_run {
            // Counter test mode with terminal display only
            counter_mode::run_counter_dry_run_mode(
                width,
                height,
                fps,
                sender_config.color_mode,
            ).await?;
        } else {
            // Counter test mode with blockchain submission
            let metadata = monegle_core::StreamMetadata {
                fps: sender_config.fps,
                width: sender_config.resolution[0],
                height: sender_config.resolution[1],
                compression_type: sender_config.compression,
                character_set: sender_config.character_set,
                color_mode: sender_config.color_mode,
                frames_per_batch: sender_config.frames_per_batch,
            };

            let target_address = args.target
                .unwrap_or_else(|| sender_config.target_address.clone());

            let blockchain_sender = BlockchainSender::new(
                &network_config.rpc_url,
                &private_key,
                &target_address,
            ).await?;

            counter_mode::run_counter_blockchain_mode(
                width,
                height,
                fps,
                sender_config.color_mode,
                metadata,
                stream_id,
                sender_config.max_batch_size,
                sender_config.keyframe_interval,
                blockchain_sender,
            ).await?;
        }
    } else if args.dry_run {
        // DRY RUN MODE: Camera → ASCII → Terminal Display
        run_dry_run_mode(camera_device, fps, ascii_converter).await?;
    } else {
        // NORMAL MODE: Camera → ASCII → Batch → Blockchain

        // Create stream metadata to include in each batch
        let metadata = monegle_core::StreamMetadata {
            fps: sender_config.fps,
            width: sender_config.resolution[0],
            height: sender_config.resolution[1],
            compression_type: sender_config.compression,
            character_set: sender_config.character_set,
            color_mode: sender_config.color_mode,
            frames_per_batch: sender_config.frames_per_batch,
        };

        let frame_batcher = FrameBatcher::new(
            stream_id,
            metadata,
            sender_config.max_batch_size,
            sender_config.keyframe_interval,
        );

        let target_address = args.target
            .unwrap_or_else(|| sender_config.target_address.clone());

        let blockchain_sender = BlockchainSender::new(
            &network_config.rpc_url,
            &private_key,
            &target_address,
        ).await?;

        run_normal_mode(
            camera_device,
            fps,
            ascii_converter,
            frame_batcher,
            blockchain_sender,
        ).await?;
    }

    info!("Monegle Sender stopped");

    Ok(())
}

/// Run in dry-run mode: display ASCII video in terminal
async fn run_dry_run_mode(
    camera_device: u32,
    fps: u32,
    ascii_converter: AsciiConverter,
) -> Result<()> {
    let (capture_tx, capture_rx) = mpsc::channel(10);
    let (convert_tx, mut convert_rx) = mpsc::channel::<String>(10);

    // Spawn camera capture
    let mut capture_handle = tokio::task::spawn_blocking(move || {
        match VideoCapture::new(camera_device, fps, 640, 480) {
            Ok(video_capture) => {
                info!("Camera opened successfully!");
                if let Err(e) = video_capture.start_capture_loop_blocking(capture_tx) {
                    error!("Capture loop error: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to initialize camera: {}", e);
            }
        }
    });

    // Spawn ASCII conversion
    let mut convert_handle = tokio::spawn(async move {
        if let Err(e) = ascii_converter.start_conversion_loop(capture_rx, convert_tx).await {
            error!("Conversion loop error: {}", e);
        }
    });

    // Display loop - show ASCII frames in terminal
    info!("\n\n========== ASCII VIDEO PREVIEW ==========\n");

    let mut frame_count = 0u64;
    let start_time = std::time::Instant::now();

    let mut capture_done = false;
    let mut convert_done = false;

    loop {
        tokio::select! {
            Some(ascii_frame) = convert_rx.recv() => {
                frame_count += 1;

                // Clear terminal and show frame
                print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top

                // Show frame info
                println!("╔════════════════════════════════════════════════════════╗");
                println!("║  Monegle Dry Run - ASCII Video Preview                ║");
                println!("╠════════════════════════════════════════════════════════╣");
                println!("║  Frame: {:<10}  FPS: {:<5}  Time: {:<7.1}s      ║",
                    frame_count,
                    fps,
                    start_time.elapsed().as_secs_f32()
                );
                println!("╚════════════════════════════════════════════════════════╝");
                println!();

                // Display ASCII frame
                println!("{}", ascii_frame);

                println!();
                println!("Press Ctrl+C to stop");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("\nReceived Ctrl+C, shutting down...");
                break;
            }
            _ = &mut capture_handle, if !capture_done => {
                info!("Capture task ended");
                capture_done = true;
            }
            _ = &mut convert_handle, if !convert_done => {
                info!("Conversion task ended");
                convert_done = true;
            }
        }

        if capture_done && convert_done {
            break;
        }
    }

    let duration = start_time.elapsed().as_secs_f32();
    let avg_fps = frame_count as f32 / duration;

    info!("\nDry run complete!");
    info!("Total frames: {}", frame_count);
    info!("Duration: {:.1}s", duration);
    info!("Average FPS: {:.1}", avg_fps);

    Ok(())
}

/// Run in normal mode: send to blockchain
async fn run_normal_mode(
    camera_device: u32,
    fps: u32,
    ascii_converter: AsciiConverter,
    frame_batcher: FrameBatcher,
    blockchain_sender: BlockchainSender,
) -> Result<()> {
    let (capture_tx, capture_rx) = mpsc::channel(10);
    let (convert_tx, convert_rx) = mpsc::channel(10);
    let (batch_tx, batch_rx) = mpsc::channel(5);

    // Spawn camera capture
    let capture_handle = tokio::task::spawn_blocking(move || {
        match VideoCapture::new(camera_device, fps, 640, 480) {
            Ok(video_capture) => {
                if let Err(e) = video_capture.start_capture_loop_blocking(capture_tx) {
                    error!("Capture loop error: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to initialize camera: {}", e);
            }
        }
    });

    // Spawn ASCII conversion
    let convert_handle = tokio::spawn(async move {
        if let Err(e) = ascii_converter.start_conversion_loop(capture_rx, convert_tx).await {
            error!("Conversion loop error: {}", e);
        }
    });

    // Spawn frame batching
    let batch_handle = tokio::spawn(async move {
        if let Err(e) = frame_batcher.start_batching_loop(convert_rx, batch_tx).await {
            error!("Batching loop error: {}", e);
        }
    });

    // Spawn blockchain submission
    let blockchain_handle = tokio::spawn(async move {
        if let Err(e) = blockchain_sender.start_submission_loop(batch_rx).await {
            error!("Blockchain submission error: {}", e);
        }
    });

    info!("Pipeline started! Press Ctrl+C to stop.");

    // Wait for Ctrl+C or task completion
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = capture_handle => {
            info!("Capture task ended");
        }
        _ = convert_handle => {
            info!("Conversion task ended");
        }
        _ = batch_handle => {
            info!("Batching task ended");
        }
        _ = blockchain_handle => {
            info!("Blockchain task ended");
        }
    }

    Ok(())
}
