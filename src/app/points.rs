use super::{AxisMapping, CurcatApp};
use crate::interp::XYPoint;
use egui::Pos2;
use std::cmp::Ordering;

impl CurcatApp {
    pub(crate) const fn mark_points_dirty(&mut self) {
        self.points_numeric_dirty = true;
        self.sorted_preview_dirty = true;
        self.sorted_numeric_dirty = true;
    }

    pub(crate) fn ensure_point_numeric_cache(
        &mut self,
        x_mapping: Option<&AxisMapping>,
        y_mapping: Option<&AxisMapping>,
    ) {
        let mapping_changed =
            self.last_x_mapping.as_ref() != x_mapping || self.last_y_mapping.as_ref() != y_mapping;
        if mapping_changed {
            self.last_x_mapping = x_mapping.cloned();
            self.last_y_mapping = y_mapping.cloned();
            self.mark_points_dirty();
        }
        if self.points_numeric_dirty {
            for p in &mut self.points {
                p.x_numeric = x_mapping.and_then(|xm| xm.numeric_at(p.pixel));
                p.y_numeric = y_mapping.and_then(|ym| ym.numeric_at(p.pixel));
            }
            self.points_numeric_dirty = false;
        }
    }

    pub(crate) fn sorted_preview_segments(&mut self) -> &[(f64, Pos2)] {
        if self.sorted_preview_dirty {
            self.cached_sorted_preview.clear();
            for point in &self.points {
                if let Some(xn) = point.x_numeric {
                    self.cached_sorted_preview.push((xn, point.pixel));
                }
            }
            self.cached_sorted_preview
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
            self.sorted_preview_dirty = false;
        }
        &self.cached_sorted_preview
    }

    pub(crate) fn sorted_numeric_points_cache(&mut self) -> &[XYPoint] {
        if self.sorted_numeric_dirty {
            self.cached_sorted_numeric.clear();
            for point in &self.points {
                if let (Some(x), Some(y)) = (point.x_numeric, point.y_numeric) {
                    self.cached_sorted_numeric.push(XYPoint { x, y });
                }
            }
            self.cached_sorted_numeric
                .sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
            self.sorted_numeric_dirty = false;
        }
        &self.cached_sorted_numeric
    }

    pub(crate) fn push_curve_point(&mut self, pixel_hint: Pos2) {
        let resolved = self.resolve_curve_pick(pixel_hint);
        self.points.push(super::PickedPoint::new(resolved));
        self.mark_points_dirty();
    }

    pub(crate) fn push_curve_point_snapped(&mut self, snapped: Pos2) {
        self.points.push(super::PickedPoint::new(snapped));
        self.mark_points_dirty();
    }

    pub(crate) fn resolve_curve_pick(&mut self, pixel_hint: Pos2) -> Pos2 {
        self.snap_pixel_if_requested(pixel_hint)
    }

    pub(crate) fn clear_all_points(&mut self) {
        self.points.clear();
        self.mark_points_dirty();
    }

    pub(crate) fn undo_last_point(&mut self) {
        if self.points.pop().is_some() {
            self.mark_points_dirty();
        }
    }
}
