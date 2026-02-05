use anyhow::{anyhow, Result};
use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use std::io::Write;

use crate::{CompressedFrame, CompressionType};

/// Trait for frame encoding strategies
pub trait FrameEncoder: Send + Sync {
    /// Encode a frame, optionally using the previous frame for delta encoding
    fn encode(&self, current: &str, previous: Option<&str>) -> Result<Vec<u8>>;

    /// Decode a frame, optionally using the previous frame for delta decoding
    fn decode(&self, data: &[u8], previous: Option<&str>) -> Result<String>;

    /// Get the compression type
    fn compression_type(&self) -> CompressionType;
}

/// No compression - store raw ASCII
pub struct NoneEncoder;

impl FrameEncoder for NoneEncoder {
    fn encode(&self, current: &str, _previous: Option<&str>) -> Result<Vec<u8>> {
        Ok(current.as_bytes().to_vec())
    }

    fn decode(&self, data: &[u8], _previous: Option<&str>) -> Result<String> {
        String::from_utf8(data.to_vec()).map_err(|e| anyhow!("UTF-8 decode error: {}", e))
    }

    fn compression_type(&self) -> CompressionType {
        CompressionType::None
    }
}

/// Run-length encoding: compress repeated characters
pub struct RleEncoder;

impl FrameEncoder for RleEncoder {
    fn encode(&self, current: &str, _previous: Option<&str>) -> Result<Vec<u8>> {
        let chars: Vec<char> = current.chars().collect();
        if chars.is_empty() {
            return Ok(Vec::new());
        }

        let mut encoded = Vec::new();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];
            let mut count = 1u16;

            // Count consecutive occurrences (max 65535)
            while i + (count as usize) < chars.len() && chars[i + (count as usize)] == ch && count < u16::MAX {
                count += 1;
            }

            // Encode as: [count_high_byte, count_low_byte, char_bytes...]
            encoded.extend_from_slice(&count.to_le_bytes());

            let mut char_buf = [0u8; 4];
            let char_bytes = ch.encode_utf8(&mut char_buf);
            encoded.push(char_bytes.len() as u8);
            encoded.extend_from_slice(char_bytes.as_bytes());

            i += count as usize;
        }

        Ok(encoded)
    }

    fn decode(&self, data: &[u8], _previous: Option<&str>) -> Result<String> {
        let mut result = String::new();
        let mut i = 0;

        while i + 3 < data.len() {
            // Read count
            let count = u16::from_le_bytes([data[i], data[i + 1]]) as usize;
            i += 2;

            // Read character length
            let char_len = data[i] as usize;
            i += 1;

            if i + char_len > data.len() {
                return Err(anyhow!("RLE decode error: incomplete character data"));
            }

            // Read character
            let ch = std::str::from_utf8(&data[i..i + char_len])
                .map_err(|e| anyhow!("RLE UTF-8 error: {}", e))?
                .chars()
                .next()
                .ok_or_else(|| anyhow!("RLE decode error: empty character"))?;

            i += char_len;

            // Repeat character
            for _ in 0..count {
                result.push(ch);
            }
        }

        Ok(result)
    }

    fn compression_type(&self) -> CompressionType {
        CompressionType::Rle
    }
}

/// Delta encoding: only store changed characters
pub struct DeltaEncoder;

impl FrameEncoder for DeltaEncoder {
    fn encode(&self, current: &str, previous: Option<&str>) -> Result<Vec<u8>> {
        let prev = previous.unwrap_or("");
        let curr_chars: Vec<char> = current.chars().collect();
        let prev_chars: Vec<char> = prev.chars().collect();

        let mut encoded = Vec::new();

        // Store the length of the current frame
        let len = curr_chars.len() as u32;
        encoded.extend_from_slice(&len.to_le_bytes());

        // Find and encode differences
        let max_len = curr_chars.len().max(prev_chars.len());
        let mut changes = 0u32;
        let changes_pos = encoded.len();
        encoded.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for change count

        for i in 0..max_len {
            let curr_ch = curr_chars.get(i).copied();
            let prev_ch = prev_chars.get(i).copied();

            if curr_ch != prev_ch {
                // Encode change: [position(u32), char_len(u8), char_bytes...]
                encoded.extend_from_slice(&(i as u32).to_le_bytes());

                match curr_ch {
                    Some(ch) => {
                        let mut char_buf = [0u8; 4];
                        let char_bytes = ch.encode_utf8(&mut char_buf);
                        encoded.push(char_bytes.len() as u8);
                        encoded.extend_from_slice(char_bytes.as_bytes());
                    }
                    None => {
                        // End of string marker
                        encoded.push(0);
                    }
                }

                changes += 1;
            }
        }

        // Write actual change count
        encoded[changes_pos..changes_pos + 4].copy_from_slice(&changes.to_le_bytes());

        Ok(encoded)
    }

    fn decode(&self, data: &[u8], previous: Option<&str>) -> Result<String> {
        if data.len() < 8 {
            return Err(anyhow!("Delta decode error: data too short"));
        }

        let mut i = 0;

        // Read length
        let len = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        i += 4;

        // Read change count
        let changes = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        i += 4;

        // Start with previous frame or empty
        let mut result: Vec<char> = previous.unwrap_or("").chars().collect();
        result.resize(len, ' ');

        // Apply changes
        for _ in 0..changes {
            if i + 5 > data.len() {
                return Err(anyhow!("Delta decode error: incomplete change data"));
            }

            // Read position
            let pos = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
            i += 4;

            // Read character
            let char_len = data[i] as usize;
            i += 1;

            if char_len == 0 {
                // End of string marker - shouldn't happen in valid data
                continue;
            }

            if i + char_len > data.len() {
                return Err(anyhow!("Delta decode error: incomplete character data"));
            }

            let ch = std::str::from_utf8(&data[i..i + char_len])
                .map_err(|e| anyhow!("Delta UTF-8 error: {}", e))?
                .chars()
                .next()
                .ok_or_else(|| anyhow!("Delta decode error: empty character"))?;

            i += char_len;

            if pos < result.len() {
                result[pos] = ch;
            }
        }

        Ok(result.into_iter().collect())
    }

    fn compression_type(&self) -> CompressionType {
        CompressionType::Delta
    }
}

/// Zlib compression
pub struct ZlibCodec;

impl FrameEncoder for ZlibCodec {
    fn encode(&self, current: &str, _previous: Option<&str>) -> Result<Vec<u8>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(current.as_bytes())?;
        encoder.finish().map_err(|e| anyhow!("Zlib encode error: {}", e))
    }

    fn decode(&self, data: &[u8], _previous: Option<&str>) -> Result<String> {
        let mut decoder = ZlibDecoder::new(Vec::new());
        decoder.write_all(data)?;
        let decoded = decoder.finish().map_err(|e| anyhow!("Zlib decode error: {}", e))?;
        String::from_utf8(decoded).map_err(|e| anyhow!("UTF-8 decode error: {}", e))
    }

    fn compression_type(&self) -> CompressionType {
        CompressionType::Zlib
    }
}

/// Hybrid encoder: automatically selects best compression
pub struct HybridEncoder {
    none: NoneEncoder,
    rle: RleEncoder,
    delta: DeltaEncoder,
    zlib: ZlibCodec,
}

impl HybridEncoder {
    pub fn new() -> Self {
        Self {
            none: NoneEncoder,
            rle: RleEncoder,
            delta: DeltaEncoder,
            zlib: ZlibCodec,
        }
    }

    /// Encode with the best compression for this frame
    pub fn encode_best(&self, current: &str, previous: Option<&str>) -> Result<CompressedFrame> {
        let _original_size = current.len();

        // Try each compression method
        let none_result = self.none.encode(current, previous)?;
        let rle_result = self.rle.encode(current, previous)?;
        let delta_result = if previous.is_some() {
            self.delta.encode(current, previous)?
        } else {
            vec![]
        };
        let zlib_result = self.zlib.encode(current, previous)?;

        // Select the smallest
        let mut best = (CompressionType::None, none_result);

        if rle_result.len() < best.1.len() {
            best = (CompressionType::Rle, rle_result);
        }

        if !delta_result.is_empty() && delta_result.len() < best.1.len() {
            best = (CompressionType::Delta, delta_result);
        }

        if zlib_result.len() < best.1.len() {
            best = (CompressionType::Zlib, zlib_result);
        }

        Ok(CompressedFrame {
            compression_type: best.0,
            data: best.1,
            frame_number: 0, // Set by caller
            is_keyframe: previous.is_none(),
        })
    }
}

impl Default for HybridEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameEncoder for HybridEncoder {
    fn encode(&self, current: &str, previous: Option<&str>) -> Result<Vec<u8>> {
        let frame = self.encode_best(current, previous)?;
        Ok(frame.data)
    }

    fn decode(&self, data: &[u8], previous: Option<&str>) -> Result<String> {
        // For hybrid decoder, we need to know the compression type
        // This is typically stored in the CompressedFrame metadata
        // Default to zlib for compatibility
        self.zlib.decode(data, previous)
    }

    fn compression_type(&self) -> CompressionType {
        CompressionType::Auto
    }
}

/// Get an encoder for the specified compression type
pub fn get_encoder(compression_type: CompressionType) -> Box<dyn FrameEncoder> {
    match compression_type {
        CompressionType::None => Box::new(NoneEncoder),
        CompressionType::Rle => Box::new(RleEncoder),
        CompressionType::Delta => Box::new(DeltaEncoder),
        CompressionType::Zlib => Box::new(ZlibCodec),
        CompressionType::Auto => Box::new(HybridEncoder::new()),
    }
}

/// Decode a compressed frame using its embedded compression type
pub fn decode_frame(frame: &CompressedFrame, previous: Option<&str>) -> Result<String> {
    let encoder = get_encoder(frame.compression_type);
    encoder.decode(&frame.data, previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_encoding() {
        let encoder = RleEncoder;
        let input = "aaabbbbcccc    dddd";
        let encoded = encoder.encode(input, None).unwrap();
        let decoded = encoder.decode(&encoded, None).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_delta_encoding() {
        let encoder = DeltaEncoder;
        let frame1 = "Hello World!";
        let frame2 = "Hello Monad!";

        let encoded = encoder.encode(frame2, Some(frame1)).unwrap();
        let decoded = encoder.decode(&encoded, Some(frame1)).unwrap();
        assert_eq!(frame2, decoded);
    }

    #[test]
    fn test_zlib_encoding() {
        let encoder = ZlibCodec;
        let input = "The quick brown fox jumps over the lazy dog";
        let encoded = encoder.encode(input, None).unwrap();
        let decoded = encoder.decode(&encoded, None).unwrap();
        assert_eq!(input, decoded);
    }
}
