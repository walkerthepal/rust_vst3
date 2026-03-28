# VST3 Concepts Deep Dive

## Table of Contents
1. [Component Architecture](#component-architecture)
2. [Parameter Normalization](#parameter-normalization)
3. [Parameter Units and Grouping](#parameter-units-and-grouping)
4. [Note Expressions & MPE](#note-expressions--mpe)
5. [Factory and Class Info](#factory-and-class-info)
6. [VST3 vs CLAP Differences](#vst3-vs-clap-differences)
7. [Host Compatibility Quirks](#host-compatibility-quirks)

---

## Component Architecture

VST3 separates every plugin into two components:

### Processor Component (`IAudioProcessor`)
- Runs in the real-time audio thread
- Methods: `process()`, `setBusArrangements()`, `getLatencySamples()`, `getTailSamples()`
- Must be thread-safe and allocation-free during `process()`
- Only communicates with Controller via parameter state (normalized floats)

### Controller Component (`IEditController`)
- Runs in the UI/main thread
- Methods: `createView()`, `setParamNormalized()`, `getParamStringByValue()`
- Owns the plugin's GUI
- Receives parameter updates from host automation
- Sends parameter changes to the Processor via the host (not directly)

**nih-plug hides this split from you.** Your `Plugin` struct IS the Processor; the `Editor` trait IS
the Controller. The framework manages communication.

### The Single-Component Extension
VST3 allows fusing Processor + Controller into one object (`IComponent`) — nih-plug uses this
approach for simplicity. Most hosts support it.

---

## Parameter Normalization

VST3's fundamental principle: **all parameters are exchanged as normalized values in [0.0, 1.0]**.

The host stores, displays automation, and sends parameters in normalized form. Your plugin is
responsible for converting:
- Normalized → plain (for internal use and display)
- Plain → normalized (for presets and automation import)

nih-plug's `FloatParam`, `IntParam`, etc. handle all normalization automatically. You access the
**plain** value (`param.value()`) internally; the framework handles the ↔ normalized conversion
when talking to the host.

---

## Parameter Units and Grouping

VST3 organizes parameters into a **Unit tree** (not the same as physical units like Hz):

```
Root Unit (ID 0)
├── "EQ" Unit
│   ├── "Low Band" Unit
│   │   ├── Frequency param
│   │   ├── Gain param
│   │   └── Q param
│   └── "High Band" Unit
│       └── ...
└── "Dynamics" Unit
    └── ...
```

nih-plug exposes this via `#[nested(group = "name")]` on `#[derive(Params)]` structs. Use nested
groups whenever you have 6+ parameters — it makes the host's parameter browser much more usable.

---

## Note Expressions & MPE

VST3 3.5+ supports **Note Expressions** — per-note continuous modulation with sample accuracy:
- Pitch bend per note
- Volume per note
- Pressure (aftertouch) per note
- Custom expressions

nih-plug maps these to `NoteEvent::PolyPressure`, `NoteEvent::PolyTuning`, etc. This is the
VST3 equivalent of MIDI 2.0 polyphonic expression.

For most plugins, you only need basic `NoteOn`/`NoteOff`. For expressive instruments (MPE synths),
handle the poly events.

---

## Factory and Class Info

When nih-plug exports a plugin with `nih_export_vst3!(MyPlugin)`, it registers the plugin in the
VST3 factory. The factory info (vendor name, URL, email) comes from your `Plugin::NAME`,
`Plugin::VENDOR`, `Plugin::URL`, and `Plugin::EMAIL` constants:

```rust
impl Plugin for MyGain {
    const NAME: &'static str = "My Gain";
    const VENDOR: &'static str = "My Company";
    const URL: &'static str = "https://mycompany.com";
    const EMAIL: &'static str = "hello@mycompany.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    // ...
}
```

The plugin's **UID** is derived from NAME + VENDOR — two plugins with the same name and vendor will
have the same UID and conflict. If you rename a plugin, users lose their automation data.

---

## VST3 vs CLAP Differences

nih-plug targets both VST3 and CLAP. Key differences to know:

| Feature | VST3 | CLAP |
|---------|------|------|
| Polyphonic modulation | Note Expressions (3.5+) | First-class `poly_modulation` |
| Optional switchable ports | Not supported | Supported |
| Parameter gestures | Via begin/end edit | Same |
| Voice info | Limited | Rich voice stack info |
| Extension mechanism | Proprietary interfaces | Open extension system |
| License | GPLv3 (bindings) | MIT |

If you use `nih_export_vst3!` + `nih_export_clap!`, the same plugin struct compiles to both formats.
Features only supported in CLAP are gated behind `#[cfg(feature = "clap")]`.

---

## Host Compatibility Quirks

### Ableton Live
- Requires the plugin to be in `~/Library/Audio/Plug-Ins/VST3/` (macOS) or
  `C:\Program Files\Common Files\VST3\` (Windows)
- Live's VST3 parameter display can lag; rely on in-plugin UI for visual feedback

### Reaper
- Excellent VST3 support; good for debugging unusual behavior
- Supports loading from any folder via preferences

### Logic Pro
- Only loads Apple Silicon binaries natively on M-series Macs (use universal binary or arm64 build)
- Strict about the bundle structure — always use `cargo xtask bundle`, never manually copy a `.dylib`

### FL Studio
- VST3 support is good but GUI scaling can behave differently
- Test window resize behavior if you have a resizable editor

### Plugin Validation
Run the **VST3 Plugin Validator** (part of the Steinberg VST3 SDK) before shipping:
```bash
# If you have the SDK installed:
validator /path/to/MyPlugin.vst3
```

nih-plug-generated plugins generally pass validation, but it's worth running to catch edge cases.
