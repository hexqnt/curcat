//! Helpers for formatting and preparing export payloads.

use super::CurcatApp;
use crate::export::{ExportExtraColumn, ExportPayload, sequential_distances, turning_angles};
use crate::interp::{XYPoint, auto_sample_count, interpolate_sorted};
use crate::types::{AngleUnit, AxisUnit, CoordSystem};

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
        if !self.calibration_ready() {
            self.set_status(match self.calibration.coord_system {
                CoordSystem::Cartesian => {
                    "Complete both axis calibrations before auto-tuning samples."
                }
                CoordSystem::Polar => {
                    "Complete origin, radius, and angle calibration before auto-tuning samples."
                }
            });
            return;
        }

        let (x_mapping, y_mapping) = self.cartesian_mappings();
        let polar_mapping = self.polar_mapping();

        let algo = self.export.interp_algorithm;
        let min_samples = super::SAMPLE_COUNT_MIN;
        let max_samples = self.config.export.samples_max_sanitized();
        let rel_tol = self.config.export.auto_rel_tolerance_sanitized();
        let ref_samples = self.config.export.auto_ref_samples_sanitized();

        self.ensure_point_numeric_cache(
            self.calibration.coord_system,
            x_mapping.as_ref(),
            y_mapping.as_ref(),
            polar_mapping.as_ref(),
        );
        let nums = self.sorted_numeric_points_cache();
        if nums.len() < 2 {
            self.set_status("Add at least two points before auto-tuning samples.");
            return;
        }

        let suggested =
            auto_sample_count(nums, algo, min_samples, max_samples, rel_tol, ref_samples);
        self.export.sample_count = suggested;
        self.set_status(format!("Sample count auto-tuned to {suggested}."));
    }

    pub(crate) fn build_export_payload(&mut self) -> Result<ExportPayload, &'static str> {
        if !self.calibration_ready() {
            return Err(match self.calibration.coord_system {
                CoordSystem::Cartesian => "Complete both axis calibrations before export.",
                CoordSystem::Polar => {
                    "Complete origin, radius, and angle calibration before export."
                }
            });
        }

        let (x_mapping, y_mapping) = self.cartesian_mappings();
        let polar_mapping = self.polar_mapping();
        let (x_label, y_label) = self.axis_labels();

        let (x_unit, y_unit, angle_unit) = match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                let x_unit = x_mapping
                    .as_ref()
                    .map(|mapping| mapping.unit)
                    .ok_or("Complete both axis calibrations before export.")?;
                let y_unit = y_mapping
                    .as_ref()
                    .map(|mapping| mapping.unit)
                    .ok_or("Complete both axis calibrations before export.")?;
                (x_unit, y_unit, None)
            }
            CoordSystem::Polar => (
                AxisUnit::Float,
                AxisUnit::Float,
                polar_mapping
                    .as_ref()
                    .map(super::super::types::PolarMapping::angle_unit),
            ),
        };

        self.ensure_point_numeric_cache(
            self.calibration.coord_system,
            x_mapping.as_ref(),
            y_mapping.as_ref(),
            polar_mapping.as_ref(),
        );

        match self.export.export_kind {
            super::ExportKind::Interpolated => {
                let data = self.build_interpolated_samples();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    let mut extra_columns = Vec::new();
                    if self.calibration.coord_system == CoordSystem::Polar
                        && self.export.polar_export_include_cartesian
                        && let Some(unit) = angle_unit
                    {
                        extra_columns.extend(Self::polar_cartesian_columns(&data, unit));
                    }
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        x_label: x_label.to_string(),
                        y_label: y_label.to_string(),
                        coord_system: self.calibration.coord_system,
                        angle_unit,
                        extra_columns,
                    })
                }
            }
            super::ExportKind::RawPoints => {
                let data = self.collect_numeric_points_in_order();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    let mut extras = self.build_raw_extra_columns(&data);
                    if self.calibration.coord_system == CoordSystem::Polar
                        && self.export.polar_export_include_cartesian
                        && let Some(unit) = angle_unit
                    {
                        extras.extend(Self::polar_cartesian_columns(&data, unit));
                    }
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        x_label: x_label.to_string(),
                        y_label: y_label.to_string(),
                        coord_system: self.calibration.coord_system,
                        angle_unit,
                        extra_columns: extras,
                    })
                }
            }
        }
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
    ) -> Vec<ExportExtraColumn> {
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
        vec![
            ExportExtraColumn::new("x", xs),
            ExportExtraColumn::new("y", ys),
        ]
    }
}
