use anyhow::Result;
use image::{DynamicImage, GenericImageView, imageops};
use monegle_core::{CharacterSet, ColorMode, brightness_to_ascii, brightness_to_ascii_colored, aspect_ratio_correction};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// ASCII conversion component
pub struct AsciiConverter {
    target_width: u32,
    target_height: u32,
    charset: CharacterSet,
    color_mode: ColorMode,
}

impl AsciiConverter {
    pub fn new(target_width: u32, target_height: u32, charset: CharacterSet, color_mode: ColorMode) -> Self {
        info!(
            "Initializing ASCII converter: {}x{} chars, charset: {:?}, color: {:?}",
            target_width, target_height, charset, color_mode
        );

        Self {
            target_width,
            target_height,
            charset,
            color_mode,
        }
    }

    /// Convert a single image to ASCII
    pub fn convert(&self, image: &DynamicImage) -> String {
        // Resize to target dimensions, accounting for aspect ratio
        let corrected_height = (self.target_height as f32 * aspect_ratio_correction()) as u32;

        let resized = imageops::resize(
            image,
            self.target_width,
            corrected_height,
            imageops::FilterType::Lanczos3,
        );

        // For RGB mode, keep colors; otherwise convert to grayscale
        let (source_image, use_rgb) = if self.color_mode == ColorMode::Rgb {
            (DynamicImage::ImageRgba8(resized), true)
        } else {
            (DynamicImage::ImageRgba8(resized).grayscale(), false)
        };

        // Convert to ASCII (with or without colors)
        let capacity = match self.color_mode {
            ColorMode::None => (self.target_width * self.target_height) as usize,
            _ => (self.target_width * self.target_height * 20) as usize, // Extra space for ANSI codes
        };
        let mut result = String::with_capacity(capacity);

        for y in 0..self.target_height {
            // Map back to the resized image coordinates
            let img_y = (y as f32 * corrected_height as f32 / self.target_height as f32) as u32;

            for x in 0..self.target_width {
                let pixel = source_image.get_pixel(x, img_y);

                if use_rgb {
                    // Use actual RGB colors from the pixel
                    let r = pixel[0];
                    let g = pixel[1];
                    let b = pixel[2];
                    let brightness = ((0.299 * r as f32) + (0.587 * g as f32) + (0.114 * b as f32)) as u8;
                    let ch = brightness_to_ascii(brightness, self.charset);
                    result.push_str(&format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, ch));
                } else if self.color_mode == ColorMode::None {
                    // Monochrome
                    let brightness = pixel[0]; // Grayscale, so R=G=B
                    let ch = brightness_to_ascii(brightness, self.charset);
                    result.push(ch);
                } else {
                    // Gradient color modes (Purple, Blue, Green)
                    let brightness = pixel[0];
                    let colored = brightness_to_ascii_colored(brightness, self.charset, self.color_mode);
                    result.push_str(&colored);
                }
            }

            if y < self.target_height - 1 {
                result.push('\n');
            }
        }

        debug!("Converted frame to {} characters", result.len());

        result
    }

    /// Start conversion loop
    pub async fn start_conversion_loop(
        self,
        mut rx: mpsc::Receiver<DynamicImage>,
        tx: mpsc::Sender<String>,
    ) -> Result<()> {
        info!("Starting conversion loop");

        let mut frame_count = 0u64;

        while let Some(image) = rx.recv().await {
            let ascii = self.convert(&image);
            frame_count += 1;

            if frame_count % 100 == 0 {
                info!("Converted {} frames", frame_count);
            }

            if tx.send(ascii).await.is_err() {
                info!("Conversion channel closed, stopping conversion loop");
                break;
            }
        }

        info!("Conversion loop stopped after {} frames", frame_count);
        Ok(())
    }
}
