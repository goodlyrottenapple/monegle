use anyhow::Result;
use monegle_core::{ColorMode, FrameBatch, StreamMetadata};
use rand::Rng;
use tokio::sync::mpsc;
use tracing::info;

use crate::batcher::FrameBatcher;
use crate::blockchain::BlockchainSender;

/// Generate a test frame with random noise and a counter box
pub fn generate_counter_frame(width: u32, height: u32, counter: u64, color_mode: ColorMode) -> String {
    let mut rng = rand::thread_rng();

    // Character set for random noise
    let noise_chars = " .:-=+*#%@";

    // Box dimensions (centered)
    let box_width = 20;
    let box_height = 7;
    let box_x = (width - box_width) / 2;
    let box_y = (height - box_height) / 2;

    let mut result = String::with_capacity((width * height * 20) as usize);

    for y in 0..height {
        for x in 0..width {
            // Check if we're inside the box
            let in_box = x >= box_x && x < box_x + box_width
                      && y >= box_y && y < box_y + box_height;

            let ch = if in_box {
                // Inside the box - white background
                let border = x == box_x || x == box_x + box_width - 1
                          || y == box_y || y == box_y + box_height - 1;

                if border {
                    '█' // Box border
                } else {
                    // Counter text in the middle
                    let text_y = box_y + box_height / 2;
                    let counter_str = format!("{}", counter);
                    let text_x_start = box_x + (box_width - counter_str.len() as u32) / 2;

                    if y == text_y && x >= text_x_start && x < text_x_start + counter_str.len() as u32 {
                        counter_str.chars().nth((x - text_x_start) as usize).unwrap_or(' ')
                    } else {
                        ' ' // White space inside box
                    }
                }
            } else {
                // Outside the box - random noise
                noise_chars.chars().nth(rng.gen_range(0..noise_chars.len())).unwrap()
            };

            // Apply color if RGB mode
            if color_mode == ColorMode::Rgb && !in_box {
                // Random colors for noise
                let r = rng.gen_range(50..200);
                let g = rng.gen_range(50..200);
                let b = rng.gen_range(50..200);
                result.push_str(&format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, ch));
            } else if in_box && color_mode == ColorMode::Rgb {
                // White for box
                result.push_str(&format!("\x1b[38;2;255;255;255m{}\x1b[0m", ch));
            } else {
                result.push(ch);
            }
        }

        if y < height - 1 {
            result.push('\n');
        }
    }

    result
}

/// Run counter mode with dry-run (terminal display only)
pub async fn run_counter_dry_run_mode(
    width: u32,
    height: u32,
    fps: u32,
    color_mode: ColorMode,
) -> Result<()> {
    info!("Starting counter test mode (dry-run)");
    info!("Resolution: {}x{}, FPS: {}, Color: {:?}", width, height, fps, color_mode);

    let frame_interval = tokio::time::Duration::from_secs_f32(1.0 / fps as f32);
    let mut interval = tokio::time::interval(frame_interval);

    let start_time = std::time::Instant::now();
    let mut frame_count = 0u64;

    loop {
        interval.tick().await;

        // Counter increments every second
        let counter = start_time.elapsed().as_secs();

        // Generate test frame
        let frame = generate_counter_frame(width, height, counter, color_mode);

        // Display in terminal
        print!("\x1B[2J\x1B[H"); // Clear screen
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║  Monegle Counter Test Mode - Press Ctrl+C to stop    ║");
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║  Frame: {}  Counter: {}  Time: {:.1}s              ║",
            frame_count, counter, start_time.elapsed().as_secs_f32());
        println!("╚════════════════════════════════════════════════════════╝");
        println!("{}", frame);

        frame_count += 1;
    }
}

/// Run counter mode with blockchain submission
pub async fn run_counter_blockchain_mode(
    width: u32,
    height: u32,
    fps: u32,
    color_mode: ColorMode,
    metadata: StreamMetadata,
    stream_id: [u8; 32],
    max_batch_size: usize,
    keyframe_interval: u64,
    blockchain_sender: BlockchainSender,
) -> Result<()> {
    info!("Starting counter test mode (blockchain)");
    info!("Resolution: {}x{}, FPS: {}, Color: {:?}", width, height, fps, color_mode);

    // Channel for frames → batcher
    let (convert_tx, convert_rx) = mpsc::channel::<String>(10);
    // Channel for batches → blockchain
    let (batch_tx, batch_rx) = mpsc::channel::<FrameBatch>(5);

    // Spawn batcher
    let frame_batcher = FrameBatcher::new(
        stream_id,
        metadata,
        max_batch_size,
        keyframe_interval,
    );

    let batcher_handle = tokio::spawn(async move {
        frame_batcher.start_batching_loop(convert_rx, batch_tx).await
    });

    // Spawn blockchain sender
    let blockchain_handle = tokio::spawn(async move {
        blockchain_sender.start_submission_loop(batch_rx).await
    });

    // Generate and send frames
    let frame_interval = tokio::time::Duration::from_secs_f32(1.0 / fps as f32);
    let mut interval = tokio::time::interval(frame_interval);

    let start_time = std::time::Instant::now();
    let mut frame_count = 0u64;
    let mut last_log_time = start_time;

    loop {
        interval.tick().await;

        // Counter increments every second
        let counter = start_time.elapsed().as_secs();

        // Generate test frame
        let frame = generate_counter_frame(width, height, counter, color_mode);

        if convert_tx.send(frame).await.is_err() {
            info!("Batch channel closed");
            break;
        }

        frame_count += 1;

        // Log every 10 frames or every 2 seconds
        if frame_count % 10 == 0 || last_log_time.elapsed().as_secs() >= 2 {
            let elapsed = start_time.elapsed().as_secs_f32();
            let generation_fps = frame_count as f32 / elapsed;
            info!("🎬 GENERATED frame {} (counter: {}) | Total: {} frames | Generation FPS: {:.1} | Elapsed: {:.1}s",
                frame_count, counter, frame_count, generation_fps, elapsed);
            last_log_time = std::time::Instant::now();
        }
    }

    batcher_handle.abort();
    blockchain_handle.abort();

    Ok(())
}
