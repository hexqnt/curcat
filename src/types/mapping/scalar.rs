//! Shared scalar interpolation for Cartesian and polar calibration.

use crate::types::ScaleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScalarMappingError {
    NonFiniteValue,
    NonPositiveLogValue,
}

/// Finite endpoints are checked at construction; logarithmic interpolation stores their precomputed logarithms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ScalarMapping(ScalarInterpolation);

#[derive(Debug, Clone, Copy, PartialEq)]
enum ScalarInterpolation {
    Linear { start: f64, end: f64 },
    Log10 { start_log10: f64, end_log10: f64 },
}

impl ScalarMapping {
    pub(super) fn try_new(
        start: f64,
        end: f64,
        scale: ScaleKind,
    ) -> Result<Self, ScalarMappingError> {
        if !start.is_finite() || !end.is_finite() {
            return Err(ScalarMappingError::NonFiniteValue);
        }
        match scale {
            ScaleKind::Linear => Ok(Self(ScalarInterpolation::Linear { start, end })),
            ScaleKind::Log10 if start > 0.0 && end > 0.0 => Ok(Self(ScalarInterpolation::Log10 {
                start_log10: start.log10(),
                end_log10: end.log10(),
            })),
            ScaleKind::Log10 => Err(ScalarMappingError::NonPositiveLogValue),
        }
    }

    pub(super) fn sample(self, t: f64) -> Option<f64> {
        if !t.is_finite() {
            return None;
        }
        let value = match self.0 {
            ScalarInterpolation::Linear { start, end } => (end - start).mul_add(t, start),
            ScalarInterpolation::Log10 {
                start_log10,
                end_log10,
            } => 10f64.powf((end_log10 - start_log10).mul_add(t, start_log10)),
        };
        value.is_finite().then_some(value)
    }
}
