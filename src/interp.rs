#[derive(Debug, Clone)]
pub struct XYPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpAlgorithm {
    Linear,
    StepHold,
    NaturalCubic,
}

impl InterpAlgorithm {
    pub const ALL: [Self; 3] = [Self::Linear, Self::StepHold, Self::NaturalCubic];

    pub fn label(&self) -> &'static str {
        match self {
            InterpAlgorithm::Linear => "Linear",
            InterpAlgorithm::StepHold => "Step (previous)",
            InterpAlgorithm::NaturalCubic => "Natural cubic spline",
        }
    }
}

pub fn interpolate_sorted(
    points: &[XYPoint],
    samples: usize,
    algo: InterpAlgorithm,
) -> Vec<XYPoint> {
    if points.is_empty() {
        return vec![];
    }
    if points.len() == 1 || samples <= 1 {
        return points.to_vec();
    }

    let sample_xs = build_sample_positions(points, samples);
    match algo {
        InterpAlgorithm::Linear => interpolate_linear(points, &sample_xs),
        InterpAlgorithm::StepHold => interpolate_step(points, &sample_xs),
        InterpAlgorithm::NaturalCubic => interpolate_cubic(points, &sample_xs),
    }
}

fn build_sample_positions(points: &[XYPoint], samples: usize) -> Vec<f64> {
    if samples == 0 {
        return vec![];
    }
    let mut xs = Vec::with_capacity(samples);
    let x_min = points.first().unwrap().x;
    let x_max = points.last().unwrap().x;
    if (x_max - x_min).abs() <= f64::EPSILON {
        xs.resize(samples, x_min);
        return xs;
    }
    let step = (x_max - x_min) / (samples.saturating_sub(1) as f64);
    for i in 0..samples {
        if i + 1 == samples {
            xs.push(x_max);
        } else {
            xs.push(x_min + step * (i as f64));
        }
    }
    xs
}

fn interpolate_linear(points: &[XYPoint], sample_xs: &[f64]) -> Vec<XYPoint> {
    let mut out = Vec::with_capacity(sample_xs.len());
    let mut j = 0usize;
    for &sx in sample_xs {
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

fn interpolate_step(points: &[XYPoint], sample_xs: &[f64]) -> Vec<XYPoint> {
    let mut out = Vec::with_capacity(sample_xs.len());
    let mut j = 0usize;
    for &sx in sample_xs {
        while j + 1 < points.len() && points[j + 1].x <= sx {
            j += 1;
        }
        out.push(XYPoint {
            x: sx,
            y: points[j].y,
        });
    }
    out
}

fn interpolate_cubic(points: &[XYPoint], sample_xs: &[f64]) -> Vec<XYPoint> {
    let unique = unique_by_x(points);
    if unique.len() < 2 {
        return interpolate_linear(points, sample_xs);
    }
    let Some(segments) = build_natural_cubic_segments(&unique) else {
        return interpolate_linear(&unique, sample_xs);
    };

    let mut out = Vec::with_capacity(sample_xs.len());
    let mut seg_idx = 0usize;
    let last_x = unique.last().unwrap().x;
    for &sx in sample_xs {
        while seg_idx + 1 < segments.len() && sx >= segments[seg_idx + 1].x {
            seg_idx += 1;
        }
        if sx > last_x {
            seg_idx = segments.len().saturating_sub(1);
        }
        let seg = &segments[seg_idx];
        let dx = sx - seg.x;
        let y = seg.a + seg.b * dx + seg.c * dx * dx + seg.d * dx * dx * dx;
        out.push(XYPoint { x: sx, y });
    }
    out
}

fn unique_by_x(points: &[XYPoint]) -> Vec<XYPoint> {
    let mut unique: Vec<XYPoint> = Vec::with_capacity(points.len());
    for p in points {
        if let Some(last) = unique.last_mut()
            && (last.x - p.x).abs() <= f64::EPSILON
        {
            *last = p.clone();
            continue;
        }
        unique.push(p.clone());
    }
    unique
}

#[derive(Debug, Clone)]
struct CubicSegment {
    x: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

fn build_natural_cubic_segments(points: &[XYPoint]) -> Option<Vec<CubicSegment>> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len();
    let mut h = vec![0.0; n - 1];
    for i in 0..(n - 1) {
        let delta = points[i + 1].x - points[i].x;
        if delta.abs() <= f64::EPSILON {
            return None;
        }
        h[i] = delta;
    }

    let mut alpha = vec![0.0; n];
    for i in 1..(n - 1) {
        alpha[i] = (3.0 / h[i]) * (points[i + 1].y - points[i].y)
            - (3.0 / h[i - 1]) * (points[i].y - points[i - 1].y);
    }

    let mut l = vec![0.0; n];
    let mut mu = vec![0.0; n];
    let mut z = vec![0.0; n];

    l[0] = 1.0;
    mu[0] = 0.0;
    z[0] = 0.0;

    for i in 1..(n - 1) {
        l[i] = 2.0 * (points[i + 1].x - points[i - 1].x) - h[i - 1] * mu[i - 1];
        if l[i].abs() <= f64::EPSILON {
            return None;
        }
        mu[i] = h[i] / l[i];
        z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i];
    }

    l[n - 1] = 1.0;
    z[n - 1] = 0.0;

    let mut c = vec![0.0; n];
    let mut b = vec![0.0; n - 1];
    let mut d = vec![0.0; n - 1];

    for j in (0..=(n - 2)).rev() {
        c[j] = z[j] - mu[j] * c[j + 1];
        b[j] = (points[j + 1].y - points[j].y) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
        d[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
    }

    let mut segments = Vec::with_capacity(n - 1);
    for i in 0..(n - 1) {
        segments.push(CubicSegment {
            x: points[i].x,
            a: points[i].y,
            b: b[i],
            c: c[i],
            d: d[i],
        });
    }
    Some(segments)
}
