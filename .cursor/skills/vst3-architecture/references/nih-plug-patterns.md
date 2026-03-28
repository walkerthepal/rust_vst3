# nih-plug Advanced Patterns

## Table of Contents
1. [EnumParam](#enumparam)
2. [Nested Params Groups](#nested-params-groups)
3. [Parameter Change Callbacks](#parameter-change-callbacks)
4. [Oversampling](#oversampling)
5. [Editor Integration](#editor-integration)
6. [Polyphonic Modulation (CLAP)](#polyphonic-modulation-clap)
7. [State Serialization](#state-serialization)
8. [Utility Functions](#utility-functions)

---

## EnumParam

```rust
#[derive(Enum, PartialEq)]
enum FilterType {
    #[id = "lp"]
    LowPass,
    #[id = "hp"]
    HighPass,
    #[id = "bp"]
    BandPass,
    #[id = "notch"]
    Notch,
}

// In params struct:
#[id = "ftype"]
pub filter_type: EnumParam<FilterType>,

// Usage in process():
match self.params.filter_type.value() {
    FilterType::LowPass => { /* ... */ }
    FilterType::HighPass => { /* ... */ }
    // ...
}
```

## Nested Params Groups

Group related parameters using nested structs:

```rust
#[derive(Params)]
struct EqParams {
    #[nested(group = "low")]
    pub low: BandParams,

    #[nested(group = "mid")]
    pub mid: BandParams,

    #[nested(group = "high")]
    pub high: BandParams,
}

#[derive(Params)]
struct BandParams {
    #[id = "freq"]
    pub frequency: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "q"]
    pub q: FloatParam,
}
```

Hosts display nested groups as a tree, which makes complex plugins much easier to browse.

## Parameter Change Callbacks

Run code when a parameter changes (useful for updating internal state without checking every block):

```rust
FloatParam::new("Sample Rate Divisor", 1.0, FloatRange::Linear { min: 1.0, max: 16.0 })
    .with_step_size(1.0)
    .with_callback({
        let should_update = should_update.clone();  // Arc<AtomicBool>
        Arc::new(move |_value| {
            should_update.store(true, Ordering::Relaxed);
        })
    })
```

Then in `process()`:
```rust
if self.should_update.load(Ordering::Relaxed) {
    self.should_update.store(false, Ordering::Relaxed);
    self.recompute_coefficients();
}
```

This pattern is better than checking `param.value()` every block when recomputation is expensive.

## Oversampling

nih-plug has a built-in oversampling utility:

```rust
use nih_plug::util::oversampling::Oversampling;

// In your plugin struct:
oversampler: Oversampling<2>,  // 2x oversampling, generic over N channels

// In initialize():
self.oversampler.initialize(buffer_config.sample_rate, buffer_config.max_buffer_size);

// In process():
self.oversampler.process(buffer, |upsampled_buffer| {
    // This closure runs at 2x sample rate
    // Process upsampled_buffer here
});
```

Report the latency introduced by the anti-aliasing filters in `initialize()`.

## Editor Integration

### egui (simplest)

```rust
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};

// In params:
#[persist = "editor-state"]
pub editor_state: Arc<EguiState>,

// In Plugin impl:
fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        self.params.editor_state.clone(),
        (),  // initialization data
        |_, _| {},  // init callback
        move |egui_ctx, setter, _state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.label("Gain");
                ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
            });
        },
    )
}
```

### vizia (more powerful)

Use `nih_plug_vizia` for a richer widget set and CSS-like styling. See the
nih-plug vizia example for the full setup.

### Key Editor rules
- Editor state (window size, etc.) should be persisted with `#[persist = "..."]` on `EguiState`.
- Never read or write audio-thread state directly from the editor — use `ParamSetter` for param
  changes, which routes through the host.
- `Arc<Params>` is shared between the Plugin and the Editor closure safely.

## Polyphonic Modulation (CLAP)

VST3 doesn't support polyphonic modulation. CLAP does. If you're building a synth and want per-voice
modulation, declare `CLAP_POLY_MODULATION_CONFIG` and handle `NoteEvent::PolyModulation` events.

For VST3 only, you handle voice state entirely yourself — typically tracking a fixed-size voice array
indexed by note number.

## State Serialization

Beyond `#[derive(Params)]`, you can persist arbitrary data using `#[persist]`:

```rust
#[derive(Params)]
struct MyParams {
    // Params fields...

    #[persist = "custom-data"]
    pub extra: Arc<Mutex<MyCustomData>>,  // Wrapped in Arc<Mutex> for thread safety
}

#[derive(Serialize, Deserialize, Default)]
struct MyCustomData {
    history: Vec<f32>,
    preset_name: String,
}
```

nih-plug will call `serde_json::to_value` on serialization and `serde_json::from_value` on restore.
The `Mutex` here is fine because state save/restore happens outside the audio thread.

## Utility Functions

Useful conversions from `nih_plug::util`:

```rust
use nih_plug::util;

// dB ↔ linear amplitude
let gain_linear = util::db_to_gain(-6.0);   // 0.5
let gain_db = util::gain_to_db(0.5);        // -6.0

// Frequency to MIDI note (and back)
let note = util::freq_to_midi_note(440.0);  // 69.0 (A4)
let freq = util::midi_note_to_freq(69);     // 440.0

// Smoothing step sizes
SmoothingStyle::Linear(50.0)        // 50ms linear ramp
SmoothingStyle::Logarithmic(50.0)   // 50ms logarithmic (good for gain/freq)
SmoothingStyle::Exponential(0.999)  // IIR-style: coefficient close to 1 = slow
SmoothingStyle::None                // Instant (for discrete params like mode switches)
```
