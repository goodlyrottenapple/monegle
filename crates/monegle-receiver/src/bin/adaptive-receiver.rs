use anyhow::Result;
use clap::Parser;
use monegle_core::{decode_frame, FrameBatch};
use std::collections::VecDeque;
use std::io::Write;
use tokio::sync::mpsc;
use tracing::{info, warn};

mod listener {
    pub use monegle_receiver::listener::*;
}

use listener::TransactionListener;

#[derive(Parser, Debug)]
#[command(name = "adaptive-receiver")]
#[command(about = "Adaptive buffering receiver for smooth playback")]
struct Args {
    /// Sender address to monitor
    #[arg(short, long)]
    sender_address: String,

    /// WebSocket URL
    #[arg(long)]
    ws_url: String,

    /// Target FPS for playback
    #[arg(long, default_value = "5")]
    fps: u32,

    /// Initial buffer threshold (multiplier of FPS)
    #[arg(long, default_value = "20")]
    initial_buffer_multiplier: u32,

    /// Resume buffer threshold (multiplier of FPS)
    #[arg(long, default_value = "5")]
    resume_buffer_multiplier: u32,
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

    let initial_buffer_size = args.fps * args.initial_buffer_multiplier;
    let resume_buffer_size = args.fps * args.resume_buffer_multiplier;

    info!("Adaptive Receiver starting...");
    info!("Monitoring: {}", args.sender_address);
    info!("WebSocket: {}", args.ws_url);
    info!("Target FPS: {}", args.fps);
    info!("Initial buffer: {} frames ({}×FPS)", initial_buffer_size, args.initial_buffer_multiplier);
    info!("Resume buffer: {} frames ({}×FPS)", resume_buffer_size, args.resume_buffer_multiplier);

    // Initialize listener
    let listener = TransactionListener::new(&args.sender_address)?;
    let (batch_tx, batch_rx) = mpsc::channel::<FrameBatch>(100);

    // Spawn WebSocket listener
    tokio::spawn(async move {
        if let Err(e) = listener.start_websocket_loop(&args.ws_url, batch_tx).await {
            warn!("WebSocket loop error: {}", e);
        }
    });

    // Create channels for decoded frames
    let (decoded_tx, decoded_rx) = mpsc::channel::<DecodedFrame>(1000);

    // Spawn decoder task
    tokio::spawn(decode_task(batch_rx, decoded_tx));

    // Run playback task
    playback_task(decoded_rx, args.fps, initial_buffer_size as usize, resume_buffer_size as usize).await?;

    Ok(())
}

#[derive(Debug, Clone)]
struct DecodedFrame {
    sequence: u64,
    frame_index: usize,
    text: String,
}

/// Decoder task: receives batches, decodes frames, sends to playback
async fn decode_task(
    mut batch_rx: mpsc::Receiver<FrameBatch>,
    decoded_tx: mpsc::Sender<DecodedFrame>,
) {
    info!("Decoder task started");

    let mut previous_frame: Option<String> = None;
    let mut total_decoded = 0u64;
    let start_time = std::time::Instant::now();

    while let Some(batch) = batch_rx.recv().await {
        let sequence = batch.sequence;
        let num_frames = batch.frames.len();

        for (idx, compressed_frame) in batch.frames.iter().enumerate() {
            // Decode frame
            match decode_frame(
                compressed_frame,
                if compressed_frame.is_keyframe {
                    None
                } else {
                    previous_frame.as_deref()
                },
            ) {
                Ok(text) => {
                    previous_frame = Some(text.clone());
                    total_decoded += 1;

                    let decoded = DecodedFrame {
                        sequence,
                        frame_index: idx,
                        text,
                    };

                    if decoded_tx.send(decoded).await.is_err() {
                        warn!("Playback channel closed, stopping decoder");
                        return;
                    }

                    if total_decoded % 10 == 0 {
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let decode_fps = total_decoded as f32 / elapsed;
                        info!("🔧 DECODED {} frames | Rate: {:.1} FPS | Elapsed: {:.1}s",
                            total_decoded, decode_fps, elapsed);
                    }
                }
                Err(e) => {
                    warn!("Failed to decode frame {}.{}: {}", sequence, idx, e);
                }
            }
        }
    }

    info!("Decoder task ended: {} frames decoded", total_decoded);
}

/// Playback task: buffers decoded frames, displays at target FPS
async fn playback_task(
    mut decoded_rx: mpsc::Receiver<DecodedFrame>,
    target_fps: u32,
    initial_buffer_size: usize,
    resume_buffer_size: usize,
) -> Result<()> {
    info!("Playback task started");

    let mut buffer: VecDeque<DecodedFrame> = VecDeque::with_capacity(initial_buffer_size * 2);
    let frame_interval = tokio::time::Duration::from_secs_f32(1.0 / target_fps as f32);

    let mut stdout = std::io::stdout();
    let mut current_frame: Option<DecodedFrame> = None;
    let mut frames_displayed = 0u64;
    let start_time = std::time::Instant::now();
    let mut is_playing = false;

    info!("⏸️  BUFFERING: Collecting initial {} frames...", initial_buffer_size);

    loop {
        tokio::select! {
            // Receive new decoded frames
            Some(frame) = decoded_rx.recv() => {
                buffer.push_back(frame);

                // Start playing once we reach initial buffer size
                if !is_playing && buffer.len() >= initial_buffer_size {
                    info!("▶️  PLAYING: Buffer full ({} frames), starting playback at {} FPS", buffer.len(), target_fps);
                    is_playing = true;
                }

                // Log buffer state periodically
                if buffer.len() % 50 == 0 {
                    info!("📦 Buffer: {} frames", buffer.len());
                }
            }

            // Display frames at target FPS (only when playing)
            _ = tokio::time::sleep(frame_interval), if is_playing => {
                if let Some(frame) = buffer.pop_front() {
                    // Display frame
                    print!("\x1B[2J\x1B[H"); // Clear screen
                    println!("╔════════════════════════════════════════════════════════╗");
                    println!("║  Monegle Adaptive Receiver - Press Ctrl+C to stop    ║");
                    println!("╠════════════════════════════════════════════════════════╣");
                    println!("║  Frame: {}  Buffer: {}  Seq: {}  FPS: {:.1}     ║",
                        frames_displayed,
                        buffer.len(),
                        frame.sequence,
                        target_fps
                    );
                    println!("╚════════════════════════════════════════════════════════╝");
                    println!();
                    println!("{}", frame.text);
                    println!();

                    let elapsed = start_time.elapsed().as_secs_f32();
                    let actual_fps = frames_displayed as f32 / elapsed;
                    println!("Displayed: {} | Actual FPS: {:.1} | Elapsed: {:.1}s | Buffer: {} frames",
                        frames_displayed, actual_fps, elapsed, buffer.len());

                    stdout.flush()?;

                    current_frame = Some(frame);
                    frames_displayed += 1;

                    // Check if buffer is running low
                    if buffer.is_empty() {
                        info!("⏸️  PAUSED: Buffer empty, waiting for {} frames to resume...", resume_buffer_size);
                        is_playing = false;
                    }
                } else {
                    // Buffer empty but we're supposed to be playing - this shouldn't happen
                    // but handle it gracefully
                    warn!("⚠️  Buffer underrun during playback");
                    is_playing = false;
                }
            }

            else => {
                // No more frames being received
                break;
            }
        }

        // Resume playback when buffer reaches resume threshold
        if !is_playing && buffer.len() >= resume_buffer_size {
            info!("▶️  RESUMED: Buffer at {} frames, resuming playback", buffer.len());
            is_playing = true;
        }
    }

    let elapsed = start_time.elapsed().as_secs_f32();
    let avg_fps = frames_displayed as f32 / elapsed;
    info!("Playback ended: {} frames in {:.1}s ({:.1} FPS average)",
        frames_displayed, elapsed, avg_fps);

    Ok(())
}
