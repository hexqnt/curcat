use super::super::CurcatApp;
use super::{common, icons, style};
use crate::app::widgets::{ActionButton, LanguageSelector, ToggleRow, ZoomAction, ZoomSelector};
use crate::i18n::{TextKey, UiLanguage};

impl CurcatApp {
    pub(crate) fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.add_space(style::SPACE_TIGHT);
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = style::SPACE_ROW;
            ui.add_space(style::SPACE_TIGHT);
            let has_image = self.image.image.is_some();
            let can_save_project = self.image.meta.as_ref().and_then(|m| m.path()).is_some();
            let file_menu_response = self.ui_file_menu(ui, can_save_project);
            self.paint_attention_outline_if(
                ui,
                file_menu_response.rect,
                self.image.image.is_none(),
            );
            common::bar_separator(ui);

            self.ui_side_toggle(ui);
            common::bar_separator(ui);

            self.ui_appearance_menu(ui, has_image);
            self.ui_transform_buttons(ui, has_image);

            let has_points = !self.points.points.is_empty();
            self.ui_zoom_controls(ui);
            common::bar_separator(ui);

            self.ui_middle_pan_toggle(ui);
            common::bar_separator(ui);

            self.ui_point_edit_buttons(ui, has_points);
            ui.add_space(style::SPACE_TIGHT);
        });
        ui.add_space(1.0);
    }

    pub(super) fn ui_language_selector(&mut self, ui: &mut egui::Ui) {
        let mut language = self.ui.language;
        if ui
            .add(LanguageSelector::new(
                &mut language,
                self.t(TextKey::LanguageSwitcherHover),
            ))
            .changed()
        {
            self.set_ui_language(language);
        }
    }

    fn ui_file_menu(&mut self, ui: &mut egui::Ui, can_save_project: bool) -> egui::Response {
        let button = ActionButton::new(icons::ICON_MENU, self.t(TextKey::File));
        let menu = button.show_menu(ui, |ui| {
            if ui
                .add(
                    ActionButton::new(icons::ICON_OPEN_IMAGE, self.t(TextKey::OpenImage))
                        .shortcut_text("Ctrl+O"),
                )
                .on_hover_text(self.t(TextKey::OpenImageHover))
                .clicked()
            {
                self.open_image_dialog();
                ui.close();
            }

            if ui
                .add(
                    ActionButton::new(icons::ICON_PASTE_IMAGE, self.t(TextKey::PasteImage))
                        .shortcut_text("Ctrl+V"),
                )
                .on_hover_text(self.t(TextKey::PasteImageHover))
                .clicked()
            {
                self.paste_image_from_clipboard(ui.ctx());
                ui.close();
            }

            ui.separator();

            if ui
                .add(
                    ActionButton::new(icons::ICON_LOAD_PROJECT, self.t(TextKey::LoadProject))
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
                    ActionButton::new(icons::ICON_SAVE_PROJECT, self.t(TextKey::SaveProject))
                        .shortcut_text("Ctrl+S"),
                )
                .on_hover_text(self.t(TextKey::SaveProjectHover))
                .clicked()
            {
                self.save_project_dialog();
                ui.close();
            }
        });
        menu.response
    }

    fn ui_side_toggle(&mut self, ui: &mut egui::Ui) {
        let side_label = if self.ui.side_open {
            self.t(TextKey::HideSide)
        } else {
            self.t(TextKey::ShowSide)
        };
        let button = ActionButton::new(icons::ICON_SIDE_TOGGLE, side_label).shortcut_text("Ctrl+B");
        let menu = button.show_menu(ui, |ui| {
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
        menu.response
            .on_hover_text(self.t(TextKey::ToggleSidePanelHover));
    }

    fn ui_appearance_menu(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let button = ActionButton::new(icons::ICON_MENU, self.t(TextKey::Appearance));
        button
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_menu(ui, |ui| {
                let points_label = self.t(TextKey::PointsStats);
                let points_hover = self.t(TextKey::PointsStatsHover);
                let filters_label = self.t(TextKey::Filters);
                let filters_hover = self.t(TextKey::FiltersHover);
                let trace_label = self.t(TextKey::AutoTrace);
                let trace_hover = self.t(TextKey::AutoTraceHover);
                let info_label = self.t(TextKey::ImageInfo);
                let info_hover = self.t(TextKey::ImageInfoHover);

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.points_info_window_open,
                    icons::ICON_STATS,
                    points_label,
                    points_hover,
                );

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.image_filters_window_open,
                    icons::ICON_FILTERS,
                    filters_label,
                    filters_hover,
                );

                Self::ui_toggle_menu_item(
                    ui,
                    &mut self.ui.auto_trace_window_open,
                    icons::ICON_AUTO_TRACE,
                    trace_label,
                    trace_hover,
                );

                ui.add_enabled_ui(has_image || self.ui.info_window_open, |ui| {
                    Self::ui_toggle_menu_item(
                        ui,
                        &mut self.ui.info_window_open,
                        icons::ICON_INFO,
                        info_label,
                        info_hover,
                    );
                });
            });
    }

    fn ui_toggle_menu_item(
        ui: &mut egui::Ui,
        state: &mut bool,
        icon: icons::Icon,
        label: &str,
        hover: &str,
    ) {
        ui.add(
            ToggleRow::new(state, label, hover)
                .icon(icon)
                .clickable_caption(),
        );
    }

    fn ui_transform_buttons(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let info_hover = |ui: &mut egui::Ui, action: &str, title: &str| {
            ui.label(title);
            ui.label(action);
        };
        let info_button =
            |ui: &mut egui::Ui, icon: icons::Icon, label: &str, action: &str, title: &str| {
                ui.add_enabled(
                    has_image,
                    ActionButton::new(icon, label).accessible_label(action),
                )
                .on_hover_ui(|ui| info_hover(ui, action, title))
            };

        if info_button(
            ui,
            icons::ICON_ROTATE_CCW,
            "90°",
            self.t(TextKey::Rotate90Ccw),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.rotate_image(false);
        }
        if info_button(
            ui,
            icons::ICON_ROTATE_CW,
            "90°",
            self.t(TextKey::Rotate90Cw),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.rotate_image(true);
        }
        if info_button(
            ui,
            icons::ICON_FLIP_H,
            self.t(TextKey::FlipH),
            self.t(TextKey::FlipHorizontally),
            self.t(TextKey::TransformsTogether),
        )
        .clicked()
        {
            self.flip_image(true);
        }
        if info_button(
            ui,
            icons::ICON_FLIP_V,
            self.t(TextKey::FlipV),
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
        match ZoomSelector::new(self.image.zoom, self.i18n())
            .show(ui)
            .inner
        {
            Some(ZoomAction::Fit) => self.fit_image_to_viewport(),
            Some(ZoomAction::Reset) => self.reset_view(),
            Some(ZoomAction::Preset(zoom)) => self.set_zoom_about_viewport_center(zoom),
            None => {}
        }
    }

    fn ui_middle_pan_toggle(&mut self, ui: &mut egui::Ui) {
        let label = self.t(TextKey::MmbPan);
        let caption_hover = self.t(TextKey::MmbPanHover);
        let hover = self.t(TextKey::PanWithMiddleButton);
        let toggle_response = ui.add(
            ToggleRow::new(&mut self.interaction.middle_pan_enabled, label, hover)
                .caption_hover(caption_hover),
        );
        if toggle_response.changed() && !self.interaction.middle_pan_enabled {
            self.image.touch_pan_active = false;
            self.image.touch_pan_last = None;
        }
    }

    fn ui_point_edit_buttons(&mut self, ui: &mut egui::Ui, has_points: bool) {
        let button_height = ui.spacing().interact_size.y;
        let action_width = match self.ui.language {
            UiLanguage::En => 162.0,
            UiLanguage::Ru => 198.0,
        };
        let resp_clear = ui
            .add_enabled(
                has_points,
                ActionButton::new(icons::ICON_CLEAR, self.t(TextKey::ClearPoints))
                    .shortcut_text("Ctrl+Shift+D")
                    .min_size(egui::vec2(action_width, button_height)),
            )
            .on_hover_text(self.t(TextKey::ClearPointsHover));
        if resp_clear.clicked() {
            self.clear_all_points();
        }
        ui.add_space(style::SPACE_SMALL);
        let resp_undo = ui
            .add_enabled(
                has_points,
                ActionButton::new(icons::ICON_UNDO, self.t(TextKey::Undo))
                    .shortcut_text("Ctrl+Z")
                    .min_size(egui::vec2(action_width, button_height)),
            )
            .on_hover_text(self.t(TextKey::UndoHover));
        if resp_undo.clicked() {
            self.undo_last_point();
        }
    }
}
