use super::super::{CurcatApp, NativeDialog};
use egui_file_dialog::FileDialog;
use std::path::Path;

impl CurcatApp {
    pub(crate) fn open_image_dialog(&mut self) {
        let mut dialog = Self::make_open_dialog(self.last_image_dir.as_deref());
        dialog.pick_file();
        self.active_dialog = Some(NativeDialog::Open(dialog));
    }

    pub(crate) fn open_project_dialog(&mut self) {
        let mut dialog = FileDialog::new()
            .title("Open project")
            .add_file_filter_extensions("Curcat project", vec!["curcat"])
            .default_file_filter("Curcat project");
        if let Some(dir) = self.last_project_dir.as_deref() {
            dialog = dialog.initial_directory(dir.to_path_buf());
        }
        dialog.pick_file();
        self.active_dialog = Some(NativeDialog::OpenProject(dialog));
    }

    pub(crate) fn save_project_dialog(&mut self) {
        let default_name = self
            .last_project_path
            .as_ref()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "project.curcat".to_string());
        let mut dialog = Self::make_save_dialog(
            "Save project",
            &default_name,
            &["curcat"],
            self.last_project_dir.as_deref(),
        );
        dialog.save_file();
        self.active_dialog = Some(NativeDialog::SaveProject(dialog));
    }

    pub(crate) fn start_export_csv(&mut self) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog(
                    "Export CSV",
                    "curve.csv",
                    &["csv"],
                    self.last_export_dir.as_deref(),
                );
                dialog.save_file();
                self.active_dialog = Some(NativeDialog::SaveCsv { dialog, payload });
            }
            Err(msg) => self.set_status(msg),
        }
    }

    pub(crate) fn start_export_xlsx(&mut self) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog(
                    "Export Excel",
                    "curve.xlsx",
                    &["xlsx"],
                    self.last_export_dir.as_deref(),
                );
                dialog.save_file();
                self.active_dialog = Some(NativeDialog::SaveXlsx { dialog, payload });
            }
            Err(msg) => self.set_status(msg),
        }
    }

    pub(crate) fn start_export_json(&mut self) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog(
                    "Export JSON",
                    "curve.json",
                    &["json"],
                    self.last_export_dir.as_deref(),
                );
                dialog.save_file();
                self.active_dialog = Some(NativeDialog::SaveJson { dialog, payload });
            }
            Err(msg) => self.set_status(msg),
        }
    }

    pub(crate) fn make_open_dialog(initial_dir: Option<&Path>) -> FileDialog {
        // Keep in sync with enabled `image` crate features.
        // Add separate presets for frequent formats.
        let mut dialog = FileDialog::new()
            .title("Open image")
            // Combined filter
            .add_file_filter_extensions(
                "All images",
                vec![
                    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tga", "tiff", "tif", "pnm",
                    "pbm", "pgm", "ppm", "hdr", "dds",
                ],
            )
            // Individual format presets
            .add_file_filter_extensions("PNG", vec!["png"])
            .add_file_filter_extensions("JPEG/JPG", vec!["jpg", "jpeg"])
            .add_file_filter_extensions("BMP", vec!["bmp"])
            .add_file_filter_extensions("TIFF", vec!["tiff", "tif"])
            .default_file_filter("All images");
        if let Some(dir) = initial_dir {
            dialog = dialog.initial_directory(dir.to_path_buf());
        }
        dialog
    }

    pub(crate) fn make_save_dialog(
        title: &str,
        default_name: &str,
        extensions: &[&str],
        initial_dir: Option<&Path>,
    ) -> FileDialog {
        let mut dialog = FileDialog::new()
            .title(title)
            .default_file_name(default_name);
        let mut first_label: Option<String> = None;
        for ext in extensions {
            let label = format!("*.{ext}");
            if first_label.is_none() {
                first_label = Some(label.clone());
            }
            dialog = dialog.add_save_extension(&label, ext);
        }
        if let Some(label) = first_label.as_deref() {
            dialog = dialog.default_save_extension(label);
        }
        if let Some(dir) = initial_dir {
            dialog = dialog.initial_directory(dir.to_path_buf());
        }
        dialog
    }
}
