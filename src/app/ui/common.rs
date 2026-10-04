//! Shared UI components and presentation helpers.

use super::super::{AxisCalUi, CurcatApp};
use super::style;
use crate::util::rounded_u8;
use egui::{Color32, CornerRadius, StrokeKind};

/// Create a nonresizable tool window; callers can opt into collapsing.
pub(super) fn tool_window<'a>(title: impl egui::IntoAtoms<'a>) -> egui::Window<'a> {
    egui::Window::new(title).resizable(false).collapsible(false)
}

pub(super) fn bar_separator(ui: &mut egui::Ui) {
    ui.add_space(style::SPACE_TIGHT);
    ui.separator();
    ui.add_space(style::SPACE_TIGHT);
}

pub(super) fn section_heading(ui: &mut egui::Ui, title: impl Into<egui::RichText>) {
    ui.add_space(style::SPACE_ROW);
    ui.heading(title.into());
}

pub(in crate::app) fn point_coordinates(ui: &mut egui::Ui, point: egui::Pos2, indent: f32) {
    ui.horizontal(|ui| {
        ui.add_space(indent);
        ui.label(
            egui::RichText::new(format!("@ ({:.1},{:.1})", point.x, point.y))
                .small()
                .weak(),
        );
    });
}

/// Draw a side-panel section as a card.
fn side_section_card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::group(ui.style())
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(style::CARD_MARGIN)
        .show(ui, |ui| {
            ui.take_available_width();
            add_contents(ui)
        })
}

/// Draw a side-panel card with a collapsible heading.
pub(super) fn side_section_card_collapsible(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    title: impl Into<egui::WidgetText>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    side_section_card(ui, |ui| {
        ui.scope(|ui| {
            ui.spacing_mut().indent += style::COLLAPSING_TEXT_OFFSET;
            egui::CollapsingHeader::new(title.into().heading())
                .id_salt(id_salt)
                .default_open(true)
                .icon(|ui, openness, response| {
                    let enlarged_rect = egui::Rect::from_center_size(
                        response.rect.center(),
                        response.rect.size() * style::COLLAPSING_ICON_SCALE,
                    )
                    .translate(egui::vec2(-0.5 * style::COLLAPSING_TEXT_OFFSET, 0.0));
                    let enlarged_response = response.clone().with_new_rect(enlarged_rect);
                    egui::containers::collapsing_header::paint_default_icon(
                        ui,
                        openness,
                        &enlarged_response,
                    );
                })
                .show_unindented(ui, add_contents);
        });
    });
}

impl CurcatApp {
    /// Compute a pulsing highlight color based on the UI time.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn attention_color(ctx: &egui::Context, base: Color32) -> Color32 {
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

    /// Paint a blinking outline when an element needs attention.
    pub(crate) fn paint_attention_outline_if(&self, ui: &egui::Ui, rect: egui::Rect, active: bool) {
        if !active || !ui.is_rect_visible(rect) {
            return;
        }
        let mut stroke = self.config.attention_highlight.stroke();
        stroke.color = Self::attention_color(ui.ctx(), stroke.color);
        ui.painter().rect_stroke(
            rect.expand(super::super::ATTENTION_OUTLINE_PAD),
            CornerRadius::ZERO,
            stroke,
            StrokeKind::Outside,
        );
    }
}

/// Return true when calibration inputs are missing or invalid.
pub(in crate::app) fn axis_needs_attention(cal: &AxisCalUi) -> bool {
    let (v1_invalid, v2_invalid) = cal.value_invalid_flags();
    v1_invalid || v2_invalid || cal.p1.is_none() || cal.p2.is_none()
}
