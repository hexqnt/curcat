//! Validated axis values and calibration coordinate mappings.

mod scalar;

use egui::Pos2;

use super::axis::{AxisUnit, AxisValue};
use super::coord::{AngleDirection, AngleUnit, ScaleKind};
use scalar::{ScalarMapping, ScalarMappingError};

/// Errors constructing Cartesian calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisMappingError {
    CoincidentPoints,
    NonFinitePoint,
    UnitValueMismatch,
    NonFiniteValue,
    EqualValues,
    LogScaleRequiresPositiveValues,
    LogScaleUnsupportedForDateTime,
}

/// Two values with matching units and a compatible scale; private fields preserve these invariants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCalibrationValues {
    unit: AxisUnit,
    decreasing: bool,
    numeric: ScalarMapping,
}

impl AxisCalibrationValues {
    /// Check value and scale compatibility; the values determine the axis unit.
    pub fn try_new(
        start: AxisValue,
        end: AxisValue,
        scale: ScaleKind,
    ) -> Result<Self, AxisMappingError> {
        match (start, end) {
            (AxisValue::Float(a), AxisValue::Float(b)) => {
                if (a - b).abs() <= f64::EPSILON {
                    return Err(AxisMappingError::EqualValues);
                }
            }
            (AxisValue::DateTime(a), AxisValue::DateTime(b)) => {
                if scale != ScaleKind::Linear {
                    return Err(AxisMappingError::LogScaleUnsupportedForDateTime);
                }
                if a == b {
                    return Err(AxisMappingError::EqualValues);
                }
            }
            _ => return Err(AxisMappingError::UnitValueMismatch),
        }
        let start_scalar = start.to_scalar();
        let end_scalar = end.to_scalar();
        let numeric =
            ScalarMapping::try_new(start_scalar, end_scalar, scale).map_err(
                |error| match error {
                    ScalarMappingError::NonFiniteValue => AxisMappingError::NonFiniteValue,
                    ScalarMappingError::NonPositiveLogValue => {
                        AxisMappingError::LogScaleRequiresPositiveValues
                    }
                },
            )?;
        Ok(Self {
            unit: start.unit(),
            decreasing: end_scalar < start_scalar,
            numeric,
        })
    }

    pub const fn unit(&self) -> AxisUnit {
        self.unit
    }
}

/// Calibration segment with validated geometry and axis values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisMapping {
    p1: Pos2,
    p2: Pos2,
    length_squared: f32,
    values: AxisCalibrationValues,
}

impl AxisMapping {
    /// Build calibration from validated values and a finite, nonzero segment.
    pub fn try_new(
        p1: Pos2,
        p2: Pos2,
        values: AxisCalibrationValues,
    ) -> Result<Self, AxisMappingError> {
        let length_squared = (p2 - p1).length_sq();
        if !p1.is_finite() || !p2.is_finite() || !length_squared.is_finite() {
            return Err(AxisMappingError::NonFinitePoint);
        }
        if length_squared <= f32::EPSILON {
            return Err(AxisMappingError::CoincidentPoints);
        }
        Ok(Self {
            p1,
            p2,
            length_squared,
            values,
        })
    }

    /// Unit vector toward increasing values; construction guarantees a nonzero segment.
    pub fn increasing_direction(&self) -> egui::Vec2 {
        let direction = (self.p2 - self.p1) / self.length_squared.sqrt();
        if self.values.decreasing {
            -direction
        } else {
            direction
        }
    }

    pub const fn unit(&self) -> AxisUnit {
        self.values.unit()
    }

    /// Projection parameter along the calibration segment; extrapolation extends beyond [0, 1].
    pub fn t_of_point(&self, point: Pos2) -> f64 {
        let direction = self.p2 - self.p1;
        f64::from((point - self.p1).dot(direction) / self.length_squared)
    }

    pub fn numeric_at(&self, point: Pos2) -> Option<f64> {
        self.numeric_at_t(self.t_of_point(point))
    }

    /// Return a finite scalar; endpoint logarithms are computed when calibration is constructed.
    pub fn numeric_at_t(&self, t: f64) -> Option<f64> {
        self.values.numeric.sample(t)
    }

    pub fn value_at(&self, point: Pos2) -> Option<AxisValue> {
        self.numeric_at(point)
            .and_then(|scalar| AxisValue::from_scalar(self.unit(), scalar))
    }
}

/// Errors constructing polar calibration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolarMappingError {
    NonFiniteInput,
    CoincidentRadiusPoints,
    EqualRadiusValues,
    LogScaleRequiresPositiveRadius,
    EqualAngleValues,
    ZeroAngleSpan,
}

/// Raw polar calibration parameters before checking invariants.
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

/// Polar calibration mapping pixel positions to angle and radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarMapping {
    origin: Pos2,
    radius_distance_start: f64,
    radius_distance_end: f64,
    radius_values: ScalarMapping,
    angle_pixel_start_rad: f64,
    angle_span_rad: f64,
    angle_value_start: f64,
    angle_value_end: f64,
    angle_unit: AngleUnit,
    angle_direction: AngleDirection,
}

impl PolarMapping {
    /// Check geometry and values once when constructing calibration.
    pub fn try_new(params: PolarMappingParams) -> Result<Self, PolarMappingError> {
        if !params.origin.is_finite()
            || !params.radius_distance1.is_finite()
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
        if (params.radius_value2 - params.radius_value1).abs() <= f64::EPSILON {
            return Err(PolarMappingError::EqualRadiusValues);
        }
        let radius_values = ScalarMapping::try_new(
            params.radius_value1,
            params.radius_value2,
            params.radius_scale,
        )
        .map_err(|error| match error {
            ScalarMappingError::NonFiniteValue => PolarMappingError::NonFiniteInput,
            ScalarMappingError::NonPositiveLogValue => {
                PolarMappingError::LogScaleRequiresPositiveRadius
            }
        })?;
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
            radius_distance_start: params.radius_distance1,
            radius_distance_end: params.radius_distance2,
            radius_values,
            angle_pixel_start_rad: a1,
            angle_span_rad: span,
            angle_value_start: params.angle_value1,
            angle_value_end: params.angle_value2,
            angle_unit: params.angle_unit,
            angle_direction: params.angle_direction,
        })
    }

    /// Return the calibrated radius at a pixel position.
    pub fn radius_at(&self, p: Pos2) -> Option<f64> {
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        let dist = dx.hypot(dy);
        let t = (dist - self.radius_distance_start)
            / (self.radius_distance_end - self.radius_distance_start);
        self.radius_values.sample(t)
    }

    /// Return the calibrated angle; the origin and non-finite positions have no defined angle.
    pub fn angle_at(&self, p: Pos2) -> Option<f64> {
        if !p.is_finite() {
            return None;
        }
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return None;
        }
        let raw = normalize_angle_rad(dy.atan2(dx));
        let delta = angle_delta(self.angle_pixel_start_rad, raw, self.angle_direction);
        let t = delta / self.angle_span_rad;
        let angle =
            (self.angle_value_end - self.angle_value_start).mul_add(t, self.angle_value_start);
        angle.is_finite().then_some(angle)
    }

    /// Unit of the calibrated angle values.
    pub const fn angle_unit(&self) -> AngleUnit {
        self.angle_unit
    }
}

fn normalize_angle_rad(angle: f64) -> f64 {
    // Normalize to [0, 2π).
    angle.rem_euclid(std::f64::consts::TAU)
}

fn angle_delta(start: f64, end: f64, direction: AngleDirection) -> f64 {
    // Positive angular delta in the chosen direction.
    match direction {
        AngleDirection::Ccw => (end - start).rem_euclid(std::f64::consts::TAU),
        AngleDirection::Cw => (start - end).rem_euclid(std::f64::consts::TAU),
    }
}
