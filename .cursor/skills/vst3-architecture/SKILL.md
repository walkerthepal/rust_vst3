---
name: vst3-architecture
description: >
  Expert guide for VST3 plugin architecture and nih-plug framework design decisions. Use this skill
  whenever the user is building, designing, or troubleshooting a VST3 audio plugin in Rust using
  nih-plug. Trigger on questions about: plugin structure, parameter design, bus configurations,
  sidechain inputs, MIDI handling, latency reporting, GUI/editor setup, xtask bundling, processing
  patterns, state serialization, or any DSP architecture decisions. Also trigger when the user asks
  why their plugin isn't working correctly in a DAW, or wants to add a new feature to an existing
  nih-plug project. Think of this as an always-on senior audio engineer who knows the VST3 spec and
  nih-plug API deeply.
---

# VST3 Architecture & nih-plug Expert Guide

You are helping build a professional VST3 audio plugin in Rust using the **nih-plug** framework.
This skill gives you deep knowledge of the VST3 spec, nih-plug idioms, and common pitfalls — use
it to make informed architectural decisions and write correct, DAW-compatible code.

## Quick Reference

| Task | Go to |
|------|-------|
| Understand VST3 concepts (Components, Controllers, parameter IDs) | [VST3 Concepts](#vst3-core-concepts) |
| Set up parameters correctly | [Parameter Design](#parameter-design) |
| Configure audio buses (stereo, mono, sidechain) | [Bus Arrangements](#bus-arrangements) |
| Handle MIDI / note events | [MIDI & Note Input](#midi--note-input) |
| Report latency (lookahead, etc.) | [Latency Reporting](#latency-reporting) |
| Build & bundle the plugin | [Building & Bundling](#building--bundling) |
| Deep-dive references | `references/nih-plug-patterns.md`, `references/vst3-concepts.md` |
| Pitfalls & gotchas | `references/pitfalls.md` |

---

## VST3 Core Concepts

VST3 separates a plugin into two components that can run in different threads:

- **Processor** — does audio work (samples in → samples out). This is the real-time path; no
  allocations, no locks, no blocking I/O here.
- **Controller** — owns the UI and responds to user interaction. Communicates with the host
  asynchronously.

nih-plug abstracts this separation for you — your `Plugin` struct and its `process()` method *are*
the Processor. The Editor (if any) is the Controller. You rarely touch the VST3 internals directly.

**Parameter IDs** are stable integers (0 – 2,147,483,647) that identify parameters across DAW
sessions. Once you ship a parameter with a given ID, that ID is permanent. nih-plug's `#[id = "..."]`
attribute on `#[derive(Params)]` fields handles this — use short, descriptive string IDs that get
hashed to a stable integer.

---

## Parameter Design

Parameters are the heart of a plugin's long-term compatibility. Get them right early.

```rust
#[derive(Params)]
struct GainParams {
    // String ID → stable hash. Never change these after shipping.
    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "bypass"]
    pub bypass: BoolParam,

    #[id = "mode"]
    pub mode: EnumParam<ProcessMode>,
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),          // Default: 0 dBFS
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))  // 50ms log smoothing
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            bypass: BoolParam::new("Bypass", false),
            mode: EnumParam::new("Mode", ProcessMode::Clean),
        }
    }
}
```

**Key rules:**
- Store values in their *natural unit* (linear amplitude for gain, Hz for frequency) — not dB or
  other display units. Convert only at display time using `value_to_string`/`string_to_value`.
- Use `FloatRange::Skewed` for perceptual quantities (gain, frequency) so the knob feels natural.
- Apply smoothing to any continuously-varying parameter that feeds the audio path — this eliminates
  zipper noise. Use `Logarithmic` for gain/frequency, `Linear` for timing values.
- Read smoothed values in `process()` with `param.smoothed.next()` per sample, or
  `param.smoothed.next_block(&mut block, count)` for block-based processing.

Read `references/nih-plug-patterns.md` for advanced patterns: EnumParam, nested param groups,
callbacks on parameter change.

---

## Bus Arrangements

Declare all supported I/O configurations in `AUDIO_IO_LAYOUTS`. The **first entry is the default**
and should be the most common case (usually stereo in / stereo out).

```rust
const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
    // Primary: stereo in, stereo out
    AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        ..AudioIOLayout::const_default()
    },
    // Alternative: mono in, mono out (important for host compatibility)
    AudioIOLayout {
        main_input_channels: NonZeroU32::new(1),
        main_output_channels: NonZeroU32::new(1),
        aux_input_ports: &[],
        ..AudioIOLayout::const_default()
    },
];
```

**Sidechain input** — add an auxiliary input port:

```rust
AudioIOLayout {
    main_input_channels: NonZeroU32::new(2),
    main_output_channels: NonZeroU32::new(2),
    aux_input_ports: &[new_nonzero_u32(2)],   // stereo sidechain bus
    ..AudioIOLayout::const_default()
},
```

Access the sidechain in `process()` via `aux.inputs[0].as_slice()`. The host activates this bus and
handles routing in its UI (works in Ableton, Reaper, Cubase, Bitwig, etc.).

---

## MIDI & Note Input

```rust
// In your Plugin impl:
const MIDI_INPUT: MidiConfig = MidiConfig::Basic;  // or MidiConfig::MidiCCs

fn process(&mut self, buffer: &mut Buffer, aux: &mut AuxiliaryBuffers,
           context: &mut impl ProcessContext<Self>) -> ProcessStatus {
    while let Some(event) = context.next_event() {
        match event {
            NoteEvent::NoteOn { timing, channel, note, velocity, .. } => {
                // timing is the sample offset within this block — sample-accurate
                self.voice_manager.note_on(note, velocity, timing);
            }
            NoteEvent::NoteOff { timing, note, .. } => {
                self.voice_manager.note_off(note, timing);
            }
            NoteEvent::MidiCC { timing, cc, value, .. } => {
                // Handle CC automation
            }
            _ => {}
        }
    }
    ProcessStatus::Normal
}
```

MIDI events in nih-plug are **sample-accurate** — `timing` tells you the exact sample within the
current block where the event occurred. Always process events interleaved with your audio loop when
this matters (e.g., synthesizers).

---

## Latency Reporting

If your plugin introduces lookahead delay (common in limiters, transient designers), report it so
the DAW can compensate:

```rust
fn initialize(&mut self, _audio_io_layout: &AudioIOLayout,
              buffer_config: &BufferConfig,
              context: &mut impl InitContext<Self>) -> bool {
    // Compute your lookahead in samples from the sample rate
    self.lookahead_samples = (LOOKAHEAD_MS / 1000.0 * buffer_config.sample_rate) as usize;
    context.set_latency_samples(self.lookahead_samples as u32);
    true
}
```

If latency **changes at runtime** (e.g., the user switches an oversampling ratio), notify the host:
```rust
context.set_latency_samples(new_latency);
```

Never change latency silently mid-stream — the DAW will hear a sudden time shift.

---

## Building & Bundling

Standard xtask setup. Your `Cargo.toml` workspace needs:

```toml
[workspace]
members = ["xtask", "."]

[package.metadata.nih-plug]
assets = []
```

And `.cargo/config.toml`:
```toml
[alias]
xtask = "run --package xtask --"
```

Build and bundle for the current platform:
```bash
cargo xtask bundle <crate-name> --release
```

The `.vst3` bundle lands in `target/bundled/`. On macOS it's a `.vst3` bundle directory; on Windows
it's a `.vst3` DLL.

Cross-compile for Apple Silicon from Intel Mac (or vice versa):
```bash
cargo xtask bundle <crate-name> --release --target aarch64-apple-darwin
```

---

## Processing Patterns

The three main patterns for iterating audio in nih-plug:

```rust
// 1. Per-sample, all channels — most flexible, good for sample-accurate automation
for mut sample_channels in buffer.iter_samples() {
    let gain = self.params.gain.smoothed.next();
    for sample in sample_channels.iter_mut() {
        *sample *= gain;
    }
}

// 2. Per-channel block — good for vectorized/SIMD processing
for channel_samples in buffer.as_slice() {
    // channel_samples is &mut [f32] for the full block
    for sample in channel_samples.iter_mut() {
        *sample *= current_gain;
    }
}

// 3. Per-block with explicit indexing — when you need cross-channel access (e.g., mid-side)
let buf = buffer.as_slice();
let n = buf[0].len();
for i in 0..n {
    let mid  = (buf[0][i] + buf[1][i]) * 0.5;
    let side = (buf[0][i] - buf[1][i]) * 0.5;
    // process mid/side...
    buf[0][i] = mid + side;
    buf[1][i] = mid - side;
}
```

**Critical rule for the audio thread:** never allocate memory, lock a mutex, or do any I/O
(disk, network) inside `process()`. Pre-allocate all buffers in `initialize()`.

---

## State Serialization

nih-plug automatically serializes all `#[derive(Params)]` fields. For additional state (e.g., an
internal ring buffer seed, or a selected IR file path), use custom state:

```rust
#[derive(Serialize, Deserialize)]
struct PluginState {
    ir_path: PathBuf,
    seed: u64,
}

// In Plugin impl:
fn save_state(&self) -> Option<serde_json::Value> { ... }
fn restore_state(&mut self, state: &serde_json::Value) -> bool { ... }
```

State is restored at a safe point between process calls — never mid-block.

---

## Reference Files

For deeper information, read these files when needed:

- **`references/nih-plug-patterns.md`** — Advanced nih-plug patterns: EnumParam, nested Params,
  callbacks, editor integration (egui/vizia/iced), oversampling, polyphonic modulation.
- **`references/vst3-concepts.md`** — VST3 spec deep dive: Components vs Controllers, parameter
  value normalization, Units (parameter grouping), Note Expressions, factory info, VST vs CLAP
  differences, host compatibility quirks.
- **`references/pitfalls.md`** — Common mistakes and how to avoid them: parameter ID stability,
  blocking in audio thread, latency notification, buffer size assumptions, cross-platform issues.
