use crate::{CharacterSet, ColorMode, CompressedFrame, FrameBatch, CompressionType, StreamId, StreamMetadata};
use rand::Rng;

/// Synthetic frame generator for testing (no camera needed)
pub struct SyntheticFrameGenerator {
    width: u16,
    height: u16,
    frame_count: u64,
}

impl SyntheticFrameGenerator {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            frame_count: 0,
        }
    }

    /// Generate a random ASCII frame (for testing compression)
    pub fn generate_frame(&mut self) -> String {
        let size = (self.width * self.height) as usize;
        let charset = " .:-=+*#%@";

        let mut rng = rand::thread_rng();
        let data: String = (0..size)
            .map(|_| {
                charset
                    .chars()
                    .nth(rng.gen_range(0..charset.len()))
                    .unwrap()
            })
            .collect();

        self.frame_count += 1;

        data
    }

    /// Generate a batch of frames
    pub fn generate_batch(
        &mut self,
        count: usize,
        sequence: u64,
        stream_id: StreamId,
        compression_type: CompressionType,
    ) -> FrameBatch {
        let frames: Vec<CompressedFrame> = (0..count)
            .map(|i| {
                let ascii = self.generate_frame();
                let data = ascii.as_bytes().to_vec(); // Simple encoding for test

                CompressedFrame {
                    compression_type,
                    data,
                    frame_number: self.frame_count - 1,
                    is_keyframe: i == 0,
                }
            })
            .collect();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let metadata = StreamMetadata {
            fps: 15,
            width: self.width,
            height: self.height,
            compression_type,
            character_set: CharacterSet::Standard,
            color_mode: ColorMode::None,
            frames_per_batch: count as u8,
        };

        FrameBatch {
            stream_id,
            sequence,
            metadata,
            frames,
            timestamp,
        }
    }

    /// Generate a batch with mostly static content (high compression ratio)
    pub fn generate_static_batch(
        &mut self,
        count: usize,
        sequence: u64,
        stream_id: StreamId,
        compression_type: CompressionType,
    ) -> FrameBatch {
        let base_frame = self.generate_frame();

        let frames: Vec<CompressedFrame> = (0..count)
            .map(|i| CompressedFrame {
                compression_type,
                data: base_frame.as_bytes().to_vec(),
                frame_number: self.frame_count + i as u64,
                is_keyframe: i == 0,
            })
            .collect();

        self.frame_count += count as u64;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let metadata = StreamMetadata {
            fps: 15,
            width: self.width,
            height: self.height,
            compression_type,
            character_set: CharacterSet::Standard,
            color_mode: ColorMode::None,
            frames_per_batch: count as u8,
        };

        FrameBatch {
            stream_id,
            sequence,
            metadata,
            frames,
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_generation() {
        let mut gen = SyntheticFrameGenerator::new(80, 60);
        let frame = gen.generate_frame();
        assert_eq!(frame.len(), 4800);
    }

    #[test]
    fn test_batch_generation() {
        let mut gen = SyntheticFrameGenerator::new(80, 60);
        let stream_id = [0u8; 32];
        let batch = gen.generate_batch(6, 0, stream_id, CompressionType::None);
        assert_eq!(batch.frames.len(), 6);
        assert_eq!(batch.sequence, 0);
    }

    #[test]
    fn test_static_batch() {
        let mut gen = SyntheticFrameGenerator::new(80, 60);
        let stream_id = [0u8; 32];
        let batch = gen.generate_static_batch(6, 0, stream_id, CompressionType::None);

        // All frames should have identical data
        let first_data = &batch.frames[0].data;
        assert!(batch.frames.iter().all(|f| &f.data == first_data));
    }
}
