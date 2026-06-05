# gcode-thumb
A CLI thumbnailer for G-code files. It can extract embedded thumbnails (if present) or render a thumbnail from the G-code itself.

## Usage
```bash
# Generate a thumbnail from a G-code file.
# This will try to extract an embedded thumbnail, and if it doesn't find one, it will render a thumbnail from the G-code.
gcode-thumb my_file.gcode -o my_thumb.png

# Force rendering a thumbnail from the G-code, even if an embedded thumbnail is present.
gcode-thumb my_file.gcode -o my_thumb.png --source generate
# Only try to extract an embedded thumbnail, and fail if it doesn't find one.
gcode-thumb my_file.gcode -o my_thumb.png --source embedded

# Don't ignore the priming line
gcode-thumb my_file.gcode -o my_thumb.png --no-ignore-priming-line
# Don't ignore the adhesion helpers (skirt, brim, raft)
gcode-thumb my_file.gcode -o my_thumb.png --no-ignore-adhesion
```

## Benchmarks
Benchmarks are available in the `benches` directory. They may be run with `cargo bench --features _bench`.

For reference, generating a thumbnail for a 3DBenchy model (~170k lines of G-code) takes around 190ms on a mid-range laptop.
