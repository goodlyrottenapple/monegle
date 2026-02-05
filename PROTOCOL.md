# Monegle Protocol Specification

## Overview

Monegle uses raw blockchain transactions on Monad to stream ASCII video. Each transaction contains a `FrameBatch` with complete metadata about the stream configuration.

## Data Structures

### FrameBatch

Each transaction sent to the blockchain contains a `FrameBatch` with the following structure:

```rust
pub struct FrameBatch {
    pub stream_id: StreamId,           // 32-byte unique identifier
    pub sequence: SequenceNumber,      // u64 monotonic counter
    pub metadata: StreamMetadata,      // Stream configuration (NEW!)
    pub frames: Vec<CompressedFrame>,  // Compressed ASCII frames
    pub timestamp: u64,                // Unix timestamp in milliseconds
}
```

**Key Update**: As of v0.1.0, `metadata` is now included in **every batch**. This ensures:
- Receivers can decode any batch independently
- Stream parameters can change mid-stream
- No separate metadata channel needed
- Complete self-describing protocol

### StreamMetadata

Complete stream configuration included in each batch:

```rust
pub struct StreamMetadata {
    pub fps: u8,                           // Target frames per second
    pub width: u16,                        // Width in ASCII characters
    pub height: u16,                       // Height in ASCII characters
    pub compression_type: CompressionType, // Compression strategy
    pub character_set: CharacterSet,       // ASCII character palette
    pub color_mode: ColorMode,             // Terminal color mode (NEW!)
    pub frames_per_batch: u8,              // Frames per transaction
}
```

### ColorMode

Terminal color rendering mode (v0.1.0+):

```rust
pub enum ColorMode {
    None,    // Monochrome ASCII
    Purple,  // Purple/magenta gradient
    Blue,    // Blue gradient
    Green,   // Green/matrix style
    Rgb,     // True RGB colors from video (RECOMMENDED!)
}
```

**RGB Mode**: Each ASCII character is colored with the actual RGB values from the corresponding pixel in the video. This creates photorealistic colored ASCII art where the sky is actually blue, grass is green, etc.

### CharacterSet

```rust
pub enum CharacterSet {
    Standard,  // 10 chars: " .:-=+*#%@"
    Dense,     // 70 chars: high detail
    Blocks,    // 5 chars: " ░▒▓█"
    Detailed,  // 45 chars: balanced quality (RECOMMENDED!)
}
```

### CompressionType

```rust
pub enum CompressionType {
    None,   // Raw ASCII
    Rle,    // Run-length encoding
    Delta,  // Delta encoding (only changes)
    Zlib,   // Zlib compression
    Auto,   // Automatic selection
}
```

### CompressedFrame

```rust
pub struct CompressedFrame {
    pub compression_type: CompressionType, // Compression used
    pub data: Vec<u8>,                     // Compressed data
    pub frame_number: u64,                 // Global frame number
    pub is_keyframe: bool,                 // Full frame or delta
}
```

## Protocol Flow

### Sender

1. **Capture** video from camera at native resolution (640x480 or 1920x1080)
2. **Convert** to ASCII:
   - Resize to target resolution (e.g., 220x120 chars)
   - For RGB mode: preserve colors from each pixel
   - For gradient modes: convert to grayscale, apply color gradient
   - For monochrome: convert to grayscale only
   - Map brightness to ASCII characters
3. **Batch** frames (typically 6 frames per batch @ 15 FPS)
4. **Compress** using delta/RLE/zlib
5. **Create FrameBatch** with complete metadata
6. **Encode** to binary using bincode
7. **Submit** as raw transaction calldata to Monad

### Receiver

1. **Subscribe** to blockchain via WebSocket (`eth_subscribe("newHeads")`)
2. **Filter** transactions from sender address
3. **Extract** calldata and decode `FrameBatch`
4. **Read metadata** from batch:
   - Resolution (width × height)
   - FPS for playback timing
   - Character set for display
   - **Color mode for terminal rendering**
   - Compression type for decoding
5. **Decompress** frames using metadata
6. **Display** in terminal with correct colors:
   - RGB mode: Use ANSI truecolor codes `\x1b[38;2;R;G;Bm`
   - Gradient modes: Use ANSI 256-color codes
   - Monochrome: Plain ASCII

## Color Rendering

### RGB Mode (Truecolor)

For each ASCII character:
1. Get original pixel RGB values (r, g, b)
2. Calculate brightness: `(0.299*r + 0.587*g + 0.114*b)`
3. Select ASCII character based on brightness
4. Render with actual pixel color: `\x1b[38;2;{r};{g};{b}m{char}\x1b[0m`

Result: Photorealistic colored ASCII where colors match the original video.

### Gradient Modes (Purple/Blue/Green)

For each ASCII character:
1. Convert pixel to grayscale
2. Calculate brightness
3. Select ASCII character based on brightness
4. Map brightness to color gradient (8 levels)
5. Render with 256-color code: `\x1b[38;5;{code}m{char}\x1b[0m`

### Monochrome

1. Convert to grayscale
2. Select ASCII character based on brightness
3. Render plain character (no color codes)

## Size Estimates

### Per Batch (6 frames @ 220×120, RGB mode)

```
StreamMetadata:     ~50 bytes
  - fps:             1 byte
  - width:           2 bytes
  - height:          2 bytes
  - compression:     1 byte
  - character_set:   1 byte
  - color_mode:      1 byte (NEW!)
  - frames_per_batch: 1 byte
  - (padding)        ~40 bytes

Frames:             ~25-30KB (with compression)
Total per batch:    ~30KB
```

**Note**: Including metadata in each batch adds ~50 bytes overhead but provides critical benefits for protocol robustness and receiver compatibility.

## Latency

Based on real testing on Monad testnet:

- Block time: ~400ms
- Transaction confirmation: ~7.2 seconds average
- End-to-end latency: **7-8 seconds** (video capture → blockchain → display)

This makes Monegle suitable for:
- ✅ One-way video streaming
- ✅ Demonstrations and recordings
- ✅ Artistic/experimental video broadcasts
- ❌ Real-time video calls (too much latency)
- ❌ Interactive applications

## Gas Costs

Based on testing (with compression enabled):

- Gas per batch: ~570,000 gas (with delta encoding)
- Transactions per hour @ 15 FPS: 9,000 tx/hour
- At testnet gas prices: Variable (depends on network)

**Optimization**: The protocol uses delta encoding and RLE compression to minimize data size and gas costs.

## Compatibility

### Sender Requirements
- Rust 1.70+
- Camera access (or synthetic mode)
- Monad testnet MON tokens

### Receiver Requirements
- Rust 1.70+
- Modern terminal with truecolor support for RGB mode:
  - ✅ iTerm2 (macOS)
  - ✅ Alacritty
  - ✅ Kitty
  - ✅ VS Code terminal
  - ✅ Windows Terminal
  - ⚠️ Terminal.app (limited color support)

## Future Enhancements

- Adaptive compression based on content
- Multiple concurrent streams
- Stream discovery mechanism
- Audio support (encoded as visual waveforms?)
- Interactive controls via on-chain transactions
