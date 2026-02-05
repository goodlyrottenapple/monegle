use anyhow::{anyhow, Result};
use minifb::{Key, Window, WindowOptions};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn main() -> Result<()> {
    println!("=== Camera Test App ===");
    println!("This will open your camera and display the feed in a window.");
    println!("Press ESC to exit.\n");

    // Try to open camera with different strategies
    println!("Attempting to open camera...");
    
    let camera_index = CameraIndex::Index(0);
    
    // Strategy 1: Any format
    println!("Strategy 1: Trying with any available format...");
    let mut camera = match Camera::new(
        camera_index.clone(),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
    ) {
        Ok(cam) => {
            println!("✓ Camera opened successfully!");
            cam
        }
        Err(e) => {
            println!("✗ Strategy 1 failed: {}", e);
            
            // Strategy 2: Try 640x480 YUYV
            println!("\nStrategy 2: Trying 640x480...");
            Camera::new(
                camera_index,
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
                    nokhwa::utils::CameraFormat::new(
                        nokhwa::utils::Resolution::new(WIDTH as u32, HEIGHT as u32),
                        nokhwa::utils::FrameFormat::YUYV,
                        30,
                    ),
                )),
            ).map_err(|e| anyhow!("All strategies failed. Last error: {}", e))?
        }
    };

    // Open camera stream
    camera.open_stream()
        .map_err(|e| anyhow!("Failed to start camera stream: {}", e))?;

    let info = camera.info();
    println!("\nCamera Info:");
    println!("  Name: {}", info.human_name());
    println!("  Description: {}", info.description());
    println!("\nCamera is running! Opening window...");

    // Create window
    let mut window = Window::new(
        "Camera Test - Press ESC to exit",
        WIDTH,
        HEIGHT,
        WindowOptions::default(),
    ).map_err(|e| anyhow!("Failed to create window: {}", e))?;

    window.limit_update_rate(Some(std::time::Duration::from_millis(33))); // ~30 FPS

    println!("Window opened! You should see your camera feed.");
    println!("Press ESC to exit.\n");

    let mut frame_count = 0u64;
    let start_time = std::time::Instant::now();

    // Main loop
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Capture frame
        match camera.frame() {
            Ok(frame) => {
                frame_count += 1;

                // Decode frame to RGB
                match frame.decode_image::<RgbFormat>() {
                    Ok(image) => {
                        // Convert RGB image to u32 buffer for minifb
                        let mut buffer: Vec<u32> = Vec::with_capacity(WIDTH * HEIGHT);
                        
                        for pixel in image.pixels() {
                            let r = pixel[0] as u32;
                            let g = pixel[1] as u32;
                            let b = pixel[2] as u32;
                            // Pack RGB into u32: 0RGB
                            buffer.push((r << 16) | (g << 8) | b);
                        }

                        // Update window
                        window.update_with_buffer(&buffer, WIDTH, HEIGHT)
                            .map_err(|e| anyhow!("Failed to update window: {}", e))?;

                        // Print stats every 30 frames
                        if frame_count % 30 == 0 {
                            let elapsed = start_time.elapsed().as_secs_f32();
                            let fps = frame_count as f32 / elapsed;
                            println!("Frames: {} | FPS: {:.1} | Time: {:.1}s", 
                                frame_count, fps, elapsed);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to decode frame: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to capture frame: {}", e);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    let elapsed = start_time.elapsed().as_secs_f32();
    let avg_fps = frame_count as f32 / elapsed;

    println!("\n=== Test Complete ===");
    println!("Total frames: {}", frame_count);
    println!("Duration: {:.1}s", elapsed);
    println!("Average FPS: {:.1}", avg_fps);

    Ok(())
}
