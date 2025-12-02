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

    pub const fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::StepHold => "Step (previous)",
            Self::NaturalCubic => "Natural cubic spline",
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

/// Heuristic auto-selection of sample count for exporting an interpolated curve.
///
/// The goal is to find the smallest `samples` such that a polyline through the
/// exported samples approximates the underlying interpolated curve within a
/// relative tolerance on Y.
///
/// - `points` are expected to be sorted by `x`.
/// - `min_samples`/`max_samples` bound the search (inclusive).
/// - `rel_tolerance` is the allowed fraction of the Y-range (0–1).
/// - `ref_target_samples` controls the resolution of the internal reference curve.
pub fn auto_sample_count(
    points: &[XYPoint],
    algo: InterpAlgorithm,
    min_samples: usize,
    max_samples: usize,
    rel_tolerance: f64,
    ref_target_samples: usize,
) -> usize {
    let mut min_samples = min_samples.max(2);
    let max_samples = max_samples.max(min_samples);

    if points.len() < 2 {
        return min_samples;
    }

    // Build a "reference" curve at relatively high resolution that
    // represents the underlying interpolation as closely as we need.
    const MIN_REF_SAMPLES: usize = 16;
    const MIN_ABS_TOLERANCE: f64 = 1.0e-9;

    let ref_tolerance = rel_tolerance.clamp(1.0e-6, 1.0);

    let ref_samples = ref_target_samples
        .max(MIN_REF_SAMPLES)
        .min(max_samples.max(min_samples).max(MIN_REF_SAMPLES));
    if ref_samples <= 1 {
        return min_samples;
    }

    let ref_xs = build_sample_positions(points, ref_samples);
    let ref_curve = match algo {
        InterpAlgorithm::Linear => interpolate_linear(points, &ref_xs),
        InterpAlgorithm::StepHold => interpolate_step(points, &ref_xs),
        InterpAlgorithm::NaturalCubic => interpolate_cubic(points, &ref_xs),
    };

    // Compute Y-range on the reference curve to derive an absolute tolerance.
    let mut y_min = ref_curve[0].y;
    let mut y_max = ref_curve[0].y;
    for p in &ref_curve[1..] {
        if p.y < y_min {
            y_min = p.y;
        }
        if p.y > y_max {
            y_max = p.y;
        }
    }
    let mut y_range = y_max - y_min;
    if y_range < 0.0 {
        y_range = -y_range;
    }

    // If the curve is almost flat, any reasonable sample count is fine.
    if y_range <= f64::EPSILON {
        return min_samples;
    }

    let abs_tolerance = (y_range * ref_tolerance).max(MIN_ABS_TOLERANCE);

    // Clamp min_samples so that it is not larger than max_samples.
    if min_samples > max_samples {
        min_samples = max_samples;
    }

    let mut current = min_samples;

    loop {
        if current >= max_samples {
            return max_samples;
        }

        let coarse = interpolate_sorted(points, current, algo);
        if coarse.len() < 2 {
            return current;
        }

        // Approximate the coarse curve at the reference X positions.
        // Use step reconstruction for step-hold to avoid smoothing away jumps.
        let approx = match algo {
            InterpAlgorithm::StepHold => interpolate_step(&coarse, &ref_xs),
            _ => interpolate_linear(&coarse, &ref_xs),
        };

        let mut max_err = 0.0;
        for (ref_pt, approx_pt) in ref_curve.iter().zip(approx.iter()) {
            let err = (ref_pt.y - approx_pt.y).abs();
            if err > max_err {
                max_err = err;
                if max_err > abs_tolerance {
                    break;
                }
            }
        }

        if max_err <= abs_tolerance {
            return current;
        }

        let next = current.saturating_mul(2).saturating_sub(1);
        if next <= current {
            break;
        }
        current = next.min(max_samples);
    }

    max_samples
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
    let denom = samples.saturating_sub(1);
    let step = if denom == 0 {
        0.0
    } else {
        (x_max - x_min) / usize_to_f64(denom)
    };
    for i in 0..samples {
        if i + 1 == samples {
            xs.push(x_max);
        } else {
            xs.push(x_min + step * usize_to_f64(i));
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
            (y1 - y0).mul_add(t, y0)
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
    let point_count = points.len();
    let mut interval_widths = vec![0.0; point_count - 1];
    for i in 0..(point_count - 1) {
        let delta = points[i + 1].x - points[i].x;
        if delta.abs() <= f64::EPSILON {
            return None;
        }
        interval_widths[i] = delta;
    }

    let mut slope_diffs = vec![0.0; point_count];
    for i in 1..(point_count - 1) {
        slope_diffs[i] = (3.0 / interval_widths[i]) * (points[i + 1].y - points[i].y)
            - (3.0 / interval_widths[i - 1]) * (points[i].y - points[i - 1].y);
    }

    let mut tri_diagonal = vec![0.0; point_count];
    let mut upper_ratio = vec![0.0; point_count];
    let mut rhs = vec![0.0; point_count];

    tri_diagonal[0] = 1.0;
    rhs[0] = 0.0;

    for i in 1..(point_count - 1) {
        tri_diagonal[i] =
            2.0 * (points[i + 1].x - points[i - 1].x) - interval_widths[i - 1] * upper_ratio[i - 1];
        if tri_diagonal[i].abs() <= f64::EPSILON {
            return None;
        }
        upper_ratio[i] = interval_widths[i] / tri_diagonal[i];
        rhs[i] = (slope_diffs[i] - interval_widths[i - 1] * rhs[i - 1]) / tri_diagonal[i];
    }

    tri_diagonal[point_count - 1] = 1.0;
    rhs[point_count - 1] = 0.0;

    let mut coeff_c = vec![0.0; point_count];
    let mut coeff_b = vec![0.0; point_count - 1];
    let mut coeff_d = vec![0.0; point_count - 1];

    for j in (0..=(point_count - 2)).rev() {
        coeff_c[j] = rhs[j] - upper_ratio[j] * coeff_c[j + 1];
        coeff_b[j] = (points[j + 1].y - points[j].y) / interval_widths[j]
            - interval_widths[j] * (coeff_c[j + 1] + 2.0 * coeff_c[j]) / 3.0;
        coeff_d[j] = (coeff_c[j + 1] - coeff_c[j]) / (3.0 * interval_widths[j]);
    }

    let mut segments = Vec::with_capacity(point_count - 1);
    for i in 0..(point_count - 1) {
        segments.push(CubicSegment {
            x: points[i].x,
            a: points[i].y,
            b: coeff_b[i],
            c: coeff_c[i],
            d: coeff_d[i],
        });
    }
    Some(segments)
}

const fn usize_to_f64(value: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}
