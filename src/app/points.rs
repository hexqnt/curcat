use super::{CalibrationMapping, CurcatApp};
use crate::interp::XYPoint;
use egui::Pos2;

#[derive(Debug, Clone)]
pub struct PickedPoint {
    pub(super) pixel: Pos2,
    pub(super) x_numeric: Option<f64>,
    pub(super) y_numeric: Option<f64>,
}

impl PickedPoint {
    pub(super) const fn new(pixel: Pos2) -> Self {
        Self {
            pixel,
            x_numeric: None,
            y_numeric: None,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
pub struct PointsState {
    pub(super) points: Vec<PickedPoint>,
    pub(super) points_numeric_dirty: bool,
    pub(super) cached_sorted_preview: Vec<(f64, Pos2)>,
    pub(super) cached_sorted_numeric: Vec<XYPoint>,
    pub(super) sorted_preview_dirty: bool,
    pub(super) sorted_numeric_dirty: bool,
    pub(super) last_mapping: Option<CalibrationMapping>,
    pub(super) show_curve_segments: bool,
}

impl CurcatApp {
    pub(crate) const fn mark_points_dirty(&mut self) {
        self.points.points_numeric_dirty = true;
        self.points.sorted_preview_dirty = true;
        self.points.sorted_numeric_dirty = true;
    }

    pub(super) fn ensure_point_numeric_cache(&mut self, mapping: CalibrationMapping) {
        if self.points.last_mapping != Some(mapping) {
            self.points.last_mapping = Some(mapping);
            self.mark_points_dirty();
        }

        if self.points.points_numeric_dirty {
            match mapping {
                CalibrationMapping::Cartesian { x, y } => {
                    for p in &mut self.points.points {
                        p.x_numeric = x.as_ref().and_then(|axis| axis.numeric_at(p.pixel));
                        p.y_numeric = y.as_ref().and_then(|axis| axis.numeric_at(p.pixel));
                    }
                }
                CalibrationMapping::Polar(polar) => {
                    for p in &mut self.points.points {
                        p.x_numeric = polar.as_ref().and_then(|mapping| mapping.angle_at(p.pixel));
                        p.y_numeric = polar
                            .as_ref()
                            .and_then(|mapping| mapping.radius_at(p.pixel));
                    }
                }
            }
            self.points.points_numeric_dirty = false;
        }
    }

    pub(crate) fn sorted_preview_segments(&mut self) -> &[(f64, Pos2)] {
        if self.points.sorted_preview_dirty {
            self.points.cached_sorted_preview.clear();
            for point in &self.points.points {
                if let Some(xn) = point.x_numeric {
                    self.points.cached_sorted_preview.push((xn, point.pixel));
                }
            }
            self.points
                .cached_sorted_preview
                .sort_by(|a, b| a.0.total_cmp(&b.0));
            self.points.sorted_preview_dirty = false;
        }
        &self.points.cached_sorted_preview
    }

    pub(crate) fn sorted_numeric_points_cache(&mut self) -> &[XYPoint] {
        if self.points.sorted_numeric_dirty {
            self.points.cached_sorted_numeric.clear();
            for point in &self.points.points {
                if let (Some(x), Some(y)) = (point.x_numeric, point.y_numeric) {
                    self.points.cached_sorted_numeric.push(XYPoint { x, y });
                }
            }
            self.points
                .cached_sorted_numeric
                .sort_by(|a, b| a.x.total_cmp(&b.x));
            self.points.sorted_numeric_dirty = false;
        }
        &self.points.cached_sorted_numeric
    }

    pub(crate) fn push_curve_point(&mut self, pixel_hint: Pos2) {
        let resolved = self.resolve_curve_pick(pixel_hint);
        self.points.points.push(PickedPoint::new(resolved));
        self.mark_points_dirty();
    }

    pub(crate) fn push_curve_point_snapped(&mut self, snapped: Pos2) {
        self.points.points.push(PickedPoint::new(snapped));
        self.mark_points_dirty();
    }

    pub(crate) fn resolve_curve_pick(&mut self, pixel_hint: Pos2) -> Pos2 {
        self.snap_pixel_if_requested(pixel_hint)
    }

    pub(crate) fn clear_all_points(&mut self) {
        self.points.points.clear();
        self.mark_points_dirty();
    }

    pub(crate) fn undo_last_point(&mut self) {
        if self.points.points.pop().is_some() {
            self.mark_points_dirty();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AngleDirection, AngleUnit, AxisCalibrationValues, AxisMapping, AxisValue, PolarMapping,
        PolarMappingParams, ScaleKind,
    };
    use egui::pos2;

    fn axis_mapping(start: f64, end: f64) -> AxisMapping {
        let values = AxisCalibrationValues::try_new(
            AxisValue::Float(start),
            AxisValue::Float(end),
            ScaleKind::Linear,
        )
        .unwrap();
        AxisMapping::try_new(pos2(0.0, 0.0), pos2(10.0, 0.0), values).unwrap()
    }

    fn assert_single_numeric_point(app: &mut CurcatApp, expected: (f64, f64)) {
        let [point] = app.sorted_numeric_points_cache() else {
            panic!("expected one numeric point");
        };
        assert!(
            (point.x - expected.0).abs() < 1.0e-6,
            "unexpected x: {point:?}"
        );
        assert!(
            (point.y - expected.1).abs() < 1.0e-6,
            "unexpected y: {point:?}"
        );
    }

    #[test]
    fn numeric_cache_tracks_partial_calibration_and_changed_values() {
        let mut app = CurcatApp::default();
        let pixel = pos2(5.0, 0.0);
        app.points.points.push(PickedPoint::new(pixel));
        let partial = CalibrationMapping::Cartesian {
            x: Some(axis_mapping(0.0, 10.0)),
            y: None,
        };
        assert!(!partial.is_ready());
        app.ensure_point_numeric_cache(partial);
        assert_eq!(app.sorted_preview_segments(), [(5.0, pixel)]);
        assert!(app.sorted_numeric_points_cache().is_empty());

        let complete = CalibrationMapping::Cartesian {
            x: Some(axis_mapping(0.0, 10.0)),
            y: Some(axis_mapping(0.0, 20.0)),
        };
        assert!(complete.is_ready());
        app.ensure_point_numeric_cache(complete);
        assert_single_numeric_point(&mut app, (5.0, 10.0));
        app.sorted_preview_segments();
        app.ensure_point_numeric_cache(complete);
        assert!(!app.points.sorted_numeric_dirty);
        assert!(!app.points.sorted_preview_dirty);

        let changed = CalibrationMapping::Cartesian {
            x: Some(axis_mapping(0.0, 30.0)),
            y: Some(axis_mapping(0.0, 20.0)),
        };
        app.ensure_point_numeric_cache(changed);
        assert_eq!(app.sorted_preview_segments(), [(15.0, pixel)]);
        assert_single_numeric_point(&mut app, (15.0, 10.0));
        app.ensure_point_numeric_cache(partial);
        assert!(app.sorted_numeric_points_cache().is_empty());
        assert_eq!(app.points.points[0].y_numeric, None);
    }

    #[test]
    fn numeric_cache_switches_coordinate_system_and_clears_missing_mapping() {
        let mut app = CurcatApp::default();
        app.points.points.push(PickedPoint::new(pos2(1.5, 0.0)));
        let cartesian = CalibrationMapping::Cartesian {
            x: Some(axis_mapping(0.0, 10.0)),
            y: Some(axis_mapping(0.0, 20.0)),
        };
        app.ensure_point_numeric_cache(cartesian);
        assert_single_numeric_point(&mut app, (1.5, 3.0));

        let polar = PolarMapping::try_new(PolarMappingParams {
            origin: Pos2::ZERO,
            radius_distance1: 1.0,
            radius_distance2: 2.0,
            radius_value1: 10.0,
            radius_value2: 20.0,
            radius_scale: ScaleKind::Linear,
            angle_pixel1: 0.0,
            angle_pixel2: std::f64::consts::FRAC_PI_2,
            angle_value1: 0.0,
            angle_value2: 90.0,
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Ccw,
        })
        .unwrap();
        app.ensure_point_numeric_cache(CalibrationMapping::Polar(Some(polar)));
        assert_single_numeric_point(&mut app, (0.0, 15.0));
        app.ensure_point_numeric_cache(CalibrationMapping::Polar(None));
        assert!(app.sorted_numeric_points_cache().is_empty());
        assert_eq!(app.sorted_preview_segments(), []);
        app.ensure_point_numeric_cache(cartesian);
        assert_single_numeric_point(&mut app, (1.5, 3.0));
    }
}
