use super::CurcatApp;
use crate::export::{ExportExtraColumn, ExportPayload};
use crate::interp::{XYPoint, interpolate_sorted};
use crate::types::AxisValue;

pub(crate) fn format_overlay_value(value: &AxisValue) -> String {
    match value {
        AxisValue::Float(v) => format!("{v:.3}"),
        AxisValue::DateTime(_) => value.format(),
    }
}

impl CurcatApp {
    pub(crate) fn collect_numeric_points_in_order(&self) -> Vec<XYPoint> {
        self.points
            .iter()
            .filter_map(|p| match (p.x_numeric, p.y_numeric) {
                (Some(x), Some(y)) => Some(XYPoint { x, y }),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn build_interpolated_samples(&mut self) -> Vec<XYPoint> {
        let sample_count = self.sample_count;
        let algo = self.interp_algorithm;
        let nums = self.sorted_numeric_points_cache();
        if nums.len() < 2 {
            return Vec::new();
        }
        interpolate_sorted(nums, sample_count, algo)
    }

    pub(crate) fn build_export_payload(&mut self) -> Result<ExportPayload, &'static str> {
        let x_mapping = self.cal_x.mapping();
        let y_mapping = self.cal_y.mapping();
        let x_unit = match x_mapping.as_ref() {
            Some(mapping) => mapping.unit,
            None => return Err("Complete both axis calibrations before export."),
        };
        let y_unit = match y_mapping.as_ref() {
            Some(mapping) => mapping.unit,
            None => return Err("Complete both axis calibrations before export."),
        };

        self.ensure_point_numeric_cache(x_mapping.as_ref(), y_mapping.as_ref());

        match self.export_kind {
            super::ExportKind::Interpolated => {
                let data = self.build_interpolated_samples();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        extra_columns: Vec::new(),
                    })
                }
            }
            super::ExportKind::RawPoints => {
                let data = self.collect_numeric_points_in_order();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    let extras = self.build_raw_extra_columns(&data);
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        extra_columns: extras,
                    })
                }
            }
        }
    }

    fn build_raw_extra_columns(&self, raw_points: &[XYPoint]) -> Vec<ExportExtraColumn> {
        let mut extras = Vec::new();
        if self.raw_include_distances {
            extras.push(ExportExtraColumn::new(
                "distance",
                Self::sequential_distances(raw_points),
            ));
        }
        if self.raw_include_angles {
            extras.push(ExportExtraColumn::new(
                "angle_deg",
                Self::turning_angles(raw_points),
            ));
        }
        extras
    }

    fn sequential_distances(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
        let len = raw_points.len();
        let mut values = vec![None; len];
        for i in 1..len {
            let prev = &raw_points[i - 1];
            let curr = &raw_points[i];
            let dx = curr.x - prev.x;
            let dy = curr.y - prev.y;
            values[i] = Some(dx.hypot(dy));
        }
        values
    }

    fn turning_angles(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
        let len = raw_points.len();
        let mut values = vec![None; len];
        if len < 3 {
            return values;
        }
        for i in 1..(len - 1) {
            let prev = &raw_points[i - 1];
            let curr = &raw_points[i];
            let next = &raw_points[i + 1];
            let v1 = (curr.x - prev.x, curr.y - prev.y);
            let v2 = (next.x - curr.x, next.y - curr.y);
            let mag1 = v1.0.hypot(v1.1);
            let mag2 = v2.0.hypot(v2.1);
            if mag1 <= f64::EPSILON || mag2 <= f64::EPSILON {
                continue;
            }
            let dot = v1.0 * v2.0 + v1.1 * v2.1;
            let cos_theta = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
            values[i] = Some(cos_theta.acos().to_degrees());
        }
        values
    }
}
