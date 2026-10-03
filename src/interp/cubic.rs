//! Natural cubic coefficients and a cursor over a nonempty spline.

use super::XYPoint;

/// Store the first segment separately so every spline has a sampleable segment.
pub(super) struct NaturalCubic {
    first: CubicSegment,
    rest: Vec<CubicSegment>,
}

impl NaturalCubic {
    #[allow(clippy::suboptimal_flops)]
    pub(super) fn try_new(points: &[XYPoint]) -> Option<Self> {
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
            let slope_diff =
                (3.0 / width) * (next.y - curr.y) - (3.0 / prev_width) * (curr.y - prev.y);
            let diagonal = 2.0 * (next.x - prev.x) - prev_width * upper_ratio[i];
            if diagonal.abs() <= f64::EPSILON {
                return None;
            }
            upper_ratio[i + 1] = width / diagonal;
            rhs[i + 1] = (slope_diff - prev_width * rhs[i]) / diagonal;
        }

        // Back substitution needs only the next c coefficient, so assemble segments directly.
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
        let first = segments.pop()?;
        segments.reverse();
        Some(Self {
            first,
            rest: segments,
        })
    }

    pub(super) fn cursor(&self) -> CubicCursor<'_> {
        CubicCursor {
            current: &self.first,
            rest: &self.rest,
        }
    }

    #[cfg(test)]
    fn segments(&self) -> impl DoubleEndedIterator<Item = &CubicSegment> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

/// Advance through segments only; coefficients stay borrowed from the prepared spline.
pub(super) struct CubicCursor<'a> {
    current: &'a CubicSegment,
    rest: &'a [CubicSegment],
}

impl CubicCursor<'_> {
    pub(super) fn sample(&mut self, x: f64) -> f64 {
        while let Some((next, rest)) = self.rest.split_first() {
            if x < next.x {
                break;
            }
            self.current = next;
            self.rest = rest;
        }
        self.current.sample(x)
    }
}

#[derive(Debug)]
struct CubicSegment {
    x: f64,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl CubicSegment {
    #[allow(clippy::suboptimal_flops)]
    fn sample(&self, x: f64) -> f64 {
        let dx = x - self.x;
        self.a + self.b * dx + self.c * dx * dx + self.d * dx * dx * dx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
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
        let spline = NaturalCubic::try_new(&points).unwrap();
        let segments: Vec<_> = spline.segments().collect();
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
}
