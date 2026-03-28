---
name: egui-implementation
description: >
  Expert guide for implementing egui GUIs in nih-plug VST3/CLAP audio plugins. Use this skill whenever
  the user wants to build, design, style, or troubleshoot a plugin GUI using egui. Trigger on: editor
  setup, egui widgets, custom knobs, sliders, peak meters, waveform displays, spectrum analyzers,
  XY pads, egui painting/drawing, egui layout, egui theming, parameter binding to UI controls,
  ParamSlider, create_egui_editor, EguiState, ResizableWindow, custom egui widgets, egui animations,
  texture rendering, or any visual aspect of an audio plugin. Also trigger when the user mentions
  "GUI", "UI", "editor", "interface", "controls", "visuals", or "display" in the context of their
  plugin, even if they don't say "egui" explicitly. This skill complements vst3-architecture — use
  both when the question spans DSP and GUI.
---

# egui Implementation Guide for Audio Plugins

You are helping build a professional plugin GUI using **egui** through the **nih_plug_egui** adapter.
This skill gives you deep knowledge of egui's immediate-mode paradigm, nih-plug's editor integration,
and patterns for building audio-specific UI components like meters, knobs, waveform displays, and
parameter controls.

## Quick Reference

| Task | Go to |
|------|-------|
| Wire up egui editor from scratch | [Editor Setup](#editor-setup) |
| Bind parameters to UI controls | [Parameter Binding](#parameter-binding) |
| Build custom audio widgets (knobs, meters, visualizers) | [Custom Audio Widgets](#custom-audio-widgets) |
| egui layout, panels, and responsive sizing | [Layout & Panels](#layout--panels) |
| Custom drawing with Painter API | [Custom Painting](#custom-painting) |
| Theming and visual styling | [Theming & Styling](#theming--styling) |
| Share data between audio thread and GUI | [Thread-Safe Data Sharing](#thread-safe-data-sharing) |
| Deep-dive egui patterns and APIs | `references/egui-deep-dive.md` |

---

## Editor Setup

This is the foundational wiring that connects egui to your nih-plug plugin. Every egui-based editor
follows this pattern.

### Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
nih_plug = { git = "https://github.com/robbert-vdh/nih-plug.git", features = ["assert_process_allocs"] }
nih_plug_egui = { git = "https://github.com/robbert-vdh/nih-plug.git" }
```

Pin both to the same git revision to avoid version mismatches between nih-plug and the egui adapter.

### Minimal Editor

The editor lives in a closure passed to `create_egui_editor`. The closure receives three things:
the egui context (for building UI), a `ParamSetter` (for changing parameter values through the host),
and your optional user state.

```rust
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::Arc;

// 1. Add EguiState to your params struct — this persists window size across sessions
#[derive(Params)]
struct MyParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "gain"]
    pub gain: FloatParam,
}

impl Default for MyParams {
    fn default() -> Self {
        Self {
            // Initial window size in logical pixels (before DPI scaling)
            editor_state: EguiState::from_size(400, 300),
            gain: FloatParam::new(/* ... */),
        }
    }
}

// 2. Implement the editor() method on your Plugin
impl Plugin for MyPlugin {
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),                    // user state — () if you don't need any
            |_, _| {},             // build callback — runs once when editor opens
            move |egui_ctx, setter, _state| {
                // This closure runs every frame while the editor is open.
                // Build your entire UI here.
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.label("My Plugin");
                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
                });
            },
        )
    }
}
```

### User State

The second argument to `create_egui_editor` is optional state that persists across frames but not
across editor open/close cycles. Use it for things like animation timers, scroll positions, or
temporary UI state that doesn't belong in your Params.

```rust
struct EditorState {
    show_advanced: bool,
    animation_phase: f32,
}

create_egui_editor(
    self.params.editor_state.clone(),
    EditorState { show_advanced: false, animation_phase: 0.0 },
    |_, _| {},
    move |egui_ctx, setter, state| {
        // `state` is &mut EditorState — mutable access each frame
        state.animation_phase += 0.01;
        // ...
    },
)
```

### ResizableWindow

nih_plug_egui provides `ResizableWindow` for editors that should be user-resizable. It tracks size
changes back into `EguiState` so the host remembers the window dimensions.

```rust
use nih_plug_egui::resizable_window::ResizableWindow;

ResizableWindow::new("main-window")
    .min_size(egui::Vec2::new(200.0, 150.0))
    .show(egui_ctx, params.editor_state.as_ref(), |ui| {
        // UI contents here
    });
```

---

## Parameter Binding

The core pattern: never modify parameter values directly. Always go through `ParamSetter` so the
host (DAW) knows about the change and can record automation.

### ParamSlider (built-in)

The simplest way to expose a parameter:

```rust
ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
```

`ParamSlider` automatically handles: display formatting, range mapping, host notification,
begin/end gestures (for undo grouping in the DAW), and right-click reset to default.

### Manual Parameter Control

When you build a custom widget (a knob, an XY pad, etc.), use this three-step protocol:

```rust
// 1. Tell the host "user started touching this parameter"
setter.begin_set_parameter(&params.gain);

// 2. Set the new value (can be called many times during a drag)
setter.set_parameter(&params.gain, new_linear_value);

// 3. Tell the host "user stopped touching this parameter"
setter.end_set_parameter(&params.gain);
```

The begin/end calls are important — they tell the DAW to group all intermediate values into a
single undo step. Without them, every tiny drag movement becomes a separate undo entry.

### Reading Parameter Values in the UI

Read the current value with `params.gain.value()` (or `.unmodulated_plain_value()` to ignore
host automation). For display, use the parameter's built-in formatter:

```rust
let display_text = params.gain.to_string();  // e.g., "0.00 dB"
```

---

## Custom Audio Widgets

Audio plugins need specialized controls that egui doesn't ship with. Here are patterns for the
most common ones.

### Rotary Knob

A knob is a circular control the user drags vertically (or rotationally) to change a value.

```rust
fn knob_widget(ui: &mut egui::Ui, param: &FloatParam, setter: &ParamSetter) -> egui::Response {
    let desired_size = egui::vec2(48.0, 48.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::drag());

    // Normalize current value to 0.0..=1.0
    let normalized = param.unmodulated_normalized_value();

    // Handle drag interaction
    if response.dragged() {
        let delta = -response.drag_delta().y * 0.005;  // vertical drag, inverted
        let new_normalized = (normalized + delta).clamp(0.0, 1.0);
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, new_normalized);
        setter.end_set_parameter(param);
    }

    // Draw the knob
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = rect.width() * 0.4;

        // Background arc
        let start_angle = std::f32::consts::PI * 0.75;
        let end_angle = std::f32::consts::PI * 2.25;

        // Track
        draw_arc(&painter, center, radius, start_angle, end_angle,
                  egui::Stroke::new(3.0, egui::Color32::from_gray(60)));

        // Value arc
        let value_angle = start_angle + (end_angle - start_angle) * normalized;
        draw_arc(&painter, center, radius, start_angle, value_angle,
                  egui::Stroke::new(3.0, egui::Color32::from_rgb(80, 160, 255)));

        // Indicator dot
        let dot_pos = center + radius * egui::Vec2::new(
            value_angle.cos(), value_angle.sin()
        );
        painter.circle_filled(dot_pos, 4.0, egui::Color32::WHITE);
    }

    response
}
```

### Peak Meter

Share peak data from the audio thread using `Arc<AtomicF32>`:

```rust
fn peak_meter(ui: &mut egui::Ui, peak_db: f32) {
    let desired_size = egui::vec2(ui.available_width(), 16.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 2.0, egui::Color32::from_gray(30));

        // Meter fill — normalize dB to 0..1
        let normalized = ((peak_db + 60.0) / 60.0).clamp(0.0, 1.0);
        let fill_rect = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width() * normalized, rect.height()),
        );

        // Color gradient: green → yellow → red
        let color = if normalized > 0.9 {
            egui::Color32::from_rgb(255, 50, 50)
        } else if normalized > 0.7 {
            egui::Color32::from_rgb(255, 200, 50)
        } else {
            egui::Color32::from_rgb(80, 200, 80)
        };

        painter.rect_filled(fill_rect, 2.0, color);

        // dB label
        let text = if peak_db > util::MINUS_INFINITY_DB {
            format!("{peak_db:.1} dB")
        } else {
            "-inf dB".to_string()
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
}
```

### Waveform / Oscilloscope Display

For real-time waveform visualization, use a ring buffer shared between threads:

```rust
use std::sync::Arc;
use nih_plug::util::permit_alloc;

// Shared buffer — write from audio thread, read from GUI
struct WaveformBuffer {
    data: AtomicRefCell<Vec<f32>>,  // or use a lock-free ring buffer
    write_pos: AtomicUsize,
}

fn waveform_display(ui: &mut egui::Ui, buffer: &WaveformBuffer, num_samples: usize) {
    let desired_size = egui::vec2(ui.available_width(), 100.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));

        let data = buffer.data.borrow();
        let points: Vec<egui::Pos2> = (0..num_samples)
            .map(|i| {
                let x = rect.min.x + (i as f32 / num_samples as f32) * rect.width();
                let sample = data.get(i).copied().unwrap_or(0.0);
                let y = rect.center().y - sample * rect.height() * 0.45;
                egui::pos2(x, y)
            })
            .collect();

        // Draw as a connected line
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 200, 255));
        for window in points.windows(2) {
            painter.line_segment([window[0], window[1]], stroke);
        }

        // Zero line
        painter.line_segment(
            [egui::pos2(rect.min.x, rect.center().y), egui::pos2(rect.max.x, rect.center().y)],
            egui::Stroke::new(0.5, egui::Color32::from_gray(60)),
        );
    }

    // Request continuous repaint while visible
    ui.ctx().request_repaint();
}
```

For more widget patterns (spectrum analyzer, XY pad, envelope editor), see `references/egui-deep-dive.md`.

---

## Layout & Panels

egui uses an immediate-mode layout system — you describe your UI top-to-bottom, and egui
calculates positions. Understanding the layout primitives is important for building plugin UIs
that look professional.

### Panel Types

```rust
// Fixed panel on the left (e.g., for navigation or preset browser)
egui::SidePanel::left("side-panel")
    .resizable(false)
    .exact_width(120.0)
    .show(egui_ctx, |ui| { /* ... */ });

// Fixed panel at top (e.g., for plugin title bar)
egui::TopBottomPanel::top("header")
    .exact_height(40.0)
    .show(egui_ctx, |ui| { /* ... */ });

// CentralPanel fills whatever space remains — use exactly one
egui::CentralPanel::default().show(egui_ctx, |ui| { /* ... */ });
```

### Horizontal and Vertical Groups

```rust
ui.horizontal(|ui| {
    // Children laid out left-to-right
    knob_widget(ui, &params.attack, setter);
    knob_widget(ui, &params.release, setter);
    knob_widget(ui, &params.ratio, setter);
});

ui.vertical(|ui| {
    // Children laid out top-to-bottom (default behavior)
    ui.label("Section Title");
    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
});
```

### Spacing and Alignment

```rust
ui.add_space(8.0);                              // Add gap
ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);  // Between items

// Center content horizontally
ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
    ui.label("Centered text");
});

// Right-align
ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
    ui.label("Right-aligned");
});
```

### Grid Layout

Grids are great for aligning labels with controls:

```rust
egui::Grid::new("params-grid")
    .num_columns(2)
    .spacing([8.0, 4.0])
    .show(ui, |ui| {
        ui.label("Gain");
        ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
        ui.end_row();

        ui.label("Attack");
        ui.add(widgets::ParamSlider::for_param(&params.attack, setter));
        ui.end_row();
    });
```

---

## Custom Painting

egui's `Painter` API lets you draw arbitrary shapes — essential for audio visualizations.

### Painter Basics

```rust
// Get a painter scoped to a rect
let (rect, response) = ui.allocate_exact_size(egui::vec2(200.0, 100.0), egui::Sense::hover());
let painter = ui.painter_at(rect);

// Primitives
painter.rect_filled(rect, 4.0, egui::Color32::from_gray(30));           // Rounded rect
painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE));  // Border
painter.circle_filled(rect.center(), 20.0, egui::Color32::RED);         // Circle
painter.line_segment([pos_a, pos_b], egui::Stroke::new(2.0, color));    // Line

// Text
painter.text(
    rect.center(),
    egui::Align2::CENTER_CENTER,
    "Hello",
    egui::FontId::proportional(14.0),
    egui::Color32::WHITE,
);
```

### Drawing Arcs and Curves

egui doesn't have a built-in arc primitive, so approximate with line segments:

```rust
fn draw_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    stroke: egui::Stroke,
) {
    let segments = 32;
    let points: Vec<egui::Pos2> = (0..=segments)
        .map(|i| {
            let t = i as f32 / segments as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            center + radius * egui::Vec2::new(angle.cos(), angle.sin())
        })
        .collect();

    for window in points.windows(2) {
        painter.line_segment([window[0], window[1]], stroke);
    }
}
```

### Shapes and Meshes

For complex filled shapes (gradient fills, custom polygons), use `Shape::mesh`:

```rust
use egui::epaint::{Mesh, Vertex};

let mut mesh = Mesh::default();

// Add vertices with positions, UVs, and colors
mesh.vertices.push(Vertex {
    pos: egui::pos2(0.0, 0.0),
    uv: egui::pos2(0.0, 0.0),   // UV only matters for textured meshes
    color: egui::Color32::RED,
});
// ... add more vertices and indices ...

painter.add(egui::Shape::mesh(mesh));
```

### Repaint Strategy

By default, egui only repaints when something changes (a mouse move, a click, etc.). For
continuously-animating displays (meters, waveforms, analyzers), you need to request repaints:

```rust
// Request a repaint next frame — call this in your update closure
ui.ctx().request_repaint();

// Or request at a specific interval (less CPU when idle)
ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));  // ~60fps
```

Only request continuous repaints when the editor is actually showing animated content. Check
`params.editor_state.is_open()` in your audio thread to avoid computing visualization data when
the editor is closed.

---

## Theming & Styling

### Global Style

Set the visual style at the start of your update closure:

```rust
move |egui_ctx, setter, _state| {
    let mut style = (*egui_ctx.style()).clone();

    // Dark theme base
    style.visuals = egui::Visuals::dark();

    // Customize colors
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(40);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(55);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 120, 200);

    // Rounding
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(4.0);
    style.visuals.window_rounding = egui::Rounding::same(8.0);

    // Spacing
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);

    egui_ctx.set_style(style);

    // Now build the UI...
    egui::CentralPanel::default().show(egui_ctx, |ui| { /* ... */ });
}
```

### Per-Widget Styling

Override styles locally without affecting the rest of the UI:

```rust
// Temporarily change style for one section
ui.scope(|ui| {
    ui.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(40, 60, 80);
    ui.add(widgets::ParamSlider::for_param(&params.filter_freq, setter));
});
```

### Custom Fonts

Load fonts at editor build time:

```rust
create_egui_editor(
    self.params.editor_state.clone(),
    (),
    |egui_ctx, _| {
        // Build callback — runs once when editor opens
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "my-font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/MyFont.ttf"))),
        );
        fonts.families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "my-font".to_owned());
        egui_ctx.set_fonts(fonts);
    },
    move |egui_ctx, setter, _state| {
        // update closure...
    },
)
```

---

## Thread-Safe Data Sharing

The audio thread and the GUI thread run concurrently. You need lock-free communication between
them — never use `Mutex` on the audio path.

### AtomicF32 for Single Values

Best for simple scalar data (peak levels, current frequency, etc.):

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// In your plugin struct
pub peak_meter: Arc<AtomicF32>,

// Audio thread writes:
self.peak_meter.store(current_peak, Ordering::Relaxed);

// GUI reads:
let peak = self.peak_meter.load(Ordering::Relaxed);
```

nih-plug provides `AtomicF32` — it's a newtype around `AtomicU32` using float bit-casting.

### Triple Buffer for Larger Data

For array data (waveform snapshots, FFT bins), a triple buffer avoids blocking on either side:

```rust
// Use the `triple_buffer` crate, or roll your own with three AtomicPtr-swapped buffers.
// The writer (audio thread) fills one buffer while the reader (GUI) reads another.
// A third buffer sits ready for the next swap.
```

### Conditional Computation

Avoid computing visualization data when the editor is closed:

```rust
fn process(&mut self, buffer: &mut Buffer, /* ... */) -> ProcessStatus {
    // Only compute peak meter when the editor is visible
    if self.params.editor_state.is_open() {
        let amplitude = /* ... */;
        self.peak_meter.store(amplitude, Ordering::Relaxed);
    }

    // Audio processing always runs regardless
    for sample in buffer.iter_samples() { /* ... */ }
    ProcessStatus::Normal
}
```

---

## Reference Files

For deeper coverage, read these when needed:

- **`references/egui-deep-dive.md`** — Advanced egui patterns: spectrum analyzer widget,
  XY pad widget, envelope editor, complex meshes, texture loading, tooltip overlays,
  keyboard shortcuts in the editor, accessibility, animation easing, and performance tips for
  large UIs.
