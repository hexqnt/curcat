use crate::app::CurcatApp;
use crate::image::ImageFilters;
use egui::RichText;

impl CurcatApp {
    pub(crate) fn ui_image_filters_window(&mut self, ctx: &egui::Context) {
        if !self.ui.image_filters_window_open {
            return;
        }

        let mut open = self.ui.image_filters_window_open;
        egui::Window::new("Image filters")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui_image_filters_section(ui);
            });
        self.ui.image_filters_window_open = open;
    }

    fn ui_image_filters_section(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Affects display and snapping only.").small());
        ui.add_space(4.0);

        let has_image = self.image.image.is_some();
        let mut changed = false;

        ui.add_enabled_ui(has_image, |ui| {
            ui.spacing_mut().slider_width = 150.0;

            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.brightness, -1.0..=1.0)
                        .text("brightness"),
                )
                .changed();
            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.contrast, -1.0..=1.0)
                        .text("contrast"),
                )
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut self.image.filters.gamma, 0.2..=3.0).text("gamma"))
                .changed();
            changed |= ui
                .checkbox(&mut self.image.filters.invert, "invert")
                .changed();

            ui.horizontal(|ui| {
                changed |= ui
                    .checkbox(&mut self.image.filters.threshold_enabled, "threshold")
                    .changed();
                let resp = ui.add_enabled(
                    self.image.filters.threshold_enabled,
                    egui::Slider::new(&mut self.image.filters.threshold, 0.0..=1.0).text("level"),
                );
                if resp.changed() {
                    changed = true;
                }
            });

            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.blur_radius, 0..=12)
                        .text("blur radius"),
                )
                .changed();

            if ui.button("Reset filters").clicked() {
                self.image.filters = ImageFilters::default();
                changed = true;
            }
        });

        if !has_image {
            ui.label(RichText::new("Load an image to adjust filters.").small());
        }

        if changed && has_image {
            self.apply_filters_to_loaded_image();
        }
    }
}
