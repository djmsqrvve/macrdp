use std::time::Duration;
use macrdp_capture::{CgFallbackCapturer, CaptureConfig, CapturePixelFormat};

fn main() {
    // Enable screenshot debugging
    std::env::set_var("MACRDP_SCREENSHOT_DEBUG", "1");
    
    let config = CaptureConfig {
        width: 2560,
        height: 1440,
        frame_rate: 30,
        pixel_format: CapturePixelFormat::Bgra,
    };
    
    println!("Testing CgFallbackCapturer with screenshot debugging...");
    println!("Screenshots will be saved to /tmp/macrdp_screenshots/");
    
    let capturer = CgFallbackCapturer::new(&config);
    
    // Capture 5 frames with screenshots
    for i in 0..5 {
        println!("Capturing frame {}...", i);
        if let Some(frame) = capturer.capture_frame() {
            println!("  Frame captured: {}x{}, stride: {}", frame.width, frame.height, frame.stride);
        } else {
            println!("  Failed to capture frame");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    
    println!("Test complete! Check /tmp/macrdp_screenshots/ for screenshots.");
}