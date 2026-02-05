use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Efficient RGB frame encoding: stores characters and colors separately
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficientRgbFrame {
    /// Width × Height ASCII characters (no ANSI codes)
    pub chars: String,

    /// RGB colors as binary (3 bytes per character: R, G, B)
    /// Length must be chars.len() * 3
    pub colors: Vec<u8>,

    /// Width of frame
    pub width: u16,

    /// Height of frame
    pub height: u16,
}

impl EfficientRgbFrame {
    /// Parse ANSI-encoded frame into efficient format
    pub fn from_ansi_frame(ansi_text: &str, width: u16, height: u16) -> Result<Self> {
        let mut chars = String::with_capacity((width * height) as usize);
        let mut colors = Vec::with_capacity((width * height * 3) as usize);

        let mut iter = ansi_text.chars().peekable();

        while let Some(ch) = iter.next() {
            if ch == '\x1b' {
                // Parse ANSI escape sequence
                if iter.next() == Some('[') {
                    let mut code = String::new();

                    // Read until 'm'
                    while let Some(c) = iter.next() {
                        if c == 'm' {
                            break;
                        }
                        code.push(c);
                    }

                    // Parse RGB: "38;2;R;G;B"
                    if code.starts_with("38;2;") {
                        let parts: Vec<&str> = code[5..].split(';').collect();
                        if parts.len() >= 3 {
                            let r = parts[0].parse::<u8>().unwrap_or(0);
                            let g = parts[1].parse::<u8>().unwrap_or(0);
                            let b = parts[2].parse::<u8>().unwrap_or(0);

                            // Next character is the actual character
                            if let Some(actual_ch) = iter.next() {
                                if actual_ch != '\x1b' && actual_ch != '\n' {
                                    chars.push(actual_ch);
                                    colors.push(r);
                                    colors.push(g);
                                    colors.push(b);
                                }
                            }

                            // Skip reset code "\x1b[0m"
                            if iter.peek() == Some(&'\x1b') {
                                iter.next(); // \x1b
                                iter.next(); // [
                                while let Some(c) = iter.next() {
                                    if c == 'm' { break; }
                                }
                            }
                        }
                    }
                }
            } else if ch == '\n' {
                // Skip newlines
            } else {
                // Plain character (monochrome or no color)
                chars.push(ch);
                colors.push(128); // Default gray
                colors.push(128);
                colors.push(128);
            }
        }

        Ok(Self {
            chars,
            colors,
            width,
            height,
        })
    }

    /// Convert back to ANSI-encoded frame
    pub fn to_ansi_frame(&self) -> String {
        let mut result = String::with_capacity(self.chars.len() * 25);
        let chars: Vec<char> = self.chars.chars().collect();

        for (i, ch) in chars.iter().enumerate() {
            let idx = i * 3;
            if idx + 2 < self.colors.len() {
                let r = self.colors[idx];
                let g = self.colors[idx + 1];
                let b = self.colors[idx + 2];

                result.push_str(&format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, ch));
            } else {
                result.push(*ch);
            }

            // Add newline at end of each row
            if (i + 1) % self.width as usize == 0 && i + 1 < chars.len() {
                result.push('\n');
            }
        }

        result
    }

    /// Encode to binary with color RLE compression
    pub fn encode_compressed(&self) -> Result<Vec<u8>> {
        let mut encoded = Vec::new();

        // Header: width, height
        encoded.extend_from_slice(&self.width.to_le_bytes());
        encoded.extend_from_slice(&self.height.to_le_bytes());

        // Encode characters (plain text, already small)
        let char_bytes = self.chars.as_bytes();
        encoded.extend_from_slice(&(char_bytes.len() as u32).to_le_bytes());
        encoded.extend_from_slice(char_bytes);

        // Encode colors with RLE (colors often repeat)
        let color_rle = Self::rle_encode_colors(&self.colors);
        encoded.extend_from_slice(&(color_rle.len() as u32).to_le_bytes());
        encoded.extend_from_slice(&color_rle);

        Ok(encoded)
    }

    /// Decode from binary
    pub fn decode_compressed(data: &[u8]) -> Result<Self> {
        let mut pos = 0;

        // Read header
        if data.len() < 4 {
            return Err(anyhow!("Invalid data: too short"));
        }

        let width = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let height = u16::from_le_bytes([data[pos], data[pos + 1]]);
        pos += 2;

        // Read characters
        if data.len() < pos + 4 {
            return Err(anyhow!("Invalid data: no char length"));
        }
        let char_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if data.len() < pos + char_len {
            return Err(anyhow!("Invalid data: incomplete chars"));
        }
        let chars = String::from_utf8(data[pos..pos + char_len].to_vec())?;
        pos += char_len;

        // Read colors
        if data.len() < pos + 4 {
            return Err(anyhow!("Invalid data: no color length"));
        }
        let color_len = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if data.len() < pos + color_len {
            return Err(anyhow!("Invalid data: incomplete colors"));
        }
        let colors = Self::rle_decode_colors(&data[pos..pos + color_len])?;

        Ok(Self {
            chars,
            colors,
            width,
            height,
        })
    }

    /// RLE encode color data (RGB triplets)
    fn rle_encode_colors(colors: &[u8]) -> Vec<u8> {
        if colors.len() < 3 {
            return colors.to_vec();
        }

        let mut encoded = Vec::new();
        let mut i = 0;

        while i + 2 < colors.len() {
            let r = colors[i];
            let g = colors[i + 1];
            let b = colors[i + 2];

            // Count how many consecutive pixels have same color
            let mut count = 1u16;
            while i + (count as usize * 3) + 2 < colors.len() && count < 255 {
                let next_i = i + (count as usize * 3);
                if colors[next_i] == r && colors[next_i + 1] == g && colors[next_i + 2] == b {
                    count += 1;
                } else {
                    break;
                }
            }

            // Encode: count (1 byte), R, G, B
            encoded.push(count as u8);
            encoded.push(r);
            encoded.push(g);
            encoded.push(b);

            i += count as usize * 3;
        }

        encoded
    }

    /// RLE decode color data
    fn rle_decode_colors(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoded = Vec::new();
        let mut i = 0;

        while i + 3 < data.len() {
            let count = data[i] as usize;
            let r = data[i + 1];
            let g = data[i + 2];
            let b = data[i + 3];
            i += 4;

            for _ in 0..count {
                decoded.push(r);
                decoded.push(g);
                decoded.push(b);
            }
        }

        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rle_colors() {
        // Test repeated colors
        let colors = vec![
            255, 0, 0, // Red
            255, 0, 0, // Red
            255, 0, 0, // Red
            0, 255, 0, // Green
            0, 255, 0, // Green
        ];

        let encoded = EfficientRgbFrame::rle_encode_colors(&colors);
        let decoded = EfficientRgbFrame::rle_decode_colors(&encoded).unwrap();

        assert_eq!(colors, decoded);
        assert!(encoded.len() < colors.len()); // Should compress
    }
}
