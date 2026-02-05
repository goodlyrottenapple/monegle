use anyhow::Result;
use monegle_core::{decode_frame, CompressedFrame, FrameBatch, StreamMetadata};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Frame decoder component
pub struct FrameDecoder {
    previous_frame: Option<String>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            previous_frame: None,
        }
    }

    /// Decode a single compressed frame
    pub fn decode_frame(&mut self, frame: &CompressedFrame) -> Result<String> {
        let decoded = decode_frame(
            frame,
            if frame.is_keyframe {
                None
            } else {
                self.previous_frame.as_deref()
            },
        )?;

        // Store for next delta decode
        self.previous_frame = Some(decoded.clone());

        debug!(
            "Decoded frame {}: {} chars (keyframe: {})",
            frame.frame_number,
            decoded.len(),
            frame.is_keyframe
        );

        Ok(decoded)
    }

    /// Decode all frames in a batch
    pub fn decode_batch(&mut self, batch: &FrameBatch) -> Result<Vec<String>> {
        let mut decoded_frames = Vec::with_capacity(batch.frames.len());

        for frame in &batch.frames {
            match self.decode_frame(frame) {
                Ok(ascii) => decoded_frames.push(ascii),
                Err(e) => {
                    warn!(
                        "Failed to decode frame {} in batch {}: {}",
                        frame.frame_number, batch.sequence, e
                    );
                    // Continue with other frames
                }
            }
        }

        Ok(decoded_frames)
    }

    /// Start decoding loop
    pub async fn start_decoding_loop(
        mut self,
        mut rx: mpsc::Receiver<FrameBatch>,
        tx: mpsc::Sender<(StreamMetadata, u64, Vec<String>)>,
    ) -> Result<()> {
        info!("Starting decoding loop");

        let mut decoded_count = 0u64;
        let mut last_metadata: Option<StreamMetadata> = None;

        while let Some(batch) = rx.recv().await {
            // Log metadata changes
            if last_metadata.as_ref() != Some(&batch.metadata) {
                info!(
                    "Stream metadata: {}x{} @ {} FPS, charset: {:?}, color: {:?}",
                    batch.metadata.width,
                    batch.metadata.height,
                    batch.metadata.fps,
                    batch.metadata.character_set,
                    batch.metadata.color_mode
                );
                last_metadata = Some(batch.metadata.clone());
            }

            match self.decode_batch(&batch) {
                Ok(frames) => {
                    decoded_count += frames.len() as u64;

                    if decoded_count % 100 == 0 {
                        info!("Decoded {} frames", decoded_count);
                    }

                    // Send metadata, batch sequence, and decoded frames
                    if tx.send((batch.metadata.clone(), batch.sequence, frames)).await.is_err() {
                        warn!("Decoding channel closed, stopping decoding loop");
                        break;
                    }
                }
                Err(e) => {
                    warn!("Failed to decode batch {}: {}", batch.sequence, e);
                }
            }
        }

        info!("Decoding loop stopped after {} frames", decoded_count);
        Ok(())
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}
