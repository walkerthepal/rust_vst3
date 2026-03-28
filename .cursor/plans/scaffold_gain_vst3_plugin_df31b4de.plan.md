---
name: Scaffold Gain VST3 Plugin
overview: Set up a complete nih-plug gain plugin project from scratch with cargo xtask bundling, producing a .vst3 that can be loaded into Ableton.
todos:
  - id: cargo-toml
    content: Create root Cargo.toml with workspace config, cdylib crate type, and nih_plug dependency
    status: completed
  - id: cargo-config
    content: Create .cargo/config.toml with xtask alias
    status: completed
  - id: xtask
    content: Create xtask/Cargo.toml and xtask/src/main.rs boilerplate
    status: completed
  - id: params
    content: Create src/params.rs with GainParams struct (gain FloatParam, -30 to +30 dB)
    status: completed
  - id: lib
    content: Create src/lib.rs with Gain plugin struct, Plugin + Vst3Plugin impls, nih_export_vst3 macro
    status: completed
  - id: build-verify
    content: Run cargo xtask bundle rust_vst3 --release and verify .vst3 output
    status: completed
isProject: false
---

# Scaffold a Rust VST3 Gain Plugin

The workspace is currently empty (just a README and .gitignore). We will create a full nih-plug project with a gain plugin and xtask bundling.

## File structure to create

```
rust_vst3/
├── .cargo/
│   └── config.toml       <- cargo xtask alias
├── Cargo.toml             <- workspace root
├── xtask/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
└── src/
    ├── lib.rs             <- plugin entry + Process impl
    └── params.rs          <- parameter definitions
```

No `dsp/` or `editor/` directories yet -- those come later when you add signal processing beyond simple gain and a GUI.

## 1. Root `Cargo.toml` (workspace + plugin crate)

- Workspace with `members = ["xtask"]`
- The root crate itself is the plugin (`crate-type = ["cdylib"]`)
- Depend on `nih_plug` from git with `features = ["assert_process_allocs"]`
- Default features include `vst3` (enabled by default in nih-plug)
- Release profile with `lto = "thin"` and `strip = "symbols"` for smaller binaries

```toml
[package]
name = "rust_vst3"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
nih_plug = { git = "https://github.com/robbert-vdh/nih-plug.git", features = ["assert_process_allocs"] }

[workspace]
members = ["xtask"]

[profile.release]
lto = "thin"
strip = "symbols"
```

## 2. `.cargo/config.toml`

Defines the `cargo xtask` alias so `cargo xtask bundle rust_vst3 --release` works:

```toml
[alias]
xtask = "run --package xtask --release --"
```

## 3. `xtask/` crate

Minimal boilerplate -- just calls `nih_plug_xtask::main()`:

- `**xtask/Cargo.toml**`: depends on `nih_plug_xtask` from the same nih-plug git repo
- `**xtask/src/main.rs**`: single `main()` function delegating to the xtask library

## 4. `src/params.rs` -- Parameter definitions

A clean `GainParams` struct with the `#[derive(Params)]` macro:

- One `FloatParam` for gain (stored as linear gain, displayed in dB using `formatters::v2s_f32_gain_to_db`)
- Range: -30 dB to +30 dB with `FloatRange::Skewed` and `gain_skew_factor`
- Logarithmic smoothing (50ms) for click-free automation

## 5. `src/lib.rs` -- Plugin entry point

A `Gain` struct implementing `Plugin` and `Vst3Plugin`:

- Stereo + mono audio layouts
- `process()` applies smoothed gain per sample
- `SAMPLE_ACCURATE_AUTOMATION = true` for sample-accurate parameter changes
- VST3 class ID and subcategories
- `nih_export_vst3!(Gain)` macro (no CLAP export since VST3-only was selected)

The implementation follows the official nih-plug gain example but simplified:

- No nested/array parameter groups (those are demo-only)
- No persisted `random_data` field
- Params split into a separate `params.rs` module for cleanliness

## 6. Build and verify

After scaffolding, the build/bundle workflow is:

```bash
cargo xtask bundle rust_vst3 --release
```

This produces `target/bundled/rust_vst3.vst3` which can be copied to `/Library/Audio/Plug-Ins/VST3/` (or the user-level equivalent) and loaded in Ableton.