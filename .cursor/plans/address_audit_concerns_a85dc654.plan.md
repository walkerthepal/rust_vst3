---
name: Address Audit Concerns
overview: "Address all architectural concerns from the project audit: pin the nih-plug dependency, replace the VST3 class ID with a UUID, add ClapPlugin stub, add initialize/reset/deactivate lifecycle methods, create dsp/ and editor/ module stubs, rename plugin to \"Quaker Waker\" by \"Walkerware\", and commit everything."
todos:
  - id: pin-rev
    content: Pin nih-plug git dependency to rev 28b149ec in both Cargo.toml files
    status: completed
  - id: uuid-classid
    content: Generate UUID and replace VST3_CLASS_ID in src/lib.rs
    status: completed
  - id: rename-plugin
    content: Rename plugin to Quaker Waker / Walkerware in src/lib.rs
    status: completed
  - id: clap-stub
    content: Add ClapPlugin stub impl to src/lib.rs
    status: completed
  - id: lifecycle-stubs
    content: Add initialize(), reset(), deactivate() stubs to Plugin impl
    status: completed
  - id: dsp-editor-mods
    content: Create src/dsp/mod.rs and src/editor/mod.rs stubs, wire into lib.rs
    status: completed
  - id: build-and-commit
    content: Verify build with cargo xtask bundle, then commit all files
    status: completed
isProject: false
---

# Address Audit Concerns

## 1. Pin nih-plug git dependency to a known-good revision

The locked revision from the successful build is `28b149ec`. Pin it in both Cargo.toml files so `cargo update` cannot silently pull breaking changes.

In [Cargo.toml](Cargo.toml):

```toml
nih_plug = { git = "https://github.com/robbert-vdh/nih-plug.git", rev = "28b149ec", features = ["assert_process_allocs"] }
```

In [xtask/Cargo.toml](xtask/Cargo.toml):

```toml
nih_plug_xtask = { git = "https://github.com/robbert-vdh/nih-plug.git", rev = "28b149ec" }
```

## 2. Replace VST3 Class ID with a random UUID

Generate a random UUID and convert it to a `[u8; 16]` byte array in [src/lib.rs](src/lib.rs). Add a comment marking it as permanent. The current `*b"RustVst3GainPlug"` will be replaced with something like:

```rust
// PERMANENT: changing this breaks all saved DAW projects using this plugin
const VST3_CLASS_ID: [u8; 16] = [0xA3, 0x7F, ...];  // generated UUID
```

Will use `uuidgen` on macOS to produce the value.

## 3. Rename plugin to "Quaker Waker" / "Walkerware"

In [src/lib.rs](src/lib.rs), update the `Plugin` impl constants:

- `NAME` -> `"Quaker Waker"`
- `VENDOR` -> `"Walkerware"`

Also update `VST3_SUBCATEGORIES` -- `Fx` + `Tools` is correct for a gain utility.

Note: the crate name in `Cargo.toml` stays `rust_vst3` (it's the project name, not the display name). The bundle command remains `cargo xtask bundle rust_vst3 --release`.

## 4. Add `ClapPlugin` stub impl

Add a minimal `ClapPlugin` implementation to [src/lib.rs](src/lib.rs) for forward-compatibility. No `nih_export_clap!` macro call -- just the trait impl so future nih-plug versions won't break compilation if they add it as a bound.

```rust
impl ClapPlugin for Gain {
    const CLAP_ID: &'static str = "com.walkerware.quaker-waker";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A simple gain plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}
```

## 5. Add `initialize()`, `reset()`, and `deactivate()` lifecycle stubs

Add these methods to the `Plugin` impl in [src/lib.rs](src/lib.rs). For now they are no-ops that return the correct types, but they establish the pattern for when stateful DSP is added:

```rust
fn initialize(
    &mut self,
    _audio_io_layout: &AudioIOLayout,
    _buffer_config: &BufferConfig,
    _context: &mut impl InitContext<Self>,
) -> bool {
    true
}

fn reset(&mut self) {}

fn deactivate(&mut self) {}
```

## 6. Create `dsp/` and `editor/` module stubs

Create two new files and wire them into [src/lib.rs](src/lib.rs):

- `**src/dsp/mod.rs**` -- empty module with a placeholder comment and no public API yet
- `**src/editor/mod.rs**` -- empty module with a placeholder comment and no public API yet

In `lib.rs`, add:

```rust
mod dsp;
mod editor;
```

These are declared but unused for now. They will suppress "no such module" errors when you start building DSP or GUI code.

## 7. Verify build and commit

After all changes, run `cargo xtask bundle rust_vst3 --release` to verify everything still compiles and bundles. Then commit all untracked and modified files (`.cargo/`, `Cargo.toml`, `Cargo.lock`, `src/`, `xtask/`).