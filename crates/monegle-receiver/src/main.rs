mod listener;
mod decoder;
mod buffer;
mod display;

use anyhow::{anyhow, Result};
use clap::Parser;
use monegle_core::{Config, FrameBatch};
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use listener::TransactionListener;
use decoder::FrameDecoder;
use display::TerminalDisplay;

#[derive(Parser, Debug)]
#[command(name = "monegle-receiver")]
#[command(about = "Monegle ASCII Video Streaming Receiver", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Sender address to monitor (transactions FROM this address)
    #[arg(short, long)]
    sender_address: Option<String>,

    /// WebSocket URL (overrides config)
    #[arg(long)]
    ws_url: Option<String>,

    /// Use HTTP polling instead of WebSocket
    #[arg(long)]
    no_websocket: bool,

    /// Disable terminal display (headless mode)
    #[arg(long)]
    no_display: bool,
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

    info!("Monegle Receiver starting...");

    // Load configuration
    let config = Config::from_file(&args.config)?;

    let receiver_config = config.receiver
        .ok_or_else(|| anyhow!("Receiver configuration not found"))?;

    let network_config = config.network;

    // Get sender address
    let sender_address = args.sender_address
        .or(receiver_config.sender_address)
        .ok_or_else(|| anyhow!("Sender address not specified (use --sender-address)"))?;

    info!("Monitoring transactions FROM: {}", sender_address);

    // Initialize transaction listener
    let listener = TransactionListener::new(&sender_address)?;

    // Create channels for pipeline
    let (batch_tx, mut batch_rx) = mpsc::channel::<FrameBatch>(10);
    let (frame_tx, mut frame_rx) = mpsc::channel::<String>(100);

    // Determine connection method
    let use_websocket = !args.no_websocket && receiver_config.use_websocket;

    if use_websocket {
        let ws_url = args.ws_url
            .or(Some(network_config.ws_url.clone()))
            .ok_or_else(|| anyhow!("WebSocket URL not specified"))?;

        info!("Using WebSocket subscription: {}", ws_url);

        // Spawn WebSocket listener
        let batch_tx_clone = batch_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = listener.start_websocket_loop(&ws_url, batch_tx_clone).await {
                error!("WebSocket listener error: {}", e);
            }
        });
    } else {
        let rpc_url = network_config.rpc_url.clone();
        let poll_interval = receiver_config.polling_interval;

        info!("Using HTTP polling: {} (interval: {}ms)", rpc_url, poll_interval);

        // Spawn HTTP polling listener
        let batch_tx_clone = batch_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = listener.start_polling_loop(&rpc_url, batch_tx_clone, poll_interval).await {
                error!("Polling listener error: {}", e);
            }
        });
    }

    // Spawn decoder task
    let mut last_metadata: Option<monegle_core::StreamMetadata> = None;
    tokio::spawn(async move {
        let mut decoder = FrameDecoder::new();

        while let Some(batch) = batch_rx.recv().await {
            // Log metadata changes
            if last_metadata.as_ref() != Some(&batch.metadata) {
                info!(
                    "Stream metadata: {}x{} @ {} FPS, charset: {:?}, color: {:?}, compression: {:?}",
                    batch.metadata.width,
                    batch.metadata.height,
                    batch.metadata.fps,
                    batch.metadata.character_set,
                    batch.metadata.color_mode,
                    batch.metadata.compression_type
                );
                last_metadata = Some(batch.metadata.clone());
            }

            match decoder.decode_batch(&batch) {
                Ok(frames) => {
                    for frame in frames {
                        if frame_tx.send(frame).await.is_err() {
                            info!("Frame channel closed, stopping decoder");
                            return;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to decode batch {}: {}", batch.sequence, e);
                }
            }
        }

        info!("Decoder stopped");
    });

    // Display frames
    if !args.no_display && receiver_config.display_terminal {
        info!("Starting terminal display");
        info!("Waiting for stream metadata from first batch...");

        // Use sensible defaults for display parameters
        // (These will be overridden by actual metadata from the stream)
        let display = TerminalDisplay::new(
            15,  // Default FPS (will be updated from metadata)
            80,  // Default width (will be updated from metadata)
            60,  // Default height (will be updated from metadata)
            sender_address.clone(),  // Stream ID
        );

        if let Err(e) = display.start_display_loop(frame_rx).await {
            error!("Display error: {}", e);
        }
    } else {
        info!("Headless mode - receiving frames without display");

        while let Some(_frame) = frame_rx.recv().await {
            // Just receive and discard (for testing)
        }
    }

    info!("Monegle Receiver stopped");

    Ok(())
}
