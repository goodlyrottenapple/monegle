# Dry Run Mode Guide

## What is Dry Run Mode?

Dry run mode captures video from your camera, converts it to ASCII art, and displays it in your terminal **without sending anything to the blockchain**. This is perfect for:

- ✅ Testing camera setup
- ✅ Verifying ASCII conversion quality
- ✅ Checking FPS and performance
- ✅ Adjusting resolution and character sets
- ✅ No blockchain costs!

## Usage

```bash
./target/release/monegle-sender --dry-run --config config.toml
```

### Options

```bash
# Use different camera
./target/release/monegle-sender --dry-run --camera 1

# Override config file
./target/release/monegle-sender --dry-run --config my-config.toml

# With debug logging
RUST_LOG=debug ./target/release/monegle-sender --dry-run
```

## What You'll See

```
╔════════════════════════════════════════════════════════╗
║  Monegle Dry Run - ASCII Video Preview                ║
╠════════════════════════════════════════════════════════╣
║  Frame: 42         FPS: 15     Time: 2.8s             ║
╚════════════════════════════════════════════════════════╝

[ASCII art video of you will display here]
                    
Press Ctrl+C to stop
```

The terminal will:
- Clear and refresh for each frame
- Show frame count, FPS, and elapsed time
- Display the live ASCII video
- Update at the configured FPS

## Configuration Tips

Edit `config.toml` to adjust quality:

```toml
[sender]
fps = 15                    # Lower = smoother on slower machines
resolution = [80, 60]       # [width, height] - try [60, 40] for smaller
character_set = "Detailed"  # "Standard", "Dense", "Blocks", or "Detailed"
color_mode = "Purple"       # "None", "Purple", "Blue", "Green", or "Rgb"
```

### Character Sets

**Standard** (10 chars): ` .:-=+*#%@`
- Simple, fast, good for basic shapes
- Best for testing

**Dense** (70 chars): ` .'^\`\",:;Il!i...`
- More detail, slower to render
- Better for faces and complex scenes

**Blocks** (5 chars): ` ░▒▓█`
- Smooth gradients
- Good for smooth surfaces

**Detailed** (45 chars): ` .·'`,;:∙^\"~-_+<>=*×!?/|\\()[]Iil...` 🌟 **Recommended!**
- Enhanced quality with Unicode symbols
- Best balance of detail and performance
- Better shading and texture representation
- Great for faces, objects, and complex scenes

### Color Modes

**None** - Classic monochrome ASCII
- No colors, just characters
- Best compatibility with all terminals

**Purple** - Purple/magenta gradient 💜 **Recommended!**
- Dark purple → bright magenta
- Cyberpunk aesthetic
- Great for portraits and artistic videos

**Blue** - Blue gradient 💙
- Dark blue → cyan
- Cool, professional look
- Good for tech demos

**Green** - Green/matrix style 💚
- Dark green → bright green
- Classic "matrix" look
- Terminal hacker aesthetic

**Rgb** - True RGB color 🌈 **MOST REALISTIC!**
- Uses ACTUAL colors from your camera/video
- Each ASCII character is colored with the real pixel color
- Sky is blue, grass is green, skin tones are realistic!
- Requires modern terminal with truecolor support (iTerm2, Alacritty, Kitty, VS Code terminal)
- Creates photorealistic ASCII art

## Troubleshooting

### Camera Not Opening

1. **Check permissions** (macOS):
   - System Preferences → Security & Privacy → Camera
   - Enable for Terminal

2. **Try different camera**:
   ```bash
   ./target/release/monegle-sender --dry-run --camera 0
   ./target/release/monegle-sender --dry-run --camera 1
   ```

3. **Close other apps** using the camera:
   - Photo Booth, Zoom, FaceTime, browser tabs

### Low FPS / Stuttering

- Lower resolution: `resolution = [60, 40]`
- Lower FPS: `fps = 10`
- Use "Standard" character set
- Close other applications

### Terminal Too Small

- Increase terminal window size
- Or lower resolution in config
- Terminal should be at least 80×60 characters

## Next Steps

Once your camera and ASCII output look good:

1. **Get testnet MON tokens**
2. **Set private key**: `export MONAD_PRIVATE_KEY="0x..."`
3. **Run without --dry-run** to stream to blockchain:
   ```bash
   ./target/release/monegle-sender --config config.toml
   ```

## Example Session

```bash
# Terminal 1: Test camera with dry-run
./target/release/monegle-sender --dry-run

# Looks good! Now stream to blockchain
export MONAD_PRIVATE_KEY="0x..."
./target/release/monegle-sender

# Terminal 2: Watch the stream
./target/release/monegle-receiver --sender-address 0xYourAddress
```

## Tips

- Start with `--dry-run` to verify everything works
- Experiment with different resolutions and character sets
- Watch your resource usage (Activity Monitor / htop)
- The ASCII art quality depends on lighting and camera position

