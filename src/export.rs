use crate::interp::XYPoint;
use crate::types::{AxisUnit, AxisValue};
use chrono::{Datelike, Duration, Timelike};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, XlsxError};
use serde_json::{Map, Number, Value, json};
use std::io::BufWriter;

#[derive(Debug, Clone)]
pub struct ExportPayload {
    pub points: Vec<XYPoint>,
    pub x_unit: AxisUnit,
    pub y_unit: AxisUnit,
    pub extra_columns: Vec<ExportExtraColumn>,
}

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

impl ExportPayload {
    const fn row_count(&self) -> usize {
        self.points.len()
    }
}

const XLSX_MAX_ROWS: u32 = 1_048_576;
const XLSX_MAX_COLS: u16 = 16_384;

pub fn export_to_csv(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    let mut headers = vec!["x".to_string(), "y".to_string()];
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

        worksheet.write_string(0, 0, "x")?;
        worksheet.write_string(0, 1, "y")?;
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

pub fn export_to_json(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut points = Vec::with_capacity(payload.row_count());
    for row_idx in 0..payload.row_count() {
        let mut obj = Map::new();
        let p = &payload.points[row_idx];
        obj.insert(
            "x".to_string(),
            axis_value_to_json(payload.x_unit, p.x, "x")?,
        );
        obj.insert(
            "y".to_string(),
            axis_value_to_json(payload.y_unit, p.y, "y")?,
        );
        for col in &payload.extra_columns {
            debug_assert_eq!(col.values.len(), payload.row_count());
            let cell = col.values.get(row_idx).and_then(|v| *v);
            obj.insert(col.header.clone(), optional_number_json(cell));
        }
        points.push(Value::Object(obj));
    }

    let root = json!({
        "x_unit": axis_unit_label(payload.x_unit),
        "y_unit": axis_unit_label(payload.y_unit),
        "points": points
    });

    let writer = BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer_pretty(writer, &root)?;
    Ok(())
}

const fn axis_unit_label(unit: AxisUnit) -> &'static str {
    match unit {
        AxisUnit::Float => "float",
        AxisUnit::DateTime => "datetime",
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

fn optional_number_json(value: Option<f64>) -> Value {
    value.map_or(Value::Null, rounded_number_json)
}

fn rounded_number_json(value: f64) -> Value {
    // Keep parity with CSV output: 6 fractional digits, rounded.
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    Number::from_f64(rounded).map_or_else(|| Value::String(format!("{rounded}")), Value::Number)
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
