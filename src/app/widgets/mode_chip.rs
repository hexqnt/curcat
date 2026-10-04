use egui::{Color32, CornerRadius, FontId, Margin, Response, RichText, Stroke, Ui, Widget};

use crate::app::ui::style;

#[must_use = "Add the mode chip to a UI with ui.add()."]
pub(in crate::app) struct ModeChip<'a> {
    label: &'a str,
    color: Color32,
    font: &'a FontId,
}

impl<'a> ModeChip<'a> {
    pub(in crate::app) const fn new(label: &'a str, color: Color32, font: &'a FontId) -> Self {
        Self { label, color, font }
    }
}

impl Widget for ModeChip<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let [r, g, b, _] = self.color.to_array();
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(r, g, b, 44))
            .stroke(Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(r, g, b, 150),
            ))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(7, 3))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(self.label)
                        .font(self.font.clone())
                        .color(style::mode_text_color(self.color, ui.visuals())),
                );
            })
            .response
    }
}
