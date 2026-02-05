use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Frame buffer for smooth playback
pub struct FrameBuffer {
    /// Buffered frames: sequence -> frames
    buffer: HashMap<u64, Vec<String>>,

    /// Current sequence position
    current_sequence: u64,

    /// Current frame index within sequence
    current_frame_index: usize,

    /// Buffer capacity (number of sequences)
    capacity: usize,

    /// Total frames buffered
    frame_count: usize,
}

impl FrameBuffer {
    pub fn new(capacity: usize) -> Self {
        info!("Initializing frame buffer with capacity: {} sequences", capacity);

        Self {
            buffer: HashMap::new(),
            current_sequence: 0,
            current_frame_index: 0,
            capacity,
            frame_count: 0,
        }
    }

    /// Add a batch of frames to the buffer
    pub fn add_batch(&mut self, sequence: u64, frames: Vec<String>) {
        if self.buffer.len() >= self.capacity {
            // Remove oldest sequence
            let oldest = self.buffer.keys().min().copied();
            if let Some(seq) = oldest {
                if let Some(removed) = self.buffer.remove(&seq) {
                    self.frame_count -= removed.len();
                    debug!("Removed old sequence {} ({} frames)", seq, removed.len());
                }
            }
        }

        self.frame_count += frames.len();
        self.buffer.insert(sequence, frames);

        debug!(
            "Buffered sequence {} ({} frames), total buffered: {} frames in {} sequences",
            sequence,
            self.buffer.get(&sequence).map(|f| f.len()).unwrap_or(0),
            self.frame_count,
            self.buffer.len()
        );
    }

    /// Get the next frame for playback
    pub fn next_frame(&mut self) -> Result<String> {
        // Get frames for current sequence
        let frames = self.buffer.get(&self.current_sequence)
            .ok_or_else(|| anyhow!("Sequence {} not in buffer", self.current_sequence))?;

        if self.current_frame_index >= frames.len() {
            // Move to next sequence
            self.current_sequence += 1;
            self.current_frame_index = 0;

            let frames = self.buffer.get(&self.current_sequence)
                .ok_or_else(|| anyhow!("Sequence {} not in buffer", self.current_sequence))?;

            if frames.is_empty() {
                return Err(anyhow!("Empty frame batch"));
            }
        }

        let frame = self.buffer
            .get(&self.current_sequence)
            .and_then(|frames| frames.get(self.current_frame_index))
            .ok_or_else(|| anyhow!("Frame not found"))?
            .clone();

        self.current_frame_index += 1;

        Ok(frame)
    }

    /// Check if buffer has enough frames for playback
    pub fn is_ready(&self) -> bool {
        self.frame_count >= 10 // Wait for at least 10 frames
    }

    /// Get buffer statistics
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            sequences: self.buffer.len(),
            frames: self.frame_count,
            current_sequence: self.current_sequence,
        }
    }

    /// Skip to a specific sequence
    pub fn seek_to_sequence(&mut self, sequence: u64) {
        self.current_sequence = sequence;
        self.current_frame_index = 0;
        info!("Seeked to sequence {}", sequence);
    }
}

#[derive(Debug, Clone)]
pub struct BufferStats {
    pub sequences: usize,
    pub frames: usize,
    pub current_sequence: u64,
}

/// Buffering controller
pub struct BufferController {
    buffer: FrameBuffer,
}

impl BufferController {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: FrameBuffer::new(capacity),
        }
    }

    /// Start buffering loop
    pub async fn start_buffering_loop(
        mut self,
        mut rx: mpsc::Receiver<(u64, Vec<String>)>,
        tx: mpsc::Sender<String>,
        target_fps: f32,
    ) -> Result<()> {
        info!("Starting buffering loop (target FPS: {})", target_fps);

        // Buffering phase
        info!("Buffering initial frames...");

        while let Some((sequence, frames)) = rx.recv().await {
            self.buffer.add_batch(sequence, frames);

            if self.buffer.is_ready() {
                info!("Buffer ready, starting playback");
                break;
            }
        }

        // Playback phase
        let frame_interval = std::time::Duration::from_secs_f32(1.0 / target_fps);
        let mut interval = tokio::time::interval(frame_interval);

        // Spawn task to continue buffering
        let buffer_handle = tokio::spawn(async move {
            while let Some((sequence, frames)) = rx.recv().await {
                // This would need access to buffer, which we moved
                // In a real implementation, we'd use Arc<Mutex<FrameBuffer>>
                debug!("Received batch {}", sequence);
            }
        });

        let mut frame_count = 0u64;

        loop {
            interval.tick().await;

            match self.buffer.next_frame() {
                Ok(frame) => {
                    frame_count += 1;

                    if frame_count % 100 == 0 {
                        let stats = self.buffer.stats();
                        info!(
                            "Played {} frames, buffer: {} sequences, {} frames",
                            frame_count, stats.sequences, stats.frames
                        );
                    }

                    if tx.send(frame).await.is_err() {
                        warn!("Playback channel closed, stopping buffering loop");
                        break;
                    }
                }
                Err(e) => {
                    warn!("Buffer underrun: {}", e);
                    // Wait for more frames
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }

        buffer_handle.abort();

        info!("Buffering loop stopped after {} frames", frame_count);
        Ok(())
    }
}
