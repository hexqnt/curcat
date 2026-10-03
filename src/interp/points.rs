//! Borrowed interpolation input validated at the export boundary.

use super::XYPoint;
use std::borrow::Cow;
use std::fmt;

/// Finite points in nondecreasing x order; duplicate knots retain their input order.
#[derive(Debug, Clone, Copy)]
pub struct SortedPoints<'a>(&'a [XYPoint]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidInterpolationPoints {
    NonFinite { index: usize },
    DecreasingX { index: usize },
}

impl fmt::Display for InvalidInterpolationPoints {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { index } => write!(
                formatter,
                "Interpolation point {index} has a non-finite coordinate."
            ),
            Self::DecreasingX { index } => write!(
                formatter,
                "Interpolation point {index} precedes the previous x coordinate."
            ),
        }
    }
}

impl std::error::Error for InvalidInterpolationPoints {}

impl<'a> TryFrom<&'a [XYPoint]> for SortedPoints<'a> {
    type Error = InvalidInterpolationPoints;

    fn try_from(points: &'a [XYPoint]) -> Result<Self, Self::Error> {
        let mut previous_x = f64::NEG_INFINITY;
        for (index, point) in points.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(Self::Error::NonFinite { index });
            }
            if point.x < previous_x {
                return Err(Self::Error::DecreasingX { index });
            }
            previous_x = point.x;
        }
        Ok(Self(points))
    }
}

impl<'a> SortedPoints<'a> {
    pub const fn as_slice(self) -> &'a [XYPoint] {
        self.0
    }
}

/// Borrow ordinary knots; allocate only when near-equal x values must be merged.
pub(super) fn unique_by_x(points: &[XYPoint]) -> Cow<'_, [XYPoint]> {
    if !points
        .array_windows::<2>()
        .any(|[left, right]| (left.x - right.x).abs() <= f64::EPSILON)
    {
        return Cow::Borrowed(points);
    }
    let mut unique: Vec<XYPoint> = Vec::with_capacity(points.len());
    for p in points {
        if let Some(last) = unique.last_mut()
            && (last.x - p.x).abs() <= f64::EPSILON
        {
            *last = *p;
        } else {
            unique.push(*p);
        }
    }
    Cow::Owned(unique)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonfinite_and_decreasing_coordinates() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for point in [XYPoint { x: value, y: 0.0 }, XYPoint { x: 0.0, y: value }] {
                assert_eq!(
                    SortedPoints::try_from([point].as_slice()).unwrap_err(),
                    InvalidInterpolationPoints::NonFinite { index: 0 }
                );
            }
        }
        let points = [XYPoint { x: 2.0, y: 0.0 }, XYPoint { x: 1.0, y: 0.0 }];
        assert_eq!(
            SortedPoints::try_from(points.as_slice()).unwrap_err(),
            InvalidInterpolationPoints::DecreasingX { index: 1 }
        );
    }

    #[test]
    fn accepts_empty_single_and_duplicate_knots_without_copying() {
        let points = [XYPoint { x: 1.0, y: 2.0 }, XYPoint { x: 1.0, y: 3.0 }];
        for slice in [&points[..0], &points[..1], &points[..]] {
            let sorted = SortedPoints::try_from(slice).unwrap();
            assert!(std::ptr::eq(sorted.as_slice(), slice));
        }
    }
}
