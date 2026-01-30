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

/// Coordinate system for calibration and export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordSystem {
    Cartesian,
    Polar,
}

/// Angle unit for polar calibration/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AngleUnit {
    Degrees,
    Radians,
}

impl AngleUnit {
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
    Ccw,
    Cw,
}

impl AngleDirection {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ccw => "CCW",
            Self::Cw => "CW",
        }
    }
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

/// Polar calibration mapping from pixels to (angle, radius).
#[derive(Debug, Clone, PartialEq)]
pub struct PolarMapping {
    pub origin: Pos2,
    radius_d1: f64,
    radius_d2: f64,
    radius_v1: f64,
    radius_v2: f64,
    radius_scale: ScaleKind,
    angle_a1: f64,
    angle_span: f64,
    angle_v1: f64,
    angle_v2: f64,
    angle_unit: AngleUnit,
    angle_direction: AngleDirection,
}

impl PolarMapping {
    pub fn new(
        origin: Pos2,
        radius_d1: f64,
        radius_d2: f64,
        radius_v1: f64,
        radius_v2: f64,
        radius_scale: ScaleKind,
        angle_a1: f64,
        angle_a2: f64,
        angle_v1: f64,
        angle_v2: f64,
        angle_unit: AngleUnit,
        angle_direction: AngleDirection,
    ) -> Option<Self> {
        if !radius_d1.is_finite()
            || !radius_d2.is_finite()
            || !radius_v1.is_finite()
            || !radius_v2.is_finite()
        {
            return None;
        }
        if (radius_d2 - radius_d1).abs() <= f64::EPSILON {
            return None;
        }
        if radius_scale == ScaleKind::Log10 && (radius_v1 <= 0.0 || radius_v2 <= 0.0) {
            return None;
        }
        if !angle_a1.is_finite()
            || !angle_a2.is_finite()
            || !angle_v1.is_finite()
            || !angle_v2.is_finite()
        {
            return None;
        }
        if (angle_v2 - angle_v1).abs() <= f64::EPSILON {
            return None;
        }
        let a1 = normalize_angle_rad(angle_a1);
        let a2 = normalize_angle_rad(angle_a2);
        let span = angle_delta(a1, a2, angle_direction);
        if span <= f64::EPSILON {
            return None;
        }
        Some(Self {
            origin,
            radius_d1,
            radius_d2,
            radius_v1,
            radius_v2,
            radius_scale,
            angle_a1: a1,
            angle_span: span,
            angle_v1,
            angle_v2,
            angle_unit,
            angle_direction,
        })
    }

    pub fn radius_at(&self, p: Pos2) -> Option<f64> {
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        let dist = dx.hypot(dy);
        let t = (dist - self.radius_d1) / (self.radius_d2 - self.radius_d1);
        numeric_at_t(self.radius_scale, self.radius_v1, self.radius_v2, t)
    }

    pub fn angle_at(&self, p: Pos2) -> Option<f64> {
        let dx = f64::from(p.x - self.origin.x);
        let dy = f64::from(p.y - self.origin.y);
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return None;
        }
        let raw = normalize_angle_rad(dy.atan2(dx));
        let delta = angle_delta(self.angle_a1, raw, self.angle_direction);
        let t = delta / self.angle_span;
        Some((self.angle_v2 - self.angle_v1).mul_add(t, self.angle_v1))
    }

    pub const fn angle_unit(&self) -> AngleUnit {
        self.angle_unit
    }
}

fn normalize_angle_rad(angle: f64) -> f64 {
    angle.rem_euclid(std::f64::consts::TAU)
}

fn angle_delta(start: f64, end: f64, direction: AngleDirection) -> f64 {
    match direction {
        AngleDirection::Ccw => (end - start).rem_euclid(std::f64::consts::TAU),
        AngleDirection::Cw => (start - end).rem_euclid(std::f64::consts::TAU),
    }
}

fn numeric_at_t(scale: ScaleKind, v1: f64, v2: f64, t: f64) -> Option<f64> {
    match scale {
        ScaleKind::Linear => Some((v2 - v1).mul_add(t, v1)),
        ScaleKind::Log10 => {
            if v1 <= 0.0 || v2 <= 0.0 {
                return None;
            }
            let l1 = v1.log10();
            let l2 = v2.log10();
            Some(10f64.powf((l2 - l1).mul_add(t, l1)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_float_trims_trailing_zeros_and_negative_zero() {
        assert_eq!(AxisValue::Float(12.340_000).format(), "12.34");
        assert_eq!(AxisValue::Float(5.0).format(), "5");
        assert_eq!(AxisValue::Float(-0.0).format(), "0");
    }

    #[test]
    fn parse_axis_value_accepts_date_and_timezone() {
        let AxisValue::DateTime(date_only) =
            parse_axis_value("2024-01-02", AxisUnit::DateTime).expect("date only parse")
        else {
            panic!("expected datetime");
        };
        let expected_date = NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(date_only, expected_date);

        let tz_input = "2024-01-02T03:04:05+02:00";
        let AxisValue::DateTime(with_tz) =
            parse_axis_value(tz_input, AxisUnit::DateTime).expect("tz parse")
        else {
            panic!("expected datetime");
        };
        let expected_tz = NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(1, 4, 5)
            .unwrap();
        assert_eq!(with_tz, expected_tz);
    }

    #[test]
    fn from_scalar_seconds_rounds_nanos_across_second() {
        let value = AxisValue::from_scalar_seconds(AxisUnit::DateTime, 1.999_999_999_6)
            .expect("valid datetime");
        let AxisValue::DateTime(dt) = value else {
            panic!("expected datetime");
        };
        let expected = DateTime::<Utc>::from_timestamp(2, 0)
            .expect("timestamp")
            .naive_utc();
        assert_eq!(dt, expected);
    }

    #[test]
    fn axis_mapping_log10_interpolates_midpoint() {
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::Float(1.0),
            v2: AxisValue::Float(100.0),
            scale: ScaleKind::Log10,
            unit: AxisUnit::Float,
        };
        let value = mapping.numeric_at_t(0.5).expect("log10 value");
        assert!((value - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn axis_mapping_log10_rejects_nonpositive_values() {
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::Float(-1.0),
            v2: AxisValue::Float(100.0),
            scale: ScaleKind::Log10,
            unit: AxisUnit::Float,
        };
        assert!(mapping.numeric_at_t(0.5).is_none());
    }

    #[test]
    fn axis_mapping_datetime_midpoint() {
        let start = DateTime::<Utc>::from_timestamp(0, 0)
            .expect("timestamp")
            .naive_utc();
        let end = DateTime::<Utc>::from_timestamp(86_400 * 2, 0)
            .expect("timestamp")
            .naive_utc();
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::DateTime(start),
            v2: AxisValue::DateTime(end),
            scale: ScaleKind::Linear,
            unit: AxisUnit::DateTime,
        };
        let value = mapping.value_at(Pos2::new(5.0, 0.0)).expect("value");
        let expected = AxisValue::DateTime(
            DateTime::<Utc>::from_timestamp(86_400, 0)
                .expect("timestamp")
                .naive_utc(),
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn polar_mapping_linear_deg_ccw() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::new(
            origin,
            1.0,
            2.0,
            10.0,
            20.0,
            ScaleKind::Linear,
            0.0,
            std::f64::consts::FRAC_PI_2,
            0.0,
            90.0,
            AngleUnit::Degrees,
            AngleDirection::Ccw,
        )
        .expect("valid mapping");

        let r = mapping.radius_at(Pos2::new(1.5, 0.0)).expect("radius");
        assert!((r - 15.0).abs() < 1.0e-6);

        let theta = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);
    }

    #[test]
    fn polar_mapping_cw_wraps_angles() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::new(
            origin,
            1.0,
            2.0,
            1.0,
            2.0,
            ScaleKind::Linear,
            0.0,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            90.0,
            AngleUnit::Degrees,
            AngleDirection::Cw,
        )
        .expect("valid mapping");

        let theta = mapping.angle_at(Pos2::new(0.0, -1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);

        let wrap = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((wrap - 270.0).abs() < 1.0e-6);
    }
}
