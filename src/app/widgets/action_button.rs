use egui::{Button, Image, Response, Ui, Vec2, Widget, WidgetInfo, WidgetType};

use crate::app::ui::icons::{self, Icon};

/// Shares egui's button and menu behavior, including keyboard activation.
#[must_use = "Add the button to a UI or show its menu."]
pub(in crate::app) struct ActionButton<'a> {
    button: Button<'a>,
    accessible_label: Option<&'a str>,
    menu_close_behavior: Option<egui::PopupCloseBehavior>,
}

impl<'a> ActionButton<'a> {
    pub(in crate::app) fn new(icon: Icon, text: impl Into<egui::WidgetText>) -> Self {
        Self::from_button(
            Button::image_and_text(icons::image(icon, icons::BUTTON_ICON_SIZE), text)
                .image_tint_follows_text_color(true),
        )
    }

    pub(in crate::app) fn image(image: Image<'a>, label: &'a str) -> Self {
        Self::from_button(Button::image(image)).accessible_label(label)
    }

    pub(in crate::app) fn icon_only(icon: Icon, label: &'a str) -> Self {
        Self::image(icons::image(icon, icons::BUTTON_ICON_SIZE), label).tint_follows_text()
    }

    const fn from_button(button: Button<'a>) -> Self {
        Self {
            button,
            accessible_label: None,
            menu_close_behavior: None,
        }
    }

    pub(in crate::app) const fn accessible_label(mut self, label: &'a str) -> Self {
        self.accessible_label = Some(label);
        self
    }

    pub(in crate::app) fn shortcut_text(mut self, text: impl egui::IntoAtoms<'a>) -> Self {
        self.button = self.button.shortcut_text(text);
        self
    }

    pub(in crate::app) fn min_size(mut self, size: Vec2) -> Self {
        self.button = self.button.min_size(size);
        self
    }

    pub(in crate::app) fn frame(mut self, frame: bool) -> Self {
        self.button = self.button.frame(frame);
        self
    }

    pub(in crate::app) fn fill(mut self, color: egui::Color32) -> Self {
        self.button = self.button.fill(color);
        self
    }

    pub(in crate::app) fn stroke(mut self, stroke: egui::Stroke) -> Self {
        self.button = self.button.stroke(stroke);
        self
    }

    pub(in crate::app) fn tint_follows_text(mut self) -> Self {
        self.button = self.button.image_tint_follows_text_color(true);
        self
    }

    pub(in crate::app) const fn close_behavior(
        mut self,
        behavior: egui::PopupCloseBehavior,
    ) -> Self {
        self.menu_close_behavior = Some(behavior);
        self
    }

    pub(in crate::app) fn show_menu<R>(
        self,
        ui: &mut Ui,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let mut menu = egui::containers::menu::MenuButton::from_button(self.button);
        if let Some(behavior) = self.menu_close_behavior {
            menu = menu.config(egui::containers::menu::MenuConfig::new().close_behavior(behavior));
        }
        let (response, inner) = menu.ui(ui, contents);
        egui::InnerResponse {
            response: label_response(response, self.accessible_label),
            inner: inner.map(|inner| inner.inner),
        }
    }
}

fn label_response(response: Response, label: Option<&str>) -> Response {
    if let Some(label) = label {
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, response.enabled(), label));
    }
    response
}

impl Widget for ActionButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        label_response(ui.add(self.button), self.accessible_label)
    }
}
