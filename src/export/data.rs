//! Prepared export data with consistent coordinate metadata and column lengths.

use crate::interp::XYPoint;
use crate::types::{AngleUnit, AxisUnit, CoordSystem};
use std::fmt;

/// Polar export always has numeric axes and requires an angle unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportCoordinates {
    Cartesian { x_unit: AxisUnit, y_unit: AxisUnit },
    Polar { angle_unit: AngleUnit },
}

impl ExportCoordinates {
    const fn axis_units(self) -> [AxisUnit; 2] {
        match self {
            Self::Cartesian { x_unit, y_unit } => [x_unit, y_unit],
            Self::Polar { .. } => [AxisUnit::Float; 2],
        }
    }

    const fn system(self) -> CoordSystem {
        match self {
            Self::Cartesian { .. } => CoordSystem::Cartesian,
            Self::Polar { .. } => CoordSystem::Polar,
        }
    }

    pub const fn angle_unit(self) -> Option<AngleUnit> {
        match self {
            Self::Cartesian { .. } => None,
            Self::Polar { angle_unit } => Some(angle_unit),
        }
    }
}

/// Additional numeric column before checking row alignment.
#[derive(Debug, Clone)]
pub struct ExportExtraColumn {
    pub header: String,
    pub values: Vec<Option<f64>>,
}

impl ExportExtraColumn {
    pub fn new(header: impl Into<String>, values: Vec<Option<f64>>) -> Self {
        Self {
            header: header.into(),
            values,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraColumnLengthMismatch {
    pub column_index: usize,
    pub column_header: String,
    pub actual_rows: usize,
    pub expected_rows: usize,
}

impl fmt::Display for ExtraColumnLengthMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Extra column '{}' (index {}) has {} rows, expected {}.",
            self.column_header, self.column_index, self.actual_rows, self.expected_rows
        )
    }
}

impl std::error::Error for ExtraColumnLengthMismatch {}

/// Immutable serializer input; each extra column has exactly one cell per point.
#[derive(Debug, Clone)]
pub struct ExportPayload {
    points: Vec<XYPoint>,
    coordinates: ExportCoordinates,
    labels: [String; 2],
    extra_columns: Vec<ExportExtraColumn>,
}

impl ExportPayload {
    /// Check column lengths when preparing data, before opening an output file.
    pub fn try_new(
        points: Vec<XYPoint>,
        coordinates: ExportCoordinates,
        labels: [String; 2],
        extra_columns: Vec<ExportExtraColumn>,
    ) -> Result<Self, ExtraColumnLengthMismatch> {
        let expected_rows = points.len();
        if let Some((column_index, column)) = extra_columns
            .iter()
            .enumerate()
            .find(|(_, column)| column.values.len() != expected_rows)
        {
            return Err(ExtraColumnLengthMismatch {
                column_index,
                column_header: column.header.clone(),
                actual_rows: column.values.len(),
                expected_rows,
            });
        }
        Ok(Self {
            points,
            coordinates,
            labels,
            extra_columns,
        })
    }

    pub(super) const fn row_count(&self) -> usize {
        self.points.len()
    }

    pub(super) fn points(&self) -> &[XYPoint] {
        &self.points
    }

    pub(super) fn extra_columns(&self) -> &[ExportExtraColumn] {
        &self.extra_columns
    }

    pub(super) const fn x_unit(&self) -> AxisUnit {
        self.coordinates.axis_units()[0]
    }

    pub(super) const fn y_unit(&self) -> AxisUnit {
        self.coordinates.axis_units()[1]
    }

    pub(super) fn x_label(&self) -> &str {
        &self.labels[0]
    }

    pub(super) fn y_label(&self) -> &str {
        &self.labels[1]
    }

    pub(super) const fn coord_system(&self) -> CoordSystem {
        self.coordinates.system()
    }

    pub(super) const fn angle_unit(&self) -> Option<AngleUnit> {
        self.coordinates.angle_unit()
    }
}
