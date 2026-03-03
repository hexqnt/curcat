//! Axis and polar mapping helpers for calibration.

use egui::Pos2;

use super::axis::{AxisUnit, AxisValue};
use super::coord::{AngleDirection, AngleUnit, ScaleKind};

/// Validation errors for cartesian axis mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisMappingError {
    CoincidentPoints,
    UnitValueMismatch,
    NonFiniteValue,
    EqualValues,
    LogScaleRequiresPositiveValues,
    LogScaleUnsupportedForDateTime,
}

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
    /// Build a validated axis mapping.
    pub fn try_new(
        p1: Pos2,
        p2: Pos2,
        v1: AxisValue,
        v2: AxisValue,
        scale: ScaleKind,
        unit: AxisUnit,
    ) -> Result<Self, AxisMappingError> {
        if (p2 - p1).length_sq() <= f32::EPSILON {
            return Err(AxisMappingError::CoincidentPoints);
        }
        Self::validate_value_pair(scale, unit, &v1, &v2)?;
        Ok(Self {
            p1,
            p2,
            v1,
            v2,
            scale,
            unit,
        })
    }

    /// Validate a value pair for the target unit/scale combination.
    pub fn validate_value_pair(
        scale: ScaleKind,
        unit: AxisUnit,
        v1: &AxisValue,
        v2: &AxisValue,
    ) -> Result<(), AxisMappingError> {
        match (unit, v1, v2) {
            (AxisUnit::Float, AxisValue::Float(a), AxisValue::Float(b)) => {
                if !a.is_finite() || !b.is_finite() {
                    return Err(AxisMappingError::NonFiniteValue);
                }
                if (*a - *b).abs() <= f64::EPSILON {
                    return Err(AxisMappingError::EqualValues);
                }
                if scale == ScaleKind::Log10 && (*a <= 0.0 || *b <= 0.0) {
                    return Err(AxisMappingError::LogScaleRequiresPositiveValues);
                }
                Ok(())
            }
            (AxisUnit::DateTime, AxisValue::DateTime(a), AxisValue::DateTime(b)) => {
                if scale != ScaleKind::Linear {
                    return Err(AxisMappingError::LogScaleUnsupportedForDateTime);
                }
                if a == b {
                    return Err(AxisMappingError::EqualValues);
                }
                Ok(())
            }
            _ => Err(AxisMappingError::UnitValueMismatch),
        }
    }

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

/// Validation errors for polar mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolarMappingError {
    NonFiniteInput,
    CoincidentRadiusPoints,
    LogScaleRequiresPositiveRadius,
    EqualAngleValues,
    ZeroAngleSpan,
}

/// Raw parameters required to build a polar mapping.
#[derive(Debug, Clone, Copy)]
pub struct PolarMappingParams {
    pub origin: Pos2,
    pub radius_distance1: f64,
    pub radius_distance2: f64,
    pub radius_value1: f64,
    pub radius_value2: f64,
    pub radius_scale: ScaleKind,
    pub angle_pixel1: f64,
    pub angle_pixel2: f64,
    pub angle_value1: f64,
    pub angle_value2: f64,
    pub angle_unit: AngleUnit,
    pub angle_direction: AngleDirection,
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
    /// Build a validated polar mapping from raw calibration parameters.
    pub fn try_new(params: PolarMappingParams) -> Result<Self, PolarMappingError> {
        if !params.radius_distance1.is_finite()
            || !params.radius_distance2.is_finite()
            || !params.radius_value1.is_finite()
            || !params.radius_value2.is_finite()
            || !params.angle_pixel1.is_finite()
            || !params.angle_pixel2.is_finite()
            || !params.angle_value1.is_finite()
            || !params.angle_value2.is_finite()
        {
            return Err(PolarMappingError::NonFiniteInput);
        }
        if (params.radius_distance2 - params.radius_distance1).abs() <= f64::EPSILON {
            return Err(PolarMappingError::CoincidentRadiusPoints);
        }
        if params.radius_scale == ScaleKind::Log10
            && (params.radius_value1 <= 0.0 || params.radius_value2 <= 0.0)
        {
            return Err(PolarMappingError::LogScaleRequiresPositiveRadius);
        }
        if (params.angle_value2 - params.angle_value1).abs() <= f64::EPSILON {
            return Err(PolarMappingError::EqualAngleValues);
        }
        let a1 = normalize_angle_rad(params.angle_pixel1);
        let a2 = normalize_angle_rad(params.angle_pixel2);
        let span = angle_delta(a1, a2, params.angle_direction);
        if span <= f64::EPSILON {
            return Err(PolarMappingError::ZeroAngleSpan);
        }
        Ok(Self {
            origin: params.origin,
            radius_d1: params.radius_distance1,
            radius_d2: params.radius_distance2,
            radius_v1: params.radius_value1,
            radius_v2: params.radius_value2,
            radius_scale: params.radius_scale,
            angle_a1: a1,
            angle_span: span,
            angle_v1: params.angle_value1,
            angle_v2: params.angle_value2,
            angle_unit: params.angle_unit,
            angle_direction: params.angle_direction,
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
