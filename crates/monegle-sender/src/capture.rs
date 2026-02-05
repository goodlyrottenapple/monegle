use anyhow::{anyhow, Result};
use image::DynamicImage;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution};
use nokhwa::Camera;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Video capture component
pub struct VideoCapture {
    camera: Camera,
    fps: u32,
}

impl VideoCapture {
    /// Initialize a new video capture
    /// Note: width/height are target ASCII dimensions, camera opens at native resolution
    pub fn new(device_index: u32, fps: u32, _width: u32, _height: u32) -> Result<Self> {
        info!(
            "Initializing camera {} at {} FPS (camera opens at native resolution)",
            device_index, fps
        );

        let camera_index = CameraIndex::Index(device_index);

        // Try multiple strategies to open the camera
        // Note: We always open at native camera resolutions (640x480 or 320x240)
        // The converter will resize to the target ASCII dimensions
        let mut camera = None;
        let mut last_error = String::new();

        // Strategy 1: Try with any format (most flexible)
        info!("Strategy 1: Trying with any available format...");
        match Camera::new(
            camera_index.clone(),
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
        ) {
            Ok(cam) => {
                info!("✓ Camera opened with default format");
                camera = Some(cam);
            }
            Err(e) => {
                last_error = format!("Strategy 1 failed: {}", e);
                warn!("{}", last_error);
            }
        }

        // Strategy 2: Try with 640x480 and YUYV (common format that works with FaceTime HD)
        if camera.is_none() {
            info!("Strategy 2: Trying 640x480 YUYV...");
            match Camera::new(
                camera_index.clone(),
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
                    nokhwa::utils::CameraFormat::new(
                        Resolution::new(640, 480),
                        nokhwa::utils::FrameFormat::YUYV,
                        30, // Use standard 30 FPS for camera, we'll throttle in capture loop
                    ),
                )),
            ) {
                Ok(cam) => {
                    info!("✓ Camera opened at 640x480 YUYV @ 30 FPS");
                    camera = Some(cam);
                }
                Err(e) => {
                    last_error = format!("Strategy 2 failed: {}", e);
                    warn!("{}", last_error);
                }
            }
        }

        // Strategy 3: Try with 320x240 (very common low resolution)
        if camera.is_none() {
            info!("Strategy 3: Trying 320x240...");
            match Camera::new(
                camera_index,
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
                    nokhwa::utils::CameraFormat::new(
                        Resolution::new(320, 240),
                        nokhwa::utils::FrameFormat::YUYV,
                        15,
                    ),
                )),
            ) {
                Ok(cam) => {
                    info!("✓ Camera opened at 320x240");
                    camera = Some(cam);
                }
                Err(e) => {
                    last_error = format!("Strategy 3 failed: {}", e);
                    warn!("{}", last_error);
                }
            }
        }

        let mut camera = camera.ok_or_else(|| {
            anyhow!(
                "Failed to open camera after trying all strategies.\n\
                Last error: {}\n\
                \n\
                Troubleshooting:\n\
                1. Check if another app is using the camera (Photo Booth, Zoom, etc.)\n\
                2. On macOS: System Preferences → Security & Privacy → Camera → Enable for Terminal\n\
                3. Try a different camera with --camera 1\n\
                4. List available cameras: system_profiler SPCameraDataType",
                last_error
            )
        })?;

        camera
            .open_stream()
            .map_err(|e| anyhow!("Failed to start camera stream: {}", e))?;

        let info = camera.info();
        info!("✓ Camera initialized successfully!");
        info!("Camera: {}", info.human_name());

        Ok(Self {
            camera,
            fps,
        })
    }

    /// Capture a single frame
    pub fn capture_frame(&mut self) -> Result<DynamicImage> {
        let frame = self
            .camera
            .frame()
            .map_err(|e| anyhow!("Failed to capture frame: {}", e))?;

        let image = frame.decode_image::<RgbFormat>()
            .map_err(|e| anyhow!("Failed to decode frame: {}", e))?;

        let dynamic = DynamicImage::ImageRgb8(image);

        debug!("Captured frame: {}x{}", dynamic.width(), dynamic.height());

        Ok(dynamic)
    }

    /// Start capturing frames and send them through a channel
    /// This runs in a blocking context since Camera is not Send
    pub fn start_capture_loop_blocking(
        mut self,
        tx: mpsc::Sender<DynamicImage>,
    ) -> Result<()> {
        info!("Starting capture loop at {} FPS", self.fps);

        let frame_interval = Duration::from_secs_f32(1.0 / self.fps as f32);
        let mut next_frame_time = std::time::Instant::now();

        let mut frame_count = 0u64;
        let mut error_count = 0u32;

        loop {
            // Wait until next frame time
            let now = std::time::Instant::now();
            if now < next_frame_time {
                std::thread::sleep(next_frame_time - now);
            }
            next_frame_time += frame_interval;

            match self.capture_frame() {
                Ok(image) => {
                    frame_count += 1;
                    error_count = 0;

                    if frame_count % (self.fps as u64 * 10) == 0 {
                        info!("Captured {} frames", frame_count);
                    }

                    if tx.blocking_send(image).is_err() {
                        warn!("Capture channel closed, stopping capture loop");
                        break;
                    }
                }
                Err(e) => {
                    error_count += 1;
                    warn!("Frame capture error ({}): {}", error_count, e);

                    if error_count > 10 {
                        return Err(anyhow!("Too many consecutive capture errors"));
                    }

                    // Brief pause before retrying
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }

        info!("Capture loop stopped after {} frames", frame_count);
        Ok(())
    }
}
