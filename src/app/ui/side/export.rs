use super::super::icons;
use crate::app::{CurcatApp, ExportKind, SAMPLE_COUNT_MIN};
use crate::interp::InterpAlgorithm;

impl CurcatApp {
    fn export_action_button(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        label: String,
        shortcut: &str,
        hint: &str,
        on_click: fn(&mut Self),
    ) {
        let resp = ui
            .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
            .on_hover_text(hint);
        if resp.clicked() {
            on_click(self);
        }
    }

    pub(crate) fn ui_export_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Export points");
        let has_points = !self.points.points.is_empty();
        let calibrated = self.calibration_ready();
        let can_export = has_points && calibrated;
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.export.export_kind,
                ExportKind::Interpolated,
                "Interpolated curve",
            )
            .on_hover_text("Export evenly spaced samples of the curve");
            ui.radio_value(
                &mut self.export.export_kind,
                ExportKind::RawPoints,
                "Raw picked points",
            )
            .on_hover_text("Export only the points you clicked, in order");
        });
        ui.add_space(4.0);

        match self.export.export_kind {
            ExportKind::Interpolated => {
                ui.label("Interpolation:")
                    .on_hover_text("Choose how to interpolate between control points");
                let combo = egui::ComboBox::from_id_salt("interp_algo_combo")
                    .selected_text(self.export.interp_algorithm.label())
                    .show_ui(ui, |ui| {
                        for algo in InterpAlgorithm::ALL.iter().copied() {
                            ui.selectable_value(
                                &mut self.export.interp_algorithm,
                                algo,
                                algo.label(),
                            );
                        }
                    });
                combo
                    .response
                    .on_hover_text("Algorithm used to generate the interpolated samples");

                ui.label("Samples:")
                    .on_hover_text("Number of evenly spaced samples to export");
                ui.spacing_mut().slider_width = 150.0;
                ui.horizontal(|ui| {
                    let max_samples = self.config.export.samples_max_sanitized();
                    self.export.sample_count = self
                        .export
                        .sample_count
                        .clamp(SAMPLE_COUNT_MIN, max_samples);
                    let sresp = ui.add(
                        egui::Slider::new(
                            &mut self.export.sample_count,
                            SAMPLE_COUNT_MIN..=max_samples,
                        )
                        .text("count"),
                    );
                    sresp.on_hover_text(format!(
                        "Higher values give a denser interpolated curve (max {max_samples})"
                    ));
                    if ui
                        .button("Auto")
                        .on_hover_text(
                            "Automatically choose a sample count based on curve smoothness",
                        )
                        .clicked()
                    {
                        self.auto_tune_sample_count();
                    }
                });
            }
            ExportKind::RawPoints => {
                ui.label("Extra columns:")
                    .on_hover_text("Optional metrics for the picked points");
                let dist = ui.checkbox(
                    &mut self.export.raw_include_distances,
                    "Include distance to previous point",
                );
                dist.on_hover_text(
                    "Adds a column with distances between consecutive picked points",
                );
                let ang = ui.checkbox(&mut self.export.raw_include_angles, "Include angle (deg)");
                ang.on_hover_text(
                    "Adds a column with angles at each interior point (first/last stay empty)",
                );
            }
        }

        if matches!(
            self.calibration.coord_system,
            crate::types::CoordSystem::Polar
        ) {
            let cart = ui.checkbox(
                &mut self.export.polar_export_include_cartesian,
                "Include Cartesian x/y columns",
            );
            cart.on_hover_text("Adds x and y columns computed from angle and radius");
        }

        ui.separator();
        let csv_hint = if !has_points {
            "Add points before exporting to CSV"
        } else if !calibrated {
            match self.calibration.coord_system {
                crate::types::CoordSystem::Cartesian => {
                    "Complete both axis calibrations before exporting to CSV"
                }
                crate::types::CoordSystem::Polar => {
                    "Complete origin, radius, and angle calibration before exporting to CSV"
                }
            }
        } else {
            "Export data to CSV (Ctrl+Shift+C)"
        };
        self.export_action_button(
            ui,
            can_export,
            format!("{} Export CSV…", icons::ICON_EXPORT_CSV),
            "Ctrl+Shift+C",
            csv_hint,
            Self::start_export_csv,
        );

        let json_hint = if !has_points {
            "Add points before exporting to JSON"
        } else if !calibrated {
            match self.calibration.coord_system {
                crate::types::CoordSystem::Cartesian => {
                    "Complete both axis calibrations before exporting to JSON"
                }
                crate::types::CoordSystem::Polar => {
                    "Complete origin, radius, and angle calibration before exporting to JSON"
                }
            }
        } else {
            "Export data to JSON (Ctrl+Shift+J)"
        };
        self.export_action_button(
            ui,
            can_export,
            format!("{} Export JSON…", icons::ICON_EXPORT_JSON),
            "Ctrl+Shift+J",
            json_hint,
            Self::start_export_json,
        );

        let xlsx_hint = if !has_points {
            "Add points before exporting to Excel"
        } else if !calibrated {
            match self.calibration.coord_system {
                crate::types::CoordSystem::Cartesian => {
                    "Complete both axis calibrations before exporting to Excel"
                }
                crate::types::CoordSystem::Polar => {
                    "Complete origin, radius, and angle calibration before exporting to Excel"
                }
            }
        } else {
            "Export data to Excel (Ctrl+Shift+E)"
        };
        self.export_action_button(
            ui,
            can_export,
            format!("{} Export Excel…", icons::ICON_EXPORT_XLSX),
            "Ctrl+Shift+E",
            xlsx_hint,
            Self::start_export_xlsx,
        );
    }
}
