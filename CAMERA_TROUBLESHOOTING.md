# Camera Troubleshooting Guide

## Error: "Failed to open camera"

### Quick Fixes

#### 1. Check Camera Permissions (macOS)

```bash
# Grant camera access in System Preferences
# System Preferences → Security & Privacy → Camera
# Enable access for Terminal or your app
```

#### 2. Try Different Camera Index

```bash
# Try camera 0 (default)
./target/release/monegle-sender --camera 0

# Try camera 1 (if you have multiple cameras)
./target/release/monegle-sender --camera 1
```

#### 3. Close Other Apps Using Camera

- Close Photo Booth, Zoom, Teams, FaceTime, etc.
- Check if any browser tabs are using the camera

#### 4. Use Test Mode (No Camera)

If camera issues persist, use synthetic frames for testing:

```bash
# Use the test binary which doesn't need a camera
./target/release/monegle-sender-test \
  --rpc-url https://testnet-rpc.monad.xyz \
  --target-address 0x0000000000000000000000000000000000000001 \
  --fps 15 --width 80 --height 60 --duration 60
```

### Common Issues

**Issue**: "Cannot fulfill request: RequestedFormat"
- **Cause**: Camera doesn't support requested format (MJPEG)
- **Fix**: Updated code now uses flexible format (any available)

**Issue**: Permission denied
- **Fix**: Grant camera permissions in system settings

**Issue**: Camera in use
- **Fix**: Close other applications using the camera

**Issue**: No camera found
- **Fix**: Check `system_profiler SPCameraDataType` to list cameras

### Alternative: Synthetic Frame Mode

For testing without a camera, you can modify the sender to use synthetic frames:

1. Use the existing `monegle-sender-test` binary
2. Or implement a `--synthetic` flag (future enhancement)

### Verify Camera Access

```bash
# List available cameras (macOS)
system_profiler SPCameraDataType

# List available cameras (Linux)
ls -l /dev/video*
v4l2-ctl --list-devices
```

### Debug Mode

Run with debug logging to see more details:

```bash
RUST_LOG=debug ./target/release/monegle-sender --config config.toml
```

Look for lines like:
```
INFO monegle_sender::capture: Camera info: ...
```

### If Still Not Working

1. **Use test mode**: Run `monegle-sender-test` instead (no camera needed)
2. **Report issue**: The camera library (nokhwa) may need platform-specific fixes
3. **Alternative**: Use synthetic frame generator for development/testing

