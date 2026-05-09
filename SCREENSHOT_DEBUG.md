# Screenshot Debugging for Screen Capture

## Overview

The macrdp capture system now includes screenshot debugging functionality to help verify that both CGDisplayStream and CgFallbackCapturer are working correctly. This is particularly useful for testing and validation.

## Features

- **Automatic screenshot saving**: First 10 frames from each capture method are saved as PNG files
- **Environment-controlled**: Only active when `MACRDP_SCREENSHOT_DEBUG` environment variable is set
- **Separate labeling**: CGDisplayStream frames labeled as "cgstream", fallback frames labeled as "cgfallback"
- **Timestamp tracking**: Each screenshot includes millisecond timestamp for debugging
- **PNG format**: Screenshots saved as standard PNG files for easy viewing

## Usage

### Enable Screenshot Debugging

```bash
# Set environment variable before running the server
export MACRDP_SCREENSHOT_DEBUG=1
cargo run --release --bin macrdp-server

# Or inline
MACRDP_SCREENSHOT_DEBUG=1 cargo run --release --bin macrdp-server
```

### Screenshot Location

Screenshots are saved to: `/tmp/macrdp_screenshots/`

File naming format:
- CGDisplayStream: `cgstream_<timestamp>.png`
- CgFallbackCapturer: `cgfallback_<timestamp>.png`

### Standalone Testing

A standalone test binary is available to test capture without the full RDP server:

```bash
cd crates/macrdp-capture
MACRDP_SCREENSHOT_DEBUG=1 cargo run --release --bin test_capture
```

This will:
- Test CgFallbackCapturer functionality
- Capture 5 frames with screenshots
- Save screenshots to `/tmp/macrdp_screenshots/`

## Example Output

```
Testing CgFallbackCapturer with screenshot debugging...
Screenshots will be saved to /tmp/macrdp_screenshots/
Capturing frame 0...
  Frame captured: 2560x1440, stride: 10240
Capturing frame 1...
  Frame captured: 2560x1440, stride: 10240
Test complete! Check /tmp/macrdp_screenshots/ for screenshots.
```

## Implementation Details

### CGDisplayStream Screenshots
- Saved in `OutputHandler::extract_frame_bgra()`
- First 10 frames only (to avoid disk space issues)
- Uses atomic counter for thread safety
- Converts BGRA to RGBA for PNG encoding

### CgFallbackCapturer Screenshots
- Saved in `CgFallbackCapturer::capture_frame()`
- First 10 frames only
- Uses atomic counter for thread safety
- Direct BGRA to RGBA conversion

### Image Processing
- BGRA → RGBA color conversion (swapping red and blue channels)
- Row-by-row copying to handle stride/padding
- Uses `image` crate for PNG encoding
- Error handling prevents crashes if screenshot fails

## Verification

To verify screenshots are working:

1. Enable screenshot debugging
2. Run the server or test binary
3. Check screenshot directory: `ls -la /tmp/macrdp_screenshots/`
4. Verify PNG files: `file /tmp/macrdp_screenshots/*.png`
5. Open screenshots in image viewer to verify content

## Performance Impact

- **Minimal when disabled**: Zero overhead when `MACRDP_SCREENSHOT_DEBUG` is not set
- **Low when enabled**: Only first 10 frames saved, PNG encoding is fast
- **Disk usage**: ~4.2MB per screenshot for 2560x1440 resolution
- **Thread-safe**: Uses atomic operations for frame counting

## Troubleshooting

### Screenshots not appearing
- Verify environment variable is set: `echo $MACRDP_SCREENSHOT_DEBUG`
- Check directory permissions: `ls -la /tmp/macrdp_screenshots/`
- Check server logs for "Saved screenshot" messages

### Invalid PNG files
- Check frame format (should be BGRA)
- Verify stride calculations are correct
- Check image dependency is properly linked

### Permission errors
- Ensure `/tmp/` directory is writable
- Check screenshot directory creation succeeded

## Future Enhancements

Potential improvements:
- Configurable frame limit (not hardcoded to 10)
- Compression options for screenshots
- Automatic cleanup of old screenshots
- Screenshot on error conditions
- Video recording of capture session
