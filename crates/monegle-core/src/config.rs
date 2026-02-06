use serde::{Deserialize, Serialize};
use crate::{CharacterSet, ColorMode, CompressionType};

/// Complete configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub sender: Option<SenderConfig>,
    pub receiver: Option<ReceiverConfig>,
}

/// Network configuration for Monad
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    /// HTTP RPC URL (for sender)
    pub rpc_url: String,

    /// WebSocket RPC URL (for receiver)
    #[serde(default = "default_ws_url")]
    pub ws_url: String,

    /// Chain ID (10143 for Monad testnet)
    pub chain_id: u64,

    /// Contract address (optional - not needed for WebSocket approach)
    pub contract_address: Option<String>,
}

fn default_ws_url() -> String {
    "wss://testnet-rpc.monad.xyz".to_string()
}

/// Sender configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SenderConfig {
    /// Frames per second (5-30)
    pub fps: u8,

    /// Resolution [width, height] in characters
    pub resolution: [u16; 2],

    /// ASCII character set
    pub character_set: CharacterSet,

    /// Color mode for terminal output
    #[serde(default = "default_color_mode")]
    pub color_mode: ColorMode,

    /// Compression strategy
    pub compression: CompressionType,

    /// Frames per batch (calculated from FPS and block time)
    pub frames_per_batch: u8,

    /// Camera device index (0 for default camera)
    pub camera_device: u32,

    /// Maximum batch size in bytes (safety margin below 128KB)
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,

    /// Keyframe interval (full frame every N frames)
    #[serde(default = "default_keyframe_interval")]
    pub keyframe_interval: u64,

    /// Target address for transactions
    #[serde(default = "default_target_address")]
    pub target_address: String,
}

/// Receiver configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReceiverConfig {
    /// Sender address to monitor (transactions FROM this address)
    pub sender_address: Option<String>,

    /// Number of blocks to buffer ahead
    #[serde(default = "default_buffer_blocks")]
    pub buffer_blocks: usize,

    /// Display in terminal
    #[serde(default = "default_true")]
    pub display_terminal: bool,

    /// Use WebSocket for events (fallback to polling if false)
    #[serde(default = "default_true")]
    pub use_websocket: bool,

    /// Polling interval in milliseconds (if not using WebSocket)
    #[serde(default = "default_polling_interval")]
    pub polling_interval: u64,
}

fn default_max_batch_size() -> usize {
    120_000 // 120KB
}

fn default_keyframe_interval() -> u64 {
    30 // Every 30 frames
}

fn default_target_address() -> String {
    "0x0000000000000000000000000000000000000001".to_string()
}

fn default_color_mode() -> ColorMode {
    ColorMode::None // Default to monochrome
}

fn default_buffer_blocks() -> usize {
    5
}

fn default_true() -> bool {
    true
}

fn default_polling_interval() -> u64 {
    400 // 400ms (Monad block time)
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("MONEGLE"))
            .build()?;

        settings.try_deserialize().map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(sender) = &self.sender {
            if sender.fps < 1 || sender.fps > 60 {
                anyhow::bail!("FPS must be between 1 and 60");
            }

            if sender.resolution[0] < 10 || sender.resolution[1] < 10 {
                anyhow::bail!("Resolution must be at least 10x10 characters");
            }

            if sender.frames_per_batch < 1 {
                anyhow::bail!("Frames per batch must be at least 1");
            }
        }

        Ok(())
    }
}
