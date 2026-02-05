use crate::{CharacterSet, ColorMode};

/// Convert a grayscale brightness value (0-255) to an ASCII character
pub fn brightness_to_ascii(brightness: u8, charset: CharacterSet) -> char {
    let palette = charset.palette();
    let index = (brightness as usize * (palette.len() - 1)) / 255;
    palette.chars().nth(index).unwrap_or(' ')
}

/// Convert brightness to ASCII with optional colorization
pub fn brightness_to_ascii_colored(brightness: u8, charset: CharacterSet, color_mode: ColorMode) -> String {
    let ch = brightness_to_ascii(brightness, charset);
    color_mode.colorize(ch, brightness)
}

/// Calculate brightness from RGB values
pub fn rgb_to_brightness(r: u8, g: u8, b: u8) -> u8 {
    // Standard luminance formula
    ((0.299 * r as f32) + (0.587 * g as f32) + (0.114 * b as f32)) as u8
}

/// Convert an image buffer to ASCII art
pub fn image_to_ascii(
    pixels: &[u8],
    width: u32,
    height: u32,
    charset: CharacterSet,
) -> String {
    let mut result = String::with_capacity((width * height) as usize);

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize; // RGBA format

            if idx + 2 < pixels.len() {
                let r = pixels[idx];
                let g = pixels[idx + 1];
                let b = pixels[idx + 2];

                let brightness = rgb_to_brightness(r, g, b);
                let ch = brightness_to_ascii(brightness, charset);
                result.push(ch);
            } else {
                result.push(' ');
            }
        }

        if y < height - 1 {
            result.push('\n');
        }
    }

    result
}

/// Convert an image buffer to colored ASCII art
pub fn image_to_ascii_colored(
    pixels: &[u8],
    width: u32,
    height: u32,
    charset: CharacterSet,
    color_mode: ColorMode,
) -> String {
    let mut result = String::with_capacity((width * height * 20) as usize); // More space for ANSI codes

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize; // RGBA format

            if idx + 2 < pixels.len() {
                let r = pixels[idx];
                let g = pixels[idx + 1];
                let b = pixels[idx + 2];

                let brightness = rgb_to_brightness(r, g, b);

                // For RGB mode, use actual pixel colors; for others, use brightness-based gradients
                let colored_char = if color_mode == ColorMode::Rgb {
                    let ch = brightness_to_ascii(brightness, charset);
                    // Use actual RGB color from pixel
                    format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, ch)
                } else {
                    brightness_to_ascii_colored(brightness, charset, color_mode)
                };

                result.push_str(&colored_char);
            } else {
                result.push(' ');
            }
        }

        if y < height - 1 {
            result.push('\n');
        }
    }

    result
}

/// Calculate the aspect ratio correction for ASCII characters
/// (most terminal fonts are taller than they are wide)
pub fn aspect_ratio_correction() -> f32 {
    2.0 // Typical terminal character is roughly 2:1 height:width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brightness_conversion() {
        assert_eq!(brightness_to_ascii(0, CharacterSet::Standard), ' ');
        assert_eq!(brightness_to_ascii(255, CharacterSet::Standard), '@');
    }

    #[test]
    fn test_rgb_to_brightness() {
        assert_eq!(rgb_to_brightness(0, 0, 0), 0);
        assert_eq!(rgb_to_brightness(255, 255, 255), 255);
    }
}
