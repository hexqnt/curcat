//! Export helpers for writing picked points to CSV, JSON, RON, and XLSX formats.

use crate::interp::XYPoint;
use crate::types::{AngleUnit, AxisUnit, AxisValue, CoordSystem};
use chrono::{Datelike, Duration, Timelike};
use ron::ser::PrettyConfig;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, XlsxError};
use serde::Serialize;
use serde::ser::Serializer;
use serde_json::{Map, Number, Value};
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};

/// Export-ready dataset plus axis units and optional computed columns.
#[derive(Debug, Clone)]
pub struct ExportPayload {
    pub points: Vec<XYPoint>,
    pub x_unit: AxisUnit,
    pub y_unit: AxisUnit,
    pub x_label: String,
    pub y_label: String,
    pub coord_system: CoordSystem,
    pub angle_unit: Option<AngleUnit>,
    pub extra_columns: Vec<ExportExtraColumn>,
}

/// Optional per-row numeric column aligned with the exported points.
#[derive(Debug, Clone)]
pub struct ExportExtraColumn {
    pub header: String,
    pub values: Vec<Option<f64>>,
}

impl ExportExtraColumn {
    /// Create a new extra column with a header and row-aligned values.
    pub fn new(header: impl Into<String>, values: Vec<Option<f64>>) -> Self {
        Self {
            header: header.into(),
            values,
        }
    }
}

impl ExportPayload {
    const fn row_count(&self) -> usize {
        self.points.len()
    }
}

/// Compute per-point distances to the previous point (first entry is `None`).
pub fn sequential_distances(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
    let len = raw_points.len();
    let mut values = vec![None; len];
    for i in 1..len {
        let prev = &raw_points[i - 1];
        let curr = &raw_points[i];
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        values[i] = Some(dx.hypot(dy));
    }
    values
}

/// Compute turning angles (degrees) at each interior point.
pub fn turning_angles(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
    let len = raw_points.len();
    let mut values = vec![None; len];
    if len < 3 {
        return values;
    }
    for i in 1..(len - 1) {
        let prev = &raw_points[i - 1];
        let curr = &raw_points[i];
        let next = &raw_points[i + 1];
        let v1 = (curr.x - prev.x, curr.y - prev.y);
        let v2 = (next.x - curr.x, next.y - curr.y);
        let mag1 = v1.0.hypot(v1.1);
        let mag2 = v2.0.hypot(v2.1);
        if mag1 <= f64::EPSILON || mag2 <= f64::EPSILON {
            continue;
        }
        let dot = v1.0 * v2.0 + v1.1 * v2.1;
        let cos_theta = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
        values[i] = Some(cos_theta.acos().to_degrees());
    }
    values
}

const XLSX_MAX_ROWS: u32 = 1_048_576;
const XLSX_MAX_COLS: u16 = 16_384;

/// Write the payload to CSV at the provided path.
///
/// Floats are formatted with 6 fractional digits; `DateTime` values are emitted
/// as formatted strings. Returns an error if any value is not representable.
pub fn export_to_csv(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    let mut headers = vec![payload.x_label.clone(), payload.y_label.clone()];
    headers.extend(payload.extra_columns.iter().map(|c| c.header.clone()));
    wtr.write_record(headers)?;

    for row_idx in 0..payload.row_count() {
        let p = &payload.points[row_idx];
        let xv = axis_value_from_scalar_for_export(payload.x_unit, p.x, "x")?;
        let yv = axis_value_from_scalar_for_export(payload.y_unit, p.y, "y")?;
        let mut record = vec![xv.format(), yv.format()];
        for col in &payload.extra_columns {
            debug_assert_eq!(col.values.len(), payload.row_count());
            let cell = col
                .values
                .get(row_idx)
                .and_then(|v| v.map(|val| format!("{val:.6}")))
                .unwrap_or_default();
            record.push(cell);
        }
        wtr.write_record(record)?;
    }
    wtr.flush()?;
    Ok(())
}

/// Write the payload to an Excel XLSX workbook at the provided path.
///
/// The export respects Excel row/column limits, splitting data across sheets
/// when needed. Non-finite numbers and unrepresentable datetimes return errors.
pub fn export_to_xlsx(path: &std::path::Path, payload: &ExportPayload) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let total_columns = payload.extra_columns.len().saturating_add(2);
    let total_columns_u16 = u16::try_from(total_columns)
        .map_err(|_| XlsxError::ParameterError("XLSX export exceeds column index range.".into()))?;
    if total_columns_u16 > XLSX_MAX_COLS {
        return Err(XlsxError::ParameterError(format!(
            "XLSX export has {total_columns} columns, exceeding Excel's {XLSX_MAX_COLS} column limit."
        )));
    }

    let max_rows_per_sheet = XLSX_MAX_ROWS.saturating_sub(1) as usize;
    let total_rows = payload.row_count();
    let sheet_count = if total_rows == 0 {
        1
    } else {
        total_rows.div_ceil(max_rows_per_sheet)
    };

    // Keep parity with CSV/JSON (6 fractional digits).
    let num_format = Format::new().set_num_format("0.000000");
    let datetime_format = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss.000");
    let blank_format = Format::new();

    for sheet_index in 0..sheet_count {
        let worksheet = workbook.add_worksheet();
        let sheet_name = if sheet_index == 0 {
            "Data".to_string()
        } else {
            format!("Data {}", sheet_index + 1)
        };
        worksheet.set_name(&sheet_name)?;

        worksheet.write_string(0, 0, &payload.x_label)?;
        worksheet.write_string(0, 1, &payload.y_label)?;
        for (idx, col) in payload.extra_columns.iter().enumerate() {
            let col_idx = u16::try_from(idx + 2)
                .map_err(|_| XlsxError::ParameterError("XLSX column index overflow.".into()))?;
            worksheet.write_string(0, col_idx, &col.header)?;
        }

        let start = sheet_index * max_rows_per_sheet;
        let end = (start + max_rows_per_sheet).min(total_rows);
        let slice = &payload.points[start..end];
        for (row_offset, p) in slice.iter().enumerate() {
            let row = u32::try_from(row_offset + 1)
                .map_err(|_| XlsxError::ParameterError("XLSX row index overflow.".into()))?;
            match payload.x_unit {
                AxisUnit::Float => {
                    if !p.x.is_finite() {
                        return Err(XlsxError::ParameterError(format!(
                            "XLSX export cannot represent non-finite x value {x}.",
                            x = p.x
                        )));
                    }
                    worksheet.write_number_with_format(row, 0, p.x, &num_format)?;
                }
                AxisUnit::DateTime => {
                    let xv = axis_value_from_scalar_for_xlsx(payload.x_unit, p.x, "x")?;
                    if let Some(excel_dt) = axis_value_to_excel_datetime(&xv) {
                        worksheet.write_datetime_with_format(
                            row,
                            0,
                            &excel_dt,
                            &datetime_format,
                        )?;
                    } else {
                        worksheet.write_string(row, 0, xv.format())?;
                    }
                }
            }

            match payload.y_unit {
                AxisUnit::Float => {
                    if !p.y.is_finite() {
                        return Err(XlsxError::ParameterError(format!(
                            "XLSX export cannot represent non-finite y value {y}.",
                            y = p.y
                        )));
                    }
                    worksheet.write_number_with_format(row, 1, p.y, &num_format)?;
                }
                AxisUnit::DateTime => {
                    let yv = axis_value_from_scalar_for_xlsx(payload.y_unit, p.y, "y")?;
                    if let Some(excel_dt) = axis_value_to_excel_datetime(&yv) {
                        worksheet.write_datetime_with_format(
                            row,
                            1,
                            &excel_dt,
                            &datetime_format,
                        )?;
                    } else {
                        worksheet.write_string(row, 1, yv.format())?;
                    }
                }
            }

            for (col_idx, col) in payload.extra_columns.iter().enumerate() {
                let col_num = u16::try_from(col_idx + 2)
                    .map_err(|_| XlsxError::ParameterError("XLSX column index overflow.".into()))?;
                debug_assert_eq!(col.values.len(), payload.row_count());
                match col.values.get(start + row_offset).and_then(|v| *v) {
                    Some(value) => {
                        if !value.is_finite() {
                            return Err(XlsxError::ParameterError(format!(
                                "XLSX export cannot represent non-finite value {value}."
                            )));
                        }
                        worksheet.write_number_with_format(row, col_num, value, &num_format)?;
                    }
                    None => {
                        worksheet.write_blank(row, col_num, &blank_format)?;
                    }
                }
            }
        }
    }

    workbook.save(path)
}

/// Write the payload to JSON at the provided path.
///
/// The output contains `x_unit`, `y_unit`, and a `points` array. Floats are
/// rounded to 6 fractional digits; `DateTime` values are emitted as strings.
pub fn export_to_json(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut points = Vec::with_capacity(payload.row_count());
    for row_idx in 0..payload.row_count() {
        let mut obj = Map::new();
        let p = &payload.points[row_idx];
        obj.insert(
            payload.x_label.clone(),
            axis_value_to_json(payload.x_unit, p.x, &payload.x_label)?,
        );
        obj.insert(
            payload.y_label.clone(),
            axis_value_to_json(payload.y_unit, p.y, &payload.y_label)?,
        );
        for col in &payload.extra_columns {
            debug_assert_eq!(col.values.len(), payload.row_count());
            let cell = col.values.get(row_idx).and_then(|v| *v);
            obj.insert(col.header.clone(), optional_number_json(cell));
        }
        points.push(Value::Object(obj));
    }

    let mut root = Map::new();
    root.insert(
        "coord_system".to_string(),
        Value::String(coord_system_label(payload.coord_system).to_string()),
    );
    root.insert(
        "x_unit".to_string(),
        Value::String(axis_unit_label(payload.x_unit).to_string()),
    );
    root.insert(
        "y_unit".to_string(),
        Value::String(axis_unit_label(payload.y_unit).to_string()),
    );
    root.insert(
        "x_label".to_string(),
        Value::String(payload.x_label.clone()),
    );
    root.insert(
        "y_label".to_string(),
        Value::String(payload.y_label.clone()),
    );
    if let Some(unit) = payload.angle_unit {
        root.insert(
            "angle_unit".to_string(),
            Value::String(angle_unit_label(unit).to_string()),
        );
    }
    root.insert("points".to_string(), Value::Array(points));

    let writer = BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer_pretty(writer, &Value::Object(root))?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct RonExport {
    coord_system: &'static str,
    x_unit: &'static str,
    y_unit: &'static str,
    x_label: String,
    y_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    angle_unit: Option<&'static str>,
    points: Vec<BTreeMap<String, RonValue>>,
}

#[derive(Debug, Clone)]
enum RonValue {
    Number(f64),
    String(String),
    None,
}

impl Serialize for RonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Number(value) => serializer.serialize_f64(*value),
            Self::String(value) => serializer.serialize_str(value),
            Self::None => serializer.serialize_none(),
        }
    }
}

/// Write the payload to RON at the provided path.
///
/// The output mirrors the JSON structure, using `None` for missing values.
pub fn export_to_ron(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut points = Vec::with_capacity(payload.row_count());
    for row_idx in 0..payload.row_count() {
        let mut row = BTreeMap::new();
        let p = &payload.points[row_idx];
        row.insert(
            payload.x_label.clone(),
            axis_value_to_ron(payload.x_unit, p.x, &payload.x_label)?,
        );
        row.insert(
            payload.y_label.clone(),
            axis_value_to_ron(payload.y_unit, p.y, &payload.y_label)?,
        );
        for col in &payload.extra_columns {
            debug_assert_eq!(col.values.len(), payload.row_count());
            let cell = col.values.get(row_idx).and_then(|v| *v);
            row.insert(col.header.clone(), optional_number_ron(cell));
        }
        points.push(row);
    }

    let doc = RonExport {
        coord_system: coord_system_label(payload.coord_system),
        x_unit: axis_unit_label(payload.x_unit),
        y_unit: axis_unit_label(payload.y_unit),
        x_label: payload.x_label.clone(),
        y_label: payload.y_label.clone(),
        angle_unit: payload.angle_unit.map(angle_unit_label),
        points,
    };

    let ron_string = ron::ser::to_string_pretty(&doc, PrettyConfig::default())?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(ron_string.as_bytes())?;
    Ok(())
}

const fn axis_unit_label(unit: AxisUnit) -> &'static str {
    match unit {
        AxisUnit::Float => "float",
        AxisUnit::DateTime => "datetime",
    }
}

const fn coord_system_label(system: CoordSystem) -> &'static str {
    match system {
        CoordSystem::Cartesian => "cartesian",
        CoordSystem::Polar => "polar",
    }
}

const fn angle_unit_label(unit: AngleUnit) -> &'static str {
    match unit {
        AngleUnit::Degrees => "deg",
        AngleUnit::Radians => "rad",
    }
}

fn axis_value_to_json(
    unit: AxisUnit,
    scalar_seconds: f64,
    axis_label: &str,
) -> anyhow::Result<Value> {
    match unit {
        AxisUnit::Float => {
            if !scalar_seconds.is_finite() {
                anyhow::bail!("Cannot export non-finite float value {scalar_seconds}.");
            }
            Ok(rounded_number_json(scalar_seconds))
        }
        AxisUnit::DateTime => {
            let value = axis_value_from_scalar_for_export(unit, scalar_seconds, axis_label)?;
            Ok(Value::String(value.format()))
        }
    }
}

fn axis_value_to_ron(
    unit: AxisUnit,
    scalar_seconds: f64,
    axis_label: &str,
) -> anyhow::Result<RonValue> {
    match unit {
        AxisUnit::Float => {
            if !scalar_seconds.is_finite() {
                anyhow::bail!("Cannot export non-finite float value {scalar_seconds}.");
            }
            Ok(number_to_ron_value(scalar_seconds))
        }
        AxisUnit::DateTime => {
            let value = axis_value_from_scalar_for_export(unit, scalar_seconds, axis_label)?;
            Ok(RonValue::String(value.format()))
        }
    }
}

fn optional_number_json(value: Option<f64>) -> Value {
    value.map_or(Value::Null, rounded_number_json)
}

fn optional_number_ron(value: Option<f64>) -> RonValue {
    value.map_or(RonValue::None, number_to_ron_value)
}

fn number_to_ron_value(value: f64) -> RonValue {
    let rounded = rounded_f64(value);
    if rounded.is_finite() {
        RonValue::Number(rounded)
    } else {
        RonValue::String(format!("{rounded}"))
    }
}

fn rounded_number_json(value: f64) -> Value {
    // Keep parity with CSV output: 6 fractional digits, rounded.
    let rounded = rounded_f64(value);
    Number::from_f64(rounded).map_or_else(|| Value::String(format!("{rounded}")), Value::Number)
}

fn rounded_f64(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn axis_value_to_excel_datetime(value: &AxisValue) -> Option<ExcelDateTime> {
    let AxisValue::DateTime(dt) = value else {
        return None;
    };
    let rounded = dt.and_utc() + Duration::nanoseconds(500_000);
    let year = u16::try_from(rounded.year()).ok()?;
    let month = u8::try_from(rounded.month()).ok()?;
    let day = u8::try_from(rounded.day()).ok()?;
    let hour = u16::try_from(rounded.hour()).ok()?;
    let minute = u8::try_from(rounded.minute()).ok()?;
    let second = u8::try_from(rounded.second()).ok()?;
    let millis = u16::try_from(rounded.timestamp_subsec_millis()).ok()?;
    let base = ExcelDateTime::from_ymd(year, month, day).ok()?;
    base.and_hms_milli(hour, minute, second, millis).ok()
}

fn axis_value_from_scalar_for_export(
    unit: AxisUnit,
    scalar: f64,
    axis_label: &str,
) -> anyhow::Result<AxisValue> {
    AxisValue::from_scalar_seconds(unit, scalar).ok_or_else(|| {
        anyhow::anyhow!(
            "Cannot export {axis_label} value {scalar}: not representable as {}.",
            axis_unit_label(unit)
        )
    })
}

fn axis_value_from_scalar_for_xlsx(
    unit: AxisUnit,
    scalar: f64,
    axis_label: &str,
) -> Result<AxisValue, XlsxError> {
    AxisValue::from_scalar_seconds(unit, scalar).ok_or_else(|| {
        XlsxError::ParameterError(format!(
            "XLSX export cannot represent {axis_label} value {scalar} as {}.",
            axis_unit_label(unit)
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ron::value::{Map, Value};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn map_value<'a>(map: &'a Map, key: &str) -> &'a Value {
        map.get(&Value::String(key.to_string()))
            .unwrap_or_else(|| panic!("missing key {key}"))
    }

    fn string_value<'a>(map: &'a Map, key: &str) -> &'a str {
        match map_value(map, key) {
            Value::String(value) => value,
            other => panic!("expected string for {key}, got {other:?}"),
        }
    }

    fn number_value(map: &Map, key: &str) -> f64 {
        match map_value(map, key) {
            Value::Number(value) => (*value).into_f64(),
            other => panic!("expected number for {key}, got {other:?}"),
        }
    }

    #[test]
    fn export_ron_rounds_and_preserves_none() {
        let payload = ExportPayload {
            points: vec![
                XYPoint {
                    x: 1.234_567_89,
                    y: 2.0,
                },
                XYPoint { x: 3.0, y: 4.0 },
            ],
            x_unit: AxisUnit::Float,
            y_unit: AxisUnit::Float,
            x_label: "X".to_string(),
            y_label: "Y".to_string(),
            coord_system: CoordSystem::Cartesian,
            angle_unit: None,
            extra_columns: vec![ExportExtraColumn::new(
                "extra",
                vec![None, Some(9.876_543_21)],
            )],
        };

        let path = std::env::temp_dir().join(format!(
            "curcat_ron_export_test_{}.ron",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        export_to_ron(&path, &payload).expect("RON export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read RON output");
        let parsed: Value = ron::de::from_str(&text).expect("failed to parse RON output");
        let _ = std::fs::remove_file(&path);

        let root = match parsed {
            Value::Map(map) => map,
            other => panic!("expected map root, got {other:?}"),
        };

        assert_eq!(string_value(&root, "coord_system"), "cartesian");
        assert_eq!(string_value(&root, "x_unit"), "float");
        assert_eq!(string_value(&root, "y_unit"), "float");
        assert_eq!(string_value(&root, "x_label"), "X");
        assert_eq!(string_value(&root, "y_label"), "Y");
        let angle_unit = root.get(&Value::String("angle_unit".to_string()));
        if let Some(value) = angle_unit {
            match value {
                Value::Option(None) => {}
                other => panic!("expected angle_unit None, got {other:?}"),
            }
        }

        let points = match map_value(&root, "points") {
            Value::Seq(values) => values,
            other => panic!("expected points array, got {other:?}"),
        };
        assert_eq!(points.len(), 2);

        let first = match &points[0] {
            Value::Map(map) => map,
            other => panic!("expected map point, got {other:?}"),
        };
        let second = match &points[1] {
            Value::Map(map) => map,
            other => panic!("expected map point, got {other:?}"),
        };

        let x0 = number_value(first, "X");
        let y0 = number_value(first, "Y");
        let extra0 = map_value(first, "extra");
        assert!((x0 - 1.234_568).abs() < 1e-9);
        assert!((y0 - 2.0).abs() < 1e-9);
        match extra0 {
            Value::Option(None) => {}
            other => panic!("expected extra None, got {other:?}"),
        }

        let x1 = number_value(second, "X");
        let y1 = number_value(second, "Y");
        let extra1 = number_value(second, "extra");
        assert!((x1 - 3.0).abs() < 1e-9);
        assert!((y1 - 4.0).abs() < 1e-9);
        assert!((extra1 - 9.876_543).abs() < 1e-9);
    }
}
