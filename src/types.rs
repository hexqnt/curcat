use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use egui::Pos2;

const NANOS_PER_SEC: f64 = 1_000_000_000.0;
const NANOS_I64: i64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleKind {
    Linear,
    Log10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisUnit {
    Float,
    DateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AxisValue {
    Float(f64),
    DateTime(NaiveDateTime),
}

impl AxisValue {
    pub fn to_scalar_seconds(&self) -> f64 {
        match self {
            AxisValue::Float(v) => *v,
            AxisValue::DateTime(dt) => {
                let utc = dt.and_utc();
                int_to_f64(utc.timestamp())
                    + f64::from(utc.timestamp_subsec_nanos()) / NANOS_PER_SEC
            }
        }
    }

    pub fn from_scalar_seconds(unit: AxisUnit, s: f64) -> Self {
        match unit {
            AxisUnit::Float => AxisValue::Float(s),
            AxisUnit::DateTime => {
                let secs_floor = s.floor();
                let mut secs = float_to_i64(secs_floor);
                let mut nanos = float_to_i64(((s - secs_floor) * NANOS_PER_SEC).round());
                if nanos >= NANOS_I64 {
                    nanos -= NANOS_I64;
                    secs = secs.saturating_add(1);
                } else if nanos < 0 {
                    nanos += NANOS_I64;
                    secs = secs.saturating_sub(1);
                }
                let nanos = non_negative_i64_to_u32(nanos.clamp(0, NANOS_I64 - 1));
                let base = DateTime::<Utc>::from_timestamp(secs, nanos).map_or_else(
                    || DateTime::<Utc>::UNIX_EPOCH.naive_utc(),
                    |dt| dt.naive_utc(),
                );
                AxisValue::DateTime(base)
            }
        }
    }

    pub fn format(&self) -> String {
        match self {
            AxisValue::Float(v) => format!("{v}"),
            AxisValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

fn int_to_f64(value: i64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

fn float_to_i64(value: f64) -> i64 {
    #[allow(clippy::cast_possible_truncation)]
    {
        value as i64
    }
}

fn non_negative_i64_to_u32(value: i64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value as u32
    }
}

pub fn parse_axis_value(input: &str, unit: AxisUnit) -> Option<AxisValue> {
    match unit {
        AxisUnit::Float => input.trim().parse::<f64>().ok().map(AxisValue::Float),
        AxisUnit::DateTime => parse_datetime(input).map(AxisValue::DateTime),
    }
}

fn parse_datetime(input: &str) -> Option<NaiveDateTime> {
    let s = input.trim();
    // Try several common formats
    let fmts = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
        "%d.%m.%Y %H:%M:%S",
        "%d.%m.%Y %H:%M",
        "%Y-%m-%d",
        "%d.%m.%Y",
        "%Y/%m/%d",
    ];
    for f in fmts {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Some(dt);
        }
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            // Assume midnight
            return Some(d.and_hms_opt(0, 0, 0).unwrap());
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct AxisMapping {
    pub p1: Pos2,
    pub p2: Pos2,
    pub v1: AxisValue,
    pub v2: AxisValue,
    pub scale: ScaleKind,
    pub unit: AxisUnit,
}

impl AxisMapping {
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

    pub fn numeric_at(&self, p: Pos2) -> Option<f64> {
        let t = self.t_of_point(p);
        self.numeric_at_t(t)
    }

    pub fn numeric_at_t(&self, t: f64) -> Option<f64> {
        let s1 = self.v1.to_scalar_seconds();
        let s2 = self.v2.to_scalar_seconds();
        match (self.scale, self.unit) {
            (ScaleKind::Linear, _) => Some(s1 + (s2 - s1) * t),
            (ScaleKind::Log10, AxisUnit::Float) => {
                if s1 <= 0.0 || s2 <= 0.0 {
                    return None;
                }
                let l1 = s1.log10();
                let l2 = s2.log10();
                Some(10f64.powf(l1 + (l2 - l1) * t))
            }
            (ScaleKind::Log10, AxisUnit::DateTime) => None,
        }
    }

    pub fn value_at(&self, p: Pos2) -> Option<AxisValue> {
        self.numeric_at(p)
            .map(|s| AxisValue::from_scalar_seconds(self.unit, s))
    }
}
