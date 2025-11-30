use super::super::{AxisCalUi, CurcatApp, rounded_u8};
use egui::{Color32, CornerRadius, StrokeKind, pos2};

impl CurcatApp {
    pub(crate) fn attention_color(&self, ctx: &egui::Context, base: Color32) -> Color32 {
        let [r, g, b, a] = base.to_array();
        let base_alpha = f32::from(a) / 255.0;
        let time = ctx.input(|i| i.time) as f32;
        let blink = (time * super::super::ATTENTION_BLINK_SPEED)
            .sin()
            .mul_add(0.5, 0.5)
            .clamp(0.0, 1.0);
        let eased = blink * blink * 2.0f32.mul_add(-blink, 3.0);
        let intensity = egui::lerp(
            super::super::ATTENTION_ALPHA_MIN..=super::super::ATTENTION_ALPHA_MAX,
            eased,
        );
        let alpha = rounded_u8(base_alpha * intensity * 255.0);
        Color32::from_rgba_unmultiplied(r, g, b, alpha)
    }

    pub(crate) fn paint_attention_outline_if(&self, ui: &egui::Ui, rect: egui::Rect, active: bool) {
        if !active || !ui.is_rect_visible(rect) {
            return;
        }
        let mut stroke = self.config.attention_highlight.stroke();
        stroke.color = self.attention_color(ui.ctx(), stroke.color);
        ui.painter().rect_stroke(
            rect.expand(super::super::ATTENTION_OUTLINE_PAD),
            CornerRadius::ZERO,
            stroke,
            StrokeKind::Outside,
        );
    }
}

pub fn axis_needs_attention(cal: &AxisCalUi) -> bool {
    let (v1_invalid, v2_invalid) = cal.value_invalid_flags();
    v1_invalid || v2_invalid || cal.p1.is_none() || cal.p2.is_none()
}

pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(
        ui.spacing().interact_size.y * 1.8,
        ui.spacing().interact_size.y,
    );
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, *on);
        let radius = rect.height() / 2.0;
        ui.painter().rect(
            rect,
            CornerRadius::same(rounded_u8(radius)),
            visuals.bg_fill,
            visuals.bg_stroke,
            StrokeKind::Middle,
        );

        let knob_radius = radius - 2.0;
        let knob_x = egui::lerp(
            (rect.left() + knob_radius + 2.0)..=(rect.right() - knob_radius - 2.0),
            if *on { 1.0 } else { 0.0 },
        );
        let knob_center = pos2(knob_x, rect.center().y);
        ui.painter()
            .circle_filled(knob_center, knob_radius, visuals.fg_stroke.color);
    }

    response
}
