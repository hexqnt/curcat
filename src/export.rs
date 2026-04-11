//! Export helpers for writing picked points to CSV, XLSX, JSON, RON, HTML, XML, and Markdown formats.

use crate::interp::XYPoint;
use crate::types::{AngleUnit, AxisUnit, AxisValue, CoordSystem};
use chrono::{Datelike, Duration, Timelike};
use maud::{DOCTYPE, html};
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

    pub fn export(self, path: &std::path::Path, payload: &ExportPayload) -> Result<(), String> {
        match self {
            Self::Csv => export_to_csv(path, payload).map_err(|e| e.to_string()),
            Self::Xlsx => export_to_xlsx(path, payload).map_err(|e| e.to_string()),
            Self::Json => export_to_json(path, payload).map_err(|e| e.to_string()),
            Self::Ron => export_to_ron(path, payload).map_err(|e| e.to_string()),
            Self::Html => export_to_html(path, payload).map_err(|e| e.to_string()),
            Self::Xml => export_to_xml(path, payload).map_err(|e| e.to_string()),
            Self::Markdown => export_to_markdown(path, payload).map_err(|e| e.to_string()),
        }
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
#[allow(clippy::suboptimal_flops)]
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

fn validate_extra_columns(payload: &ExportPayload) -> Result<(), String> {
    let expected_rows = payload.row_count();
    if let Some((index, column)) = payload
        .extra_columns
        .iter()
        .enumerate()
        .find(|(_, col)| col.values.len() != expected_rows)
    {
        return Err(format!(
            "Extra column '{}' (index {index}) has {} rows, expected {expected_rows}.",
            column.header,
            column.values.len()
        ));
    }
    Ok(())
}

struct TabularExport {
    headers: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

fn build_tabular_export(payload: &ExportPayload) -> anyhow::Result<TabularExport> {
    if let Err(err) = validate_extra_columns(payload) {
        anyhow::bail!(err);
    }
    let mut headers = vec![payload.x_label.clone(), payload.y_label.clone()];
    headers.extend(payload.extra_columns.iter().map(|c| c.header.clone()));

    let mut rows = Vec::with_capacity(payload.row_count());
    for row_idx in 0..payload.row_count() {
        let p = &payload.points[row_idx];
        let xv = axis_value_from_scalar_for_export(payload.x_unit, p.x, "x")?;
        let yv = axis_value_from_scalar_for_export(payload.y_unit, p.y, "y")?;

        let mut row = Vec::with_capacity(headers.len());
        row.push(Some(xv.format()));
        row.push(Some(yv.format()));
        for col in &payload.extra_columns {
            debug_assert_eq!(col.values.len(), payload.row_count());
            let cell = col
                .values
                .get(row_idx)
                .and_then(|v| *v)
                .map(format_extra_value);
            row.push(cell);
        }
        rows.push(row);
    }

    Ok(TabularExport { headers, rows })
}

fn format_extra_value(value: f64) -> String {
    format!("{value:.6}")
}

fn metadata_pairs(payload: &ExportPayload) -> Vec<(&'static str, String)> {
    let mut pairs = vec![
        (
            "coord_system",
            coord_system_label(payload.coord_system).to_string(),
        ),
        ("x_unit", axis_unit_label(payload.x_unit).to_string()),
        ("y_unit", axis_unit_label(payload.y_unit).to_string()),
        ("x_label", payload.x_label.clone()),
        ("y_label", payload.y_label.clone()),
    ];
    if let Some(unit) = payload.angle_unit {
        pairs.push(("angle_unit", angle_unit_label(unit).to_string()));
    }
    pairs
}

fn escape_xml_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_xml_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_markdown_cell(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len());
    for ch in normalized.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\n' => out.push_str("<br>"),
            _ => out.push(ch),
        }
    }
    out
}

/// Write the payload to CSV at the provided path.
///
/// Floats are formatted with 6 fractional digits; `DateTime` values are emitted
/// as formatted strings. Returns an error if any value is not representable.
pub fn export_to_csv(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let table = build_tabular_export(payload)?;
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(&table.headers)?;

    for row in table.rows {
        let record: Vec<String> = row.into_iter().map(Option::unwrap_or_default).collect();
        wtr.write_record(record)?;
    }
    wtr.flush()?;
    Ok(())
}

/// Write the payload to an HTML document containing metadata and a data table.
pub fn export_to_html(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let table = build_tabular_export(payload)?;
    let metadata = metadata_pairs(payload);
    let doc = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                title { "Curcat export" }
            }
            body {
                h1 { "Curcat export" }
                dl {
                    @for (name, value) in &metadata {
                        dt { (name) }
                        dd { (value) }
                    }
                }
                table {
                    thead {
                        tr {
                            @for header in &table.headers {
                                th { (header) }
                            }
                        }
                    }
                    tbody {
                        @for row in &table.rows {
                            tr {
                                @for cell in row {
                                    td {
                                        @if let Some(value) = cell {
                                            (value)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(doc.into_string().as_bytes())?;
    writer.flush()?;
    Ok(())
}

/// Write the payload to XML mirroring JSON metadata and point rows.
pub fn export_to_xml(path: &std::path::Path, payload: &ExportPayload) -> anyhow::Result<()> {
    let table = build_tabular_export(payload)?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);
    writer.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<curcat_export")?;
    for (name, value) in metadata_pairs(payload) {
        let name = escape_xml_attr(name);
        let value = escape_xml_attr(&value);
        write!(writer, " {name}=\"{value}\"")?;
    }
    writer.write_all(b">\n  <points>\n")?;

    for row in &table.rows {
        writer.write_all(b"    <point>\n")?;
        for (header, cell) in table.headers.iter().zip(row) {
            let escaped_header = escape_xml_attr(header);
            match cell {
                Some(value) => {
                    let escaped_value = escape_xml_text(value);
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
    let table = build_tabular_export(payload)?;
    let mut writer = BufWriter::new(std::fs::File::create(path)?);

    writer.write_all(b"|")?;
    for header in &table.headers {
        let escaped = escape_markdown_cell(header);
        write!(writer, " {escaped} |")?;
    }
    writer.write_all(b"\n|")?;
    for _ in &table.headers {
        writer.write_all(b" --- |")?;
    }
    writer.write_all(b"\n")?;

    for row in &table.rows {
        writer.write_all(b"|")?;
        for cell in row {
            let escaped = cell
                .as_deref()
                .map(escape_markdown_cell)
                .unwrap_or_default();
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
    if let Err(err) = validate_extra_columns(payload) {
        return Err(XlsxError::ParameterError(err));
    }
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
    if let Err(err) = validate_extra_columns(payload) {
        anyhow::bail!(err);
    }
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
    if let Err(err) = validate_extra_columns(payload) {
        anyhow::bail!(err);
    }
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
    use std::path::PathBuf;
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
    fn export_rejects_mismatched_extra_column_lengths() {
        let payload = ExportPayload {
            points: vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
            x_unit: AxisUnit::Float,
            y_unit: AxisUnit::Float,
            x_label: "X".to_string(),
            y_label: "Y".to_string(),
            coord_system: CoordSystem::Cartesian,
            angle_unit: None,
            extra_columns: vec![ExportExtraColumn::new("extra", vec![Some(1.0)])],
        };

        let check_err = validate_extra_columns(&payload).expect_err("must reject mismatch");
        assert!(check_err.contains("expected 2"));

        let path = temp_export_path("csv_export_mismatch_test", "csv");
        let export_err = export_to_csv(&path, &payload).expect_err("CSV export must fail");
        assert!(export_err.to_string().contains("expected 2"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_html_contains_doctype_metadata_and_escaping() {
        let payload = ExportPayload {
            points: vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
            x_unit: AxisUnit::Float,
            y_unit: AxisUnit::Float,
            x_label: "x<&\"'>".to_string(),
            y_label: "y".to_string(),
            coord_system: CoordSystem::Cartesian,
            angle_unit: None,
            extra_columns: vec![ExportExtraColumn::new("extra<&\"'>", vec![None, Some(7.5)])],
        };

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
        let payload = ExportPayload {
            points: vec![XYPoint { x: 1.0, y: 2.0 }],
            x_unit: AxisUnit::Float,
            y_unit: AxisUnit::Float,
            x_label: "x\"line\nnext".to_string(),
            y_label: "y".to_string(),
            coord_system: CoordSystem::Cartesian,
            angle_unit: None,
            extra_columns: vec![ExportExtraColumn::new("<extra&name>", vec![None])],
        };

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
        let payload = ExportPayload {
            points: vec![XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 3.0, y: 4.0 }],
            x_unit: AxisUnit::Float,
            y_unit: AxisUnit::Float,
            x_label: "x|\nhead".to_string(),
            y_label: "y\\head".to_string(),
            coord_system: CoordSystem::Cartesian,
            angle_unit: None,
            extra_columns: vec![ExportExtraColumn::new("c|d", vec![None, Some(5.1)])],
        };

        let path = temp_export_path("markdown_export_test", "md");
        export_to_markdown(&path, &payload).expect("Markdown export failed");
        let text = std::fs::read_to_string(&path).expect("failed to read Markdown output");
        let _ = std::fs::remove_file(&path);

        let expected = "| x\\|<br>head | y\\\\head | c\\|d |\n| --- | --- | --- |\n| 1 | 2 |  |\n| 3 | 4 | 5.100000 |\n";
        assert_eq!(text, expected);
    }
}
