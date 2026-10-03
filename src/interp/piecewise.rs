//! Sequential sampling of linear and step curves.

use super::XYPoint;

#[derive(Clone, Copy)]
pub(super) enum PiecewiseKind {
    Linear,
    Step,
}

/// Cursor over a nonempty curve for queries with nondecreasing x.
pub(super) struct PiecewiseCursor<'a> {
    current: XYPoint,
    rest: &'a [XYPoint],
    kind: PiecewiseKind,
}

impl<'a> PiecewiseCursor<'a> {
    pub(super) fn new(points: &'a [XYPoint], kind: PiecewiseKind) -> Option<Self> {
        let (&current, rest) = points.split_first()?;
        Some(Self {
            current,
            rest,
            kind,
        })
    }

    pub(super) fn sample(&mut self, x: f64) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_piecewise(
        points: &[XYPoint],
        xs: impl ExactSizeIterator<Item = f64>,
        kind: PiecewiseKind,
    ) -> Vec<XYPoint> {
        let mut sampler = PiecewiseCursor::new(points, kind).unwrap();
        xs.map(|x| XYPoint {
            x,
            y: sampler.sample(x),
        })
        .collect()
    }

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
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
        let linear = sample_piecewise(&points, xs.into_iter(), PiecewiseKind::Linear);
        let step = sample_piecewise(&points, xs.into_iter(), PiecewiseKind::Step);
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
}
