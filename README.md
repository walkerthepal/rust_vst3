# Quaker Waker

A VST3 gain plugin built in Rust with [nih-plug](https://github.com/robbert-vdh/nih-plug).

## What it does

Applies a smoothed gain parameter (-30 dB to +30 dB) to stereo or mono audio with sample-accurate automation. That's it -- this is a learning project and a foundation for more complex DSP.

## Project structure

```
rust_vst3/
├── .cargo/config.toml       # cargo xtask alias
├── Cargo.toml                # workspace root + plugin crate
├── xtask/                    # nih-plug bundler tooling
│   └── src/main.rs
└── src/
    ├── lib.rs                # plugin entry point, process() impl
    ├── params.rs             # parameter definitions (GainParams)
    ├── dsp/mod.rs            # signal processing (stub)
    └── editor/mod.rs         # GUI (stub)
```

## Building

```bash
cargo xtask bundle rust_vst3 --release
```

The `.vst3` bundle is created at `target/bundled/rust_vst3.vst3`.

## Installing (macOS)

Copy or symlink the bundle to your VST3 directory:

```bash
# Copy
cp -r target/bundled/rust_vst3.vst3 ~/Library/Audio/Plug-Ins/VST3/

# Or symlink for development (auto-updates on rebuild)
ln -sf "$(pwd)/target/bundled/rust_vst3.vst3" ~/Library/Audio/Plug-Ins/VST3/
```

Then rescan plugins in your DAW. The plugin appears as **Quaker Waker** by **Walkerware** under Fx > Tools.

## Plugin details

| Field | Value |
|-------|-------|
| Name | Quaker Waker |
| Vendor | Walkerware |
| Format | VST3 |
| I/O | Stereo + Mono |
| Parameters | Gain (-30 to +30 dB, log smoothed) |
| Framework | nih-plug @ `28b149ec` |
