//! Streaming JSON and RON serialization over borrowed, aligned export columns.

use super::{
    ExportPayload, angle_unit_label, axis_unit_label, axis_value_from_scalar_for_export,
    coord_system_label, rounded_f64,
};
use crate::interp::XYPoint;
use crate::types::{AxisUnit, AxisValue};
use serde::Serialize;
use serde::ser::{Error as _, SerializeMap, SerializeSeq, Serializer};
use std::collections::BTreeMap;

/// Validate axes before opening the destination; only column metadata is allocated.
#[derive(Serialize)]
pub(super) struct ExportDocument<'a> {
    coord_system: &'static str,
    x_unit: &'static str,
    y_unit: &'static str,
    x_label: &'a str,
    y_label: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    angle_unit: Option<&'static str>,
    points: ExportRows<'a>,
}

impl<'a> ExportDocument<'a> {
    pub(super) fn try_new(payload: &'a ExportPayload) -> anyhow::Result<Self> {
        for point in payload.points() {
            axis_value_from_scalar_for_export(payload.x_unit(), point.x, payload.x_label())?;
            axis_value_from_scalar_for_export(payload.y_unit(), point.y, payload.y_label())?;
        }
        // Resolve duplicate headers once, preserving the last-column-wins behavior of map exports.
        let mut columns = BTreeMap::new();
        columns.insert(payload.x_label(), ExportColumn::X(payload.x_unit()));
        columns.insert(payload.y_label(), ExportColumn::Y(payload.y_unit()));
        for column in payload.extra_columns() {
            columns.insert(column.header.as_str(), ExportColumn::Extra(&column.values));
        }
        Ok(Self {
            coord_system: coord_system_label(payload.coord_system()),
            x_unit: axis_unit_label(payload.x_unit()),
            y_unit: axis_unit_label(payload.y_unit()),
            x_label: payload.x_label(),
            y_label: payload.y_label(),
            angle_unit: payload.angle_unit().map(angle_unit_label),
            points: ExportRows {
                points: payload.points(),
                columns,
            },
        })
    }
}

struct ExportRows<'a> {
    points: &'a [XYPoint],
    columns: BTreeMap<&'a str, ExportColumn<'a>>,
}

enum ExportColumn<'a> {
    X(AxisUnit),
    Y(AxisUnit),
    Extra(&'a [Option<f64>]),
}

impl Serialize for ExportRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.points.len()))?;
        for (index, point) in self.points.iter().enumerate() {
            sequence.serialize_element(&ExportRow {
                point,
                index,
                columns: &self.columns,
            })?;
        }
        sequence.end()
    }
}

struct ExportRow<'a> {
    point: &'a XYPoint,
    index: usize,
    columns: &'a BTreeMap<&'a str, ExportColumn<'a>>,
}

impl Serialize for ExportRow<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.columns.len()))?;
        for (&header, column) in self.columns {
            let value = match column {
                ExportColumn::X(unit) => ExportValue::axis_value(*unit, self.point.x, header),
                ExportColumn::Y(unit) => ExportValue::axis_value(*unit, self.point.y, header),
                ExportColumn::Extra(values) => {
                    Ok(values[self.index].map_or(ExportValue::Missing, ExportValue::number))
                }
            }
            .map_err(S::Error::custom)?;
            map.serialize_entry(header, &value)?;
        }
        map.end()
    }
}

/// Missing cells become JSON null or RON None; non-finite extras remain textual.
enum ExportValue {
    Number(f64),
    Text(String),
    Missing,
}

impl ExportValue {
    fn axis_value(unit: AxisUnit, scalar: f64, label: &str) -> anyhow::Result<Self> {
        let value = axis_value_from_scalar_for_export(unit, scalar, label)?;
        Ok(match value {
            AxisValue::Float(value) => Self::number(value),
            value @ AxisValue::DateTime(_) => Self::Text(value.format()),
        })
    }

    fn number(value: f64) -> Self {
        let rounded = rounded_f64(value);
        if rounded.is_finite() {
            Self::Number(rounded)
        } else {
            Self::Text(rounded.to_string())
        }
    }
}

impl Serialize for ExportValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Number(value) => serializer.serialize_f64(*value),
            Self::Text(value) => serializer.serialize_str(value),
            Self::Missing => serializer.serialize_none(),
        }
    }
}
