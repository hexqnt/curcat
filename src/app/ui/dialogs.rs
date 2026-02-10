use super::super::{CurcatApp, NativeDialog};
use crate::export::ExportFormat;
use egui_file_dialog::FileDialog;
use std::path::Path;

impl CurcatApp {
    pub(crate) fn open_image_dialog(&mut self) {
        let mut dialog = Self::make_open_dialog(self.project.last_image_dir.as_deref());
        dialog.pick_file();
        self.project.active_dialog = Some(NativeDialog::Open(dialog));
    }

    pub(crate) fn open_project_dialog(&mut self) {
        let mut dialog = FileDialog::new()
            .title("Open project")
            .add_file_filter_extensions("Curcat project", vec!["curcat"])
            .default_file_filter("Curcat project");
        if let Some(dir) = self.project.last_project_dir.as_deref() {
            dialog = dialog.initial_directory(dir.to_path_buf());
        }
        dialog.pick_file();
        self.project.active_dialog = Some(NativeDialog::OpenProject(dialog));
    }

    pub(crate) fn save_project_dialog(&mut self) {
        let default_name = self
            .project
            .last_project_path
            .as_ref()
            .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "project.curcat".to_string());
        let mut dialog = Self::make_save_dialog(
            "Save project",
            &default_name,
            &["curcat"],
            self.project.last_project_dir.as_deref(),
        );
        dialog.save_file();
        self.project.active_dialog = Some(NativeDialog::SaveProject(dialog));
    }

    pub(crate) fn start_export_csv(&mut self) {
        self.start_export(ExportFormat::Csv);
    }

    pub(crate) fn start_export_xlsx(&mut self) {
        self.start_export(ExportFormat::Xlsx);
    }

    pub(crate) fn start_export_json(&mut self) {
        self.start_export(ExportFormat::Json);
    }

    pub(crate) fn start_export_ron(&mut self) {
        self.start_export(ExportFormat::Ron);
    }

    pub(crate) fn start_export(&mut self, format: ExportFormat) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog(
                    format.dialog_title(),
                    format.default_filename(),
                    &[format.extension()],
                    self.project.last_export_dir.as_deref(),
                );
                dialog.save_file();
                self.project.active_dialog = Some(NativeDialog::SaveExport {
                    dialog,
                    payload,
                    format,
                });
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
