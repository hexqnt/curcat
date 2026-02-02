//! Core calibration types (axis units, mappings, and coordinate systems).

mod axis;
mod coord;
mod mapping;

pub use axis::{AxisUnit, AxisValue, parse_axis_value};
pub use coord::{AngleDirection, AngleUnit, CoordSystem, ScaleKind};
pub use mapping::{AxisMapping, PolarMapping};

#[cfg(test)]
mod tests {
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
    fn from_scalar_seconds_rounds_nanos_across_second() {
        let value = AxisValue::from_scalar_seconds(AxisUnit::DateTime, 1.999_999_999_6)
            .expect("valid datetime");
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
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::Float(1.0),
            v2: AxisValue::Float(100.0),
            scale: ScaleKind::Log10,
            unit: AxisUnit::Float,
        };
        let value = mapping.numeric_at_t(0.5).expect("log10 value");
        assert!((value - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn axis_mapping_log10_rejects_nonpositive_values() {
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::Float(-1.0),
            v2: AxisValue::Float(100.0),
            scale: ScaleKind::Log10,
            unit: AxisUnit::Float,
        };
        assert!(mapping.numeric_at_t(0.5).is_none());
    }

    #[test]
    fn axis_mapping_datetime_midpoint() {
        let start = DateTime::<Utc>::from_timestamp(0, 0)
            .expect("timestamp")
            .naive_utc();
        let end = DateTime::<Utc>::from_timestamp(86_400 * 2, 0)
            .expect("timestamp")
            .naive_utc();
        let mapping = AxisMapping {
            p1: Pos2::new(0.0, 0.0),
            p2: Pos2::new(10.0, 0.0),
            v1: AxisValue::DateTime(start),
            v2: AxisValue::DateTime(end),
            scale: ScaleKind::Linear,
            unit: AxisUnit::DateTime,
        };
        let value = mapping.value_at(Pos2::new(5.0, 0.0)).expect("value");
        let expected = AxisValue::DateTime(
            DateTime::<Utc>::from_timestamp(86_400, 0)
                .expect("timestamp")
                .naive_utc(),
        );
        assert_eq!(value, expected);
    }

    #[test]
    fn polar_mapping_linear_deg_ccw() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::new(
            origin,
            1.0,
            2.0,
            10.0,
            20.0,
            ScaleKind::Linear,
            0.0,
            std::f64::consts::FRAC_PI_2,
            0.0,
            90.0,
            AngleUnit::Degrees,
            AngleDirection::Ccw,
        )
        .expect("valid mapping");

        let r = mapping.radius_at(Pos2::new(1.5, 0.0)).expect("radius");
        assert!((r - 15.0).abs() < 1.0e-6);

        let theta = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);
    }

    #[test]
    fn polar_mapping_cw_wraps_angles() {
        let origin = Pos2::new(0.0, 0.0);
        let mapping = PolarMapping::new(
            origin,
            1.0,
            2.0,
            1.0,
            2.0,
            ScaleKind::Linear,
            0.0,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            90.0,
            AngleUnit::Degrees,
            AngleDirection::Cw,
        )
        .expect("valid mapping");

        let theta = mapping.angle_at(Pos2::new(0.0, -1.0)).expect("angle");
        assert!((theta - 90.0).abs() < 1.0e-6);

        let wrap = mapping.angle_at(Pos2::new(0.0, 1.0)).expect("angle");
        assert!((wrap - 270.0).abs() < 1.0e-6);
    }
}
