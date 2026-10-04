use egui::{Color32, CornerRadius, Response, Stroke, StrokeKind, Ui, Vec2, Widget};

use crate::app::ui::style;

/// A selectable color sample with the same activation behavior as other egui controls.
struct ColorSwatch {
    color: Color32,
    selected: bool,
    size: f32,
}

impl Widget for ColorSwatch {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::splat(self.size), egui::Sense::click());
        let selected = self.selected || response.clicked();
        if response.clicked() && !self.selected {
            response.mark_changed();
        }
        response.widget_info(|| {
            let [r, g, b, _] = self.color.to_array();
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                response.enabled(),
                selected,
                format!("RGB {r}, {g}, {b}"),
            )
        });
        if ui.is_rect_visible(rect) {
            let stroke = if selected {
                Stroke::new(2.0, ui.visuals().text_color())
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke
            };
            let rounding = CornerRadius::same(4);
            ui.painter().rect_filled(rect, rounding, self.color);
            ui.painter()
                .rect_stroke(rect, rounding, stroke, StrokeKind::Outside);
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.expand(3.0),
                    rounding,
                    Stroke::new(1.0, ui.visuals().selection.bg_fill),
                    StrokeKind::Outside,
                );
            }
        }
        response.on_hover_ui(|ui| {
            let [r, g, b, _] = self.color.to_array();
            ui.label(format!("RGB {r}, {g}, {b}"));
        })
    }
}

/// Reports both index and color so callers do not need to index the palette again.
#[must_use = "Show the palette with show()."]
pub(in crate::app) struct ColorPalette<'a> {
    colors: &'a [Color32],
    selected: usize,
    size: f32,
}

impl<'a> ColorPalette<'a> {
    pub(in crate::app) const fn new(colors: &'a [Color32], selected: usize, size: f32) -> Self {
        Self {
            colors,
            selected,
            size,
        }
    }

    pub(in crate::app) fn show(self, ui: &mut Ui) -> egui::InnerResponse<Option<(usize, Color32)>> {
        let mut result = ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = style::SPACE_ROW;
            let mut choice = None;
            for (index, color) in self.colors.iter().copied().enumerate() {
                let response = ui.add(ColorSwatch {
                    color,
                    selected: self.selected == index,
                    size: self.size,
                });
                if response.changed() {
                    choice = Some((index, color));
                }
            }
            choice
        });
        if result.inner.is_some() {
            result.response.mark_changed();
        }
        result
    }
}
