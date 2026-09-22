//! Interpolation utilities for resampling picked points.

/// A 2D point in numeric axis space.
#[derive(Debug, Clone, Copy)]
pub struct XYPoint {
    pub x: f64,
    pub y: f64,
}

/// Supported interpolation algorithms for curve export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpAlgorithm {
    Linear,
    StepHold,
    NaturalCubic,
}

const MIN_REF_SAMPLES: usize = 16;
const MIN_ABS_TOLERANCE: f64 = 1.0e-9;

impl InterpAlgorithm {
    /// Ordered list of algorithms exposed in the UI.
    pub const ALL: [Self; 3] = [Self::Linear, Self::StepHold, Self::NaturalCubic];
}

/// Resample already-sorted points into `samples` using the chosen algorithm.
///
/// The input is expected to be sorted by `x`. When there is fewer than two
/// points or `samples <= 1`, the original points are returned.
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

    let sample_xs = sample_positions(points, samples);
    match algo {
        InterpAlgorithm::Linear => interpolate_linear(points, sample_xs),
        InterpAlgorithm::StepHold => interpolate_step(points, sample_xs),
        InterpAlgorithm::NaturalCubic => interpolate_cubic(points, sample_xs),
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
    let min_samples = min_samples.max(2);
    let max_samples = max_samples.max(min_samples);

    if points.len() < 2 {
        return min_samples;
    }

    // Build a "reference" curve at relatively high resolution that
    // represents the underlying interpolation as closely as we need.
    let clamped_rel_tolerance = rel_tolerance.clamp(1.0e-6, 1.0);

    let ref_samples = ref_target_samples
        .max(MIN_REF_SAMPLES)
        .min(max_samples.max(MIN_REF_SAMPLES));

    let ref_curve = interpolate_sorted(points, ref_samples, algo);

    // Compute Y-range on the reference curve to derive an absolute tolerance.
    let Some((first_ref, rest_ref)) = ref_curve.split_first() else {
        return min_samples;
    };
    let mut y_min = first_ref.y;
    let mut y_max = first_ref.y;
    for p in rest_ref {
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

    let abs_tolerance = (y_range * clamped_rel_tolerance).max(MIN_ABS_TOLERANCE);

    let mut current = min_samples;

    loop {
        if current >= max_samples {
            return max_samples;
        }

        let coarse = interpolate_sorted(points, current, algo);
        if coarse.len() < 2 {
            return current;
        }

        // Ступенчатая реконструкция сохраняет скачки; остальные кривые сравниваются с ломаной.
        let kind = match algo {
            InterpAlgorithm::StepHold => PiecewiseKind::Step,
            _ => PiecewiseKind::Linear,
        };
        let Some(mut approx) = PiecewiseSampler::new(&coarse, kind) else {
            return current;
        };
        let mut max_err = 0.0;
        for ref_pt in &ref_curve {
            let err = (ref_pt.y - approx.sample(ref_pt.x)).abs();
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

/// Равномерная сетка без выделения памяти; последняя точка точно совпадает с правой границей.
fn sample_positions(points: &[XYPoint], samples: usize) -> impl ExactSizeIterator<Item = f64> {
    let (samples, x_min, x_max) = match (points.first(), points.last()) {
        (Some(first), Some(last)) => (samples, first.x, last.x),
        _ => (0, 0.0, 0.0),
    };
    let constant = (x_max - x_min).abs() <= f64::EPSILON;
    let denom = samples.saturating_sub(1);
    let step = if denom == 0 {
        0.0
    } else {
        (x_max - x_min) / usize_to_f64(denom)
    };
    (0..samples).map(move |i| {
        if constant {
            x_min
        } else if i + 1 == samples {
            x_max
        } else {
            step.mul_add(usize_to_f64(i), x_min)
        }
    })
}

#[derive(Clone, Copy)]
enum PiecewiseKind {
    Linear,
    Step,
}

/// Курсор по непустой кривой для последовательных запросов с неубывающим x.
struct PiecewiseSampler<'a> {
    current: XYPoint,
    rest: &'a [XYPoint],
    kind: PiecewiseKind,
}

impl<'a> PiecewiseSampler<'a> {
    fn new(points: &'a [XYPoint], kind: PiecewiseKind) -> Option<Self> {
        let (&current, rest) = points.split_first()?;
        Some(Self {
            current,
            rest,
            kind,
        })
    }

    fn sample(&mut self, x: f64) -> f64 {
        while let Some((&next, rest)) = self.rest.split_first() {
            let advance = match self.kind {
                PiecewiseKind::Linear => next.x < x,
                PiecewiseKind::Step => next.x <= x,
            };
            if !advance {
                break;
            }
            self.current = next;
            self.rest = rest;
        }
        let left = self.current;
        match self.kind {
            PiecewiseKind::Step => left.y,
            PiecewiseKind::Linear => {
                let right = self.rest.first().copied().unwrap_or(left);
                if (right.x - left.x).abs() <= f64::EPSILON {
                    left.y
                } else {
                    let t = (x - left.x) / (right.x - left.x);
                    (right.y - left.y).mul_add(t, left.y)
                }
            }
        }
    }
}

fn interpolate_piecewise(
    points: &[XYPoint],
    sample_xs: impl ExactSizeIterator<Item = f64>,
    kind: PiecewiseKind,
) -> Vec<XYPoint> {
    let Some(mut sampler) = PiecewiseSampler::new(points, kind) else {
        return Vec::new();
    };
    sample_xs
        .map(|x| XYPoint {
            x,
            y: sampler.sample(x),
        })
        .collect()
}

fn interpolate_linear(
    points: &[XYPoint],
    sample_xs: impl ExactSizeIterator<Item = f64>,
) -> Vec<XYPoint> {
    interpolate_piecewise(points, sample_xs, PiecewiseKind::Linear)
}

fn interpolate_step(
    points: &[XYPoint],
    sample_xs: impl ExactSizeIterator<Item = f64>,
) -> Vec<XYPoint> {
    interpolate_piecewise(points, sample_xs, PiecewiseKind::Step)
}

#[allow(clippy::suboptimal_flops)]
fn interpolate_cubic(
    points: &[XYPoint],
    sample_xs: impl ExactSizeIterator<Item = f64>,
) -> Vec<XYPoint> {
    let unique = unique_by_x(points);
    if unique.len() < 2 {
        return interpolate_linear(points, sample_xs);
    }
    let Some(segments) = build_natural_cubic_segments(&unique) else {
        return interpolate_linear(&unique, sample_xs);
    };

    let mut out = Vec::with_capacity(sample_xs.len());
    let Some(last) = unique.last() else {
        return interpolate_linear(points, sample_xs);
    };
    let Some(mut seg) = segments.first() else {
        return interpolate_linear(points, sample_xs);
    };
    let Some(last_segment) = segments.last() else {
        return interpolate_linear(points, sample_xs);
    };
    let mut rest = segments.iter().skip(1).peekable();
    let last_x = last.x;
    for sx in sample_xs {
        while let Some(next) = rest.next_if(|next| sx >= next.x) {
            seg = next;
        }
        if sx > last_x {
            seg = last_segment;
        }
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
            *last = *p;
            continue;
        }
        unique.push(*p);
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

#[allow(clippy::suboptimal_flops)]
fn build_natural_cubic_segments(points: &[XYPoint]) -> Option<Vec<CubicSegment>> {
    if points.len() < 2 {
        return None;
    }
    let point_count = points.len();
    let mut interval_widths = vec![0.0; point_count - 1];
    for (slot, [left, right]) in interval_widths.iter_mut().zip(points.array_windows::<2>()) {
        let delta = right.x - left.x;
        if delta.abs() <= f64::EPSILON {
            return None;
        }
        *slot = delta;
    }

    let mut upper_ratio = vec![0.0; point_count];
    let mut rhs = vec![0.0; point_count];

    for (i, ([prev, curr, next], &[prev_width, width])) in points
        .array_windows::<3>()
        .zip(interval_widths.array_windows::<2>())
        .enumerate()
    {
        let slope_diff = (3.0 / width) * (next.y - curr.y) - (3.0 / prev_width) * (curr.y - prev.y);
        let diagonal = 2.0 * (next.x - prev.x) - prev_width * upper_ratio[i];
        if diagonal.abs() <= f64::EPSILON {
            return None;
        }
        upper_ratio[i + 1] = width / diagonal;
        rhs[i + 1] = (slope_diff - prev_width * rhs[i]) / diagonal;
    }

    // Обратному ходу нужен только следующий коэффициент c; сегменты собираются сразу.
    let mut segments = Vec::with_capacity(point_count - 1);
    let mut next_c = 0.0;
    for ((([left, right], &width), &ratio), &rhs) in points
        .array_windows::<2>()
        .zip(&interval_widths)
        .zip(&upper_ratio)
        .zip(&rhs)
        .rev()
    {
        let c = rhs - ratio * next_c;
        let b = (right.y - left.y) / width - width * (next_c + 2.0 * c) / 3.0;
        let d = (next_c - c) / (3.0 * width);
        segments.push(CubicSegment {
            x: left.x,
            a: left.y,
            b,
            c,
            d,
        });
        next_c = c;
    }
    segments.reverse();
    Some(segments)
}

const fn usize_to_f64(value: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn interpolate_linear_basic() {
        let points = vec![XYPoint { x: 0.0, y: 0.0 }, XYPoint { x: 10.0, y: 10.0 }];
        let out = interpolate_sorted(&points, 3, InterpAlgorithm::Linear);
        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0].x, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].x, 5.0, 1.0e-9));
        assert!(approx_eq(out[2].x, 10.0, 1.0e-9));
        assert!(approx_eq(out[0].y, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].y, 5.0, 1.0e-9));
        assert!(approx_eq(out[2].y, 10.0, 1.0e-9));
    }

    #[test]
    fn interpolate_step_basic() {
        let points = vec![XYPoint { x: 0.0, y: 0.0 }, XYPoint { x: 10.0, y: 10.0 }];
        let out = interpolate_sorted(&points, 3, InterpAlgorithm::StepHold);
        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0].x, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].x, 5.0, 1.0e-9));
        assert!(approx_eq(out[2].x, 10.0, 1.0e-9));
        assert!(approx_eq(out[0].y, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].y, 0.0, 1.0e-9));
        assert!(approx_eq(out[2].y, 10.0, 1.0e-9));
    }

    #[test]
    fn piecewise_interpolation_preserves_duplicate_knot_boundaries() {
        let points = [
            XYPoint { x: 0.0, y: 1.0 },
            XYPoint { x: 1.0, y: 2.0 },
            XYPoint { x: 1.0, y: 5.0 },
            XYPoint { x: 2.0, y: 9.0 },
        ];
        let xs = [-1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 3.0];
        let linear = interpolate_linear(&points, xs.into_iter());
        let step = interpolate_step(&points, xs.into_iter());
        for (curve, expected) in [
            (linear, [0.0, 1.0, 1.5, 2.0, 7.0, 9.0, 9.0]),
            (step, [1.0, 1.0, 1.0, 5.0, 5.0, 9.0, 9.0]),
        ] {
            assert_eq!(curve.len(), xs.len());
            for ((point, x), y) in curve.iter().zip(xs).zip(expected) {
                assert!(approx_eq(point.x, x, 1.0e-12));
                assert!(approx_eq(point.y, y, 1.0e-12));
            }
        }
    }

    #[test]
    fn sample_grid_preserves_endpoints_and_collapsed_ranges() {
        let points = [XYPoint { x: -0.7, y: 0.0 }, XYPoint { x: 1.1, y: 1.0 }];
        for count in [0, 1, 2, 7, 100] {
            let xs: Vec<_> = sample_positions(&points, count).collect();
            assert_eq!(xs.len(), count);
            if let Some(last) = xs.last() {
                assert_eq!(last.to_bits(), points[1].x.to_bits());
            }
            if count > 1 {
                assert_eq!(xs[0].to_bits(), points[0].x.to_bits());
                assert!(xs.array_windows::<2>().all(|[a, b]| a < b));
            }
        }
        let collapsed = [
            XYPoint { x: -0.0, y: 1.0 },
            XYPoint {
                x: f64::EPSILON,
                y: 2.0,
            },
        ];
        assert!(sample_positions(&collapsed, 5).all(|x| x.to_bits() == (-0.0_f64).to_bits()));
        assert_eq!(sample_positions(&[], 5).len(), 0);
    }

    #[test]
    fn interpolate_cubic_preserves_knots_and_keeps_last_duplicate() {
        let points = [
            XYPoint { x: 0.0, y: 0.0 },
            XYPoint { x: 1.0, y: 1.0 },
            XYPoint { x: 2.0, y: 0.0 },
        ];
        let duplicates = [points[0], XYPoint { x: 1.0, y: -8.0 }, points[1], points[2]];
        for input in [&points[..], &duplicates[..]] {
            let out = interpolate_sorted(input, 5, InterpAlgorithm::NaturalCubic);
            let expected = [
                (0.0, 0.0),
                (0.5, 0.6875),
                (1.0, 1.0),
                (1.5, 0.6875),
                (2.0, 0.0),
            ];
            assert_eq!(out.len(), expected.len());
            for (point, (x, y)) in out.iter().zip(expected) {
                assert!(approx_eq(point.x, x, 1.0e-9));
                assert!(approx_eq(point.y, y, 1.0e-9));
            }
        }
    }

    #[test]
    #[allow(clippy::suboptimal_flops)]
    fn natural_cubic_is_smooth_on_nonuniform_intervals() {
        let points = [
            XYPoint { x: -3.0, y: 4.0 },
            XYPoint { x: -0.5, y: -2.0 },
            XYPoint { x: 0.0, y: 3.0 },
            XYPoint { x: 7.0, y: 1.0 },
            XYPoint { x: 8.0, y: 5.0 },
        ];
        let segments = build_natural_cubic_segments(&points).unwrap();
        assert_eq!(segments.len(), points.len() - 1);
        for (segment, [left, right]) in segments.iter().zip(points.array_windows::<2>()) {
            let dx = right.x - left.x;
            let y = segment.a + segment.b * dx + segment.c * dx * dx + segment.d * dx * dx * dx;
            assert!(approx_eq(segment.a, left.y, 1.0e-9));
            assert!(approx_eq(y, right.y, 1.0e-9));
        }
        for [left, right] in segments.array_windows::<2>() {
            let dx = right.x - left.x;
            let first_derivative = left.b + 2.0 * left.c * dx + 3.0 * left.d * dx * dx;
            let second_derivative = 2.0 * left.c + 6.0 * left.d * dx;
            assert!(approx_eq(first_derivative, right.b, 1.0e-9));
            assert!(approx_eq(second_derivative, 2.0 * right.c, 1.0e-9));
        }
        assert!(approx_eq(segments[0].c, 0.0, 1.0e-9));
        let last = segments.last().unwrap();
        let dx = points.last().unwrap().x - last.x;
        assert!(approx_eq(2.0 * last.c + 6.0 * last.d * dx, 0.0, 1.0e-9));
    }

    #[test]
    fn natural_cubic_two_points_matches_linear() {
        let points = [XYPoint { x: -2.0, y: 5.0 }, XYPoint { x: 6.0, y: -3.0 }];
        let cubic = interpolate_sorted(&points, 17, InterpAlgorithm::NaturalCubic);
        let linear = interpolate_sorted(&points, 17, InterpAlgorithm::Linear);
        for (cubic, linear) in cubic.iter().zip(linear) {
            assert!(approx_eq(cubic.x, linear.x, 1.0e-9));
            assert!(approx_eq(cubic.y, linear.y, 1.0e-9));
        }
    }

    #[test]
    fn auto_sample_count_linear_returns_min() {
        let points = vec![XYPoint { x: 0.0, y: 0.0 }, XYPoint { x: 10.0, y: 10.0 }];
        let min_samples = 10;
        let max_samples = 100;
        let rel_tol = 1.0e-3;
        let ref_samples = 128;
        let out = auto_sample_count(
            &points,
            InterpAlgorithm::Linear,
            min_samples,
            max_samples,
            rel_tol,
            ref_samples,
        );
        assert_eq!(out, min_samples);
    }
}
