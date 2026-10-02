//! Axis value parsing, formatting, and conversion helpers.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

const NANOS_PER_SEC: f64 = 1_000_000_000.0;
const NANOS_I64: i64 = 1_000_000_000;
const DEFAULT_FLOAT_DECIMALS: usize = 6;

// Timezone-aware formats (offset required), converted to UTC.
const TZ_FORMATS: [&str; 4] = [
    "%Y-%m-%d %H:%M:%S%.f%:z",
    "%Y-%m-%dT%H:%M:%S%.f%:z",
    "%Y/%m/%d %H:%M:%S%.f%:z",
    "%d.%m.%Y %H:%M:%S%.f%:z",
];

// Naive date-time formats without a timezone.
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

// Date-only formats (time defaults to 00:00:00).
const DATE_FORMATS: [&str; 5] = ["%Y-%m-%d", "%Y/%m/%d", "%d.%m.%Y", "%d/%m/%Y", "%m/%d/%Y"];

/// Units used for axis values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisUnit {
    /// Plain numeric axis values.
    Float,
    /// UTC timestamps stored as naive date-times.
    DateTime,
}

/// Axis value (floating-point number or timestamp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisValue {
    /// Numeric axis value (unitless or caller-defined).
    Float(f64),
    /// Date-time value interpreted as UTC when converted to seconds.
    DateTime(NaiveDateTime),
}

impl AxisValue {
    /// The value variant determines its unit without separate state.
    pub const fn unit(self) -> AxisUnit {
        match self {
            Self::Float(_) => AxisUnit::Float,
            Self::DateTime(_) => AxisUnit::DateTime,
        }
    }

    /// Scalar for interpolation and sorting: numeric values remain unchanged, dates become UTC seconds with a fractional part.
    pub fn to_scalar(self) -> f64 {
        match self {
            Self::Float(v) => v,
            Self::DateTime(dt) => {
                let utc = dt.and_utc();
                int_to_f64(utc.timestamp())
                    + f64::from(utc.timestamp_subsec_nanos()) / NANOS_PER_SEC
            }
        }
    }

    /// Reconstruct a number or date; non-finite scalars and dates outside chrono's range return `None`.
    pub fn from_scalar(unit: AxisUnit, scalar: f64) -> Option<Self> {
        if !scalar.is_finite() {
            return None;
        }

        match unit {
            AxisUnit::Float => Some(Self::Float(scalar)),
            AxisUnit::DateTime => {
                let secs_floor = scalar.floor();
                let mut secs = f64_to_i64_checked(secs_floor)?;
                let mut nanos =
                    f64_to_i64_checked(((scalar - secs_floor) * NANOS_PER_SEC).round())?;
                // Rounding can carry into the next/previous second; normalize to [0, 1e9).
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
    ///
    /// Floats are trimmed to remove trailing zeros; date-times are formatted as
    /// `YYYY-MM-DD HH:MM:SS[.fraction]`.
    pub fn format(&self) -> String {
        match self {
            Self::Float(v) => format_float(*v),
            Self::DateTime(dt) => format_datetime(dt),
        }
    }
}

fn format_float(value: f64) -> String {
    // Format with fixed decimals first, then trim trailing zeros and "-0".
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
    #[allow(clippy::cast_precision_loss)]
    let min = i64::MIN as f64;
    #[allow(clippy::cast_precision_loss)]
    let max = i64::MAX as f64;
    if value < min || value > max {
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

/// Parse a finite number or a date in common formats; timezone offsets are converted to UTC.
pub fn parse_axis_value(input: &str, unit: AxisUnit) -> Option<AxisValue> {
    match unit {
        AxisUnit::Float => input
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(AxisValue::Float),
        AxisUnit::DateTime => parse_datetime(input).map(AxisValue::DateTime),
    }
}

fn parse_datetime(input: &str) -> Option<NaiveDateTime> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // Try RFC3339 first, then timezone-aware formats, then naive date/time.
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
            return d.and_hms_opt(0, 0, 0);
        }
    }
    None
}
