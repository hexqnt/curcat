use super::interaction::DragTarget;
use crate::types::{
    AngleDirection, AngleUnit, AxisMapping, AxisUnit, AxisValue, CoordSystem, PolarMapping,
    ScaleKind, parse_axis_value,
};
use egui::Pos2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickMode {
    None,
    X1,
    X2,
    Y1,
    Y2,
    Origin,
    R1,
    R2,
    A1,
    A2,
    CurveColor,
    AutoTrace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisValueField {
    X1,
    X2,
    Y1,
    Y2,
    R1,
    R2,
    A1,
    A2,
}

pub struct CalibrationState {
    pub(super) pick_mode: PickMode,
    pub(super) pending_value_focus: Option<AxisValueField>,
    pub(super) cal_x: AxisCalUi,
    pub(super) cal_y: AxisCalUi,
    pub(super) polar_cal: PolarCalUi,
    pub(super) coord_system: CoordSystem,
    pub(super) calibration_angle_snap: bool,
    pub(super) show_calibration_segments: bool,
    pub(super) dragging_handle: Option<DragTarget>,
}

#[derive(Debug, Clone)]
pub struct AxisCalUi {
    pub(super) unit: AxisUnit,
    pub(super) scale: ScaleKind,
    pub(super) p1: Option<Pos2>,
    pub(super) p2: Option<Pos2>,
    pub(super) v1_text: String,
    pub(super) v2_text: String,
}

impl AxisCalUi {
    pub(super) fn mapping(&self) -> Option<AxisMapping> {
        let (p1, p2) = (self.p1?, self.p2?);
        if !Self::points_are_distinct(p1, p2) {
            return None;
        }
        let v1 = parse_axis_value(&self.v1_text, self.unit)?;
        let v2 = parse_axis_value(&self.v2_text, self.unit)?;
        if !Self::values_are_valid(self.scale, self.unit, &v1, &v2) {
            return None;
        }
        Some(AxisMapping {
            p1,
            p2,
            v1,
            v2,
            scale: self.scale,
            unit: self.unit,
        })
    }

    fn points_are_distinct(p1: Pos2, p2: Pos2) -> bool {
        (p2 - p1).length_sq() > f32::EPSILON
    }

    fn values_are_valid(scale: ScaleKind, unit: AxisUnit, v1: &AxisValue, v2: &AxisValue) -> bool {
        match (unit, v1, v2) {
            (AxisUnit::Float, AxisValue::Float(a), AxisValue::Float(b)) => {
                let finite = a.is_finite() && b.is_finite();
                if !finite {
                    return false;
                }
                let distinct = (*a - *b).abs() > f64::EPSILON;
                let positive = scale != ScaleKind::Log10 || (*a > 0.0 && *b > 0.0);
                distinct && positive
            }
            (AxisUnit::DateTime, AxisValue::DateTime(a), AxisValue::DateTime(b)) => {
                scale == ScaleKind::Linear && a != b
            }
            _ => false,
        }
    }

    pub(super) fn value_invalid_flags(&self) -> (bool, bool) {
        let v1 = parse_axis_value(&self.v1_text, self.unit);
        let v2 = parse_axis_value(&self.v2_text, self.unit);
        let invalid_pair = if let (Some(a), Some(b)) = (&v1, &v2) {
            !Self::values_are_valid(self.scale, self.unit, a, b)
        } else {
            false
        };
        (v1.is_none() || invalid_pair, v2.is_none() || invalid_pair)
    }
}

#[derive(Debug, Clone)]
pub struct PolarCalUi {
    pub(super) origin: Option<Pos2>,
    pub(super) radius: AxisCalUi,
    pub(super) angle: AxisCalUi,
    pub(super) angle_unit: AngleUnit,
    pub(super) angle_direction: AngleDirection,
}

impl PolarCalUi {
    pub(super) fn mapping(&self) -> Option<PolarMapping> {
        let origin = self.origin?;

        if self.radius.unit != AxisUnit::Float || self.angle.unit != AxisUnit::Float {
            return None;
        }

        let AxisValue::Float(radius_v1) =
            parse_axis_value(&self.radius.v1_text, AxisUnit::Float)?
        else {
            return None;
        };
        let AxisValue::Float(radius_v2) =
            parse_axis_value(&self.radius.v2_text, AxisUnit::Float)?
        else {
            return None;
        };
        if !AxisCalUi::values_are_valid(
            self.radius.scale,
            AxisUnit::Float,
            &AxisValue::Float(radius_v1),
            &AxisValue::Float(radius_v2),
        ) {
            return None;
        }

        let AxisValue::Float(angle_v1) =
            parse_axis_value(&self.angle.v1_text, AxisUnit::Float)?
        else {
            return None;
        };
        let AxisValue::Float(angle_v2) =
            parse_axis_value(&self.angle.v2_text, AxisUnit::Float)?
        else {
            return None;
        };
        if !AxisCalUi::values_are_valid(
            ScaleKind::Linear,
            AxisUnit::Float,
            &AxisValue::Float(angle_v1),
            &AxisValue::Float(angle_v2),
        ) {
            return None;
        }

        let rp1 = self.radius.p1?;
        let rp2 = self.radius.p2?;
        let ap1 = self.angle.p1?;
        let ap2 = self.angle.p2?;

        let d1 = f64::from((rp1 - origin).length());
        let d2 = f64::from((rp2 - origin).length());
        let a1 = f64::from((ap1.y - origin.y).atan2(ap1.x - origin.x));
        let a2 = f64::from((ap2.y - origin.y).atan2(ap2.x - origin.x));

        PolarMapping::new(
            origin,
            d1,
            d2,
            radius_v1,
            radius_v2,
            self.radius.scale,
            a1,
            a2,
            angle_v1,
            angle_v2,
            self.angle_unit,
            self.angle_direction,
        )
    }
}
