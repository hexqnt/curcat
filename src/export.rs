use crate::interp::XYPoint;
use crate::types::{AxisUnit, AxisValue};
use chrono::{Datelike, Duration, Timelike};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, XlsxError};

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

pub fn export_to_csv(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    let mut headers = vec!["x".to_string(), "y".to_string()];
    headers.extend(payload.extra_columns.iter().map(|c| c.header.clone()));
    wtr.write_record(headers)?;

    for row_idx in 0..payload.row_count() {
        let p = &payload.points[row_idx];
        let xv = AxisValue::from_scalar_seconds(payload.x_unit, p.x);
        let yv = AxisValue::from_scalar_seconds(payload.y_unit, p.y);
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
    let worksheet = workbook.add_worksheet();

    worksheet.write_string(0, 0, "x")?;
    worksheet.write_string(0, 1, "y")?;
    for (idx, col) in payload.extra_columns.iter().enumerate() {
        let col_idx = u16::try_from(idx + 2).expect("column index overflow");
        worksheet.write_string(0, col_idx, &col.header)?;
    }

    let num_format = Format::new().set_num_format("0.0000");
    let datetime_format = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss.000");
    let blank_format = Format::new();

    for (i, p) in payload.points.iter().enumerate() {
        let row = u32::try_from(i + 1).expect("row index overflow");
        match payload.x_unit {
            AxisUnit::Float => {
                worksheet.write_number_with_format(row, 0, p.x, &num_format)?;
            }
            AxisUnit::DateTime => {
                let xv = AxisValue::from_scalar_seconds(payload.x_unit, p.x);
                if let Some(excel_dt) = axis_value_to_excel_datetime(&xv) {
                    worksheet.write_datetime_with_format(row, 0, &excel_dt, &datetime_format)?;
                } else {
                    worksheet.write_string(row, 0, xv.format())?;
                }
            }
        }

        match payload.y_unit {
            AxisUnit::Float => {
                worksheet.write_number_with_format(row, 1, p.y, &num_format)?;
            }
            AxisUnit::DateTime => {
                let yv = AxisValue::from_scalar_seconds(payload.y_unit, p.y);
                if let Some(excel_dt) = axis_value_to_excel_datetime(&yv) {
                    worksheet.write_datetime_with_format(row, 1, &excel_dt, &datetime_format)?;
                } else {
                    worksheet.write_string(row, 1, yv.format())?;
                }
            }
        }

        for (col_idx, col) in payload.extra_columns.iter().enumerate() {
            let col_num = u16::try_from(col_idx + 2).expect("column index overflow");
            debug_assert_eq!(col.values.len(), payload.row_count());
            match col.values.get(i).and_then(|v| *v) {
                Some(value) => {
                    worksheet.write_number_with_format(row, col_num, value, &num_format)?;
                }
                None => {
                    worksheet.write_blank(row, col_num, &blank_format)?;
                }
            }
        }
    }

    workbook.save(path)
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
