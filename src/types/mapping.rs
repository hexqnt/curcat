//! Axis and polar mapping helpers for calibration.

use egui::Pos2;

use super::axis::{AxisUnit, AxisValue};
use super::coord::{AngleDirection, AngleUnit, ScaleKind};

/// Mapping between two calibration points and their axis values.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMapping {
    /// First calibration point in pixels.
    pub p1: Pos2,
    /// Second calibration point in pixels.
    pub p2: Pos2,
    /// Value at the first calibration point.
    pub v1: AxisValue,
    /// Value at the second calibration point.
    pub v2: AxisValue,
    /// Scale kind for the axis.
    pub scale: ScaleKind,
    /// Units for the axis.
    pub unit: AxisUnit,
}

impl AxisMapping {
    /// Parameter t of the point along the calibration segment (0..1).
    ///
    /// The value is computed by projecting onto the calibration line; degenerate
    /// segments return 0.0 to avoid division by zero.
    pub fn t_of_point(&self, p: Pos2) -> f64 {
        let d = self.p2 - self.p1;
        let v = p - self.p1;
        let denom = d.dot(d);
        if denom <= f32::EPSILON {
            0.0
        } else {
            f64::from(v.dot(d) / denom)
        }
    }

    /// Numeric axis value for a pixel position.
    pub fn numeric_at(&self, p: Pos2) -> Option<f64> {
        let t = self.t_of_point(p);
        self.numeric_at_t(t)
    }

    /// Numeric value along the axis at parameter t (0..1).
    ///
    /// Log10 scaling is only supported for `Float` units.
    pub fn numeric_at_t(&self, t: f64) -> Option<f64> {
        let s1 = self.v1.to_scalar_seconds();
        let s2 = self.v2.to_scalar_seconds();
        match (self.scale, self.unit) {
            (ScaleKind::Linear, _) => Some((s2 - s1).mul_add(t, s1)),
            (ScaleKind::Log10, AxisUnit::Float) => {
                // Log scale is undefined for non-positive values.
                if s1 <= 0.0 || s2 <= 0.0 {
                    return None;
                }
                let l1 = s1.log10();
                let l2 = s2.log10();
                Some(10f64.powf((l2 - l1).mul_add(t, l1)))
            }
            (ScaleKind::Log10, AxisUnit::DateTime) => None,
        }
    }

    /// Full axis value (with unit) for a pixel position.
    pub fn value_at(&self, p: Pos2) -> Option<AxisValue> {
        self.numeric_at(p)
            .and_then(|s| AxisValue::from_scalar_seconds(self.unit, s))
    }
}

/// Polar calibration mapping from pixels to (angle, radius).
#[derive(Debug, Clone, PartialEq)]
pub struct PolarMapping {
    /// Origin (center) of the polar system in pixels.
    pub origin: Pos2,
    radius_d1: f64,
    radius_d2: f64,
    radius_v1: f64,
    radius_v2: f64,
    radius_scale: ScaleKind,
    angle_a1: f64,
    angle_span: f64,
    angle_v1: f64,
    angle_v2: f64,
    angle_unit: AngleUnit,
    angle_direction: AngleDirection,
}

impl PolarMapping {
    /// Build a polar mapping from two radial and two angular calibration points.
    ///
    /// - `radius_d1/radius_d2` are pixel distances from the origin.
    /// - `radius_v1/radius_v2` are the corresponding radius values.
    /// - `angle_a1/angle_a2` are pixel angles in radians (from `atan2`).
    /// - `angle_v1/angle_v2` are the corresponding angle values in `angle_unit`.
    ///
    /// Returns `None` when inputs are non-finite, spans are zero, or when log
    /// scaling is requested with non-positive radius values.
    pub fn new(
        origin: Pos2,
        radius_d1: f64,
        radius_d2: f64,
        radius_v1: f64,
        radius_v2: f64,
        radius_scale: ScaleKind,
        angle_a1: f64,
        angle_a2: f64,
        angle_v1: f64,
        angle_v2: f64,
        angle_unit: AngleUnit,
        angle_direction: AngleDirection,
    ) -> Option<Self> {
        if !radius_d1.is_finite()
            || !radius_d2.is_finite()
            || !radius_v1.is_finite()
            || !radius_v2.is_finite()
        {
            return None;
        }
        if (radius_d2 - radius_d1).abs() <= f64::EPSILON {
            return None;
        }
        if radius_scale == ScaleKind::Log10 && (radius_v1 <= 0.0 || radius_v2 <= 0.0) {
            return None;
        }
        if !angle_a1.is_finite()
            || !angle_a2.is_finite()
            || !angle_v1.is_finite()
            || !angle_v2.is_finite()
        {
            return None;
        }
        if (angle_v2 - angle_v1).abs() <= f64::EPSILON {
            return None;
        }
        let a1 = normalize_angle_rad(angle_a1);
        let a2 = normalize_angle_rad(angle_a2);
        let span = angle_delta(a1, a2, angle_direction);
        if span <= f64::EPSILON {
            return None;
        }
        Some(Self {
            origin,
            radius_d1,
            radius_d2,
            radius_v1,
            radius_v2,
            radius_scale,
            angle_a1: a1,
            angle_span: span,
            angle_v1,
            angle_v2,
            angle_unit,
            angle_direction,
        })
    }

    /// Compute the radius value at a pixel position.
    pub fn radius_at(&self, p: Pos2) -> Option<f64> {
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        let dist = dx.hypot(dy);
        let t = (dist - self.radius_d1) / (self.radius_d2 - self.radius_d1);
        numeric_at_t(self.radius_scale, self.radius_v1, self.radius_v2, t)
    }

    /// Compute the angle value at a pixel position.
    ///
    /// Returns `None` if the point coincides with the origin (undefined angle).
    pub fn angle_at(&self, p: Pos2) -> Option<f64> {
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return None;
        }
        let raw = normalize_angle_rad(dy.atan2(dx));
        let delta = angle_delta(self.angle_a1, raw, self.angle_direction);
        let t = delta / self.angle_span;
        Some((self.angle_v2 - self.angle_v1).mul_add(t, self.angle_v1))
    }

    /// Metadata about the angle units used for `angle_v1/angle_v2` values.
    pub const fn angle_unit(&self) -> AngleUnit {
        self.angle_unit
    }
}

fn normalize_angle_rad(angle: f64) -> f64 {
    // Normalize to the [0, 2*pi) range.
    angle.rem_euclid(std::f64::consts::TAU)
}

fn angle_delta(start: f64, end: f64, direction: AngleDirection) -> f64 {
    // Return a positive delta in the selected direction.
    match direction {
        AngleDirection::Ccw => (end - start).rem_euclid(std::f64::consts::TAU),
        AngleDirection::Cw => (start - end).rem_euclid(std::f64::consts::TAU),
    }
}

fn numeric_at_t(scale: ScaleKind, v1: f64, v2: f64, t: f64) -> Option<f64> {
    match scale {
        ScaleKind::Linear => Some((v2 - v1).mul_add(t, v1)),
        ScaleKind::Log10 => {
            // Log scale is undefined for non-positive values.
            if v1 <= 0.0 || v2 <= 0.0 {
                return None;
            }
            let l1 = v1.log10();
            let l2 = v2.log10();
            Some(10f64.powf((l2 - l1).mul_add(t, l1)))
        }
    }
}
