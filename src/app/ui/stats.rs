use super::super::{AxisCalUi, CurcatApp};
use crate::util::safe_usize_to_f32;
use crate::types::{AxisUnit, AxisValue};

#[derive(Clone, Copy)]
pub enum AxisKind {
    X,
    Y,
}

#[derive(Clone, Copy)]
pub struct RangeF64 {
    pub(crate) min: f64,
    pub(crate) max: f64,
}

impl RangeF64 {
    pub(crate) const fn span(self) -> f64 {
        self.max - self.min
    }
}

#[derive(Clone, Copy)]
pub struct RangeF32 {
    pub(crate) min: f32,
    pub(crate) max: f32,
}

impl RangeF32 {
    pub(crate) const fn span(self) -> f32 {
        self.max - self.min
    }
}

pub struct AxisOrthogonality {
    pub(crate) actual_deg: f32,
    pub(crate) delta_from_right_deg: f32,
}

impl CurcatApp {
    pub(crate) fn axis_numeric_range(&self, axis: AxisKind) -> Option<RangeF64> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for p in &self.points.points {
            let val = match axis {
                AxisKind::X => p.x_numeric,
                AxisKind::Y => p.y_numeric,
            };
            if let Some(v) = val {
                min = min.min(v);
                max = max.max(v);
            }
        }
        if min.is_finite() && max.is_finite() {
            Some(RangeF64 { min, max })
        } else {
            None
        }
    }

    pub(crate) fn axis_pixel_range(&self, axis: AxisKind) -> Option<RangeF32> {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for p in &self.points.points {
            let v = match axis {
                AxisKind::X => p.pixel.x,
                AxisKind::Y => p.pixel.y,
            };
            min = min.min(v);
            max = max.max(v);
        }
        if min.is_finite() && max.is_finite() {
            Some(RangeF32 { min, max })
        } else {
            None
        }
    }

    pub(crate) fn pixel_bounds(&self) -> Option<(RangeF32, RangeF32)> {
        match (
            self.axis_pixel_range(AxisKind::X),
            self.axis_pixel_range(AxisKind::Y),
        ) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }

    pub(crate) fn pixel_step_stats(&self) -> Option<(f32, f32)> {
        if self.points.points.len() < 2 {
            return None;
        }
        let mut total = 0.0_f32;
        for pair in self.points.points.windows(2) {
            if let [a, b] = pair {
                total += (b.pixel - a.pixel).length();
            }
        }
        let denom = safe_usize_to_f32(self.points.points.len().saturating_sub(1));
        let avg = total / denom.max(f32::EPSILON);
        Some((avg, total))
    }

    pub(crate) fn axis_orthogonality(&self) -> Option<AxisOrthogonality> {
        let (xp1, xp2) = (self.calibration.cal_x.p1?, self.calibration.cal_x.p2?);
        let (yp1, yp2) = (self.calibration.cal_y.p1?, self.calibration.cal_y.p2?);
        let vx = xp2 - xp1;
        let vy = yp2 - yp1;
        let lx = vx.length();
        let ly = vy.length();
        if lx <= f32::EPSILON || ly <= f32::EPSILON {
            return None;
        }
        let dot = vx.dot(vy);
        let cos_theta = (dot / (lx * ly)).clamp(-1.0, 1.0);
        let angle_rad = cos_theta.acos();
        let delta = (std::f32::consts::FRAC_PI_2 - angle_rad).abs();
        Some(AxisOrthogonality {
            actual_deg: angle_rad.to_degrees(),
            delta_from_right_deg: delta.to_degrees(),
        })
    }
}

pub fn axis_length(cal: &AxisCalUi) -> Option<f32> {
    let (p1, p2) = (cal.p1?, cal.p2?);
    let len = (p2 - p1).length();
    if len > f32::EPSILON { Some(len) } else { None }
}

pub fn format_span(unit: AxisUnit, span: f64) -> String {
    match unit {
        AxisUnit::Float => AxisValue::from_scalar_seconds(AxisUnit::Float, span)
            .map_or_else(|| format!("{span:.6}"), |v| v.format()),
        AxisUnit::DateTime => format_duration(span),
    }
}

fn format_duration(seconds: f64) -> String {
    const MINUTE: f64 = 60.0;
    const HOUR: f64 = 3600.0;
    const DAY: f64 = 86_400.0;
    if seconds >= DAY {
        format!("{:.3} d", seconds / DAY)
    } else if seconds >= HOUR {
        format!("{:.3} h", seconds / HOUR)
    } else if seconds >= MINUTE {
        format!("{:.3} min", seconds / MINUTE)
    } else {
        format!("{seconds:.3} s")
    }
}
