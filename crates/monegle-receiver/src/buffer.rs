use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
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

        // Get sequence range for context
        let mut seqs: Vec<u64> = self.buffer.keys().copied().collect();
        seqs.sort();
        let seq_range = if !seqs.is_empty() {
            format!("{}-{}", seqs[0], seqs[seqs.len()-1])
        } else {
            "empty".to_string()
        };

        debug!(
            "Buffered sequence {} ({} frames), total: {} frames in {} seqs (range: {}), current playback: seq {} frame {}",
            sequence,
            self.buffer.get(&sequence).map(|f| f.len()).unwrap_or(0),
            self.frame_count,
            self.buffer.len(),
            seq_range,
            self.current_sequence,
            self.current_frame_index
        );
    }

    /// Get the next frame for playback
    pub fn next_frame(&mut self) -> Result<String> {
        // If current sequence is not in buffer, try to find the earliest available sequence
        if !self.buffer.contains_key(&self.current_sequence) {
            let min_seq = self.buffer.keys().min().copied();
            if let Some(seq) = min_seq {
                info!(
                    "Current sequence {} not in buffer, jumping to earliest sequence {}",
                    self.current_sequence, seq
                );
                self.current_sequence = seq;
                self.current_frame_index = 0;
            } else {
                return Err(anyhow!("Buffer is empty"));
            }
        }

        // Get frames for current sequence
        let frames = self.buffer.get(&self.current_sequence)
            .ok_or_else(|| anyhow!("Sequence {} not in buffer", self.current_sequence))?;

        if self.current_frame_index >= frames.len() {
            // Move to next sequence
            let old_sequence = self.current_sequence;
            self.current_sequence += 1;
            self.current_frame_index = 0;

            // Again check if the next sequence exists, if not jump to earliest
            if !self.buffer.contains_key(&self.current_sequence) {
                let min_seq = self.buffer.keys().min().copied();
                if let Some(seq) = min_seq {
                    if seq < old_sequence {
                        warn!("⚠️  SEQUENCE JUMP BACKWARDS: from {} to {} (buffer underrun or restart)", old_sequence, seq);
                    } else {
                        debug!("Sequence {} not available, jumping forward to {}", self.current_sequence, seq);
                    }
                    self.current_sequence = seq;
                } else {
                    return Err(anyhow!("No more sequences in buffer"));
                }
            }

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
    buffer: Arc<Mutex<FrameBuffer>>,
    initial_buffer_batches: usize,
}

impl BufferController {
    pub fn new(capacity: usize, initial_buffer_batches: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(FrameBuffer::new(capacity))),
            initial_buffer_batches,
        }
    }

    /// Start buffering and playback loop
    pub async fn start_buffering_loop(
        self,
        mut rx: mpsc::Receiver<(u64, Vec<String>)>,
        tx: mpsc::Sender<String>,
        target_fps: f32,
    ) -> Result<()> {
        info!("Starting buffering loop (target FPS: {}, initial buffer: {} batches)",
            target_fps, self.initial_buffer_batches);

        let buffer_clone = self.buffer.clone();

        // Spawn buffering task that continuously receives and buffers
        let buffering_handle = tokio::spawn(async move {
            let mut batch_count = 0;
            let mut last_log = std::time::Instant::now();

            while let Some((sequence, frames)) = rx.recv().await {
                batch_count += 1;
                {
                    let mut buffer = buffer_clone.lock().await;
                    buffer.add_batch(sequence, frames);
                }

                // Log every batch or every 5 seconds
                if batch_count % 5 == 0 || last_log.elapsed().as_secs() >= 5 {
                    let buffer = buffer_clone.lock().await;
                    let stats = buffer.stats();
                    info!(
                        "📥 Buffering: received batch {} (total: {}), buffer: {} seqs / {} frames",
                        sequence, batch_count, stats.sequences, stats.frames
                    );
                    last_log = std::time::Instant::now();
                }
            }
            warn!("⚠️ Buffering task stopped - no more batches received (total: {})", batch_count);
        });

        // Wait for initial buffer
        info!("Waiting for {} batches before starting playback...", self.initial_buffer_batches);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let buffer = self.buffer.lock().await;
            let stats = buffer.stats();

            if stats.sequences >= self.initial_buffer_batches {
                info!("Buffer ready! {} sequences, {} frames total", stats.sequences, stats.frames);
                break;
            }

            info!("Buffering... {}/{} batches", stats.sequences, self.initial_buffer_batches);
        }

        // Playback phase with adaptive FPS
        info!("Starting playback with {}s delay for smooth buffering", self.initial_buffer_batches * 7);

        let mut frame_count = 0u64;
        let start_time = std::time::Instant::now();
        let mut last_stats_time = start_time;

        loop {
            // Adaptive delay based on buffer depth
            let buffer_depth = {
                let buffer = self.buffer.lock().await;
                buffer.stats().frames
            };

            // Slow down if buffer is getting low, speed up if buffer is large
            let adaptive_fps = if buffer_depth < 10 {
                target_fps * 0.5  // Half speed if buffer low
            } else if buffer_depth > 50 {
                target_fps * 1.5  // 1.5x speed if buffer high
            } else {
                target_fps
            };

            let frame_interval = std::time::Duration::from_secs_f32(1.0 / adaptive_fps);
            tokio::time::sleep(frame_interval).await;

            // Get next frame
            let frame = {
                let mut buffer = self.buffer.lock().await;
                buffer.next_frame()
            };

            match frame {
                Ok(frame) => {
                    frame_count += 1;

                    // Log stats every 5 seconds with detailed buffer info
                    if last_stats_time.elapsed().as_secs() >= 5 {
                        let buffer = self.buffer.lock().await;
                        let stats = buffer.stats();
                        let elapsed = start_time.elapsed().as_secs_f32();
                        let actual_fps = frame_count as f32 / elapsed;

                        // Get sequence range in buffer
                        let mut seqs: Vec<u64> = buffer.buffer.keys().copied().collect();
                        seqs.sort();
                        let seq_range = if !seqs.is_empty() {
                            format!("{}-{}", seqs[0], seqs[seqs.len()-1])
                        } else {
                            "empty".to_string()
                        };

                        info!(
                            "▶️  Playback: {} frames ({:.1} FPS), buffer: {} seqs / {} frames (seq range: {}, current: {}, frame idx: {}), adaptive FPS: {:.1}",
                            frame_count, actual_fps, stats.sequences, stats.frames, seq_range,
                            buffer.current_sequence, buffer.current_frame_index, adaptive_fps
                        );
                        last_stats_time = std::time::Instant::now();
                    }

                    if tx.send(frame).await.is_err() {
                        warn!("Playback channel closed, stopping buffering loop");
                        break;
                    }
                }
                Err(e) => {
                    let (current_seq, buffer_seqs) = {
                        let buffer = self.buffer.lock().await;
                        let stats = buffer.stats();
                        let seqs: Vec<u64> = buffer.buffer.keys().copied().collect();
                        (stats.current_sequence, seqs)
                    };

                    warn!(
                        "⚠️ Buffer underrun: {} - Current seq: {}, Available seqs: {:?}",
                        e, current_seq, buffer_seqs
                    );
                    warn!("Waiting 2 seconds for more batches...");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }

        buffering_handle.abort();

        let elapsed = start_time.elapsed().as_secs_f32();
        let avg_fps = frame_count as f32 / elapsed;
        info!("Buffering loop stopped: {} frames in {:.1}s ({:.1} FPS average)",
            frame_count, elapsed, avg_fps);

        Ok(())
    }
}
