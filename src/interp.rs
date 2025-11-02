use crate::types::{AxisUnit, AxisValue};

#[derive(Debug, Clone)]
pub struct XYPoint {
    pub x: f64,
    pub y: f64,
}

pub fn linear_interpolate_sorted(points: &[XYPoint], samples: usize) -> Vec<XYPoint> {
    if points.is_empty() {
        return vec![];
    }
    if points.len() == 1 || samples <= 1 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(samples);
    let x_min = points.first().unwrap().x;
    let x_max = points.last().unwrap().x;
    if (x_max - x_min).abs() <= f64::EPSILON {
        // Vertical line, just duplicate y
        for _ in 0..samples {
            out.push(XYPoint {
                x: x_min,
                y: points[0].y,
            });
        }
        return out;
    }
    let step = (x_max - x_min) / (samples.saturating_sub(1) as f64);

    let mut j = 0usize;
    for i in 0..samples {
        let sx = if i + 1 == samples {
            x_max
        } else {
            x_min + step * (i as f64)
        };
        while j + 1 < points.len() && points[j + 1].x < sx {
            j += 1;
        }
        let (x0, y0) = (points[j].x, points[j].y);
        let (x1, y1) = if j + 1 < points.len() {
            (points[j + 1].x, points[j + 1].y)
        } else {
            (x0, y0)
        };
        let sy = if (x1 - x0).abs() <= f64::EPSILON {
            y0
        } else {
            let t = (sx - x0) / (x1 - x0);
            y0 + (y1 - y0) * t
        };
        out.push(XYPoint { x: sx, y: sy });
    }
    out
}

pub fn format_value_for_unit(unit: AxisUnit, val: f64) -> AxisValue {
    AxisValue::from_scalar_seconds(unit, val)
}
