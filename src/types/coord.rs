use serde::{Deserialize, Serialize};

/// Scale type for an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleKind {
    /// Linear interpolation in value space.
    Linear,
    /// Log10 interpolation (values must be strictly positive).
    Log10,
}

/// Coordinate system for calibration and export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordSystem {
    /// Cartesian coordinate system (x, y).
    Cartesian,
    /// Polar coordinate system (angle, radius).
    Polar,
}

/// Angle unit for polar calibration/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AngleUnit {
    /// Degrees (0-360).
    Degrees,
    /// Radians (0-2*pi).
    Radians,
}

impl AngleUnit {
    /// Short label used in UI/export metadata.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Degrees => "deg",
            Self::Radians => "rad",
        }
    }
}

/// Direction of increasing polar angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AngleDirection {
    /// Counter-clockwise angle increase.
    Ccw,
    /// Clockwise angle increase.
    Cw,
}

impl AngleDirection {
    /// Short label used in UI/export metadata.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ccw => "CCW",
            Self::Cw => "CW",
        }
    }
}
