use crate::snap::{SnapFeatureSource, SnapMapCache, SnapThresholdKind};
use egui::Color32;
use std::sync::mpsc::Receiver;

pub struct SnapBuildJob {
    pub(super) rx: Receiver<Option<SnapMapCache>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointInputMode {
    Free,
    ContrastSnap,
    CenterlineSnap,
}

pub struct SnapState {
    pub(super) point_input_mode: PointInputMode,
    pub(super) contrast_search_radius: f32,
    pub(super) contrast_threshold: f32,
    pub(super) centerline_threshold: f32,
    pub(super) snap_feature_source: SnapFeatureSource,
    pub(super) snap_threshold_kind: SnapThresholdKind,
    pub(super) snap_target_color: Color32,
    pub(super) snap_color_tolerance: f32,
    pub(super) snap_maps: Option<SnapMapCache>,
    pub(super) pending_snap_job: Option<SnapBuildJob>,
    pub(super) snap_maps_dirty: bool,
    pub(super) snap_overlay_color: Color32,
    pub(super) snap_overlay_choices: Vec<Color32>,
    pub(super) snap_overlay_choice: usize,
}
