use anyhow::{anyhow, Result};
use monegle_core::{CompressedFrame, FrameBatch, StreamMetadata, StreamId, get_encoder, CompressionType};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Frame batching component
pub struct FrameBatcher {
    stream_id: StreamId,
    metadata: StreamMetadata,
    frames_per_batch: usize,
    max_batch_size: usize,
    keyframe_interval: u64,
    compression_type: CompressionType,
    current_batch: Vec<CompressedFrame>,
    sequence_counter: u64,
    frame_counter: u64,
    previous_frame: Option<String>,
}

impl FrameBatcher {
    pub fn new(
        stream_id: StreamId,
        metadata: StreamMetadata,
        max_batch_size: usize,
        keyframe_interval: u64,
    ) -> Self {
        info!(
            "Initializing batcher: {} frames/batch, {}KB max, keyframe every {} frames",
            metadata.frames_per_batch,
            max_batch_size / 1024,
            keyframe_interval
        );
        info!(
            "Stream metadata: {}x{} @ {} FPS, charset: {:?}, color: {:?}",
            metadata.width, metadata.height, metadata.fps,
            metadata.character_set, metadata.color_mode
        );

        let frames_per_batch = metadata.frames_per_batch as usize;
        let compression_type = metadata.compression_type;

        Self {
            stream_id,
            frames_per_batch,
            compression_type,
            metadata,
            max_batch_size,
            keyframe_interval,
            current_batch: Vec::with_capacity(frames_per_batch),
            sequence_counter: 0,
            frame_counter: 0,
            previous_frame: None,
        }
    }

    /// Add a frame to the batch
    pub fn add_frame(&mut self, ascii_frame: String) -> Result<Option<FrameBatch>> {
        let is_keyframe = self.frame_counter % self.keyframe_interval == 0;

        // Compress the frame
        let compressed = self.compress_frame(
            &ascii_frame,
            if is_keyframe { None } else { self.previous_frame.as_deref() },
            is_keyframe,
        )?;

        // Store for next delta encoding
        self.previous_frame = Some(ascii_frame);

        // Check if adding this frame would exceed the limit BEFORE adding it
        // This prevents oversized batches
        if !self.current_batch.is_empty() {
            self.current_batch.push(compressed.clone());
            let estimated_size = self.estimate_batch_size();

            if estimated_size > self.max_batch_size {
                // Remove the frame we just added and finalize without it
                self.current_batch.pop();

                debug!(
                    "Adding frame {} would exceed limit ({}KB > {}KB), finalizing batch with {} frames",
                    self.frame_counter,
                    estimated_size / 1024,
                    self.max_batch_size / 1024,
                    self.current_batch.len()
                );

                // Finalize current batch
                let batch = self.finalize_batch()?;

                // Start new batch with the frame we just compressed
                self.current_batch.push(compressed);
                self.frame_counter += 1;

                return Ok(batch);
            }

            // Frame fits, keep it and increment counter
            self.frame_counter += 1;
        } else {
            // First frame in batch, always add it
            self.current_batch.push(compressed);
            self.frame_counter += 1;
        }

        // Check if batch has reached target frame count
        if self.current_batch.len() >= self.frames_per_batch {
            debug!(
                "Batch reached target size ({} frames), finalizing",
                self.current_batch.len()
            );
            return self.finalize_batch();
        }

        Ok(None)
    }

    /// Compress a single frame
    fn compress_frame(
        &self,
        frame: &str,
        previous: Option<&str>,
        is_keyframe: bool,
    ) -> Result<CompressedFrame> {
        let encoder = get_encoder(self.compression_type);

        let data = encoder.encode(frame, previous)?;

        Ok(CompressedFrame {
            compression_type: self.compression_type,
            data,
            frame_number: self.frame_counter,
            is_keyframe,
        })
    }

    /// Estimate current batch size
    fn estimate_batch_size(&self) -> usize {
        // Create a temporary batch to measure size
        let temp_batch = FrameBatch {
            stream_id: self.stream_id,
            sequence: self.sequence_counter,
            metadata: self.metadata.clone(),
            frames: self.current_batch.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };

        temp_batch.size_bytes()
    }

    /// Finalize and return the current batch
    fn finalize_batch(&mut self) -> Result<Option<FrameBatch>> {
        if self.current_batch.is_empty() {
            return Ok(None);
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!("System time error: {}", e))?
            .as_millis() as u64;

        let batch = FrameBatch {
            stream_id: self.stream_id,
            sequence: self.sequence_counter,
            metadata: self.metadata.clone(),
            frames: self.current_batch.clone(),
            timestamp,
        };

        let size = batch.size_bytes();
        debug!(
            "Finalized batch {}: {} frames, {} bytes ({}x{}, {:?}, {:?})",
            self.sequence_counter,
            batch.frames.len(),
            size,
            self.metadata.width,
            self.metadata.height,
            self.metadata.character_set,
            self.metadata.color_mode,
        );

        self.sequence_counter += 1;
        self.current_batch.clear();

        Ok(Some(batch))
    }

    /// Force finalize the current batch (for stream end)
    pub fn flush(&mut self) -> Result<Option<FrameBatch>> {
        self.finalize_batch()
    }

    /// Start batching loop
    pub async fn start_batching_loop(
        mut self,
        mut rx: mpsc::Receiver<String>,
        tx: mpsc::Sender<FrameBatch>,
    ) -> Result<()> {
        info!("Starting batching loop");

        while let Some(ascii_frame) = rx.recv().await {
            if let Some(batch) = self.add_frame(ascii_frame)? {
                info!(
                    "Batch {} ready: {} frames, {} bytes",
                    batch.sequence,
                    batch.frames.len(),
                    batch.size_bytes()
                );

                if tx.send(batch).await.is_err() {
                    warn!("Batching channel closed, stopping batching loop");
                    break;
                }
            }
        }

        // Flush remaining frames
        if let Some(batch) = self.flush()? {
            info!(
                "Flushing final batch {}: {} frames",
                batch.sequence,
                batch.frames.len()
            );
            let _ = tx.send(batch).await;
        }

        info!("Batching loop stopped");
        Ok(())
    }
}
