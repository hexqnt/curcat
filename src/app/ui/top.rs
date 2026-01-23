use super::super::CurcatApp;
use super::common::toggle_switch;
use super::icons;

impl CurcatApp {
    pub(crate) fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Use egui's built-in theme toggle so icon matches current mode.
            egui::widgets::global_theme_preference_switch(ui);
            ui.separator();

            let has_image = self.image.is_some();
            let can_save_project = self.image_meta.as_ref().and_then(|m| m.path()).is_some();
            let file_menu_response = self.ui_file_menu(ui, can_save_project);
            self.paint_attention_outline_if(ui, file_menu_response.rect, self.image.is_none());
            ui.separator();

            self.ui_side_toggle(ui);
            ui.separator();

            self.ui_stats_info_buttons(ui, has_image);
            self.ui_transform_buttons(ui, has_image);

            let has_points = !self.points.is_empty();
            self.ui_zoom_controls(ui);
            ui.separator();

            self.ui_middle_pan_toggle(ui);
            ui.separator();

            self.ui_point_edit_buttons(ui, has_points);
        });
    }

    fn ui_file_menu(&mut self, ui: &mut egui::Ui, can_save_project: bool) -> egui::Response {
        let file_menu = ui.menu_button(format!("{} File", icons::ICON_MENU), |ui| {
            if ui
                .add(egui::Button::new("Open image…").shortcut_text("Ctrl+O"))
                .on_hover_text("Open an image (Ctrl+O). You can also drag & drop into the center.")
                .clicked()
            {
                self.open_image_dialog();
                ui.close();
            }

            if ui
                .add(egui::Button::new("Paste image").shortcut_text("Ctrl+V"))
                .on_hover_text("Paste image from clipboard (Ctrl+V)")
                .clicked()
            {
                self.paste_image_from_clipboard(ui.ctx());
                ui.close();
            }

            ui.separator();

            if ui
                .add(egui::Button::new("Load project…").shortcut_text("Ctrl+Shift+P"))
                .on_hover_text("Load a saved Curcat project (Ctrl+Shift+P)")
                .clicked()
            {
                self.open_project_dialog();
                ui.close();
            }

            if ui
                .add_enabled(
                    can_save_project,
                    egui::Button::new("Save project").shortcut_text("Ctrl+S"),
                )
                .on_hover_text("Save the current session as a Curcat project (Ctrl+S)")
                .clicked()
            {
                self.save_project_dialog();
                ui.close();
            }
        });
        file_menu.response
    }

    fn ui_side_toggle(&mut self, ui: &mut egui::Ui) {
        let side_label = if self.side_open {
            "Hide side"
        } else {
            "Show side"
        };
        if ui
            .add(
                egui::Button::new(format!("{} {side_label}", icons::ICON_SIDE_TOGGLE))
                    .shortcut_text("Ctrl+B"),
            )
            .on_hover_text("Toggle side panel (Ctrl+B)")
            .clicked()
        {
            self.side_open = !self.side_open;
        }
    }

    fn ui_stats_info_buttons(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let stats_resp = ui
            .add(egui::Button::new(format!(
                "{} Points stats",
                icons::ICON_STATS
            )))
            .on_hover_text("Show stats for picked points");
        if stats_resp.clicked() {
            self.points_info_window_open = true;
        }
        let info_resp = ui
            .add_enabled(
                has_image,
                egui::Button::new(format!("{} Image info", icons::ICON_INFO))
                    .shortcut_text("Ctrl+I"),
            )
            .on_hover_text("Show file & image details (Ctrl+I)");
        if info_resp.clicked() && has_image {
            self.info_window_open = true;
        }
    }

    fn ui_transform_buttons(&mut self, ui: &mut egui::Ui, has_image: bool) {
        let warn_hover = |ui: &mut egui::Ui, action: &str| {
            ui.colored_label(
                egui::Color32::from_rgb(220, 80, 80),
                format!("{} Clears ALL points and calibration", icons::ICON_WARN),
            );
            ui.label(action);
        };
        let warn_button = |ui: &mut egui::Ui, label: String, action: &str| {
            ui.add_enabled(has_image, egui::Button::new(label))
                .on_hover_ui(|ui| warn_hover(ui, action))
        };

        if warn_button(
            ui,
            format!("{} 90°", icons::ICON_ROTATE_CCW),
            "Rotate 90° counter-clockwise.",
        )
        .clicked()
        {
            self.rotate_image(false);
        }
        if warn_button(
            ui,
            format!("{} 90°", icons::ICON_ROTATE_CW),
            "Rotate 90° clockwise.",
        )
        .clicked()
        {
            self.rotate_image(true);
        }
        if warn_button(
            ui,
            format!("{} Flip H", icons::ICON_FLIP_H),
            "Flip horizontally.",
        )
        .clicked()
        {
            self.flip_image(true);
        }
        if warn_button(
            ui,
            format!("{} Flip V", icons::ICON_FLIP_V),
            "Flip vertically.",
        )
        .clicked()
        {
            self.flip_image(false);
        }
    }

    fn ui_zoom_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Zoom:")
            .on_hover_text("Choose a preset zoom level");
        let zoom_ir = egui::ComboBox::from_id_salt("zoom_combo")
            .selected_text(Self::format_zoom(self.image_zoom))
            .show_ui(ui, |ui| {
                for &preset in super::super::ZOOM_PRESETS {
                    let label = Self::format_zoom(preset);
                    let selected = (self.image_zoom - preset).abs() < 0.0001;
                    if ui.selectable_label(selected, label).clicked() {
                        self.set_zoom_about_viewport_center(preset);
                    }
                }
            });
        zoom_ir.response.on_hover_text("Zoom presets (percent)");
        if ui
            .add(egui::Button::new(format!("{} Fit", icons::ICON_FIT)).shortcut_text("Ctrl+F"))
            .on_hover_text("Fit the image into the viewport (Ctrl+F)")
            .clicked()
        {
            self.fit_image_to_viewport();
        }
        if ui
            .add(
                egui::Button::new(format!("{} Reset view", icons::ICON_RESET_VIEW))
                    .shortcut_text("Ctrl+R"),
            )
            .on_hover_text("Reset zoom to 100% and pan to origin (Ctrl+R)")
            .clicked()
        {
            self.reset_view();
        }
    }

    fn ui_middle_pan_toggle(&mut self, ui: &mut egui::Ui) {
        let toggle_response = toggle_switch(ui, &mut self.middle_pan_enabled)
            .on_hover_text("Pan with middle mouse button");
        ui.add_space(4.0);
        ui.label("MMB pan")
            .on_hover_text("Enable/disable middle-button panning");
        if toggle_response.changed() && !self.middle_pan_enabled {
            self.touch_pan_active = false;
            self.touch_pan_last = None;
        }
    }

    fn ui_point_edit_buttons(&mut self, ui: &mut egui::Ui, has_points: bool) {
        let resp_clear = ui
            .add_enabled(
                has_points,
                egui::Button::new(format!("{} Clear points", icons::ICON_CLEAR))
                    .shortcut_text("Ctrl+Shift+D"),
            )
            .on_hover_text("Clear all points (Ctrl+Shift+D)");
        if resp_clear.clicked() {
            self.clear_all_points();
        }
        let resp_undo = ui
            .add_enabled(
                has_points,
                egui::Button::new(format!("{} Undo", icons::ICON_UNDO)).shortcut_text("Ctrl+Z"),
            )
            .on_hover_text("Undo last point (Ctrl+Z)");
        if resp_undo.clicked() {
            self.undo_last_point();
        }
    }
}
