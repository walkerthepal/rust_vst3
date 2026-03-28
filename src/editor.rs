use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui;
use std::sync::Arc;

use crate::dsp::{WaveformBuffer, WAVEFORM_LEN};
use crate::params::GainParams;

const ACCENT: egui::Color32 = egui::Color32::from_rgb(80, 160, 255);

pub fn create(
    params: Arc<GainParams>,
    waveform_buffer: Arc<WaveformBuffer>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        [0.0_f32; WAVEFORM_LEN],
        |_, _| {},
        move |egui_ctx, setter, snapshot| {
            apply_theme(egui_ctx);

            waveform_buffer.read_snapshot(snapshot);

            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(egui::Color32::from_gray(25)))
                .show(egui_ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("QUAKER WAKER")
                                .size(18.0)
                                .color(egui::Color32::from_gray(200))
                                .strong(),
                        );
                        ui.add_space(12.0);

                        waveform_display(ui, snapshot);

                        ui.add_space(16.0);

                        knob_widget(ui, &params.gain, setter);
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(params.gain.to_string())
                                .size(13.0)
                                .color(egui::Color32::from_gray(180)),
                        );
                        ui.label(
                            egui::RichText::new("Gain")
                                .size(11.0)
                                .color(egui::Color32::from_gray(120)),
                        );
                    });
                });
        },
    )
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(40);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(55);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 120, 200);
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(4);
    style.visuals.window_corner_radius = egui::CornerRadius::same(8);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    ctx.set_style(style);
}

fn draw_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    stroke: egui::Stroke,
) {
    let segments = 48;
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

fn knob_widget(ui: &mut egui::Ui, param: &FloatParam, setter: &ParamSetter) -> egui::Response {
    let desired_size = egui::vec2(64.0, 64.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::drag());

    let normalized = param.unmodulated_normalized_value();

    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.dragged() {
        let delta = -response.drag_delta().y * 0.005;
        let new_normalized = (normalized + delta).clamp(0.0, 1.0);
        setter.set_parameter_normalized(param, new_normalized);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let radius = rect.width() * 0.38;

        // 270-degree sweep: from 135 degrees to 405 degrees
        let start_angle = std::f32::consts::FRAC_PI_4 * 3.0; // 135 deg
        let end_angle = std::f32::consts::FRAC_PI_4 * 11.0; // 405 deg (135 + 270)

        // Track background
        draw_arc(
            &painter,
            center,
            radius,
            start_angle,
            end_angle,
            egui::Stroke::new(3.0, egui::Color32::from_gray(60)),
        );

        // Value arc
        let value_angle = start_angle + (end_angle - start_angle) * normalized;
        if normalized > 0.001 {
            draw_arc(
                &painter,
                center,
                radius,
                start_angle,
                value_angle,
                egui::Stroke::new(3.0, ACCENT),
            );
        }

        // Indicator dot
        let dot_pos =
            center + radius * egui::Vec2::new(value_angle.cos(), value_angle.sin());
        painter.circle_filled(dot_pos, 4.0, egui::Color32::WHITE);

        // Center cap
        painter.circle_filled(center, radius * 0.45, egui::Color32::from_gray(50));
        painter.circle_stroke(
            center,
            radius * 0.45,
            egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
        );
    }

    response
}

fn waveform_display(ui: &mut egui::Ui, data: &[f32]) {
    let desired_size = egui::vec2(ui.available_width() - 24.0, 120.0);
    let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(18));
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
            egui::StrokeKind::Inside,
        );

        // Zero line
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 2.0, rect.center().y),
                egui::pos2(rect.max.x - 2.0, rect.center().y),
            ],
            egui::Stroke::new(0.5, egui::Color32::from_gray(50)),
        );

        let num_samples = data.len();
        if num_samples > 1 {
            let points: Vec<egui::Pos2> = (0..num_samples)
                .map(|i| {
                    let x = rect.min.x
                        + 4.0
                        + (i as f32 / (num_samples - 1) as f32) * (rect.width() - 8.0);
                    let sample = data[i].clamp(-1.0, 1.0);
                    let y = rect.center().y - sample * (rect.height() * 0.42);
                    egui::pos2(x, y)
                })
                .collect();

            let stroke = egui::Stroke::new(1.5, ACCENT);
            for window in points.windows(2) {
                painter.line_segment([window[0], window[1]], stroke);
            }
        }
    }

    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(16));
}
