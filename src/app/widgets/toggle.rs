use egui::{CornerRadius, Response, StrokeKind, Ui, Widget, WidgetText, pos2};

use crate::app::ui::{icons, style};
use crate::util::rounded_u8;

/// Combines a switch with its caption without adding a second checkbox node.
#[must_use = "Add the toggle row to a UI with ui.add()."]
pub(in crate::app) struct ToggleRow<'a> {
    on: &'a mut bool,
    label: &'a str,
    caption: WidgetText,
    hover: &'a str,
    caption_hover: &'a str,
    clickable_caption: bool,
    icon: Option<icons::Icon>,
    gap: f32,
}

impl<'a> ToggleRow<'a> {
    pub(in crate::app) fn new(on: &'a mut bool, label: &'a str, hover: &'a str) -> Self {
        Self {
            on,
            label,
            caption: label.into(),
            hover,
            caption_hover: hover,
            clickable_caption: false,
            icon: None,
            gap: style::SPACE_SMALL,
        }
    }

    pub(in crate::app) fn caption(mut self, caption: impl Into<WidgetText>) -> Self {
        self.caption = caption.into();
        self
    }

    pub(in crate::app) const fn clickable_caption(mut self) -> Self {
        self.clickable_caption = true;
        self
    }

    pub(in crate::app) const fn icon(mut self, icon: icons::Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(in crate::app) const fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    pub(in crate::app) const fn caption_hover(mut self, hover: &'a str) -> Self {
        self.caption_hover = hover;
        self
    }
}

impl Widget for ToggleRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            let response = ui
                .add(ToggleSwitch::new(self.on, self.label))
                .on_hover_text(self.hover);
            ui.add_space(self.gap);
            if let Some(icon) = self.icon {
                ui.add(icons::image(icon, icons::INLINE_ICON_SIZE).tint(ui.visuals().text_color()))
                    .on_hover_text(self.hover);
                ui.add_space(style::SPACE_TIGHT);
            }
            let sense = if self.clickable_caption {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let caption = ui
                .add(egui::Label::new(self.caption).sense(sense))
                .on_hover_text(self.caption_hover);
            // Retain the switch's hit rectangle and identity for focus and accessibility.
            let mut response = response.labelled_by(caption.id);
            if self.clickable_caption && caption.clicked() {
                *self.on = !*self.on;
                response.mark_changed();
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Checkbox,
                        response.enabled(),
                        *self.on,
                        self.label,
                    )
                });
            }
            response
        })
        .inner
    }
}

/// A compact switch that borrows its state for one frame.
#[must_use = "Add the switch to a UI with ui.add()."]
struct ToggleSwitch<'a> {
    on: &'a mut bool,
    label: &'a str,
}

impl<'a> ToggleSwitch<'a> {
    const fn new(on: &'a mut bool, label: &'a str) -> Self {
        Self { on, label }
    }
}

impl egui::Widget for ToggleSwitch<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { on, label } = self;
        let desired_size = egui::vec2(
            ui.spacing().interact_size.y * 1.8,
            ui.spacing().interact_size.y,
        );
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, label)
        });

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
            let knob_x = if *on {
                rect.right() - radius
            } else {
                rect.left() + radius
            };
            let knob_center = pos2(knob_x, rect.center().y);
            ui.painter()
                .circle_filled(knob_center, knob_radius, visuals.fg_stroke.color);
        }

        response
    }
}
