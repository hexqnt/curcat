use super::super::{
    CurcatApp, describe_aspect_ratio, format_system_time, human_readable_bytes, total_pixel_count,
};

impl CurcatApp {
    pub(crate) fn ui_image_info_window(&mut self, ctx: &egui::Context) {
        if !self.info_window_open {
            return;
        }

        egui::Window::new("Image info")
            .open(&mut self.info_window_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(image) = &self.image {
                    ui.heading("File");
                    if let Some(meta) = self.image_meta.as_ref() {
                        ui.label(format!("Source: {}", meta.source_label()));
                        ui.label(format!("Name: {}", meta.display_name()));
                        if let Some(path) = meta.path() {
                            ui.label(format!("Path: {}", path.display()));
                        }
                        if let Some(bytes) = meta.byte_len() {
                            ui.label(format!(
                                "Size: {} ({bytes} bytes)",
                                human_readable_bytes(bytes),
                            ));
                        } else {
                            ui.label("Size: Unknown");
                        }
                        if let Some(modified) = meta.last_modified() {
                            ui.label(format!("Modified: {}", format_system_time(modified),));
                        } else {
                            ui.label("Modified: Unknown");
                        }
                    } else {
                        ui.label("No captured file metadata for this image.");
                    }
                    ui.add_space(6.0);
                    ui.heading("Image");
                    let [w, h] = image.size;
                    ui.label(format!("Dimensions: {w} × {h} px"));
                    if let Some(aspect_text) = describe_aspect_ratio(image.size) {
                        ui.label(format!("Aspect ratio: {aspect_text}"));
                    } else {
                        ui.label("Aspect ratio: n/a");
                    }
                    let total_pixels = total_pixel_count(image.size);
                    ui.label(format!(
                        "Pixels: {total_pixels} ({:.2} MP)",
                        total_pixels as f64 / 1_000_000.0
                    ));
                    let rgba_bytes = total_pixels.saturating_mul(4);
                    ui.label(format!(
                        "RGBA memory estimate: {} ({rgba_bytes} bytes)",
                        human_readable_bytes(rgba_bytes),
                    ));
                    ui.label(format!(
                        "Current zoom: {}",
                        Self::format_zoom(self.image_zoom)
                    ));
                } else {
                    ui.label("Load an image to inspect its metadata.");
                }
            });
    }
}
