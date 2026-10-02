//! Axis values, coordinate systems, and validated calibration mappings.

mod axis;
mod coord;
mod mapping;

pub use axis::{AxisUnit, AxisValue, parse_axis_value};
pub use coord::{AngleDirection, AngleUnit, CoordSystem, ScaleKind};
pub use mapping::{AxisCalibrationValues, AxisMapping, PolarMapping, PolarMappingParams};

#[cfg(test)]
mod tests {
    use super::mapping::{AxisMappingError, PolarMappingError};
    use super::*;
    use chrono::{DateTime, NaiveDate, Utc};
    use egui::Pos2;

    #[test]
    fn format_float_trims_trailing_zeros_and_negative_zero() {
        assert_eq!(AxisValue::Float(12.340_000).format(), "12.34");
        assert_eq!(AxisValue::Float(5.0).format(), "5");
        assert_eq!(AxisValue::Float(-0.0).format(), "0");
    }

    #[test]
    fn parse_numeric_axis_value_rejects_nonfinite_input() {
        for input in ["NaN", "inf", "-inf", "1e999", "", "text"] {
            assert_eq!(parse_axis_value(input, AxisUnit::Float), None, "{input}");
        }
        assert_eq!(
            parse_axis_value(" -12.5 ", AxisUnit::Float),
            Some(AxisValue::Float(-12.5))
        );
    }

    #[test]
    fn axis_values_reject_mixed_units_equal_and_nonfinite_values() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(
            AxisCalibrationValues::try_new(
                AxisValue::Float(1.0),
                AxisValue::DateTime(date),
                ScaleKind::Linear
            ),
            Err(AxisMappingError::UnitValueMismatch),
        );
        for (start, end) in [
            (AxisValue::Float(1.0), AxisValue::Float(1.0)),
            (AxisValue::DateTime(date), AxisValue::DateTime(date)),
        ] {
            assert_eq!(
                AxisCalibrationValues::try_new(start, end, ScaleKind::Linear),
                Err(AxisMappingError::EqualValues)
            );
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for (start, end) in [(value, 1.0), (1.0, value)] {
                assert_eq!(
                    AxisCalibrationValues::try_new(
                        AxisValue::Float(start),
                        AxisValue::Float(end),
                        ScaleKind::Linear
                    ),
                    Err(AxisMappingError::NonFiniteValue),
                );
            }
        }
    }

    #[test]
    fn axis_mapping_rejects_invalid_geometry() {
        let values = AxisCalibrationValues::try_new(
            AxisValue::Float(0.0),
            AxisValue::Float(1.0),
            ScaleKind::Linear,
        )
        .unwrap();
        assert_eq!(
            AxisMapping::try_new(Pos2::ZERO, Pos2::ZERO, values),
            Err(AxisMappingError::CoincidentPoints)
        );
        for point in [
            Pos2::new(f32::NAN, 0.0),
            Pos2::new(0.0, f32::INFINITY),
            Pos2::new(f32::MAX, 0.0),
        ] {
            for (start, end) in [(Pos2::ZERO, point), (point, Pos2::ZERO)] {
                assert_eq!(
                    AxisMapping::try_new(start, end, values),
                    Err(AxisMappingError::NonFinitePoint)
                );
            }
        }
    }

    #[test]
    fn axis_direction_follows_increasing_values_for_reversed_calibration() {
        for (pixel_start, pixel_end) in [
            (Pos2::ZERO, Pos2::new(3.0, 4.0)),
            (Pos2::new(3.0, 4.0), Pos2::ZERO),
        ] {
            for (start, end) in [(1.0, 10.0), (10.0, 1.0)] {
                let values = AxisCalibrationValues::try_new(
                    AxisValue::Float(start),
                    AxisValue::Float(end),
                    ScaleKind::Log10,
                )
                .unwrap();
                let mapping = AxisMapping::try_new(pixel_start, pixel_end, values).unwrap();
                let direction = mapping.increasing_direction();
                assert!((direction.length() - 1.0).abs() < 1.0e-6);
                let projection = direction.dot(pixel_end - pixel_start);
                assert_eq!(projection.is_sign_positive(), end > start);
            }
        }
    }

    #[test]
    fn axis_direction_keeps_pixel_orientation_when_timestamp_scalars_round_equal() {
        let start = AxisValue::DateTime(
            DateTime::from_timestamp(1_700_000_000, 0)
                .unwrap()
                .naive_utc(),
        );
        let end = AxisValue::DateTime(
            DateTime::from_timestamp(1_700_000_000, 1)
                .unwrap()
                .naive_utc(),
        );
        assert_eq!(start.to_scalar(), end.to_scalar());
        let values = AxisCalibrationValues::try_new(start, end, ScaleKind::Linear).unwrap();
        let mapping = AxisMapping::try_new(Pos2::ZERO, Pos2::new(10.0, 0.0), values).unwrap();
        assert_eq!(mapping.increasing_direction(), egui::Vec2::X);
    }

    #[test]
    fn axis_mapping_extrapolates_and_rejects_nonfinite_results() {
        for (scale, start, end, expected) in [
            (ScaleKind::Linear, 0.0, 10.0, 20.0),
            (ScaleKind::Log10, 1.0, 10.0, 100.0),
        ] {
            let values = AxisCalibrationValues::try_new(
                AxisValue::Float(start),
                AxisValue::Float(end),
                scale,
            )
            .unwrap();
            let mapping = AxisMapping::try_new(Pos2::ZERO, Pos2::new(10.0, 0.0), values).unwrap();
            assert_eq!(mapping.unit(), AxisUnit::Float);
            assert_eq!(mapping.numeric_at(Pos2::new(20.0, 0.0)), Some(expected));
            for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert_eq!(mapping.numeric_at_t(t), None);
            }
            if scale == ScaleKind::Log10 {
                assert_eq!(mapping.numeric_at_t(1000.0), None);
            }
        }
    }

    #[test]
    fn parse_axis_value_accepts_date_and_timezone() {
        let AxisValue::DateTime(date_only) =
            parse_axis_value("2024-01-02", AxisUnit::DateTime).expect("date only parse")
        else {
            panic!("expected datetime");
        };
        let expected_date = NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        assert_eq!(date_only, expected_date);

        let tz_input = "2024-01-02T03:04:05+02:00";
        let AxisValue::DateTime(with_tz) =
            parse_axis_value(tz_input, AxisUnit::DateTime).expect("tz parse")
        else {
            panic!("expected datetime");
        };
        let expected_tz = NaiveDate::from_ymd_opt(2024, 1, 2)
            .unwrap()
            .and_hms_opt(1, 4, 5)
            .unwrap();
        assert_eq!(with_tz, expected_tz);
    }

    #[test]
    fn from_scalar_rounds_nanos_across_second() {
        let value =
            AxisValue::from_scalar(AxisUnit::DateTime, 1.999_999_999_6).expect("valid datetime");
        let AxisValue::DateTime(dt) = value else {
            panic!("expected datetime");
        };
        let expected = DateTime::<Utc>::from_timestamp(2, 0)
            .expect("timestamp")
            .naive_utc();
        assert_eq!(dt, expected);
    }

    #[test]
    fn axis_mapping_log10_interpolates_midpoint() {
        let mapping = AxisMapping::try_new(
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            AxisCalibrationValues::try_new(
                AxisValue::Float(1.0),
                AxisValue::Float(100.0),
                ScaleKind::Log10,
            )
            .expect("valid values"),
        )
        .expect("valid mapping");
        let value = mapping.numeric_at_t(0.5).expect("log10 value");
        assert!((value - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn axis_mapping_log10_rejects_nonpositive_values() {
        let mapping = AxisCalibrationValues::try_new(
            AxisValue::Float(-1.0),
            AxisValue::Float(100.0),
            ScaleKind::Log10,
        );
        assert_eq!(
            mapping,
            Err(AxisMappingError::LogScaleRequiresPositiveValues)
        );
    }

    #[test]
    fn axis_mapping_datetime_midpoint() {
        let start = DateTime::<Utc>::from_timestamp(0, 0)
            .expect("timestamp")
            .naive_utc();
        let end = DateTime::<Utc>::from_timestamp(86_400 * 2, 0)
            .expect("timestamp")
            .naive_utc();
        let mapping = AxisMapping::try_new(
            Pos2::new(0.0, 0.0),
            Pos2::new(10.0, 0.0),
            AxisCalibrationValues::try_new(
                AxisValue::DateTime(start),
                AxisValue::DateTime(end),
                ScaleKind::Linear,
            )
            .expect("valid values"),
        )
        .expect("valid mapping");
        let value = mapping.value_at(Pos2::new(5.0, 0.0)).expect("value");
        let expected = AxisValue::DateTime(
            DateTime::<Utc>::from_timestamp(86_400, 0)
                .expect("timestamp")
                .naive_utc(),
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn axis_values_reject_datetime_log_scale() {
        let start = DateTime::<Utc>::from_timestamp(0, 0)
            .expect("timestamp")
            .naive_utc();
        let end = DateTime::<Utc>::from_timestamp(10, 0)
            .expect("timestamp")
            .naive_utc();
        let result = AxisCalibrationValues::try_new(
            AxisValue::DateTime(start),
            AxisValue::DateTime(end),
            ScaleKind::Log10,
        );
        assert_eq!(
            result,
            Err(AxisMappingError::LogScaleUnsupportedForDateTime)
        );
    }

    #[test]
    fn polar_mapping_linear_deg_ccw() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::try_new(PolarMappingParams {
            origin,
            radius_distance1: 1.0,
            radius_distance2: 2.0,
            radius_value1: 10.0,
            radius_value2: 20.0,
            radius_scale: ScaleKind::Linear,
            angle_pixel1: 0.0,
            angle_pixel2: std::f64::consts::FRAC_PI_2,
            angle_value1: 0.0,
            angle_value2: 90.0,
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Ccw,
        })
        .expect("valid mapping");

        let r = mapping.radius_at(Pos2::new(1.5, 0.0)).expect("radius");
        assert!((r - 15.0).abs() < 1.0e-6);

        let theta = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);
    }

    #[test]
    fn polar_mapping_cw_wraps_angles() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::try_new(PolarMappingParams {
            origin,
            radius_distance1: 1.0,
            radius_distance2: 2.0,
            radius_value1: 1.0,
            radius_value2: 2.0,
            radius_scale: ScaleKind::Linear,
            angle_pixel1: 0.0,
            angle_pixel2: -std::f64::consts::FRAC_PI_2,
            angle_value1: 0.0,
            angle_value2: 90.0,
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Cw,
        })
        .expect("valid mapping");

        let theta = mapping.angle_at(Pos2::new(0.0, -1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);

        let wrap = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((wrap - 270.0).abs() < 1.0e-6);
    }

    #[test]
    fn polar_mapping_try_new_rejects_equal_angle_values() {
        let params = PolarMappingParams {
            origin: Pos2::new(0.0, 0.0),
            radius_distance1: 1.0,
            radius_distance2: 2.0,
            radius_value1: 10.0,
            radius_value2: 20.0,
            radius_scale: ScaleKind::Linear,
            angle_pixel1: 0.0,
            angle_pixel2: std::f64::consts::FRAC_PI_2,
            angle_value1: 90.0,
            angle_value2: 90.0,
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Ccw,
        };
        assert_eq!(
            PolarMapping::try_new(params),
            Err(PolarMappingError::EqualAngleValues)
        );
    }
    #[test]
    fn polar_mapping_validates_origin_and_radius_at_construction() {
        let params = PolarMappingParams {
            origin: Pos2::ZERO,
            radius_distance1: 1.0,
            radius_distance2: 2.0,
            radius_value1: 1.0,
            radius_value2: 100.0,
            radius_scale: ScaleKind::Log10,
            angle_pixel1: 0.0,
            angle_pixel2: std::f64::consts::FRAC_PI_2,
            angle_value1: 0.0,
            angle_value2: 90.0,
            angle_unit: AngleUnit::Degrees,
            angle_direction: AngleDirection::Ccw,
        };
        let mapping = PolarMapping::try_new(params).unwrap();
        assert!((mapping.radius_at(Pos2::new(1.5, 0.0)).unwrap() - 10.0).abs() < 1.0e-6);
        assert_eq!(mapping.angle_at(Pos2::ZERO), None);
        let nonfinite_point = Pos2::new(f32::NAN, 0.0);
        assert_eq!(mapping.angle_at(nonfinite_point), None);
        assert_eq!(mapping.radius_at(nonfinite_point), None);
        assert_eq!(
            PolarMapping::try_new(PolarMappingParams {
                origin: nonfinite_point,
                ..params
            }),
            Err(PolarMappingError::NonFiniteInput)
        );
        assert_eq!(
            PolarMapping::try_new(PolarMappingParams {
                radius_value2: 1.0,
                ..params
            }),
            Err(PolarMappingError::EqualRadiusValues)
        );
        assert_eq!(
            PolarMapping::try_new(PolarMappingParams {
                radius_value1: 0.0,
                ..params
            }),
            Err(PolarMappingError::LogScaleRequiresPositiveRadius)
        );
    }
}
