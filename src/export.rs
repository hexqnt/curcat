//! Export helpers for writing picked points to CSV, XLSX, JSON, RON, HTML, XML, and Markdown formats.

mod data;
mod escape;
mod structured;

pub use data::{ExportCoordinates, ExportExtraColumn, ExportPayload, ExtraColumnLengthMismatch};

use escape::{escape_html_text, escape_markdown_cell, escape_xml_attr, escape_xml_text};

use crate::interp::XYPoint;
use crate::types::{AngleUnit, AxisUnit, AxisValue, CoordSystem};
use chrono::{Datelike, Duration, Timelike};
use ron::ser::PrettyConfig;
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, XlsxError};
use std::io::{BufWriter, Write};
use structured::ExportDocument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Xlsx,
    Json,
    Ron,
    Html,
    Xml,
    Markdown,
}

impl ExportFormat {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Xlsx => "Excel",
            Self::Json => "JSON",
            Self::Ron => "RON",
            Self::Html => "HTML",
            Self::Xml => "XML",
            Self::Markdown => "Markdown",
        }
    }

    pub const fn default_filename(self) -> &'static str {
        match self {
            Self::Csv => "curve.csv",
            Self::Xlsx => "curve.xlsx",
            Self::Json => "curve.json",
            Self::Ron => "curve.ron",
            Self::Html => "curve.html",
            Self::Xml => "curve.xml",
            Self::Markdown => "curve.md",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Json => "json",
            Self::Ron => "ron",
            Self::Html => "html",
            Self::Xml => "xml",
            Self::Markdown => "md",
        }
    }

    pub fn export(self, path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
        match self {
            Self::Csv => export_to_csv(path, payload),
            Self::Xlsx => export_to_xlsx(path, payload).map_err(Into::into),
            Self::Json => export_to_json(path, payload),
            Self::Ron => export_to_ron(path, payload),
            Self::Html => export_to_html(path, payload),
            Self::Xml => export_to_xml(path, payload),
            Self::Markdown => export_to_markdown(path, payload),
        }
    }
}

/// Compute per-point distances to the previous point (first entry is `None`).
pub fn sequential_distances(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
    let mut values = Vec::with_capacity(raw_points.len());
    if raw_points.is_empty() {
        return values;
    }
    values.push(None);
    values.extend(raw_points.array_windows::<2>().map(|[prev, curr]| {
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        Some(dx.hypot(dy))
    }));
    values
}

/// Compute turning angles (degrees) at each interior point.
#[allow(clippy::suboptimal_flops)]
pub fn turning_angles(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
    let len = raw_points.len();
    let mut values = vec![None; len];
    if len < 3 {
        return values;
    }
    for (slot, [prev, curr, next]) in values[1..len - 1]
        .iter_mut()
        .zip(raw_points.array_windows::<3>())
    {
        let v1 = (curr.x - prev.x, curr.y - prev.y);
        let v2 = (next.x - curr.x, next.y - curr.y);
        let mag1 = v1.0.hypot(v1.1);
        let mag2 = v2.0.hypot(v2.1);
        if mag1 <= f64::EPSILON || mag2 <= f64::EPSILON {
            continue;
        }
        let dot = v1.0 * v2.0 + v1.1 * v2.1;
        let cos_theta = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
        *slot = Some(cos_theta.acos().to_degrees());
    }
    values
}

const XLSX_MAX_ROWS: u32 = 1_048_576;
const XLSX_MAX_COLS: u16 = 16_384;

/// Axis values are checked before writing a row; extra cells are formatted as they are consumed.
struct TabularRow<'a> {
    axes: [String; 2],
    extra_columns: &'a [ExportExtraColumn],
    index: usize,
}

impl TabularRow<'_> {
    fn cells(self) -> impl Iterator<Item = Option<String>> {
        self.axes.into_iter().map(Some).chain(
            self.extra_columns
                .iter()
                .map(move |column| column.values[self.index].map(format_extra_value)),
        )
    }
}

impl ExportPayload {
    fn headers(&self) -> impl Iterator<Item = &str> {
        [self.x_label(), self.y_label()].into_iter().chain(
            self.extra_columns()
                .iter()
                .map(|column| column.header.as_str()),
        )
    }

    fn formatted_rows(&self) -> impl Iterator<Item = anyhow::Result<TabularRow<'_>>> {
        self.points().iter().enumerate().map(|(index, point)| {
            let x = axis_value_from_scalar_for_export(self.x_unit(), point.x, "x")?;
            let y = axis_value_from_scalar_for_export(self.y_unit(), point.y, "y")?;
            Ok(TabularRow {
                axes: [x.format(), y.format()],
                extra_columns: self.extra_columns(),
                index,
            })
        })
    }
}

fn format_extra_value(value: f64) -> String {
    format!("{value:.6}")
}

fn metadata_pairs(payload: &ExportPayload) -> impl Iterator<Item = (&'static str, &str)> {
    [
        ("coord_system", coord_system_label(payload.coord_system())),
        ("x_unit", axis_unit_label(payload.x_unit())),
        ("y_unit", axis_unit_label(payload.y_unit())),
        ("x_label", payload.x_label()),
        ("y_label", payload.y_label()),
    ]
    .into_iter()
    .chain(
        payload
            .angle_unit()
            .map(|unit| ("angle_unit", angle_unit_label(unit))),
    )
}

/// Write the payload to CSV at the provided path.
///
/// Floats are formatted with 6 fractional digits; `DateTime` values are emitted
/// as formatted strings. Returns an error if any value is not representable.
pub fn export_to_csv(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(payload.headers())?;

    for row in payload.formatted_rows() {
        let row = row?;
        wtr.write_record(row.cells().map(Option::unwrap_or_default))?;
    }
    wtr.flush()?;
    Ok(())
}

/// Write the payload to an HTML document containing metadata and a data table.
pub fn export_to_html(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(
        b"<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Curcat export</title></head><body><h1>Curcat export</h1><dl>",
    )?;
    for (name, value) in metadata_pairs(payload) {
        write!(
            writer,
            "<dt>{}</dt><dd>{}</dd>",
            escape_html_text(name),
            escape_html_text(value)
        )?;
    }
    writer.write_all(b"</dl><table><thead><tr>")?;
    for header in payload.headers() {
        write!(writer, "<th>{}</th>", escape_html_text(header))?;
    }
    writer.write_all(b"</tr></thead><tbody>")?;
    for row in payload.formatted_rows() {
        writer.write_all(b"<tr>")?;
        for cell in row?.cells() {
            writer.write_all(b"<td>")?;
            if let Some(value) = cell {
                write!(writer, "{}", escape_html_text(&value))?;
            }
            writer.write_all(b"</td>")?;
        }
        writer.write_all(b"</tr>")?;
    }
    writer.write_all(b"</tbody></table></body></html>")?;
    writer.flush()?;
    Ok(())
}

/// Write the payload to XML mirroring JSON metadata and point rows.
pub fn export_to_xml(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<curcat_export")?;
    for (name, value) in metadata_pairs(payload) {
        let name = escape_xml_attr(name);
        let value = escape_xml_attr(value);
        write!(writer, " {name}=\"{value}\"")?;
    }
    writer.write_all(b">\n  <points>\n")?;

    for row in payload.formatted_rows() {
        let row = row?;
        writer.write_all(b"    <point>\n")?;
        for (header, cell) in payload.headers().zip(row.cells()) {
            let escaped_header = escape_xml_attr(header);
            match cell {
                Some(value) => {
                    let escaped_value = escape_xml_text(&value);
                    writeln!(
                        writer,
                        "      <field name=\"{escaped_header}\">{escaped_value}</field>"
                    )?;
                }
                None => {
                    writeln!(writer, "      <field name=\"{escaped_header}\"/>")?;
                }
            }
        }
        writer.write_all(b"    </point>\n")?;
    }

    writer.write_all(b"  </points>\n</curcat_export>\n")?;
    writer.flush()?;
    Ok(())
}

/// Write the payload as a Markdown table.
pub fn export_to_markdown(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let mut writer = BufWriter::new(std::fs::File::create(path)?);

    writer.write_all(b"|")?;
    for header in payload.headers() {
        let escaped = escape_markdown_cell(header);
        write!(writer, " {escaped} |")?;
    }
    writer.write_all(b"\n|")?;
    for _ in payload.headers() {
        writer.write_all(b" --- |")?;
    }
    writer.write_all(b"\n")?;

    for row in payload.formatted_rows() {
        let row = row?;
        writer.write_all(b"|")?;
        for cell in row.cells() {
            let escaped = escape_markdown_cell(cell.as_deref().unwrap_or_default());
            write!(writer, " {escaped} |")?;
        }
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

/// Write the payload to an Excel XLSX workbook at the provided path.
///
/// The export respects Excel row/column limits, splitting data across sheets
/// when needed. Non-finite numbers and unrepresentable datetimes return errors.
#[allow(clippy::too_many_lines)]
pub fn export_to_xlsx(path: &std::path::Path, payload: &ExportPayload) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    let total_columns = payload.extra_columns().len().saturating_add(2);
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

        worksheet.write_string(0, 0, payload.x_label())?;
        worksheet.write_string(0, 1, payload.y_label())?;
        for (idx, col) in payload.extra_columns().iter().enumerate() {
            let col_idx = u16::try_from(idx + 2)
                .map_err(|_| XlsxError::ParameterError("XLSX column index overflow.".into()))?;
            worksheet.write_string(0, col_idx, &col.header)?;
        }

        let start = sheet_index * max_rows_per_sheet;
        let end = (start + max_rows_per_sheet).min(total_rows);
        let slice = &payload.points()[start..end];
        for (row_offset, p) in slice.iter().enumerate() {
            let row = u32::try_from(row_offset + 1)
                .map_err(|_| XlsxError::ParameterError("XLSX row index overflow.".into()))?;
            for (column, axis_label, unit, value) in [
                (0, "x", payload.x_unit(), p.x),
                (1, "y", payload.y_unit(), p.y),
            ] {
                match unit {
                    AxisUnit::Float => {
                        if !value.is_finite() {
                            return Err(XlsxError::ParameterError(format!(
                                "XLSX export cannot represent non-finite {axis_label} value {value}."
                            )));
                        }
                        worksheet.write_number_with_format(row, column, value, &num_format)?;
                    }
                    AxisUnit::DateTime => {
                        let axis_value = axis_value_from_scalar_for_xlsx(unit, value, axis_label)?;
                        if let Some(excel_dt) = axis_value_to_excel_datetime(&axis_value) {
                            worksheet.write_datetime_with_format(
                                row,
                                column,
                                &excel_dt,
                                &datetime_format,
                            )?;
                        } else {
                            worksheet.write_string(row, column, axis_value.format())?;
                        }
                    }
                }
            }

            for (col_idx, column) in payload.extra_columns().iter().enumerate() {
                let col_num = u16::try_from(col_idx + 2)
                    .map_err(|_| XlsxError::ParameterError("XLSX column index overflow.".into()))?;
                match column.values[start + row_offset] {
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
    let document = ExportDocument::try_new(payload)?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer_pretty(&mut writer, &document)?;
    writer.flush()?;
    Ok(())
}

/// Write the payload to RON at the provided path.
///
/// The output mirrors the JSON structure, using `None` for missing values.
pub fn export_to_ron(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let document = ExportDocument::try_new(payload)?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    ron::Options::default().to_io_writer_pretty(&mut writer, &document, PrettyConfig::default())?;
    writer.flush()?;
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

fn rounded_f64(value: f64) -> f64 {
    // Large finite floats already have no fractional digits; scaling them would overflow.
    if value.abs() >= f64::MAX / 1_000_000.0 {
        value
    } else {
        (value * 1_000_000.0).round() / 1_000_000.0
    }
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
    AxisValue::from_scalar(unit, scalar).ok_or_else(|| {
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
    AxisValue::from_scalar(unit, scalar).ok_or_else(|| {
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn structured_exports_preserve_duplicate_headers_dates_and_nonfinite_extras() {
        let payload = ExportPayload::try_new(
            vec![XYPoint { x: 0.0, y: 1.0 }, XYPoint { x: 1.5, y: 2.0 }],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::DateTime,
                y_unit: AxisUnit::Float,
            },
            ["time\"\nUTC".into(), "value".into()],
            vec![
                ExportExtraColumn::new("value", vec![Some(-1.0), Some(-2.0)]),
                ExportExtraColumn::new("value", vec![None, Some(3.123_456_78)]),
                ExportExtraColumn::new("special", vec![Some(f64::NAN), Some(f64::INFINITY)]),
                ExportExtraColumn::new("large", vec![Some(f64::MAX), Some(-f64::MAX)]),
            ],
        )
        .unwrap();
        let json_path = temp_export_path("streamed_columns", "json");
        export_to_json(&json_path, &payload).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&json_path).unwrap()).unwrap();
        std::fs::remove_file(json_path).unwrap();
        assert_eq!(json["points"][0]["time\"\nUTC"], "1970-01-01 00:00:00");
        assert_eq!(json["points"][1]["time\"\nUTC"], "1970-01-01 00:00:01.5");
        assert_eq!(json["points"][0]["value"], serde_json::Value::Null);
        assert_eq!(json["points"][1]["value"], 3.123_457);
        assert_eq!(json["points"][0]["special"], "NaN");
        assert_eq!(json["points"][1]["special"], "inf");
        assert_eq!(json["points"][0]["large"].as_f64(), Some(f64::MAX));
        assert_eq!(json["points"][1]["large"].as_f64(), Some(-f64::MAX));

        let ron_path = temp_export_path("streamed_columns", "ron");
        export_to_ron(&ron_path, &payload).unwrap();
        let ron: Value = ron::from_str(&std::fs::read_to_string(&ron_path).unwrap()).unwrap();
        std::fs::remove_file(ron_path).unwrap();
        let Value::Map(root) = ron else {
            panic!("expected root map")
        };
        let Value::Seq(points) = map_value(&root, "points") else {
            panic!("expected point sequence")
        };
        let Value::Map(first) = &points[0] else {
            panic!("expected point map")
        };
        let Value::Map(second) = &points[1] else {
            panic!("expected point map")
        };
        assert_eq!(string_value(first, "time\"\nUTC"), "1970-01-01 00:00:00");
        assert_eq!(string_value(second, "time\"\nUTC"), "1970-01-01 00:00:01.5");
        assert_eq!(map_value(first, "value"), &Value::Option(None));
        assert_eq!(number_value(second, "value"), 3.123_457);
        assert_eq!(string_value(first, "special"), "NaN");
        assert_eq!(string_value(second, "special"), "inf");
        assert_eq!(number_value(first, "large"), f64::MAX);
        assert_eq!(number_value(second, "large"), -f64::MAX);
    }

    #[test]
    fn invalid_structured_axes_do_not_truncate_the_destination() {
        for format in [ExportFormat::Json, ExportFormat::Ron] {
            for (unit, value) in [(AxisUnit::Float, f64::NAN), (AxisUnit::DateTime, f64::MAX)] {
                let payload = ExportPayload::try_new(
                    vec![XYPoint { x: value, y: 1.0 }],
                    ExportCoordinates::Cartesian {
                        x_unit: unit,
                        y_unit: AxisUnit::Float,
                    },
                    ["x".into(), "y".into()],
                    Vec::new(),
                )
                .unwrap();
                let path = temp_export_path("invalid_structured_axis", format.extension());
                std::fs::write(&path, b"existing data").unwrap();
                assert!(format.export(&path, &payload).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), b"existing data");
                std::fs::remove_file(path).unwrap();
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn structured_exports_report_buffer_flush_failures() {
        let payload = ExportPayload::try_new(
            Vec::new(),
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["x".into(), "y".into()],
            Vec::new(),
        )
        .unwrap();
        for format in [ExportFormat::Json, ExportFormat::Ron] {
            assert!(
                format
                    .export(std::path::Path::new("/dev/full"), &payload)
                    .is_err()
            );
        }
    }

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

    fn temp_export_path(stem: &str, ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "curcat_{stem}_{}.{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos(),
            ext
        ))
    }

    #[test]
    fn export_csv_preserves_quoting_dates_and_empty_cells() {
        let payload = ExportPayload::try_new(
            vec![
                XYPoint { x: 0.0, y: -0.0 },
                XYPoint {
                    x: 1.5,
                    y: 1.234_567_89,
                },
            ],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::DateTime,
                y_unit: AxisUnit::Float,
            },
            ["Время, UTC".into(), "Y\"value".into()],
            vec![ExportExtraColumn::new(
                "extra\ncolumn",
                vec![None, Some(2.5)],
            )],
        )
        .expect("valid export payload");
        let path = temp_export_path("csv_quoting", "csv");
        export_to_csv(&path, &payload).expect("CSV export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read CSV output");
        std::fs::remove_file(path).expect("failed to remove CSV output");
        assert_eq!(
            text,
            "\"Время, UTC\",\"Y\"\"value\",\"extra\ncolumn\"\n1970-01-01 00:00:00,0,\n1970-01-01 00:00:01.5,1.234568,2.500000\n"
        );
    }

    #[test]
    fn export_ron_rounds_and_preserves_none() {
        let payload = ExportPayload::try_new(
            vec![
                XYPoint {
                    x: 1.234_567_89,
                    y: 2.0,
                },
                XYPoint { x: 3.0, y: 4.0 },
            ],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["X".to_string(), "Y".to_string()],
            vec![ExportExtraColumn::new(
                "extra",
                vec![None, Some(9.876_543_21)],
            )],
        )
        .expect("valid export payload");

        let path = temp_export_path("ron_export_test", "ron");
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

    #[test]
    fn payload_rejects_mismatched_extra_column_lengths() {
        for actual_rows in [0, 1, 3] {
            let error = ExportPayload::try_new(
                vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
                ExportCoordinates::Cartesian {
                    x_unit: AxisUnit::Float,
                    y_unit: AxisUnit::Float,
                },
                ["X".into(), "Y".into()],
                vec![
                    ExportExtraColumn::new("valid", vec![None; 2]),
                    ExportExtraColumn::new("extra", vec![Some(1.0); actual_rows]),
                ],
            )
            .expect_err("must reject mismatch");
            assert_eq!(
                error,
                ExtraColumnLengthMismatch {
                    column_index: 1,
                    column_header: "extra".into(),
                    actual_rows,
                    expected_rows: 2,
                }
            );
            assert!(error.to_string().contains("expected 2"));
        }
    }

    #[test]
    fn empty_payload_preserves_headers_and_empty_json_points() {
        let payload = ExportPayload::try_new(
            Vec::new(),
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["x".into(), "y".into()],
            vec![ExportExtraColumn::new("extra", Vec::new())],
        )
        .unwrap();
        let csv_path = temp_export_path("empty_payload", "csv");
        ExportFormat::Csv.export(&csv_path, &payload).unwrap();
        assert_eq!(std::fs::read_to_string(&csv_path).unwrap(), "x,y,extra\n");
        std::fs::remove_file(csv_path).unwrap();
        let json_path = temp_export_path("empty_payload", "json");
        ExportFormat::Json.export(&json_path, &payload).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&json_path).unwrap()).unwrap();
        std::fs::remove_file(json_path).unwrap();
        assert_eq!(json["points"], serde_json::json!([]));
        assert!(json.get("angle_unit").is_none());
    }

    #[test]
    fn polar_export_preserves_angle_metadata_in_json_and_ron() {
        for (angle_unit, expected_unit) in
            [(AngleUnit::Degrees, "deg"), (AngleUnit::Radians, "rad")]
        {
            let payload = ExportPayload::try_new(
                vec![XYPoint { x: 1.0, y: 2.0 }],
                ExportCoordinates::Polar { angle_unit },
                ["theta".into(), "radius".into()],
                Vec::new(),
            )
            .unwrap();
            let json_path = temp_export_path("polar_metadata", "json");
            ExportFormat::Json.export(&json_path, &payload).unwrap();
            let json: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&json_path).unwrap()).unwrap();
            std::fs::remove_file(json_path).unwrap();
            assert_eq!(json["coord_system"], "polar");
            assert_eq!(json["x_unit"], "float");
            assert_eq!(json["y_unit"], "float");
            assert_eq!(json["angle_unit"], expected_unit);
            assert_eq!(json["points"][0]["theta"], 1.0);
            assert_eq!(json["points"][0]["radius"], 2.0);

            let ron_path = temp_export_path("polar_metadata", "ron");
            ExportFormat::Ron.export(&ron_path, &payload).unwrap();
            let ron: Value = ron::from_str(&std::fs::read_to_string(&ron_path).unwrap()).unwrap();
            std::fs::remove_file(ron_path).unwrap();
            let Value::Map(root) = ron else {
                panic!("expected RON map");
            };
            assert_eq!(string_value(&root, "coord_system"), "polar");
            assert_eq!(string_value(&root, "x_unit"), "float");
            assert_eq!(string_value(&root, "y_unit"), "float");
            let Value::Option(Some(unit)) = map_value(&root, "angle_unit") else {
                panic!("expected optional angle unit");
            };
            assert_eq!(unit.as_ref(), &Value::String(expected_unit.to_owned()));
        }
    }

    #[test]
    fn export_format_preserves_underlying_xlsx_error() {
        for (x_unit, y_unit, x, y, axis_label) in [
            (AxisUnit::Float, AxisUnit::Float, f64::NAN, 1.0, "x"),
            (AxisUnit::Float, AxisUnit::Float, 1.0, f64::INFINITY, "y"),
            (AxisUnit::DateTime, AxisUnit::Float, f64::MAX, 1.0, "x"),
            (AxisUnit::Float, AxisUnit::DateTime, 1.0, f64::MAX, "y"),
        ] {
            let payload = ExportPayload::try_new(
                vec![XYPoint { x, y }],
                ExportCoordinates::Cartesian { x_unit, y_unit },
                ["x".into(), "y".into()],
                Vec::new(),
            )
            .unwrap();
            let path = temp_export_path("xlsx_unrepresentable_axis", "xlsx");
            let error = ExportFormat::Xlsx
                .export(&path, &payload)
                .expect_err("invalid XLSX axis value must fail");
            assert!(matches!(
                error.downcast_ref::<XlsxError>(),
                Some(XlsxError::ParameterError(_))
            ));
            assert!(error.to_string().contains(&format!("{axis_label} value")));
            assert!(!path.exists());
        }
    }

    #[test]
    fn export_html_contains_doctype_metadata_and_escaping() {
        let payload = ExportPayload::try_new(
            vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["x<&\"'>".to_string(), "y".to_string()],
            vec![ExportExtraColumn::new("extra<&\"'>", vec![None, Some(7.5)])],
        )
        .expect("valid export payload");

        let path = temp_export_path("html_export_test", "html");
        export_to_html(&path, &payload).expect("HTML export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read HTML output");
        let _ = std::fs::remove_file(&path);

        assert!(text.to_ascii_lowercase().contains("<!doctype html>"));
        assert!(text.contains("<dl>"));
        assert!(
            text.contains("<dt>x_label</dt><dd>x&lt;&amp;&quot;'&gt;</dd>")
                || text.contains("<dt>x_label</dt><dd>x&lt;&amp;&quot;&#39;&gt;</dd>")
        );
        assert!(
            text.contains("<th>x&lt;&amp;&quot;'&gt;</th>")
                || text.contains("<th>x&lt;&amp;&quot;&#39;&gt;</th>")
        );
        assert!(
            text.contains("<th>extra&lt;&amp;&quot;'&gt;</th>")
                || text.contains("<th>extra&lt;&amp;&quot;&#39;&gt;</th>")
        );
        assert!(text.contains("<td></td>"));
        assert!(text.contains("<td>7.500000</td>"));
    }

    #[test]
    fn export_xml_contains_metadata_points_and_escaping() {
        let payload = ExportPayload::try_new(
            vec![XYPoint { x: 1.0, y: 2.0 }],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["x\"line\nnext".to_string(), "y".to_string()],
            vec![ExportExtraColumn::new("<extra&name>", vec![None])],
        )
        .expect("valid export payload");

        let path = temp_export_path("xml_export_test", "xml");
        export_to_xml(&path, &payload).expect("XML export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read XML output");
        let _ = std::fs::remove_file(&path);

        assert!(text.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(
            text.contains(
                "<curcat_export coord_system=\"cartesian\" x_unit=\"float\" y_unit=\"float\" x_label=\"x&quot;line&#10;next\" y_label=\"y\">"
            )
        );
        assert!(text.contains("<points>"));
        assert!(text.contains("<point>"));
        assert!(text.contains("<field name=\"x&quot;line&#10;next\">1</field>"));
        assert!(text.contains("<field name=\"&lt;extra&amp;name&gt;\"/>"));
    }

    #[test]
    fn export_markdown_writes_table_and_escapes_special_symbols() {
        let payload = ExportPayload::try_new(
            vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
            ExportCoordinates::Cartesian {
                x_unit: AxisUnit::Float,
                y_unit: AxisUnit::Float,
            },
            ["x|\nhead".to_string(), "y\\head".to_string()],
            vec![ExportExtraColumn::new("c|d", vec![None, Some(5.1)])],
        )
        .expect("valid export payload");

        let path = temp_export_path("markdown_export_test", "md");
        export_to_markdown(&path, &payload).expect("Markdown export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read Markdown output");
        let _ = std::fs::remove_file(&path);

        let expected = "| x\\|<br>head | y\\\\head | c\\|d |\n| --- | --- | --- |\n| 1 | 2 |  |\n| 3 | 4 | 5.100000 |\n";
        assert_eq!(text, expected);
    }
}
