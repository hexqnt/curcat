use super::{AxisMapping, CurcatApp};
use crate::interp::XYPoint;
use crate::types::{CoordSystem, PolarMapping};
use egui::Pos2;
use std::cmp::Ordering;

impl CurcatApp {
    pub(crate) const fn mark_points_dirty(&mut self) {
        self.points.points_numeric_dirty = true;
        self.points.sorted_preview_dirty = true;
        self.points.sorted_numeric_dirty = true;
    }

    pub(crate) fn ensure_point_numeric_cache(
        &mut self,
        coord_system: CoordSystem,
        x_mapping: Option<&AxisMapping>,
        y_mapping: Option<&AxisMapping>,
        polar_mapping: Option<&PolarMapping>,
    ) {
        let mapping_changed = match coord_system {
            CoordSystem::Cartesian => {
                self.points.last_coord_system != coord_system
                    || self.points.last_x_mapping.as_ref() != x_mapping
                    || self.points.last_y_mapping.as_ref() != y_mapping
            }
            CoordSystem::Polar => {
                self.points.last_coord_system != coord_system
                    || self.points.last_polar_mapping.as_ref() != polar_mapping
            }
        };
        if mapping_changed {
            self.points.last_coord_system = coord_system;
            self.points.last_x_mapping = x_mapping.cloned();
            self.points.last_y_mapping = y_mapping.cloned();
            self.points.last_polar_mapping = polar_mapping.cloned();
            self.mark_points_dirty();
        }
        if self.points.points_numeric_dirty {
            match coord_system {
                CoordSystem::Cartesian => {
                    for p in &mut self.points.points {
                        p.x_numeric = x_mapping.and_then(|xm| xm.numeric_at(p.pixel));
                        p.y_numeric = y_mapping.and_then(|ym| ym.numeric_at(p.pixel));
                    }
                }
                CoordSystem::Polar => {
                    for p in &mut self.points.points {
                        p.x_numeric = polar_mapping.and_then(|pm| pm.angle_at(p.pixel));
                        p.y_numeric = polar_mapping.and_then(|pm| pm.radius_at(p.pixel));
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
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
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
                .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
            self.points.sorted_numeric_dirty = false;
        }
        &self.points.cached_sorted_numeric
    }

    pub(crate) fn push_curve_point(&mut self, pixel_hint: Pos2) {
        let resolved = self.resolve_curve_pick(pixel_hint);
        self.points.points.push(super::PickedPoint::new(resolved));
        self.mark_points_dirty();
    }

    pub(crate) fn push_curve_point_snapped(&mut self, snapped: Pos2) {
        self.points.points.push(super::PickedPoint::new(snapped));
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
