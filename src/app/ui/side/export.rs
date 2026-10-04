use super::super::{icons, style};
use crate::app::widgets::{ActionButton, Choice};
use crate::app::{CurcatApp, ExportKind, SAMPLE_COUNT_MIN};
use crate::export::ExportFormat;
use crate::i18n::TextKey;
use crate::interp::InterpAlgorithm;

type ExportButtonAction = (icons::Icon, TextKey, &'static str, ExportFormat);

const EXPORT_BUTTON_ACTIONS: [ExportButtonAction; 7] = [
    (
        icons::ICON_EXPORT_CSV,
        TextKey::ExportCsv,
        "Ctrl+Shift+C",
        ExportFormat::Csv,
    ),
    (
        icons::ICON_EXPORT_JSON,
        TextKey::ExportJson,
        "Ctrl+Shift+J",
        ExportFormat::Json,
    ),
    (
        icons::ICON_EXPORT_RON,
        TextKey::ExportRon,
        "Ctrl+Shift+R",
        ExportFormat::Ron,
    ),
    (
        icons::ICON_EXPORT_XLSX,
        TextKey::ExportExcel,
        "Ctrl+Shift+E",
        ExportFormat::Xlsx,
    ),
    (
        icons::ICON_EXPORT_HTML,
        TextKey::ExportHtml,
        "Ctrl+Shift+H",
        ExportFormat::Html,
    ),
    (
        icons::ICON_EXPORT_XML,
        TextKey::ExportXml,
        "Ctrl+Shift+X",
        ExportFormat::Xml,
    ),
    (
        icons::ICON_EXPORT_MARKDOWN,
        TextKey::ExportMarkdown,
        "Ctrl+Shift+M",
        ExportFormat::Markdown,
    ),
];

impl CurcatApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_export_section(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        let has_points = !self.points.points.is_empty();
        let calibrated = self.calibration_ready();
        let can_export = has_points && calibrated;
        ui.add(
            Choice::new(
                "export_kind_combo",
                &mut self.export.export_kind,
                &[ExportKind::Interpolated, ExportKind::RawPoints],
                |kind| {
                    i18n.text(match kind {
                        ExportKind::Interpolated => TextKey::InterpolatedCurve,
                        ExportKind::RawPoints => TextKey::RawPickedPoints,
                    })
                },
            )
            .hints(|kind| {
                i18n.text(match kind {
                    ExportKind::Interpolated => TextKey::InterpolatedCurveHover,
                    ExportKind::RawPoints => TextKey::RawPickedPointsHover,
                })
            }),
        );
        ui.add_space(style::SPACE_SMALL);

        match self.export.export_kind {
            ExportKind::Interpolated => {
                ui.label(i18n.text(TextKey::Interpolation))
                    .on_hover_text(i18n.text(TextKey::InterpolationHover));
                ui.add(Choice::new(
                    "interp_algo_combo",
                    &mut self.export.interp_algorithm,
                    &InterpAlgorithm::ALL,
                    |algo| i18n.interp_algorithm_label(algo),
                ))
                .on_hover_text(i18n.text(TextKey::InterpolationAlgorithmHover));

                ui.label(i18n.text(TextKey::Samples))
                    .on_hover_text(i18n.text(TextKey::SamplesHover));
                ui.spacing_mut().slider_width = style::SLIDER_WIDTH;
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
                    sresp.on_hover_ui(|ui| {
                        let hint = match self.ui.language {
                            crate::i18n::UiLanguage::En => format!(
                                "Higher values give a denser interpolated curve (max {max_samples})"
                            ),
                            crate::i18n::UiLanguage::Ru => format!(
                                "Чем больше значение, тем плотнее интерполированная кривая (макс {max_samples})"
                            ),
                        };
                        ui.label(hint);
                    });
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
        for (icon, text_key, shortcut, format) in EXPORT_BUTTON_ACTIONS {
            if ui
                .add_enabled(
                    can_export,
                    ActionButton::new(icon, i18n.text(text_key)).shortcut_text(shortcut),
                )
                .on_hover_ui(|ui| {
                    ui.label(export_hint(format.label(), shortcut));
                })
                .clicked()
            {
                self.start_export(format);
            }
        }
    }
}
