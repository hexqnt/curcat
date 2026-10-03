//! Interpolation utilities for resampling picked points.

mod cubic;
mod piecewise;
mod points;

pub use points::{InvalidInterpolationPoints, SortedPoints};

use cubic::NaturalCubic;
use piecewise::{PiecewiseCursor, PiecewiseKind};
use points::unique_by_x;
use std::borrow::Cow;

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
/// Input order and finite coordinates are guaranteed by `SortedPoints`. When there are fewer than two points or `samples <= 1`, the original points are returned.
pub fn interpolate_sorted(
    points: SortedPoints<'_>,
    samples: usize,
    algo: InterpAlgorithm,
) -> Vec<XYPoint> {
    if points.as_slice().len() < 2 || samples <= 1 {
        return points.as_slice().to_vec();
    }
    PreparedInterpolation::new(points, algo).resample(samples)
}

/// Select an export sample count by progressively refining a uniform grid.
///
/// Increase the grid size until reconstruction approximates the interpolated curve within the relative Y tolerance or reaches the sample limit.
///
/// - `points` guarantees finite coordinates sorted by `x`.
/// - `min_samples`/`max_samples` bound the search (inclusive).
/// - `rel_tolerance` is the allowed fraction of the Y-range (0–1).
/// - `ref_target_samples` controls the resolution of the internal reference curve.
pub fn auto_sample_count(
    points: SortedPoints<'_>,
    algo: InterpAlgorithm,
    min_samples: usize,
    max_samples: usize,
    rel_tolerance: f64,
    ref_target_samples: usize,
) -> usize {
    let min_samples = min_samples.max(2);
    let max_samples = max_samples.max(min_samples);

    if points.as_slice().len() < 2 {
        return min_samples;
    }

    // Build a "reference" curve at relatively high resolution that
    // represents the underlying interpolation as closely as we need.
    let clamped_rel_tolerance = rel_tolerance.clamp(1.0e-6, 1.0);

    let ref_samples = ref_target_samples
        .max(MIN_REF_SAMPLES)
        .min(max_samples.max(MIN_REF_SAMPLES));

    let interpolation = PreparedInterpolation::new(points, algo);
    let ref_curve = interpolation.resample(ref_samples);

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
    let y_range = y_max - y_min;

    // If the curve is almost flat, any reasonable sample count is fine.
    if y_range <= f64::EPSILON {
        return min_samples;
    }

    let abs_tolerance = (y_range * clamped_rel_tolerance).max(MIN_ABS_TOLERANCE);

    let mut current = min_samples;
    let mut coarse = Vec::new();
    // Preserve jumps when reconstructing steps; compare other curves with a polyline.
    let reconstruction = match algo {
        InterpAlgorithm::StepHold => PiecewiseKind::Step,
        InterpAlgorithm::Linear | InterpAlgorithm::NaturalCubic => PiecewiseKind::Linear,
    };

    loop {
        if current >= max_samples {
            return max_samples;
        }

        interpolation.resample_into(current, &mut coarse);
        if coarse.len() < 2 {
            return current;
        }

        let Some(mut approx) = PiecewiseCursor::new(&coarse, reconstruction) else {
            return current;
        };
        if ref_curve.iter().all(|reference| {
            let error = (reference.y - approx.sample(reference.x)).abs();
            error.is_finite() && error <= abs_tolerance
        }) {
            return current;
        }

        current = current.saturating_add(current - 1).min(max_samples);
    }
}

/// An allocation-free uniform grid whose last point is exactly the right endpoint.
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

/// Coefficients are prepared once and reused for each candidate sample count.
struct PreparedInterpolation<'a> {
    points: &'a [XYPoint],
    method: PreparedMethod<'a>,
}

enum PreparedMethod<'a> {
    Piecewise {
        points: Cow<'a, [XYPoint]>,
        kind: PiecewiseKind,
    },
    Cubic(NaturalCubic),
}

impl<'a> PreparedInterpolation<'a> {
    fn new(points: SortedPoints<'a>, algo: InterpAlgorithm) -> Self {
        let points = points.as_slice();
        let kind = match algo {
            InterpAlgorithm::StepHold => PiecewiseKind::Step,
            InterpAlgorithm::Linear | InterpAlgorithm::NaturalCubic => PiecewiseKind::Linear,
        };
        let method = if algo == InterpAlgorithm::NaturalCubic && points.len() >= 2 {
            let unique = unique_by_x(points);
            if unique.len() < 2 {
                PreparedMethod::Piecewise {
                    points: Cow::Borrowed(points),
                    kind,
                }
            } else if let Some(segments) = NaturalCubic::try_new(&unique) {
                PreparedMethod::Cubic(segments)
            } else {
                PreparedMethod::Piecewise {
                    points: unique,
                    kind,
                }
            }
        } else {
            PreparedMethod::Piecewise {
                points: Cow::Borrowed(points),
                kind,
            }
        };
        Self { points, method }
    }

    fn resample(&self, samples: usize) -> Vec<XYPoint> {
        let mut out = Vec::new();
        self.resample_into(samples, &mut out);
        out
    }

    fn resample_into(&self, samples: usize, out: &mut Vec<XYPoint>) {
        out.clear();
        if self.points.len() < 2 || samples <= 1 {
            out.extend_from_slice(self.points);
            return;
        }
        let sample_xs = sample_positions(self.points, samples);
        match &self.method {
            PreparedMethod::Piecewise { points, kind } => {
                if let Some(mut cursor) = PiecewiseCursor::new(points, *kind) {
                    out.extend(sample_xs.map(|x| XYPoint {
                        x,
                        y: cursor.sample(x),
                    }));
                }
            }
            PreparedMethod::Cubic(spline) => {
                let mut cursor = spline.cursor();
                out.extend(sample_xs.map(|x| XYPoint {
                    x,
                    y: cursor.sample(x),
                }));
            }
        }
    }
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

    fn resample(points: &[XYPoint], samples: usize, algo: InterpAlgorithm) -> Vec<XYPoint> {
        super::interpolate_sorted(SortedPoints::try_from(points).unwrap(), samples, algo)
    }

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn interpolate_linear_basic() {
        let points = vec![XYPoint { x: 0.0, y: 0.0 }, XYPoint { x: 10.0, y: 10.0 }];
        let out = resample(&points, 3, InterpAlgorithm::Linear);
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
        let out = resample(&points, 3, InterpAlgorithm::StepHold);
        assert_eq!(out.len(), 3);
        assert!(approx_eq(out[0].x, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].x, 5.0, 1.0e-9));
        assert!(approx_eq(out[2].x, 10.0, 1.0e-9));
        assert!(approx_eq(out[0].y, 0.0, 1.0e-9));
        assert!(approx_eq(out[1].y, 0.0, 1.0e-9));
        assert!(approx_eq(out[2].y, 10.0, 1.0e-9));
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
            let out = resample(input, 5, InterpAlgorithm::NaturalCubic);
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
    fn natural_cubic_two_points_matches_linear() {
        let points = [XYPoint { x: -2.0, y: 5.0 }, XYPoint { x: 6.0, y: -3.0 }];
        let cubic = resample(&points, 17, InterpAlgorithm::NaturalCubic);
        let linear = resample(&points, 17, InterpAlgorithm::Linear);
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
            SortedPoints::try_from(points.as_slice()).unwrap(),
            InterpAlgorithm::Linear,
            min_samples,
            max_samples,
            rel_tol,
            ref_samples,
        );
        assert_eq!(out, min_samples);
    }

    #[test]
    fn auto_sample_count_does_not_accept_nonfinite_interpolation_errors() {
        let points = [
            XYPoint {
                x: 0.0,
                y: -f64::MAX,
            },
            XYPoint {
                x: 1.0,
                y: f64::MAX,
            },
        ];
        let sorted = SortedPoints::try_from(points.as_slice()).unwrap();
        assert_eq!(
            auto_sample_count(sorted, InterpAlgorithm::Linear, 10, 100, 1.0e-3, 16),
            100
        );
        let step = [
            XYPoint {
                x: 0.0,
                y: -f64::MAX,
            },
            XYPoint {
                x: 0.2,
                y: f64::MAX,
            },
            XYPoint { x: 1.0, y: 0.0 },
        ];
        assert_eq!(
            auto_sample_count(
                SortedPoints::try_from(step.as_slice()).unwrap(),
                InterpAlgorithm::StepHold,
                10,
                100,
                1.0e-3,
                16
            ),
            100
        );
    }

    #[test]
    fn cubic_auto_sampling_meets_tolerance_on_an_analytic_arch() {
        let points = [
            XYPoint { x: 0.0, y: 0.0 },
            XYPoint { x: 1.0, y: 1.0 },
            XYPoint { x: 2.0, y: 0.0 },
        ];
        let sorted = SortedPoints::try_from(points.as_slice()).unwrap();
        let count = auto_sample_count(sorted, InterpAlgorithm::NaturalCubic, 10, 1000, 1.0e-3, 512);
        assert!((10..=1000).contains(&count));
        let curve = super::interpolate_sorted(sorted, count, InterpAlgorithm::NaturalCubic);
        let mut sampler = PiecewiseCursor::new(&curve, PiecewiseKind::Linear).unwrap();
        for x in sample_positions(&points, 512) {
            let distance = if x <= 1.0 { x } else { 2.0 - x };
            let expected = 0.5f64.mul_add(-distance.powi(3), 1.5 * distance);
            assert!((sampler.sample(x) - expected).abs() <= 1.0e-3);
        }
    }

    #[test]
    fn cubic_keeps_original_duplicate_points_when_resampling_is_disabled() {
        let points = [XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 1.0, y: 3.0 }];
        for count in [0, 1] {
            let out = resample(&points, count, InterpAlgorithm::NaturalCubic);
            assert_eq!(out.len(), points.len());
            for (actual, expected) in out.iter().zip(points) {
                assert_eq!(actual.x, expected.x);
                assert_eq!(actual.y, expected.y);
            }
        }
    }
}
