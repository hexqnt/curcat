use super::super::CurcatApp;
use super::common::toggle_switch;
use super::icons;
use crate::i18n::{TextKey, UiLanguage};
use egui::containers::menu::MenuButton;

impl CurcatApp {
    pub(crate) fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let has_image = self.image.image.is_some();
            let can_save_project = self.image.meta.as_ref().and_then(|m| m.path()).is_some();
            let file_menu_response = self.ui_file_menu(ui, can_save_project);
            self.paint_attention_outline_if(
                ui,
                file_menu_response.rect,
                self.image.image.is_none(),
            );
            ui.separator();

            self.ui_side_toggle(ui);
            ui.separator();

            self.ui_appearance_menu(ui, has_image);
            self.ui_transform_buttons(ui, has_image);

            let has_points = !self.points.points.is_empty();
            self.ui_zoom_controls(ui);
            ui.separator();

            self.ui_middle_pan_toggle(ui);
            ui.separator();

            self.ui_point_edit_buttons(ui, has_points);
        });
    }

    fn flag_image(lang: UiLanguage, size: egui::Vec2) -> egui::Image<'static> {
        let source = match lang {
            UiLanguage::En => egui::include_image!("../../../assets/flags/us.svg"),
            UiLanguage::Ru => egui::include_image!("../../../assets/flags/ru.svg"),
        };
        egui::Image::new(source).fit_to_exact_size(size)
    }

    pub(super) fn ui_language_selector(&mut self, ui: &mut egui::Ui) {
        let button =
            egui::Button::image(Self::flag_image(self.ui.language, egui::vec2(18.0, 12.0)))
                .min_size(egui::vec2(24.0, 20.0));
        let (response, _) = MenuButton::from_button(button).ui(ui, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 6.0;
                for lang in UiLanguage::ALL {
                    let selected = lang == self.ui.language;
                    let button =
                        egui::Button::image(Self::flag_image(lang, egui::vec2(22.0, 14.0)))
                            .frame(selected)
                            .min_size(egui::vec2(28.0, 20.0));
                    if ui.add(button).clicked() {
                        self.set_ui_language(lang);
                        ui.close();
                    }
                }
            });
        });
        response.on_hover_text(self.t(TextKey::LanguageSwitcherHover));
    }

    fn ui_file_menu(&mut self, ui: &mut egui::Ui, can_save_project: bool) -> egui::Response {
        let file_menu = ui.menu_button(
            format!("{} {}", icons::ICON_MENU, self.t(TextKey::File)),
            |ui| {
                if ui
                    .add(egui::Button::new(self.t(TextKey::OpenImage)).shortcut_text("Ctrl+O"))
                    .on_hover_text(self.t(TextKey::OpenImageHover))
                    .clicked()
                {
                    self.open_image_dialog();
                    ui.close();
                }

                if ui
                    .add(egui::Button::new(self.t(TextKey::PasteImage)).shortcut_text("Ctrl+V"))
                    .on_hover_text(self.t(TextKey::PasteImageHover))
                    .clicked()
                {
                    self.paste_image_from_clipboard(ui.ctx());
                    ui.close();
                }

                ui.separator();

                if ui
                    .add(
                        egui::Button::new(self.t(TextKey::LoadProject))
                            .shortcut_text("Ctrl+Shift+P"),
                    )
                    .on_hover_text(self.t(TextKey::LoadProjectHover))
                    .clicked()
                {
                    self.open_project_dialog();
                    ui.close();
                }

                if ui
                    .add_enabled(
                        can_save_project,
                        egui::Button::new(self.t(TextKey::SaveProject)).shortcut_text("Ctrl+S"),
                    )
                    .on_hover_text(self.t(TextKey::SaveProjectHover))
                    .clicked()
                {
                    self.save_project_dialog();
                    ui.close();
                }
            },
        );
        file_menu.response
    }

    fn ui_side_toggle(&mut self, ui: &mut egui::Ui) {
        let side_label = if self.ui.side_open {
            self.t(TextKey::HideSide)
        } else {
            self.t(TextKey::ShowSide)
        };
        let button = egui::Button::new(format!("{} {side_label}", icons::ICON_SIDE_TOGGLE))
            .shortcut_text("Ctrl+B");
        let (response, _) = MenuButton::from_button(button).ui(ui, |ui| {
            let toggle_label = if self.ui.side_open {
                self.t(TextKey::HideSidePanel)
            } else {
                self.t(TextKey::ShowSidePanel)
            };
            if ui.button(toggle_label).clicked() {
                self.ui.side_open = !self.ui.side_open;
                ui.close();
            }
            ui.separator();
            ui.label(self.t(TextKey::SidePanelPosition));
            let left_selected = self.ui.side_position == super::super::SidePanelPosition::Left;
            if ui
                .selectable_label(left_selected, self.t(TextKey::Left))
                .clicked()
            {
                self.ui.side_position = super::super::SidePanelPosition::Left;
                ui.close();
            }
            if ui
                .selectable_label(!left_selected, self.t(TextKey::Right))
                .clicked()
            {
                self.ui.side_position = super::super::SidePanelPosition::Right;
                ui.close();
            }
        });
        response.on_hover_text(self.t(TextKey::ToggleSidePanelHover));
    }

    fn ui_appearance_menu(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let button = egui::Button::new(format!(
            "{} {}",
            icons::ICON_MENU,
            self.t(TextKey::Appearance)
        ));
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let _ = MenuButton::from_button(button)
            .config(menu_cfg)
            .ui(ui, |ui| {
                let points_label =
                    format!("{} {}", icons::ICON_STATS, self.t(TextKey::PointsStats));
                let points_hover = self.t(TextKey::PointsStatsHover);
                let filters_label = format!("{} {}", icons::ICON_FILTERS, self.t(TextKey::Filters));
                let filters_hover = self.t(TextKey::FiltersHover);
                let trace_label =
                    format!("{} {}", icons::ICON_AUTO_TRACE, self.t(TextKey::AutoTrace));
                let trace_hover = self.t(TextKey::AutoTraceHover);
                let info_label = format!("{} {}", icons::ICON_INFO, self.t(TextKey::ImageInfo));
                let info_hover = self.t(TextKey::ImageInfoHover);

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.points_info_window_open,
                    points_label,
                    points_hover,
                );

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.image_filters_window_open,
                    filters_label,
                    filters_hover,
                );

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.auto_trace_window_open,
                    trace_label,
                    trace_hover,
                );

                ui.add_enabled_ui(has_image || self.ui.info_window_open, |ui| {
                    Self::ui_toggle_menu_item(
                        ui,
                        &mut self.ui.info_window_open,
                        info_label,
                        info_hover,
                    );
                });
            });
    }

    fn ui_toggle_menu_item(ui: &mut egui::Ui, state: &mut bool, label: String, hover: &str) {
        ui.horizontal(|ui| {
            let _toggle_resp = toggle_switch(ui, state).on_hover_text(hover);
            ui.add_space(4.0);
            let label_resp = ui
                .add(egui::Label::new(label).sense(egui::Sense::click()))
                .on_hover_text(hover);
            if label_resp.clicked() {
                *state = !*state;
            }
        });
    }

    fn ui_transform_buttons(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let info_hover = |ui: &mut egui::Ui, action: &str, title: &str| {
            ui.label(title);
            ui.label(action);
        };
        let info_button = |ui: &mut egui::Ui, label: String, action: &str, title: &str| {
            ui.add_enabled(has_image, egui::Button::new(label))
                .on_hover_ui(|ui| info_hover(ui, action, title))
        };

        if info_button(
            ui,
            format!("{} 90°", icons::ICON_ROTATE_CCW),
            self.t(TextKey::Rotate90Ccw),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.rotate_image(false);
        }
        if info_button(
            ui,
            format!("{} 90°", icons::ICON_ROTATE_CW),
            self.t(TextKey::Rotate90Cw),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.rotate_image(true);
        }
        if info_button(
            ui,
            format!("{} {}", icons::ICON_FLIP_H, self.t(TextKey::FlipH)),
            self.t(TextKey::FlipHorizontally),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.flip_image(true);
        }
        if info_button(
            ui,
            format!("{} {}", icons::ICON_FLIP_V, self.t(TextKey::FlipV)),
            self.t(TextKey::FlipVertically),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.flip_image(false);
        }
    }

    fn ui_zoom_controls(&mut self, ui: &mut egui::Ui) {
        ui.label(self.t(TextKey::Zoom))
            .on_hover_text(self.t(TextKey::ZoomHover));
        let zoom_ir = egui::ComboBox::from_id_salt("zoom_combo")
            .selected_text(Self::format_zoom(self.image.zoom))
            .show_ui(ui, |ui| {
                if ui
                    .add(
                        egui::Button::new(format!("{} {}", icons::ICON_FIT, self.t(TextKey::Fit)))
                            .shortcut_text("Ctrl+F"),
                    )
                    .on_hover_text(self.t(TextKey::FitHover))
                    .clicked()
                {
                    self.fit_image_to_viewport();
                    ui.close();
                }
                if ui
                    .add(
                        egui::Button::new(format!(
                            "{} {}",
                            icons::ICON_RESET_VIEW,
                            self.t(TextKey::ResetView)
                        ))
                        .shortcut_text("Ctrl+R"),
                    )
                    .on_hover_text(self.t(TextKey::ResetViewHover))
                    .clicked()
                {
                    self.reset_view();
                    ui.close();
                }
                ui.separator();
                for &preset in super::super::ZOOM_PRESETS {
                    let label = Self::format_zoom(preset);
                    let selected = (self.image.zoom - preset).abs() < 0.0001;
                    if ui.selectable_label(selected, label).clicked() {
                        self.set_zoom_about_viewport_center(preset);
                    }
                }
            });
        zoom_ir
            .response
            .on_hover_text(self.t(TextKey::ZoomPresetsHover));
    }

    fn ui_middle_pan_toggle(&mut self, ui: &mut egui::Ui) {
        let toggle_response = toggle_switch(ui, &mut self.interaction.middle_pan_enabled)
            .on_hover_text(self.t(TextKey::PanWithMiddleButton));
        ui.add_space(4.0);
        ui.label(self.t(TextKey::MmbPan))
            .on_hover_text(self.t(TextKey::MmbPanHover));
        if toggle_response.changed() && !self.interaction.middle_pan_enabled {
            self.image.touch_pan_active = false;
            self.image.touch_pan_last = None;
        }
    }

    fn ui_point_edit_buttons(&mut self, ui: &mut egui::Ui, has_points: bool) {
        let resp_clear = ui
            .add_enabled(
                has_points,
                egui::Button::new(format!(
                    "{} {}",
                    icons::ICON_CLEAR,
                    self.t(TextKey::ClearPoints)
                ))
                .shortcut_text("Ctrl+Shift+D"),
            )
            .on_hover_text(self.t(TextKey::ClearPointsHover));
        if resp_clear.clicked() {
            self.clear_all_points();
        }
        let resp_undo = ui
            .add_enabled(
                has_points,
                egui::Button::new(format!("{} {}", icons::ICON_UNDO, self.t(TextKey::Undo)))
                    .shortcut_text("Ctrl+Z"),
            )
            .on_hover_text(self.t(TextKey::UndoHover));
        if resp_undo.clicked() {
            self.undo_last_point();
        }
    }
}
