use egui::{Response, Ui, Vec2, Widget};

use super::ActionButton;
use crate::{app::ui::style, i18n::UiLanguage};

#[must_use = "Add the language selector to a UI with ui.add()."]
pub(in crate::app) struct LanguageSelector<'a> {
    selected: &'a mut UiLanguage,
    label: &'a str,
}

impl<'a> LanguageSelector<'a> {
    pub(in crate::app) const fn new(selected: &'a mut UiLanguage, label: &'a str) -> Self {
        Self { selected, label }
    }
}

fn flag_image(language: UiLanguage, size: Vec2) -> egui::Image<'static> {
    let source = match language {
        UiLanguage::En => egui::include_image!("../../../assets/flags/us.svg"),
        UiLanguage::Ru => egui::include_image!("../../../assets/flags/ru.svg"),
    };
    egui::Image::new(source).fit_to_exact_size(size)
}

impl Widget for LanguageSelector<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let before = *self.selected;
        let mut response =
            ActionButton::image(flag_image(before, egui::vec2(18.0, 12.0)), self.label)
                .min_size(egui::vec2(26.0, 22.0))
                .show_menu(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = style::SPACE_ROW;
                        for language in UiLanguage::ALL {
                            let selected = language == *self.selected;
                            let label = match language {
                                UiLanguage::En => "English",
                                UiLanguage::Ru => "Русский",
                            };
                            let response = ui.add(
                                ActionButton::image(
                                    flag_image(language, egui::vec2(22.0, 14.0)),
                                    label,
                                )
                                .frame(selected)
                                .min_size(egui::vec2(30.0, 22.0)),
                            );
                            if response.clicked() {
                                *self.selected = language;
                                ui.close();
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::RadioButton,
                                    response.enabled(),
                                    language == *self.selected,
                                    label,
                                )
                            });
                        }
                    });
                })
                .response
                .on_hover_text(self.label);
        if before != *self.selected {
            response.mark_changed();
        }
        response
    }
}
