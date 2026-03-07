use crate::app::CurcatApp;
use crate::i18n::TextKey;
use crate::image::ImageFilters;
use egui::RichText;

impl CurcatApp {
    pub(crate) fn ui_image_filters_window(&mut self, ctx: &egui::Context) {
        if !self.ui.image_filters_window_open {
            return;
        }

        let mut open = self.ui.image_filters_window_open;
        egui::Window::new(self.t(TextKey::ImageFiltersWindow))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui_image_filters_section(ui);
            });
        self.ui.image_filters_window_open = open;
    }

    fn ui_image_filters_section(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.label(RichText::new(i18n.text(TextKey::FiltersAffectDisplayOnly)).small());
        ui.add_space(4.0);

        let has_image = self.image.image.is_some();
        let mut changed = false;

        ui.add_enabled_ui(has_image, |ui| {
            ui.spacing_mut().slider_width = 150.0;

            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.brightness, -1.0..=1.0)
                        .text(i18n.text(TextKey::Brightness)),
                )
                .changed();
            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.contrast, -1.0..=1.0)
                        .text(i18n.text(TextKey::Contrast)),
                )
                .changed();
            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.gamma, 0.2..=3.0)
                        .text(i18n.text(TextKey::Gamma)),
                )
                .changed();
            changed |= ui
                .checkbox(&mut self.image.filters.invert, i18n.text(TextKey::Invert))
                .changed();

            ui.horizontal(|ui| {
                changed |= ui
                    .checkbox(
                        &mut self.image.filters.threshold_enabled,
                        i18n.text(TextKey::ThresholdEnabled),
                    )
                    .changed();
                let resp = ui.add_enabled(
                    self.image.filters.threshold_enabled,
                    egui::Slider::new(&mut self.image.filters.threshold, 0.0..=1.0)
                        .text(i18n.text(TextKey::Level)),
                );
                if resp.changed() {
                    changed = true;
                }
            });

            changed |= ui
                .add(
                    egui::Slider::new(&mut self.image.filters.blur_radius, 0..=12)
                        .text(i18n.text(TextKey::BlurRadius)),
                )
                .changed();

            if ui.button(i18n.text(TextKey::ResetFilters)).clicked() {
                self.image.filters = ImageFilters::default();
                changed = true;
            }
        });

        if !has_image {
            ui.label(RichText::new(i18n.text(TextKey::LoadImageToAdjustFilters)).small());
        }

        if changed && has_image {
            self.apply_filters_to_loaded_image();
        }
    }
}
