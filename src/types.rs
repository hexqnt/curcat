use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use egui::Pos2;
use serde::{Deserialize, Serialize};

const NANOS_PER_SEC: f64 = 1_000_000_000.0;
const NANOS_I64: i64 = 1_000_000_000;
const DEFAULT_FLOAT_DECIMALS: usize = 6;

const TZ_FORMATS: [&str; 4] = [
    "%Y-%m-%d %H:%M:%S%.f%:z",
    "%Y-%m-%dT%H:%M:%S%.f%:z",
    "%Y/%m/%d %H:%M:%S%.f%:z",
    "%d.%m.%Y %H:%M:%S%.f%:z",
];

const DATETIME_FORMATS: [&str; 12] = [
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%dT%H:%M:%S",
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%dT%H:%M",
    "%Y/%m/%d %H:%M:%S",
    "%Y/%m/%d %H:%M:%S%.f",
    "%Y/%m/%d %H:%M",
    "%d.%m.%Y %H:%M:%S",
    "%d.%m.%Y %H:%M:%S%.f",
    "%d.%m.%Y %H:%M",
];

const DATE_FORMATS: [&str; 5] = ["%Y-%m-%d", "%Y/%m/%d", "%d.%m.%Y", "%d/%m/%Y", "%m/%d/%Y"];

/// Scale type for an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleKind {
    Linear,
    Log10,
}

/// Units used for axis values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisUnit {
    Float,
    DateTime,
}

/// Axis value (floating-point number or timestamp).
#[derive(Debug, Clone, PartialEq)]
pub enum AxisValue {
    Float(f64),
    DateTime(NaiveDateTime),
}

impl AxisValue {
    /// Convert to scalar seconds for interpolation and sorting.
    pub fn to_scalar_seconds(&self) -> f64 {
        match self {
            Self::Float(v) => *v,
            Self::DateTime(dt) => {
                let utc = dt.and_utc();
                int_to_f64(utc.timestamp())
                    + f64::from(utc.timestamp_subsec_nanos()) / NANOS_PER_SEC
            }
        }
    }

    /// Recreate an `AxisValue` from scalar seconds using the requested unit.
    pub fn from_scalar_seconds(unit: AxisUnit, s: f64) -> Option<Self> {
        if !s.is_finite() {
            return None;
        }
        match unit {
            AxisUnit::Float => Some(Self::Float(s)),
            AxisUnit::DateTime => {
                let secs_floor = s.floor();
                let mut secs = f64_to_i64_checked(secs_floor)?;
                let mut nanos = f64_to_i64_checked(((s - secs_floor) * NANOS_PER_SEC).round())?;
                if nanos >= NANOS_I64 {
                    nanos -= NANOS_I64;
                    secs = secs.saturating_add(1);
                } else if nanos < 0 {
                    nanos += NANOS_I64;
                    secs = secs.saturating_sub(1);
                }
                let nanos = non_negative_i64_to_u32(nanos.clamp(0, NANOS_I64 - 1));
                DateTime::<Utc>::from_timestamp(secs, nanos)
                    .map(|dt| Self::DateTime(dt.naive_utc()))
            }
        }
    }

    /// Format the value for display or export.
    pub fn format(&self) -> String {
        match self {
            Self::Float(v) => format_float(*v),
            Self::DateTime(dt) => format_datetime(dt),
        }
    }
}

fn format_float(value: f64) -> String {
    let mut text = format!("{value:.DEFAULT_FLOAT_DECIMALS$}");
    if let Some(dot) = text.find('.') {
        let mut end = text.len();
        while end > dot + 1 && text.as_bytes()[end - 1] == b'0' {
            end -= 1;
        }
        if end > dot + 1 {
            text.truncate(end);
        } else {
            text.truncate(dot);
        }
    }
    if text == "-0" { "0".to_string() } else { text }
}

const fn int_to_f64(value: i64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn f64_to_i64_checked(value: f64) -> Option<i64> {
    if !value.is_finite() {
        return None;
    }
    if value < i64::MIN as f64 || value > i64::MAX as f64 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    {
        Some(value as i64)
    }
}

const fn non_negative_i64_to_u32(value: i64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value as u32
    }
}

fn format_datetime(dt: &NaiveDateTime) -> String {
    let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
    let nanos = dt.and_utc().timestamp_subsec_nanos();
    if nanos == 0 {
        base
    } else {
        let mut frac = format!("{nanos:09}");
        while frac.ends_with('0') {
            frac.pop();
        }
        format!("{base}.{frac}")
    }
}

/// Parse a string into an axis value using the given unit.
pub fn parse_axis_value(input: &str, unit: AxisUnit) -> Option<AxisValue> {
    match unit {
        AxisUnit::Float => input.trim().parse::<f64>().ok().map(AxisValue::Float),
        AxisUnit::DateTime => parse_datetime(input).map(AxisValue::DateTime),
    }
}

fn parse_datetime(input: &str) -> Option<NaiveDateTime> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_utc());
    }
    for fmt in TZ_FORMATS {
        if let Ok(dt) = chrono::DateTime::parse_from_str(s, fmt) {
            return Some(dt.naive_utc());
        }
    }
    for fmt in DATETIME_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt);
        }
    }
    for fmt in DATE_FORMATS {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some(d.and_hms_opt(0, 0, 0).unwrap());
        }
    }
    None
}

/// Mapping between two calibration points and their axis values.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMapping {
    /// First calibration point in pixels.
    pub p1: Pos2,
    /// Second calibration point in pixels.
    pub p2: Pos2,
    /// Value at the first calibration point.
    pub v1: AxisValue,
    /// Value at the second calibration point.
    pub v2: AxisValue,
    /// Scale kind for the axis.
    pub scale: ScaleKind,
    /// Units for the axis.
    pub unit: AxisUnit,
}

impl AxisMapping {
    /// Parameter t of the point along the calibration segment (0..1).
    pub fn t_of_point(&self, p: Pos2) -> f64 {
        let d = self.p2 - self.p1;
        let v = p - self.p1;
        let denom = d.dot(d);
        if denom <= f32::EPSILON {
            0.0
        } else {
            f64::from(v.dot(d) / denom)
        }
    }

    /// Numeric axis value for a pixel position.
    pub fn numeric_at(&self, p: Pos2) -> Option<f64> {
        let t = self.t_of_point(p);
        self.numeric_at_t(t)
    }

    /// Numeric value along the axis at parameter t (0..1).
    pub fn numeric_at_t(&self, t: f64) -> Option<f64> {
        let s1 = self.v1.to_scalar_seconds();
        let s2 = self.v2.to_scalar_seconds();
        match (self.scale, self.unit) {
            (ScaleKind::Linear, _) => Some((s2 - s1).mul_add(t, s1)),
            (ScaleKind::Log10, AxisUnit::Float) => {
                if s1 <= 0.0 || s2 <= 0.0 {
                    return None;
                }
                let l1 = s1.log10();
                let l2 = s2.log10();
                Some(10f64.powf((l2 - l1).mul_add(t, l1)))
            }
            (ScaleKind::Log10, AxisUnit::DateTime) => None,
        }
    }

    /// Full axis value (with unit) for a pixel position.
    pub fn value_at(&self, p: Pos2) -> Option<AxisValue> {
        self.numeric_at(p)
            .and_then(|s| AxisValue::from_scalar_seconds(self.unit, s))
    }
}
