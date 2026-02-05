use serde::{Deserialize, Serialize};

/// Unique identifier for a stream (derived from sender address or custom ID)
pub type StreamId = [u8; 32];

/// Sequence number for ordering frame batches
pub type SequenceNumber = u64;

/// A batch of compressed frames sent in a single transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBatch {
    /// Stream identifier
    pub stream_id: StreamId,

    /// Monotonic sequence number for ordering
    pub sequence: SequenceNumber,

    /// Stream metadata (resolution, fps, character set, color mode)
    pub metadata: StreamMetadata,

    /// Compressed frames in this batch
    pub frames: Vec<CompressedFrame>,

    /// Unix timestamp (milliseconds)
    pub timestamp: u64,
}

impl FrameBatch {
    /// Encode the batch to bytes for blockchain storage
    pub fn encode_to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow::anyhow!("Failed to encode batch: {}", e))
    }

    /// Decode a batch from bytes
    pub fn decode_from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        bincode::deserialize(data).map_err(|e| anyhow::anyhow!("Failed to decode batch: {}", e))
    }

    /// Calculate total size in bytes
    pub fn size_bytes(&self) -> usize {
        self.encode_to_bytes().map(|v| v.len()).unwrap_or(0)
    }
}

/// A single compressed ASCII frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedFrame {
    /// Compression type used
    pub compression_type: CompressionType,

    /// Compressed data
    pub data: Vec<u8>,

    /// Frame number within the stream
    pub frame_number: u64,

    /// Whether this is a keyframe (full frame, not delta)
    pub is_keyframe: bool,
}

/// Compression strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    /// No compression (raw ASCII)
    None = 0,

    /// Run-length encoding (good for static areas)
    Rle = 1,

    /// Delta encoding (only changed characters)
    Delta = 2,

    /// Zlib compression
    Zlib = 3,

    /// Automatic selection based on content
    Auto = 4,
}

impl CompressionType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Rle),
            2 => Some(Self::Delta),
            3 => Some(Self::Zlib),
            4 => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Stream metadata (included in each batch for receiver to decode properly)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMetadata {
    /// Target frames per second
    pub fps: u8,

    /// Width in characters
    pub width: u16,

    /// Height in characters
    pub height: u16,

    /// Compression strategy
    pub compression_type: CompressionType,

    /// ASCII character set used
    pub character_set: CharacterSet,

    /// Color mode for display
    pub color_mode: ColorMode,

    /// Frames per batch
    pub frames_per_batch: u8,
}

/// ASCII character sets for different quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterSet {
    /// Standard ASCII: " .:-=+*#%@" (10 characters)
    Standard,

    /// Dense: " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$" (70 characters)
    Dense,

    /// Blocks: " ░▒▓█" (5 characters, smooth gradients)
    Blocks,

    /// Detailed: Enhanced quality with Unicode symbols (45 characters, recommended)
    Detailed,
}

impl CharacterSet {
    /// Get the character palette for this set
    pub fn palette(&self) -> &'static str {
        match self {
            Self::Standard => " .:-=+*#%@",
            Self::Dense => " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$",
            Self::Blocks => " ░▒▓█",
            // Carefully selected characters with good visual weight progression
            // Includes some Unicode for better shading
            Self::Detailed => " .·'`,;:∙^\"~-_+<>=*×!?/|\\()[]IiltrfjcvxnyuXYUJCLQ0OZmwqdbkhao#MW&8%B@$",
        }
    }
}

/// Color modes for terminal output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// No color (monochrome ASCII)
    None,

    /// Purple/magenta hues
    Purple,

    /// Blue hues
    Blue,

    /// Green/matrix style
    Green,

    /// Full RGB color (truecolor terminals)
    Rgb,
}

impl ColorMode {
    /// Get ANSI color code for a given brightness (0-255)
    pub fn colorize(&self, ch: char, brightness: u8) -> String {
        match self {
            Self::None => ch.to_string(),
            Self::Purple => {
                // Map brightness to purple hues (dark purple -> bright magenta)
                // Using 256-color palette codes 53-219 for purple range
                let color_code = match brightness {
                    0..=31 => 53,       // Very dark purple
                    32..=63 => 54,      // Dark purple
                    64..=95 => 55,      // Medium-dark purple
                    96..=127 => 93,     // Medium purple
                    128..=159 => 129,   // Purple
                    160..=191 => 165,   // Light purple
                    192..=223 => 177,   // Bright magenta
                    224..=255 => 219,   // Very bright magenta
                };
                format!("\x1b[38;5;{}m{}\x1b[0m", color_code, ch)
            }
            Self::Blue => {
                let color_code = match brightness {
                    0..=31 => 17,
                    32..=63 => 18,
                    64..=95 => 19,
                    96..=127 => 20,
                    128..=159 => 21,
                    160..=191 => 63,
                    192..=223 => 69,
                    224..=255 => 117,
                };
                format!("\x1b[38;5;{}m{}\x1b[0m", color_code, ch)
            }
            Self::Green => {
                let color_code = match brightness {
                    0..=31 => 22,
                    32..=63 => 28,
                    64..=95 => 34,
                    96..=127 => 40,
                    128..=159 => 46,
                    160..=191 => 82,
                    192..=223 => 118,
                    224..=255 => 154,
                };
                format!("\x1b[38;5;{}m{}\x1b[0m", color_code, ch)
            }
            Self::Rgb => {
                // Truecolor: use brightness for all RGB components equally (grayscale)
                // Or could map to actual colors from the image
                format!("\x1b[38;2;{};{};{}m{}\x1b[0m", brightness, brightness, brightness, ch)
            }
        }
    }
}
