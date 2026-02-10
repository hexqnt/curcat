use super::auto_trace::AutoTraceConfig;
use crate::config::AutoPlaceConfig;
use egui::Pos2;
use std::time::Instant;

#[derive(Debug, Default)]
pub struct AutoPlaceState {
    pub(super) hold_started_at: Option<Instant>,
    pub(super) active: bool,
    pub(super) last_pointer: Option<(Pos2, Instant)>,
    pub(super) last_snapped_point: Option<(Pos2, Instant)>,
    pub(super) speed_ewma: f32,
    pub(super) pause_started_at: Option<Instant>,
    pub(super) suppress_click: bool,
}

#[derive(Debug)]
pub struct PrimaryPressInfo {
    pub(super) pos: Pos2,
    pub(super) time: Instant,
    pub(super) in_rect: bool,
    pub(super) shift_down: bool,
}

pub struct InteractionState {
    pub(super) auto_place_cfg: AutoPlaceConfig,
    pub(super) auto_place_state: AutoPlaceState,
    pub(super) auto_trace_cfg: AutoTraceConfig,
    pub(super) primary_press: Option<PrimaryPressInfo>,
    pub(super) middle_pan_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    CurvePoint(usize),
    CalX1,
    CalX2,
    CalY1,
    CalY2,
    PolarOrigin,
    PolarR1,
    PolarR2,
    PolarA1,
    PolarA2,
}
