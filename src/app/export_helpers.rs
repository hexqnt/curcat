//! Prepare export data and computed columns.

use super::{CalibrationMapping, CurcatApp};
use crate::export::{
    ExportCoordinates, ExportExtraColumn, ExportPayload, ExtraColumnLengthMismatch,
    sequential_distances, turning_angles,
};
use crate::i18n::UiLanguage;
use crate::interp::{XYPoint, auto_sample_count, interpolate_sorted};
use crate::types::{AngleUnit, CoordSystem};
use std::fmt;

#[derive(Debug)]
pub(super) enum ExportPreparationError {
    IncompleteCalibration(CoordSystem),
    NoPoints,
    InvalidColumns(ExtraColumnLengthMismatch),
}

impl fmt::Display for ExportPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncompleteCalibration(CoordSystem::Cartesian) => {
                formatter.write_str("Complete both axis calibrations before export.")
            }
            Self::IncompleteCalibration(CoordSystem::Polar) => {
                formatter.write_str("Complete origin, radius, and angle calibration before export.")
            }
            Self::NoPoints => formatter.write_str("Nothing to export. Add data points first."),
            Self::InvalidColumns(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ExportPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidColumns(error) => Some(error),
            Self::IncompleteCalibration(_) | Self::NoPoints => None,
        }
    }
}

impl CurcatApp {
    pub(crate) fn collect_numeric_points_in_order(&self) -> Vec<XYPoint> {
        self.points
            .points
            .iter()
            .filter_map(|p| match (p.x_numeric, p.y_numeric) {
                (Some(x), Some(y)) => Some(XYPoint { x, y }),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn build_interpolated_samples(&mut self) -> Vec<XYPoint> {
        let sample_count = self.export.sample_count;
        let algo = self.export.interp_algorithm;
        let nums = self.sorted_numeric_points_cache();
        if nums.len() < 2 {
            return Vec::new();
        }
        interpolate_sorted(nums, sample_count, algo)
    }

    pub(crate) fn auto_tune_sample_count(&mut self) {
        let mapping = self.calibration_mapping();
        if !mapping.is_ready() {
            self.set_status_warn(match self.calibration.coord_system {
                CoordSystem::Cartesian => match self.ui.language {
                    UiLanguage::En => "Complete both axis calibrations before auto-tuning samples.",
                    UiLanguage::Ru => "Завершите калибровку обеих осей перед автоподбором семплов.",
                },
                CoordSystem::Polar => match self.ui.language {
                    UiLanguage::En => {
                        "Complete origin, radius, and angle calibration before auto-tuning samples."
                    }
                    UiLanguage::Ru => {
                        "Завершите калибровку начала, радиуса и угла перед автоподбором семплов."
                    }
                },
            });
            return;
        }

        let algo = self.export.interp_algorithm;
        let min_samples = super::SAMPLE_COUNT_MIN;
        let max_samples = self.config.export.samples_max_sanitized();
        let rel_tol = self.config.export.auto_rel_tolerance_sanitized();
        let ref_samples = self.config.export.auto_ref_samples_sanitized();

        self.ensure_point_numeric_cache(mapping);
        let nums = self.sorted_numeric_points_cache();
        if nums.len() < 2 {
            self.set_status_warn(match self.ui.language {
                UiLanguage::En => "Add at least two points before auto-tuning samples.",
                UiLanguage::Ru => "Добавьте как минимум две точки перед автоподбором семплов.",
            });
            return;
        }

        let suggested =
            auto_sample_count(nums, algo, min_samples, max_samples, rel_tol, ref_samples);
        self.export.sample_count = suggested;
        self.set_status(self.i18n().format_sample_count_tuned(suggested));
    }

    pub(super) fn build_export_payload(&mut self) -> Result<ExportPayload, ExportPreparationError> {
        let mapping = self.calibration_mapping();
        let (x_label, y_label) = self.axis_labels();

        let coordinates = match mapping {
            CalibrationMapping::Cartesian {
                x: Some(x),
                y: Some(y),
            } => ExportCoordinates::Cartesian {
                x_unit: x.unit(),
                y_unit: y.unit(),
            },
            CalibrationMapping::Polar(Some(polar)) => ExportCoordinates::Polar {
                angle_unit: polar.angle_unit(),
            },
            CalibrationMapping::Cartesian { .. } => {
                return Err(ExportPreparationError::IncompleteCalibration(
                    CoordSystem::Cartesian,
                ));
            }
            CalibrationMapping::Polar(None) => {
                return Err(ExportPreparationError::IncompleteCalibration(
                    CoordSystem::Polar,
                ));
            }
        };

        self.ensure_point_numeric_cache(mapping);

        let data = match self.export.export_kind {
            super::ExportKind::Interpolated => self.build_interpolated_samples(),
            super::ExportKind::RawPoints => self.collect_numeric_points_in_order(),
        };
        if data.is_empty() {
            return Err(ExportPreparationError::NoPoints);
        }

        let mut extra_columns = match self.export.export_kind {
            super::ExportKind::Interpolated => Vec::new(),
            super::ExportKind::RawPoints => self.build_raw_extra_columns(&data),
        };
        if self.export.polar_export_include_cartesian
            && let Some(unit) = coordinates.angle_unit()
        {
            extra_columns.extend(Self::polar_cartesian_columns(&data, unit));
        }
        ExportPayload::try_new(
            data,
            coordinates,
            [x_label.to_string(), y_label.to_string()],
            extra_columns,
        )
        .map_err(ExportPreparationError::InvalidColumns)
    }

    fn build_raw_extra_columns(&self, raw_points: &[XYPoint]) -> Vec<ExportExtraColumn> {
        let mut extras = Vec::new();
        if self.export.raw_include_distances {
            extras.push(ExportExtraColumn::new(
                "distance",
                sequential_distances(raw_points),
            ));
        }
        if self.export.raw_include_angles {
            extras.push(ExportExtraColumn::new(
                "angle_deg",
                turning_angles(raw_points),
            ));
        }
        extras
    }

    fn polar_cartesian_columns(
        points: &[XYPoint],
        angle_unit: AngleUnit,
    ) -> [ExportExtraColumn; 2] {
        let mut xs = Vec::with_capacity(points.len());
        let mut ys = Vec::with_capacity(points.len());
        for p in points {
            if !p.x.is_finite() || !p.y.is_finite() {
                xs.push(None);
                ys.push(None);
                continue;
            }
            let theta = match angle_unit {
                AngleUnit::Degrees => p.x.to_radians(),
                AngleUnit::Radians => p.x,
            };
            let r = p.y;
            xs.push(Some(r * theta.cos()));
            ys.push(Some(r * theta.sin()));
        }
        [
            ExportExtraColumn::new("x", xs),
            ExportExtraColumn::new("y", ys),
        ]
    }
}
