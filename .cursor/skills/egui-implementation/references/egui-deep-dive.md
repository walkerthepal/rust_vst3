# egui Deep Dive for Audio Plugin UIs

## Table of Contents
1. [Spectrum Analyzer Widget](#spectrum-analyzer-widget)
2. [XY Pad Widget](#xy-pad-widget)
3. [Envelope Editor](#envelope-editor)
4. [Texture Loading & Images](#texture-loading--images)
5. [Animation & Easing](#animation--easing)
6. [Keyboard Shortcuts](#keyboard-shortcuts)
7. [Tooltips & Context Menus](#tooltips--context-menus)
8. [Performance Tips](#performance-tips)
9. [Common Pitfalls](#common-pitfalls)

---

## Spectrum Analyzer Widget

A spectrum analyzer displays FFT magnitude data as a frequency-domain plot. The audio thread
computes FFT bins, and the GUI reads them via a shared buffer.

### FFT Data Pipeline

```rust
use std::sync::Arc;
use std::sync::atomic::Ordering;

const FFT_SIZE: usize = 2048;
const NUM_DISPLAY_BINS: usize = FFT_SIZE / 2;

/// Shared FFT data between audio and GUI threads.
/// Use triple buffering or AtomicPtr swapping for the full array.
/// For simplicity, this example uses a Mutex (acceptable on GUI thread, NOT on audio thread).
struct SpectrumData {
    magnitudes_db: parking_lot::Mutex<Vec<f32>>,
}

impl SpectrumData {
    fn new() -> Self {
        Self {
            magnitudes_db: parking_lot::Mutex::new(vec![-120.0; NUM_DISPLAY_BINS]),
        }
    }
}
```

### Drawing the Spectrum

```rust
fn spectrum_analyzer(
    ui: &mut egui::Ui,
    spectrum: &SpectrumData,
    sample_rate: f32,
) {
    let desired_size = egui::vec2(ui.available_width(), 150.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(15));

    let magnitudes = spectrum.magnitudes_db.lock();
    let num_bins = magnitudes.len();
    let nyquist = sample_rate / 2.0;

    // Map frequency bins to x-position using logarithmic scale
    // (20 Hz to Nyquist)
    let min_freq: f32 = 20.0;
    let max_freq = nyquist;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    let db_floor = -90.0_f32;
    let db_ceil = 0.0_f32;

    let mut points = Vec::with_capacity(num_bins);

    for i in 0..num_bins {
        let freq = (i as f32 / num_bins as f32) * nyquist;
        if freq < min_freq { continue; }

        // Logarithmic x mapping
        let log_freq = freq.ln();
        let x_norm = (log_freq - log_min) / (log_max - log_min);
        let x = rect.min.x + x_norm * rect.width();

        // Linear dB y mapping
        let db = magnitudes[i].clamp(db_floor, db_ceil);
        let y_norm = 1.0 - (db - db_floor) / (db_ceil - db_floor);
        let y = rect.min.y + y_norm * rect.height();

        points.push(egui::pos2(x, y));
    }

    // Draw as filled polygon (spectrum + bottom edge)
    if points.len() >= 2 {
        // Line on top
        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 180, 255));
        for window in points.windows(2) {
            painter.line_segment([window[0], window[1]], stroke);
        }

        // Optional: filled area underneath
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        // Use a semi-transparent color for the fill
        let fill_color = egui::Color32::from_rgba_premultiplied(40, 100, 200, 40);
        painter.add(egui::Shape::convex_polygon(
            fill_points,
            fill_color,
            egui::Stroke::NONE,
        ));
    }

    // Draw frequency grid lines (100, 1k, 10k Hz)
    for &freq in &[100.0, 1000.0, 10000.0_f32] {
        if freq >= max_freq { continue; }
        let x_norm = (freq.ln() - log_min) / (log_max - log_min);
        let x = rect.min.x + x_norm * rect.width();
        painter.line_segment(
            [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
            egui::Stroke::new(0.5, egui::Color32::from_gray(50)),
        );
        painter.text(
            egui::pos2(x, rect.max.y - 2.0),
            egui::Align2::CENTER_BOTTOM,
            if freq >= 1000.0 { format!("{}k", freq / 1000.0) } else { format!("{}", freq) },
            egui::FontId::proportional(9.0),
            egui::Color32::from_gray(120),
        );
    }

    ui.ctx().request_repaint();
}
```

---

## XY Pad Widget

An XY pad maps two parameters to a 2D area — common for filter cutoff + resonance, or
panning + width.

```rust
fn xy_pad(
    ui: &mut egui::Ui,
    param_x: &FloatParam,
    param_y: &FloatParam,
    setter: &ParamSetter,
) -> egui::Response {
    let desired_size = egui::vec2(150.0, 150.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    let norm_x = param_x.unmodulated_normalized_value();
    let norm_y = 1.0 - param_y.unmodulated_normalized_value();  // invert Y (screen coords)

    if response.drag_started() {
        setter.begin_set_parameter(param_x);
        setter.begin_set_parameter(param_y);
    }

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let new_x = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
            let new_y = 1.0 - ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
            setter.set_parameter_normalized(param_x, new_x);
            setter.set_parameter_normalized(param_y, new_y);
        }
    }

    if response.drag_stopped() {
        setter.end_set_parameter(param_x);
        setter.end_set_parameter(param_y);
    }

    // Draw
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(25));
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, egui::Color32::from_gray(60)));

        // Grid
        for i in 1..4 {
            let frac = i as f32 / 4.0;
            painter.line_segment(
                [egui::pos2(rect.min.x + frac * rect.width(), rect.min.y),
                 egui::pos2(rect.min.x + frac * rect.width(), rect.max.y)],
                egui::Stroke::new(0.5, egui::Color32::from_gray(40)),
            );
            painter.line_segment(
                [egui::pos2(rect.min.x, rect.min.y + frac * rect.height()),
                 egui::pos2(rect.max.x, rect.min.y + frac * rect.height())],
                egui::Stroke::new(0.5, egui::Color32::from_gray(40)),
            );
        }

        // Crosshair + dot at current value
        let dot_x = rect.min.x + norm_x * rect.width();
        let dot_y = rect.min.y + norm_y * rect.height();
        let dot_pos = egui::pos2(dot_x, dot_y);

        painter.line_segment(
            [egui::pos2(dot_x, rect.min.y), egui::pos2(dot_x, rect.max.y)],
            egui::Stroke::new(0.5, egui::Color32::from_rgba_premultiplied(100, 180, 255, 80)),
        );
        painter.line_segment(
            [egui::pos2(rect.min.x, dot_y), egui::pos2(rect.max.x, dot_y)],
            egui::Stroke::new(0.5, egui::Color32::from_rgba_premultiplied(100, 180, 255, 80)),
        );

        painter.circle_filled(dot_pos, 6.0, egui::Color32::from_rgb(80, 160, 255));
        painter.circle_stroke(dot_pos, 6.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
    }

    response
}
```

---

## Envelope Editor

An ADSR envelope editor with draggable breakpoints:

```rust
/// Draw an ADSR envelope with draggable handles.
/// Each handle corresponds to a time parameter (A, D, R) or level parameter (S).
fn envelope_display(
    ui: &mut egui::Ui,
    attack: &FloatParam,
    decay: &FloatParam,
    sustain: &FloatParam,   // level, 0..1
    release: &FloatParam,
    setter: &ParamSetter,
) {
    let desired_size = egui::vec2(ui.available_width(), 80.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(20));

    // Normalize times into the rect width
    let total_time = attack.value() + decay.value() + release.value() + 0.2; // sustain hold
    let scale_x = rect.width() / total_time;
    let s_level = sustain.value();

    // Key points
    let p0 = egui::pos2(rect.min.x, rect.max.y);  // start: bottom-left
    let p1 = egui::pos2(
        rect.min.x + attack.value() * scale_x,
        rect.min.y,  // peak: top
    );
    let p2 = egui::pos2(
        p1.x + decay.value() * scale_x,
        rect.max.y - s_level * rect.height(),  // sustain level
    );
    let p3 = egui::pos2(
        p2.x + 0.2 * scale_x,   // sustain hold segment
        p2.y,
    );
    let p4 = egui::pos2(
        p3.x + release.value() * scale_x,
        rect.max.y,  // release to zero
    );

    // Draw envelope line
    let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 220, 160));
    let points = [p0, p1, p2, p3, p4];
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], stroke);
    }

    // Draw handle dots
    let handle_color = egui::Color32::WHITE;
    for &p in &[p1, p2, p3, p4] {
        painter.circle_filled(p, 4.0, handle_color);
    }

    // Labels
    let label_color = egui::Color32::from_gray(140);
    let font = egui::FontId::proportional(9.0);
    painter.text(p1, egui::Align2::CENTER_BOTTOM, "A", font.clone(), label_color);
    painter.text(p2, egui::Align2::CENTER_BOTTOM, "D", font.clone(), label_color);
    painter.text(p3, egui::Align2::LEFT_BOTTOM, "S", font.clone(), label_color);
    painter.text(p4, egui::Align2::CENTER_TOP, "R", font, label_color);
}
```

Making the breakpoints interactive (draggable) follows the same pattern as the knob widget —
allocate hit regions around each point, check for drag, update the corresponding parameter
through the setter.

---

## Texture Loading & Images

To display images (logos, backgrounds, waveform textures) in the editor:

```rust
// In the build callback (runs once when editor opens):
|egui_ctx, _| {
    // Load from embedded bytes
    let image_data = include_bytes!("../assets/logo.png");
    let image = image::load_from_memory(image_data).unwrap().to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.as_flat_samples();

    egui_ctx.tex_manager().write().alloc(
        "logo".into(),
        egui::ImageData::Color(std::sync::Arc::new(
            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
        )),
        egui::TextureOptions::LINEAR,
    );
}

// In the update callback, display it:
let texture_id = egui_ctx.tex_manager().read().meta("logo").unwrap().id;
ui.image((texture_id, egui::vec2(64.0, 64.0)));
```

Alternatively, load textures lazily in the update closure using egui's built-in image loading
(if using the `image` feature). For plugin UIs, embedding textures at compile time via
`include_bytes!` is usually best — it keeps the plugin self-contained with no runtime file I/O.

---

## Animation & Easing

### Frame-Rate Independent Animation

Use `ui.input(|i| i.stable_dt)` for the time delta between frames:

```rust
let dt = ui.input(|i| i.stable_dt);
state.animation_t += dt * speed;
if state.animation_t > 1.0 { state.animation_t = 1.0; }

// Apply easing
let eased = ease_out_cubic(state.animation_t);
```

### Common Easing Functions

```rust
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
```

### Smooth Value Following

For meters and displays that follow a value with visual smoothing (independent of audio smoothing):

```rust
// In your editor state:
struct EditorState {
    displayed_peak: f32,
}

// In update:
let target = peak_meter.load(Ordering::Relaxed);
let smoothing = 0.15;  // lower = smoother
state.displayed_peak += (target - state.displayed_peak) * smoothing;
```

---

## Keyboard Shortcuts

Handle keyboard input in the editor for power-user features:

```rust
// In your update closure:
ui.input(|i| {
    // Ctrl+Z / Cmd+Z — note: undo is typically handled by the host, but
    // you might use shortcuts for editor-specific actions
    if i.modifiers.command && i.key_pressed(egui::Key::Z) {
        // handle undo
    }

    // Space to toggle bypass
    if i.key_pressed(egui::Key::Space) {
        let current = params.bypass.value();
        setter.begin_set_parameter(&params.bypass);
        setter.set_parameter(&params.bypass, !current);
        setter.end_set_parameter(&params.bypass);
    }
});
```

Be cautious with keyboard shortcuts — some DAWs capture certain keys before they reach the
plugin editor. Test in your target DAWs. Single-letter shortcuts are especially likely to conflict.

---

## Tooltips & Context Menus

### Tooltips

```rust
let response = ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
response.on_hover_text("Adjusts the output gain in dB.\nDouble-click to reset.");
```

### Rich Tooltips

```rust
response.on_hover_ui(|ui| {
    ui.label("Output Gain");
    ui.separator();
    ui.label(format!("Current: {:.1} dB", util::gain_to_db(params.gain.value())));
    ui.label("Range: -30 to +30 dB");
});
```

### Context Menus

```rust
response.context_menu(|ui| {
    if ui.button("Reset to default").clicked() {
        setter.begin_set_parameter(&params.gain);
        setter.set_parameter(&params.gain, params.gain.default_plain_value());
        setter.end_set_parameter(&params.gain);
        ui.close_menu();
    }
    if ui.button("Copy value").clicked() {
        ui.output_mut(|o| o.copied_text = params.gain.to_string());
        ui.close_menu();
    }
});
```

---

## Performance Tips

1. **Check visibility before drawing.** Always wrap painting code in
   `if ui.is_rect_visible(rect) { ... }`. This skips painting for off-screen elements — matters
   when you have tabbed or scrollable UI sections.

2. **Minimize allocations per frame.** Reuse `Vec` allocations across frames by storing them in
   your editor state rather than creating new ones each frame.

3. **Limit repaint rate for idle content.** Use `request_repaint_after(Duration::from_millis(33))`
   (30fps) instead of `request_repaint()` (unlimited) for content that doesn't need 60+ fps.
   Meters at 30fps look fine; save CPU for the audio thread.

4. **Batch draw calls.** Multiple `painter.line_segment()` calls are fine for small counts, but
   for thousands of line segments (dense waveforms), build a single `Shape::LineSegments` or use
   a mesh.

5. **Skip visualization computation when editor is closed.** Check
   `params.editor_state.is_open()` in your audio thread before computing FFT, peak tracking, or
   waveform snapshots.

6. **Use `egui::util::cache::FrameCache`** for expensive computations that only need to run once
   per frame (e.g., converting FFT bins to screen coordinates).

---

## Common Pitfalls

### Forgetting begin_set_parameter / end_set_parameter
Without these, DAW undo doesn't group your changes. The user drags a knob and gets 200 undo
entries instead of 1. Always call begin before the first set, end after the last.

### Blocking the audio thread from the GUI
Never use `Mutex` or any blocking primitive that the audio thread also locks. If the GUI lock
contends with the audio thread, you get audio dropouts. Use atomics for scalars, lock-free
ring buffers or triple buffers for arrays.

### Drawing outside the allocated rect
If you paint outside the rect returned by `allocate_exact_size`, your drawing will overlap other
widgets. Use `painter_at(rect)` which clips to the rect, or manually clip.

### Not persisting editor state
If you forget `#[persist = "editor-state"]` on your `Arc<EguiState>`, the window size resets
every time the user reopens the editor or reloads the project. Always persist it.

### Using response.clicked() for drag interactions
`clicked()` only fires on release. For knobs and sliders, use `dragged()` and `drag_delta()`.
Use `drag_started()` / `drag_stopped()` for begin/end parameter gestures.

### Continuous repainting killing CPU
Calling `request_repaint()` every frame means egui never stops rendering, even when idle.
This wastes battery and CPU. Only request repaints when something is actively animating.
When the plugin sits idle with no user interaction, it should not be repainting.
