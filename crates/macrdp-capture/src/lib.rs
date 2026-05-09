//! macOS screen capture via CGDisplayStream (CoreGraphics)
//! Swift-free alternative to ScreenCaptureKit for macOS 12.3+ compatibility

use std::ffi::c_void;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bytes::Bytes;
use core_foundation::{
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFMutableDictionary},
    number::CFNumber,
    string::CFString,
};
use core_graphics2::{
    error::CGError,
    display::CGDisplay,
    display_stream::*,
};
use dispatch2::{Queue, QueueAttribute};
use core_video::pixel_buffer;
use tokio::sync::mpsc;

/// Screenshot directory for debugging capture functionality
static SCREENSHOT_DIR: &str = "/tmp/macrdp_screenshots";

/// Environment variable to enable screenshot debugging
const SCREENSHOT_DEBUG_ENV: &str = "MACRDP_SCREENSHOT_DEBUG";

/// Initialize screenshot directory
fn init_screenshot_dir() -> Result<PathBuf> {
    let path = PathBuf::from(SCREENSHOT_DIR);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Check if screenshot debugging is enabled
fn screenshot_debug_enabled() -> bool {
    std::env::var(SCREENSHOT_DEBUG_ENV).is_ok()
}

/// Save BGRA frame as PNG file for debugging (only when MACRDP_SCREENSHOT_DEBUG is set)
fn maybe_save_frame_as_png(frame_data: &[u8], width: u32, height: u32, stride: usize, label: &str) {
    if !screenshot_debug_enabled() {
        return;
    }
    
    if let Err(e) = save_frame_as_png(frame_data, width, height, stride, label) {
        tracing::warn!("Failed to save screenshot: {}", e);
    }
}

/// Save BGRA frame as PNG file for debugging
fn save_frame_as_png(frame_data: &[u8], width: u32, height: u32, stride: usize, label: &str) -> Result<()> {
    let screenshot_dir = init_screenshot_dir()?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis();
    let filename = format!("{}_{}.png", label, timestamp);
    let filepath = screenshot_dir.join(filename);
    
    // Convert BGRA to RGBA for PNG encoding
    let rgba_data: Vec<u8> = frame_data
        .chunks_exact(4)
        .flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]]) // BGRA -> RGBA
        .collect();
    
    // Create image buffer
    let mut img_buf = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::new(width, height);
    
    // Copy row by row to handle stride
    for y in 0..height {
        let row_start = y as usize * stride;
        let row_end = row_start + (width as usize * 4);
        if row_end <= rgba_data.len() {
            let row_data = &rgba_data[row_start..row_end];
            for x in 0..width {
                if let Some(pixel) = row_data.get(x as usize * 4..(x as usize + 1) * 4) {
                    if pixel.len() == 4 {
                        img_buf.put_pixel(x, y, image::Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]));
                    }
                }
            }
        }
    }
    
    img_buf.save(&filepath)?;
    tracing::info!("Saved screenshot: {}", filepath.display());
    Ok(())
}

/// Check if Screen Recording permission is granted (no prompt)
/// Note: CGDisplayStream may have different permission requirements than ScreenCaptureKit
pub fn check_screen_recording_permission() -> bool {
    // CGDisplayStream doesn't have the same preflight API as ScreenCaptureKit
    // We'll check by attempting to create a stream and see if it fails
    true // Assume granted for now, will fail at runtime if not
}

/// Request Screen Recording permission (triggers system dialog if not granted)
/// Returns true if already granted. Note: even after granting, the app
/// may need to be restarted for the permission to take effect.
pub fn request_screen_recording_permission() -> bool {
    // CGDisplayStream doesn't have the same request API as ScreenCaptureKit
    // Open System Settings instead
    open_screen_recording_settings();
    false
}

/// Open System Settings to Privacy & Security page
pub fn open_screen_recording_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn();
}

/// A rectangle region
#[derive(Clone, Debug)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Pixel format for screen capture output
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CapturePixelFormat {
    /// BGRA 32-bit (default, needed for OpenH264 and bitmap fallback path)
    Bgra,
    /// NV12 (420f full-range) — zero-copy to VideoToolbox, no color conversion needed
    Nv12,
}

/// Frame pixel data — either raw BGRA bytes or a zero-copy CVPixelBuffer reference
pub enum FrameData {
    /// BGRA raw bytes copied from CVPixelBuffer (existing behavior)
    Raw(Bytes),
    /// IOSurface-backed CVPixelBuffer — zero copy, passed directly to VideoToolbox
    PixelBuffer(SafePixelBuffer),
}

impl FrameData {
    /// Get raw BGRA bytes if this is a Raw frame. Returns None for PixelBuffer frames.
    pub fn as_bgra_bytes(&self) -> Option<&[u8]> {
        match self {
            FrameData::Raw(bytes) => Some(bytes),
            FrameData::PixelBuffer(_) => None,
        }
    }
}

/// A captured screen frame
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub data: FrameData,
    /// Bytes per row (valid for FrameData::Raw only)
    pub stride: usize,
    pub timestamp_us: u64,
    /// Regions that changed since the last frame.
    /// Empty means info unavailable — treat as full frame change.
    pub dirty_rects: Vec<Rect>,
}

/// Configuration for screen capture
#[derive(Clone)]
pub struct CaptureConfig {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub pixel_format: CapturePixelFormat,
}

/// Screen capturer using CGDisplayStream
pub struct ScreenCapturer {
    _stream: CGDisplayStream,
    frame_rx: mpsc::Receiver<CapturedFrame>,
}

// SAFETY: CGDisplayStream is a CoreFoundation reference-counted object,
// which is safe to send across threads. The underlying CGDisplayStreamRef
// is managed by CoreFoundation's reference counting.
unsafe impl Send for ScreenCapturer {}

struct OutputHandler {
    frame_tx: mpsc::Sender<CapturedFrame>,
    pixel_format: CapturePixelFormat,
    width: u32,
    height: u32,
    frame_count: std::sync::atomic::AtomicU32,
}

impl OutputHandler {
    fn handle_frame(
        &self,
        status: CGDisplayStreamFrameStatus,
        timestamp: u64,
        surface: Option<io_surface::IOSurface>,
        _update: Option<CGDisplayStreamUpdate>,
    ) {
        match status {
            CGDisplayStreamFrameStatus::Stopped => {
                tracing::debug!("CGDisplayStream stopped");
                return;
            }
            CGDisplayStreamFrameStatus::FrameComplete => {}
            _ => return,
        }

        let surface = match surface {
            Some(s) => s,
            None => return,
        };

        let frame = match self.pixel_format {
            CapturePixelFormat::Nv12 => self.extract_frame_nv12(&surface, timestamp),
            CapturePixelFormat::Bgra => self.extract_frame_bgra(&surface, timestamp),
        };

        if let Some(frame) = frame {
            // Non-blocking send — drop frame if channel is full
            let _ = self.frame_tx.try_send(frame);
        }
    }

    fn extract_frame_bgra(&self, surface: &io_surface::IOSurface, timestamp: u64) -> Option<CapturedFrame> {
        use io_surface::{IOSurfaceLock, IOSurfaceUnlock, IOSurfaceLockOptions};

        // Lock the IOSurface for reading
        let mut seed = 0;
        let surface_ref = surface.as_concrete_TypeRef();
        let lock_result = unsafe { IOSurfaceLock(surface_ref, IOSurfaceLockOptions::kIOSurfaceLockReadOnly, &mut seed) };
        if lock_result != 0 {
            tracing::error!("Failed to lock IOSurface: {}", lock_result);
            return None;
        }

        // Get IOSurface properties using the C API
        let width = unsafe { io_surface::IOSurfaceGetWidth(surface_ref) };
        let height = unsafe { io_surface::IOSurfaceGetHeight(surface_ref) };
        let bytes_per_row = unsafe { io_surface::IOSurfaceGetBytesPerRow(surface_ref) };
        let base_address = unsafe { io_surface::IOSurfaceGetBaseAddress(surface_ref) } as *const u8;

        if base_address.is_null() {
            tracing::error!("IOSurface base address is null");
            unsafe { IOSurfaceUnlock(surface_ref, IOSurfaceLockOptions::kIOSurfaceLockReadOnly, &mut seed) };
            return None;
        }

        // Calculate total size
        let total_size = bytes_per_row * height;

        // Copy the pixel data
        let pixel_data = unsafe {
            std::slice::from_raw_parts(base_address, total_size).to_vec()
        };

        // Unlock the IOSurface
        unsafe { IOSurfaceUnlock(surface_ref, IOSurfaceLockOptions::kIOSurfaceLockReadOnly, &mut seed) };

        let frame = CapturedFrame {
            width: width as u32,
            height: height as u32,
            data: FrameData::Raw(Bytes::from(pixel_data.clone())),
            stride: bytes_per_row,
            timestamp_us: timestamp,
            dirty_rects: vec![Rect {
                x: 0,
                y: 0,
                width: width as u32,
                height: height as u32,
            }],
        };

        // Save screenshot for debugging (first 10 frames only)
        let frame_num = self.frame_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if frame_num < 10 {
            maybe_save_frame_as_png(&pixel_data, width as u32, height as u32, bytes_per_row, "cgstream");
        }

        Some(frame)
    }

    fn extract_frame_nv12(&self, surface: &io_surface::IOSurface, timestamp: u64) -> Option<CapturedFrame> {
        // For now, fall back to BGRA extraction from NV12 surface
        // Full NV12 support would require proper CVPixelBuffer wrapping
        // CGDisplayStream typically outputs in the requested format
        self.extract_frame_bgra(surface, timestamp)
    }
}

impl ScreenCapturer {
    /// Create a new screen capturer for the main display
    pub async fn new(config: CaptureConfig) -> Result<Self> {
        let display = CGDisplay::main();
        let display_id = display.id;

        let actual_width = if config.width == 0 {
            display.pixels_wide() as u32
        } else {
            config.width
        };
        let actual_height = if config.height == 0 {
            display.pixels_high() as u32
        } else {
            config.height
        };

        // Pixel format: BGRA for compatibility, NV12 for performance
        let pixel_format = match config.pixel_format {
            CapturePixelFormat::Nv12 => pixel_buffer::kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            CapturePixelFormat::Bgra => pixel_buffer::kCVPixelFormatType_32BGRA,
        };

        // Create properties dictionary
        let properties = CFDictionary::from_CFType_pairs(&[
            (
                CGDisplayStreamProperties::ShowCursor.into(),
                CFBoolean::true_value().as_CFType(),
            ),
            (
                CGDisplayStreamProperties::MinimumFrameTime.into(),
                CFNumber::from(1.0 / config.frame_rate as f64).as_CFType(),
            ),
        ]);

        // Channel for frames: buffer 2 frames to allow for jitter
        let (frame_tx, frame_rx) = mpsc::channel(2);

        let handler = OutputHandler {
            frame_tx,
            pixel_format: config.pixel_format,
            width: actual_width,
            height: actual_height,
            frame_count: std::sync::atomic::AtomicU32::new(0),
        };

        // Create dispatch queue for callbacks
        let queue = Queue::new(
            "com.macrdp.displaystream",
            QueueAttribute::Serial,
        );

        // Create the display stream
        let stream = match CGDisplayStream::new_with_dispatch_queue(
            display_id,
            actual_width as usize,
            actual_height as usize,
            pixel_format as i32,
            &properties,
            &queue,
            move |status, timestamp, surface, update| {
                handler.handle_frame(status, timestamp, surface, update);
            },
        ) {
            Ok(stream) => stream,
            Err(_) => {
                tracing::warn!("CGDisplayStream creation failed - this is normal in SSH/headless sessions");
                tracing::warn!("Falling back to CgFallbackCapturer (CGDisplayCreateImage)");
                return Err(anyhow::anyhow!("CGDisplayStream not available in current session"));
            }
        };

        // Start the stream
        let start_result = stream.start();
        if start_result != CGError::Success {
            anyhow::bail!("Failed to start CGDisplayStream: {:?}", start_result);
        }

        tracing::info!(
            width = actual_width,
            height = actual_height,
            fps = config.frame_rate,
            pixel_format = ?config.pixel_format,
            "CGDisplayStream capture started"
        );

        Ok(Self {
            _stream: stream,
            frame_rx,
        })
    }

    /// Receive the next captured frame (async, cancellation safe)
    pub async fn next_frame(&mut self) -> Option<CapturedFrame> {
        self.frame_rx.recv().await
    }

    /// Try to get a buffered frame without waiting. Returns None if no frame ready.
    pub fn try_next_frame(&mut self) -> Option<CapturedFrame> {
        self.frame_rx.try_recv().ok()
    }
}

/// Query the main display's resolution
pub fn detect_display_size() -> Result<(u32, u32)> {
    let display = CGDisplay::main();
    Ok((display.pixels_wide() as u32, display.pixels_high() as u32))
}

/// Fallback capturer using CGDisplayCreateImage (CoreGraphics).
/// Works during lock screen because it captures at the display level,
/// below the window server / ScreenCaptureKit layer.
pub struct CgFallbackCapturer {
    display_id: u32,
    width: u32,
    height: u32,
    frame_interval: std::time::Duration,
    frame_count: std::sync::atomic::AtomicU32,
}

impl CgFallbackCapturer {
    /// Create a fallback capturer for the main display
    pub fn new(config: &CaptureConfig) -> Self {
        let display_id = CGDisplay::main().id;
        let fps = if config.frame_rate > 0 {
            config.frame_rate
        } else {
            30
        };
        Self {
            display_id,
            width: config.width,
            height: config.height,
            frame_interval: std::time::Duration::from_micros(1_000_000 / fps as u64),
            frame_count: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// Capture a single frame using CGDisplayCreateImage
    pub fn capture_frame(&self) -> Option<CapturedFrame> {
        let display = CGDisplay::new(self.display_id);
        let image = display.new_image()?;

        let w = image.width() as u32;
        let h = image.height() as u32;
        let bpr = image.bytes_per_row();
        
        // Use a simple approach to get the image data via CFData
        let data_provider = image.data_provider()?;
        let cf_data = data_provider.copy_data()?;
        let raw = cf_data.bytes().to_vec();

        let frame = CapturedFrame {
            width: if self.width > 0 { self.width } else { w },
            height: if self.height > 0 { self.height } else { h },
            data: FrameData::Raw(Bytes::from(raw.clone())),
            stride: bpr,
            timestamp_us: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            dirty_rects: vec![Rect {
                x: 0,
                y: 0,
                width: if self.width > 0 { self.width } else { w },
                height: if self.height > 0 { self.height } else { h },
            }],
        };

        // Save screenshot for debugging (first 10 frames only)
        let frame_num = self.frame_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if frame_num < 10 {
            maybe_save_frame_as_png(&raw, frame.width, frame.height, frame.stride, "cgfallback");
        }

        Some(frame)
    }

    /// Frame interval for pacing
    pub fn frame_interval(&self) -> std::time::Duration {
        self.frame_interval
    }
}

// ---------------------------------------------------------------------------
// CoreVideo FFI for CVPixelBuffer retain/release and plane access
// ---------------------------------------------------------------------------

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferRetain(pixel_buffer: *mut c_void) -> *mut c_void;
    fn CVPixelBufferRelease(pixel_buffer: *mut c_void);
    fn CVPixelBufferLockBaseAddress(pixel_buffer: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pixel_buffer: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddressOfPlane(pixel_buffer: *mut c_void, plane: usize) -> *mut u8;
    fn CVPixelBufferGetBytesPerRowOfPlane(pixel_buffer: *mut c_void, plane: usize) -> usize;
    fn CVPixelBufferGetHeightOfPlane(pixel_buffer: *mut c_void, plane: usize) -> usize;
}

/// kCVPixelBufferLock_ReadOnly
const CV_PIXEL_BUFFER_LOCK_READ_ONLY: u64 = 0x0000_0001;

// ---------------------------------------------------------------------------
// NV12PlaneData — extracted Y and UV plane data from an NV12 pixel buffer
// ---------------------------------------------------------------------------

/// Holds copied plane data from an NV12 CVPixelBuffer.
/// Used for the OpenH264 software encoding fallback path.
pub struct NV12PlaneData {
    /// Y (luma) plane data, one byte per pixel, row-major
    pub y_data: Vec<u8>,
    /// Y plane stride (bytes per row, may include padding)
    pub y_stride: usize,
    /// UV (chroma) plane data, interleaved U/V, half resolution
    pub uv_data: Vec<u8>,
    /// UV plane stride (bytes per row, may include padding)
    pub uv_stride: usize,
    /// Width of the Y plane in pixels
    pub width: usize,
    /// Height of the Y plane in pixels
    pub height: usize,
}

// ---------------------------------------------------------------------------
// SafePixelBuffer — RAII wrapper around a retained CVPixelBufferRef
// ---------------------------------------------------------------------------

/// A safe RAII wrapper around a `CVPixelBufferRef` that manages the
/// retain/release lifecycle. Intended for zero-copy frame passing to
/// VideoToolbox (hardware encoder) while also supporting a locked-read
/// path for OpenH264 (software encoder fallback).
///
/// # Safety
///
/// The inner pointer must originate from a valid `CVPixelBufferRef`.
/// `Send` is implemented because IOSurface-backed pixel buffers are safe
/// to transfer across threads. `Sync` is deliberately NOT implemented
/// because `CVPixelBufferLockBaseAddress` / `UnlockBaseAddress` are not
/// safe for concurrent access from multiple threads.
pub struct SafePixelBuffer {
    ptr: *mut c_void,
}

// SAFETY: IOSurface-backed CVPixelBuffers can be sent across threads.
// We do NOT implement Sync — lock/unlock is not thread-safe for
// concurrent access.
unsafe impl Send for SafePixelBuffer {}

impl SafePixelBuffer {
    /// Create a `SafePixelBuffer` by retaining the given `CVPixelBufferRef`.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null `CVPixelBufferRef`.
    pub unsafe fn from_raw(ptr: *mut c_void) -> Self {
        debug_assert!(!ptr.is_null(), "CVPixelBufferRef must not be null");
        CVPixelBufferRetain(ptr);
        Self { ptr }
    }

    /// Return the raw `CVPixelBufferRef` pointer (e.g. for passing to
    /// VideoToolbox's `VTCompressionSessionEncodeFrame`).
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    /// Lock the pixel buffer, copy NV12 plane data out, and unlock.
    ///
    /// This is the software-encoding path: we lock the buffer read-only,
    /// memcpy the Y and UV planes into owned `Vec<u8>`s, then unlock.
    /// The lock is held for the shortest possible duration.
    ///
    /// Returns `None` if the lock fails or plane pointers are null.
    pub fn lock_and_read_nv12(&self) -> Option<NV12PlaneData> {
        unsafe {
            // Lock for read-only access
            let status = CVPixelBufferLockBaseAddress(self.ptr, CV_PIXEL_BUFFER_LOCK_READ_ONLY);
            if status != 0 {
                tracing::warn!(status, "CVPixelBufferLockBaseAddress failed");
                return None;
            }

            let result = self.read_nv12_planes();

            // Always unlock, even if plane read failed
            CVPixelBufferUnlockBaseAddress(self.ptr, CV_PIXEL_BUFFER_LOCK_READ_ONLY);

            result
        }
    }

    /// Read Y and UV planes while the buffer is locked.
    /// Caller must ensure the buffer is locked before calling.
    unsafe fn read_nv12_planes(&self) -> Option<NV12PlaneData> {
        // Plane 0 = Y (luma)
        let y_ptr = CVPixelBufferGetBaseAddressOfPlane(self.ptr, 0);
        let y_stride = CVPixelBufferGetBytesPerRowOfPlane(self.ptr, 0);
        let y_height = CVPixelBufferGetHeightOfPlane(self.ptr, 0);

        // Plane 1 = UV (chroma, interleaved)
        let uv_ptr = CVPixelBufferGetBaseAddressOfPlane(self.ptr, 1);
        let uv_stride = CVPixelBufferGetBytesPerRowOfPlane(self.ptr, 1);
        let uv_height = CVPixelBufferGetHeightOfPlane(self.ptr, 1);

        if y_ptr.is_null() || uv_ptr.is_null() {
            tracing::warn!("NV12 plane base address is null");
            return None;
        }

        let y_len = y_stride * y_height;
        let uv_len = uv_stride * uv_height;

        let y_data = std::slice::from_raw_parts(y_ptr, y_len).to_vec();
        let uv_data = std::slice::from_raw_parts(uv_ptr, uv_len).to_vec();

        // Width is derived from plane 0 stride and pixel format.
        // For NV12 Y plane, each pixel is one byte, but stride may include
        // padding. We use the plane height directly and report stride so
        // callers can handle padding.
        Some(NV12PlaneData {
            y_data,
            y_stride,
            uv_data,
            uv_stride,
            width: y_stride, // conservative: callers should clamp to actual width
            height: y_height,
        })
    }
}

impl Drop for SafePixelBuffer {
    fn drop(&mut self) {
        // SAFETY: ptr was retained in `from_raw`, so we must release it.
        unsafe {
            CVPixelBufferRelease(self.ptr);
        }
    }
}
