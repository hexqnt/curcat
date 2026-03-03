use super::interaction::DragTarget;
use crate::types::{
    AngleDirection, AngleUnit, AxisMapping, AxisUnit, AxisValue, CoordSystem, PolarMapping,
    PolarMappingParams, ScaleKind, parse_axis_value,
};
use egui::Pos2;
use std::cell::RefCell;

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
    parse_cache_v1: RefCell<ParsedAxisValueCache>,
    parse_cache_v2: RefCell<ParsedAxisValueCache>,
}

#[derive(Debug, Clone)]
struct ParsedAxisValueCache {
    unit: AxisUnit,
    text: String,
    value: Option<AxisValue>,
}

impl ParsedAxisValueCache {
    const fn new(unit: AxisUnit) -> Self {
        Self {
            unit,
            text: String::new(),
            value: None,
        }
    }

    fn get_or_parse(&mut self, text: &str, unit: AxisUnit) -> Option<AxisValue> {
        if self.unit != unit || self.text != text {
            self.unit = unit;
            self.text.clear();
            self.text.push_str(text);
            self.value = parse_axis_value(text, unit);
        }
        self.value.clone()
    }
}

impl AxisCalUi {
    pub(super) const fn new(unit: AxisUnit, scale: ScaleKind) -> Self {
        Self::with_values(unit, scale, None, None, String::new(), String::new())
    }

    pub(super) const fn with_values(
        unit: AxisUnit,
        scale: ScaleKind,
        p1: Option<Pos2>,
        p2: Option<Pos2>,
        v1_text: String,
        v2_text: String,
    ) -> Self {
        Self {
            unit,
            scale,
            p1,
            p2,
            v1_text,
            v2_text,
            parse_cache_v1: RefCell::new(ParsedAxisValueCache::new(unit)),
            parse_cache_v2: RefCell::new(ParsedAxisValueCache::new(unit)),
        }
    }

    fn parsed_values(&self) -> (Option<AxisValue>, Option<AxisValue>) {
        let v1 = self
            .parse_cache_v1
            .borrow_mut()
            .get_or_parse(&self.v1_text, self.unit);
        let v2 = self
            .parse_cache_v2
            .borrow_mut()
            .get_or_parse(&self.v2_text, self.unit);
        (v1, v2)
    }

    pub(super) fn mapping(&self) -> Option<AxisMapping> {
        let (p1, p2) = (self.p1?, self.p2?);
        let (v1, v2) = self.parsed_values();
        AxisMapping::try_new(p1, p2, v1?, v2?, self.scale, self.unit).ok()
    }

    pub(super) fn value_invalid_flags(&self) -> (bool, bool) {
        let (v1, v2) = self.parsed_values();
        let invalid_pair = if let (Some(a), Some(b)) = (&v1, &v2) {
            AxisMapping::validate_value_pair(self.scale, self.unit, a, b).is_err()
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

        let (radius_v1, radius_v2) = self.radius.parsed_values();
        let (AxisValue::Float(radius_v1), AxisValue::Float(radius_v2)) = (radius_v1?, radius_v2?)
        else {
            return None;
        };
        if AxisMapping::validate_value_pair(
            self.radius.scale,
            AxisUnit::Float,
            &AxisValue::Float(radius_v1),
            &AxisValue::Float(radius_v2),
        )
        .is_err()
        {
            return None;
        }

        let (angle_v1, angle_v2) = self.angle.parsed_values();
        let (AxisValue::Float(angle_v1), AxisValue::Float(angle_v2)) = (angle_v1?, angle_v2?)
        else {
            return None;
        };
        if AxisMapping::validate_value_pair(
            ScaleKind::Linear,
            AxisUnit::Float,
            &AxisValue::Float(angle_v1),
            &AxisValue::Float(angle_v2),
        )
        .is_err()
        {
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

        PolarMapping::try_new(PolarMappingParams {
            origin,
            radius_distance1: d1,
            radius_distance2: d2,
            radius_value1: radius_v1,
            radius_value2: radius_v2,
            radius_scale: self.radius.scale,
            angle_pixel1: a1,
            angle_pixel2: a2,
            angle_value1: angle_v1,
            angle_value2: angle_v2,
            angle_unit: self.angle_unit,
            angle_direction: self.angle_direction,
        })
        .ok()
    }
}
