use super::super::icons;
use crate::app::{CurcatApp, ExportKind, SAMPLE_COUNT_MIN};
use crate::export::ExportFormat;
use crate::i18n::TextKey;
use crate::interp::InterpAlgorithm;

type ExportButtonAction = (
    icons::Icon,
    TextKey,
    &'static str,
    ExportFormat,
    fn(&mut CurcatApp),
);

const EXPORT_BUTTON_ACTIONS: [ExportButtonAction; 7] = [
    (
        icons::ICON_EXPORT_CSV,
        TextKey::ExportCsv,
        "Ctrl+Shift+C",
        ExportFormat::Csv,
        CurcatApp::start_export_csv,
    ),
    (
        icons::ICON_EXPORT_JSON,
        TextKey::ExportJson,
        "Ctrl+Shift+J",
        ExportFormat::Json,
        CurcatApp::start_export_json,
    ),
    (
        icons::ICON_EXPORT_RON,
        TextKey::ExportRon,
        "Ctrl+Shift+R",
        ExportFormat::Ron,
        CurcatApp::start_export_ron,
    ),
    (
        icons::ICON_EXPORT_XLSX,
        TextKey::ExportExcel,
        "Ctrl+Shift+E",
        ExportFormat::Xlsx,
        CurcatApp::start_export_xlsx,
    ),
    (
        icons::ICON_EXPORT_HTML,
        TextKey::ExportHtml,
        "Ctrl+Shift+H",
        ExportFormat::Html,
        CurcatApp::start_export_html,
    ),
    (
        icons::ICON_EXPORT_XML,
        TextKey::ExportXml,
        "Ctrl+Shift+X",
        ExportFormat::Xml,
        CurcatApp::start_export_xml,
    ),
    (
        icons::ICON_EXPORT_MARKDOWN,
        TextKey::ExportMarkdown,
        "Ctrl+Shift+M",
        ExportFormat::Markdown,
        CurcatApp::start_export_markdown,
    ),
];

impl CurcatApp {
    #[allow(clippy::too_many_arguments)]
    fn export_action_button(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        icon: icons::Icon,
        label: &str,
        shortcut: &str,
        hint: &str,
        on_click: fn(&mut Self),
    ) {
        let resp = ui
            .add_enabled(
                enabled,
                egui::Button::image_and_text(icons::image(icon, icons::BUTTON_ICON_SIZE), label)
                    .image_tint_follows_text_color(true)
                    .shortcut_text(shortcut),
            )
            .on_hover_text(hint);
        if resp.clicked() {
            on_click(self);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_export_section(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        let has_points = !self.points.points.is_empty();
        let calibrated = self.calibration_ready();
        let can_export = has_points && calibrated;
        let export_kind_label = match self.export.export_kind {
            ExportKind::Interpolated => i18n.text(TextKey::InterpolatedCurve),
            ExportKind::RawPoints => i18n.text(TextKey::RawPickedPoints),
        };
        egui::ComboBox::from_id_salt("export_kind_combo")
            .selected_text(export_kind_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.export.export_kind,
                    ExportKind::Interpolated,
                    i18n.text(TextKey::InterpolatedCurve),
                )
                .on_hover_text(i18n.text(TextKey::InterpolatedCurveHover));
                ui.selectable_value(
                    &mut self.export.export_kind,
                    ExportKind::RawPoints,
                    i18n.text(TextKey::RawPickedPoints),
                )
                .on_hover_text(i18n.text(TextKey::RawPickedPointsHover));
            });
        ui.add_space(4.0);

        match self.export.export_kind {
            ExportKind::Interpolated => {
                ui.label(i18n.text(TextKey::Interpolation))
                    .on_hover_text(i18n.text(TextKey::InterpolationHover));
                let combo = egui::ComboBox::from_id_salt("interp_algo_combo")
                    .selected_text(i18n.interp_algorithm_label(self.export.interp_algorithm))
                    .show_ui(ui, |ui| {
                        for algo in InterpAlgorithm::ALL.iter().copied() {
                            ui.selectable_value(
                                &mut self.export.interp_algorithm,
                                algo,
                                i18n.interp_algorithm_label(algo),
                            );
                        }
                    });
                combo
                    .response
                    .on_hover_text(i18n.text(TextKey::InterpolationAlgorithmHover));

                ui.label(i18n.text(TextKey::Samples))
                    .on_hover_text(i18n.text(TextKey::SamplesHover));
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
                        .text(i18n.text(TextKey::Count)),
                    );
                    let slider_hint = match self.ui.language {
                        crate::i18n::UiLanguage::En => format!(
                            "Higher values give a denser interpolated curve (max {max_samples})"
                        ),
                        crate::i18n::UiLanguage::Ru => format!(
                            "Чем больше значение, тем плотнее интерполированная кривая (макс {max_samples})"
                        ),
                    };
                    sresp.on_hover_text(slider_hint);
                    if ui
                        .button(i18n.text(TextKey::Auto))
                        .on_hover_text(i18n.text(TextKey::AutoSamplesHover))
                        .clicked()
                    {
                        self.auto_tune_sample_count();
                    }
                });
            }
            ExportKind::RawPoints => {
                ui.label(i18n.text(TextKey::ExtraColumns))
                    .on_hover_text(i18n.text(TextKey::ExtraColumnsHover));
                let dist = ui.checkbox(
                    &mut self.export.raw_include_distances,
                    i18n.text(TextKey::IncludeDistanceToPrev),
                );
                dist.on_hover_text(i18n.text(TextKey::IncludeDistanceToPrevHover));
                let ang = ui.checkbox(
                    &mut self.export.raw_include_angles,
                    i18n.text(TextKey::IncludeAngleDeg),
                );
                ang.on_hover_text(i18n.text(TextKey::IncludeAngleDegHover));
            }
        }

        if matches!(
            self.calibration.coord_system,
            crate::types::CoordSystem::Polar
        ) {
            let cart = ui.checkbox(
                &mut self.export.polar_export_include_cartesian,
                i18n.text(TextKey::IncludeCartesianColumns),
            );
            cart.on_hover_text(i18n.text(TextKey::IncludeCartesianColumnsHover));
        }

        ui.separator();
        let coord_system = self.calibration.coord_system;
        let export_hint = |format_name: &str, shortcut: &str| -> String {
            if !has_points {
                format!(
                    "{} {format_name}",
                    i18n.text(TextKey::AddPointsBeforeExport)
                )
            } else if !calibrated {
                match coord_system {
                    crate::types::CoordSystem::Cartesian => format!(
                        "{} {format_name}",
                        i18n.text(TextKey::CompleteCalibrationBeforeExportCartesian)
                    ),
                    crate::types::CoordSystem::Polar => format!(
                        "{} {format_name}",
                        i18n.text(TextKey::CompleteCalibrationBeforeExportPolar)
                    ),
                }
            } else {
                format!(
                    "{} {format_name} ({shortcut})",
                    i18n.text(TextKey::ExportToFormat)
                )
            }
        };
        for (icon, text_key, shortcut, format, action) in EXPORT_BUTTON_ACTIONS {
            self.export_action_button(
                ui,
                can_export,
                icon,
                i18n.text(text_key),
                shortcut,
                &export_hint(format.label(), shortcut),
                action,
            );
        }
    }
}
