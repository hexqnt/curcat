use super::super::CurcatApp;
use super::common::toggle_switch;

impl CurcatApp {
    pub(crate) fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Use egui's built-in theme toggle so icon matches current mode.
            egui::widgets::global_theme_preference_switch(ui);
            ui.separator();

            let side_label = if self.side_open {
                "Hide side"
            } else {
                "Show side"
            };
            if ui
                .add(egui::Button::new(format!("⟷ {side_label}")).shortcut_text("Ctrl+B"))
                .on_hover_text("Toggle side panel (Ctrl+B)")
                .clicked()
            {
                self.side_open = !self.side_open;
            }
            ui.separator();

            let open_resp = ui
                .add(egui::Button::new("📂 Open image…").shortcut_text("Ctrl+O"))
                .on_hover_text("Open an image (Ctrl+O). You can also drag & drop into the center.");
            self.paint_attention_outline_if(ui, open_resp.rect, self.image.is_none());
            if open_resp.clicked() {
                self.open_image_dialog();
            }

            let has_image = self.image.is_some();
            let stats_resp = ui
                .add(egui::Button::new("📊 Points stats"))
                .on_hover_text("Show stats for picked points");
            if stats_resp.clicked() {
                self.points_info_window_open = true;
            }
            let info_resp = ui
                .add_enabled(
                    has_image,
                    egui::Button::new("ℹ Image info").shortcut_text("Ctrl+I"),
                )
                .on_hover_text("Show file & image details (Ctrl+I)");
            if info_resp.clicked() && has_image {
                self.info_window_open = true;
            }
            {
                let resp = ui.add_enabled(has_image, egui::Button::new("↺ 90°"));
                let resp = resp.on_hover_text("Rotate 90° counter-clockwise");
                if resp.clicked() {
                    self.rotate_image(false);
                }
            }
            {
                let resp = ui.add_enabled(has_image, egui::Button::new("↻ 90°"));
                let resp = resp.on_hover_text("Rotate 90° clockwise");
                if resp.clicked() {
                    self.rotate_image(true);
                }
            }

            ui.label("Zoom:")
                .on_hover_text("Choose a preset zoom level");
            let zoom_ir = egui::ComboBox::from_id_salt("zoom_combo")
                .selected_text(Self::format_zoom(self.image_zoom))
                .show_ui(ui, |ui| {
                    for &preset in super::super::ZOOM_PRESETS {
                        let label = Self::format_zoom(preset);
                        let selected = (self.image_zoom - preset).abs() < 0.0001;
                        if ui.selectable_label(selected, label).clicked() {
                            self.set_zoom(preset);
                        }
                    }
                });
            zoom_ir.response.on_hover_text("Zoom presets (percent)");

            ui.separator();

            let toggle_response = toggle_switch(ui, &mut self.middle_pan_enabled)
                .on_hover_text("Pan with middle mouse button");
            ui.add_space(4.0);
            ui.label("MMB pan")
                .on_hover_text("Enable/disable middle-button panning");
            if toggle_response.changed() && !self.middle_pan_enabled {
                self.touch_pan_active = false;
                self.touch_pan_last = None;
            }

            ui.separator();

            let resp_clear = ui
                .add(egui::Button::new("🗑 Clear points").shortcut_text("Ctrl+Shift+D"))
                .on_hover_text("Clear all points (Ctrl+Shift+D)");
            if resp_clear.clicked() {
                self.clear_all_points();
            }
            let resp_undo = ui
                .add(egui::Button::new("↶ Undo").shortcut_text("Ctrl+Z"))
                .on_hover_text("Undo last point (Ctrl+Z)");
            if resp_undo.clicked() {
                self.undo_last_point();
            }
        });
    }
}
