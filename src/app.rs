//! Main egui/eframe application state and UI orchestration.

use crate::config::{AppConfig, AutoPlaceConfig};
use crate::export::{self, ExportPayload};
use crate::image::{
    ImageFilters, ImageMeta, LoadedImage, apply_image_filters, describe_aspect_ratio,
    flip_color_image_horizontal, flip_color_image_vertical, format_system_time,
    human_readable_bytes, rotate_color_image_ccw, rotate_color_image_cw, total_pixel_count,
};
use crate::interp::{InterpAlgorithm, XYPoint};
use crate::project::{ImageTransformOp, ImageTransformRecord};
use crate::snap::{SnapFeatureSource, SnapMapCache, SnapThresholdKind};
use crate::types::{
    AngleDirection, AngleUnit, AxisMapping, AxisUnit, AxisValue, CoordSystem, PolarMapping,
    ScaleKind, parse_axis_value,
};
use egui::{Color32, ColorImage, Context, Key, Pos2, Vec2, pos2};

use egui_file_dialog::{DialogState, FileDialog};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    time::{Duration, Instant, SystemTime},
};

mod auto_trace;
mod clipboard;
mod export_helpers;
mod image_loader;
mod point_stats;
mod points;
mod project_state;
mod snap_helpers;
mod ui;
mod util;

pub(crate) use auto_trace::{AutoTraceConfig, AutoTraceDirection};
use project_state::{PendingProjectSave, ProjectApplyPlan, ProjectLoadPrompt};
pub(crate) use util::safe_usize_to_f32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickMode {
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
enum AxisValueField {
    X1,
    X2,
    Y1,
    Y2,
    R1,
    R2,
    A1,
    A2,
}

#[derive(Debug, Clone, Copy)]
enum ZoomAnchor {
    ViewportCenter,
    ViewportPos(Pos2),
}

#[derive(Debug, Clone, Copy)]
enum ZoomIntent {
    Anchor(ZoomAnchor),
    TargetPan(Vec2),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidePanelPosition {
    Left,
    Right,
}

enum ImageLoadRequest {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

struct PendingImageTask {
    rx: Receiver<ImageLoadResult>,
    meta: PendingImageMeta,
}

enum ImageLoadResult {
    Success(ColorImage),
    Error(String),
}

#[derive(Debug, Default)]
struct AutoPlaceState {
    hold_started_at: Option<Instant>,
    active: bool,
    last_pointer: Option<(Pos2, Instant)>,
    last_snapped_point: Option<(Pos2, Instant)>,
    speed_ewma: f32,
    pause_started_at: Option<Instant>,
    suppress_click: bool,
}

#[derive(Debug)]
struct PrimaryPressInfo {
    pos: Pos2,
    time: Instant,
    in_rect: bool,
    shift_down: bool,
}

struct ImageState {
    image: Option<LoadedImage>,
    base_pixels: Option<ColorImage>,
    filters: ImageFilters,
    meta: Option<ImageMeta>,
    transform: ImageTransformRecord,
    pan: Vec2,
    last_viewport_size: Option<Vec2>,
    skip_pan_sync_once: bool,
    pending_fit_on_load: bool,
    zoom: f32,
    zoom_target: f32,
    zoom_intent: ZoomIntent,
    touch_pan_active: bool,
    touch_pan_last: Option<Pos2>,
}

struct ProjectState {
    pending_image_task: Option<PendingImageTask>,
    pending_project_apply: Option<ProjectApplyPlan>,
    pending_project_save: Option<PendingProjectSave>,
    project_prompt: Option<ProjectLoadPrompt>,
    title: Option<String>,
    description: Option<String>,
    active_dialog: Option<NativeDialog>,
    last_project_dir: Option<PathBuf>,
    last_project_path: Option<PathBuf>,
    last_image_dir: Option<PathBuf>,
    last_export_dir: Option<PathBuf>,
}

struct CalibrationState {
    pick_mode: PickMode,
    pending_value_focus: Option<AxisValueField>,
    cal_x: AxisCalUi,
    cal_y: AxisCalUi,
    polar_cal: PolarCalUi,
    coord_system: CoordSystem,
    calibration_angle_snap: bool,
    show_calibration_segments: bool,
    dragging_handle: Option<DragTarget>,
}

struct PointsState {
    points: Vec<PickedPoint>,
    points_numeric_dirty: bool,
    cached_sorted_preview: Vec<(f64, Pos2)>,
    cached_sorted_numeric: Vec<XYPoint>,
    sorted_preview_dirty: bool,
    sorted_numeric_dirty: bool,
    last_x_mapping: Option<AxisMapping>,
    last_y_mapping: Option<AxisMapping>,
    last_polar_mapping: Option<PolarMapping>,
    last_coord_system: CoordSystem,
    show_curve_segments: bool,
}

struct SnapState {
    point_input_mode: PointInputMode,
    contrast_search_radius: f32,
    contrast_threshold: f32,
    centerline_threshold: f32,
    snap_feature_source: SnapFeatureSource,
    snap_threshold_kind: SnapThresholdKind,
    snap_target_color: Color32,
    snap_color_tolerance: f32,
    snap_maps: Option<SnapMapCache>,
    pending_snap_job: Option<SnapBuildJob>,
    snap_maps_dirty: bool,
    snap_overlay_color: Color32,
    snap_overlay_choices: Vec<Color32>,
    snap_overlay_choice: usize,
}

struct ExportState {
    sample_count: usize,
    export_kind: ExportKind,
    interp_algorithm: InterpAlgorithm,
    raw_include_distances: bool,
    raw_include_angles: bool,
    polar_export_include_cartesian: bool,
}

struct InteractionState {
    auto_place_cfg: AutoPlaceConfig,
    auto_place_state: AutoPlaceState,
    auto_trace_cfg: AutoTraceConfig,
    primary_press: Option<PrimaryPressInfo>,
    middle_pan_enabled: bool,
}

struct UiState {
    side_open: bool,
    side_position: SidePanelPosition,
    info_window_open: bool,
    points_info_window_open: bool,
    image_filters_window_open: bool,
    auto_trace_window_open: bool,
    last_status: Option<String>,
}

#[derive(Clone)]
enum PendingImageMeta {
    Path {
        path: PathBuf,
    },
    DroppedBytes {
        name: Option<String>,
        byte_len: usize,
        last_modified: Option<SystemTime>,
    },
}

impl PendingImageMeta {
    fn description(&self) -> String {
        match self {
            Self::Path { path } => path
                .file_name()
                .and_then(|s| s.to_str())
                .map_or_else(|| path.display().to_string(), str::to_string),
            Self::DroppedBytes { name, .. } => name
                .as_deref()
                .map_or_else(|| "dropped bytes".to_string(), str::to_string),
        }
    }

    fn into_image_meta(self) -> ImageMeta {
        match self {
            Self::Path { path } => ImageMeta::from_path(&path),
            Self::DroppedBytes {
                name,
                byte_len,
                last_modified,
            } => ImageMeta::from_dropped_bytes(name.as_deref(), byte_len, last_modified),
        }
    }
}

struct SnapBuildJob {
    rx: Receiver<Option<SnapMapCache>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointInputMode {
    Free,
    ContrastSnap,
    CenterlineSnap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    Interpolated,
    RawPoints,
}

const ZOOM_PRESETS: &[f32] = &[0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 8.0;
const ZOOM_SMOOTH_RESPONSE: f32 = 0.10;
const ZOOM_SNAP_EPS: f32 = 0.0005;
const PAN_SNAP_EPS: f32 = 0.5;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const POINT_HIT_RADIUS: f32 = 12.0;
const SAMPLE_COUNT_MIN: usize = 10;
const CAL_POINT_DRAW_RADIUS: f32 = 4.0;
const CAL_POINT_OUTLINE_PAD: f32 = 1.5;
const CAL_LINE_WIDTH: f32 = 1.6;
const CAL_LINE_OUTLINE_WIDTH: f32 = 3.2;
const CAL_OUTLINE_ALPHA: u8 = 180;
const CAL_ANGLE_SNAP_STEP_RAD: f32 = std::f32::consts::PI / 12.0;
const ATTENTION_BLINK_SPEED: f32 = 2.2;
const ATTENTION_ALPHA_MIN: f32 = 0.35;
const ATTENTION_ALPHA_MAX: f32 = 1.0;
const ATTENTION_OUTLINE_PAD: f32 = 2.0;

#[derive(Debug)]
enum NativeDialog {
    Open(FileDialog),
    OpenProject(FileDialog),
    SaveProject(FileDialog),
    SaveCsv {
        dialog: FileDialog,
        payload: ExportPayload,
    },
    SaveXlsx {
        dialog: FileDialog,
        payload: ExportPayload,
    },

    SaveJson {
        dialog: FileDialog,
        payload: ExportPayload,
    },
    SaveRon {
        dialog: FileDialog,
        payload: ExportPayload,
    },
}

#[derive(Debug, Clone)]
struct AxisCalUi {
    unit: AxisUnit,
    scale: ScaleKind,
    p1: Option<Pos2>,
    p2: Option<Pos2>,
    v1_text: String,
    v2_text: String,
}

impl AxisCalUi {
    fn mapping(&self) -> Option<AxisMapping> {
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

    fn value_invalid_flags(&self) -> (bool, bool) {
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
struct PolarCalUi {
    origin: Option<Pos2>,
    radius: AxisCalUi,
    angle: AxisCalUi,
    angle_unit: AngleUnit,
    angle_direction: AngleDirection,
}

impl PolarCalUi {
    fn mapping(&self) -> Option<PolarMapping> {
        let origin = self.origin?;

        if self.radius.unit != AxisUnit::Float || self.angle.unit != AxisUnit::Float {
            return None;
        }

        let radius_v1 = match parse_axis_value(&self.radius.v1_text, AxisUnit::Float)? {
            AxisValue::Float(v) => v,
            _ => return None,
        };
        let radius_v2 = match parse_axis_value(&self.radius.v2_text, AxisUnit::Float)? {
            AxisValue::Float(v) => v,
            _ => return None,
        };
        if !AxisCalUi::values_are_valid(
            self.radius.scale,
            AxisUnit::Float,
            &AxisValue::Float(radius_v1),
            &AxisValue::Float(radius_v2),
        ) {
            return None;
        }

        let angle_v1 = match parse_axis_value(&self.angle.v1_text, AxisUnit::Float)? {
            AxisValue::Float(v) => v,
            _ => return None,
        };
        let angle_v2 = match parse_axis_value(&self.angle.v2_text, AxisUnit::Float)? {
            AxisValue::Float(v) => v,
            _ => return None,
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

#[derive(Debug, Clone)]
struct PickedPoint {
    pixel: Pos2,
    x_numeric: Option<f64>,
    y_numeric: Option<f64>,
}

impl PickedPoint {
    const fn new(pixel: Pos2) -> Self {
        Self {
            pixel,
            x_numeric: None,
            y_numeric: None,
        }
    }
}

/// Top-level application state for the Curcat UI.
#[allow(clippy::struct_excessive_bools)]
pub struct CurcatApp {
    config: AppConfig,
    image: ImageState,
    project: ProjectState,
    calibration: CalibrationState,
    points: PointsState,
    snap: SnapState,
    export: ExportState,
    interaction: InteractionState,
    ui: UiState,
}

impl Default for CurcatApp {
    fn default() -> Self {
        let config = AppConfig::load();
        let auto_place_cfg = config.auto_place();
        let default_overlay_choices = Self::default_snap_overlay_choices();
        let default_overlay_color = default_overlay_choices
            .first()
            .copied()
            .unwrap_or(Color32::from_rgb(236, 214, 96));
        Self {
            config,
            image: ImageState {
                image: None,
                base_pixels: None,
                filters: ImageFilters::default(),
                meta: None,
                transform: ImageTransformRecord::identity(),
                pan: Vec2::ZERO,
                last_viewport_size: None,
                skip_pan_sync_once: false,
                pending_fit_on_load: false,
                zoom: 1.0,
                zoom_target: 1.0,
                zoom_intent: ZoomIntent::Anchor(ZoomAnchor::ViewportCenter),
                touch_pan_active: false,
                touch_pan_last: None,
            },
            project: ProjectState {
                pending_image_task: None,
                pending_project_apply: None,
                pending_project_save: None,
                project_prompt: None,
                title: None,
                description: None,
                active_dialog: None,
                last_project_dir: None,
                last_project_path: None,
                last_image_dir: None,
                last_export_dir: None,
            },
            calibration: CalibrationState {
                pick_mode: PickMode::None,
                pending_value_focus: None,
                cal_x: AxisCalUi {
                    unit: AxisUnit::Float,
                    scale: ScaleKind::Linear,
                    p1: None,
                    p2: None,
                    v1_text: String::new(),
                    v2_text: String::new(),
                },
                cal_y: AxisCalUi {
                    unit: AxisUnit::Float,
                    scale: ScaleKind::Linear,
                    p1: None,
                    p2: None,
                    v1_text: String::new(),
                    v2_text: String::new(),
                },
                polar_cal: PolarCalUi {
                    origin: None,
                    radius: AxisCalUi {
                        unit: AxisUnit::Float,
                        scale: ScaleKind::Linear,
                        p1: None,
                        p2: None,
                        v1_text: String::new(),
                        v2_text: String::new(),
                    },
                    angle: AxisCalUi {
                        unit: AxisUnit::Float,
                        scale: ScaleKind::Linear,
                        p1: None,
                        p2: None,
                        v1_text: String::new(),
                        v2_text: String::new(),
                    },
                    angle_unit: AngleUnit::Degrees,
                    angle_direction: AngleDirection::Cw,
                },
                coord_system: CoordSystem::Cartesian,
                calibration_angle_snap: false,
                show_calibration_segments: true,
                dragging_handle: None,
            },
            points: PointsState {
                points: Vec::new(),
                points_numeric_dirty: true,
                cached_sorted_preview: Vec::new(),
                cached_sorted_numeric: Vec::new(),
                sorted_preview_dirty: true,
                sorted_numeric_dirty: true,
                last_x_mapping: None,
                last_y_mapping: None,
                last_polar_mapping: None,
                last_coord_system: CoordSystem::Cartesian,
                show_curve_segments: true,
            },
            snap: SnapState {
                point_input_mode: PointInputMode::Free,
                contrast_search_radius: 12.0,
                contrast_threshold: 12.0,
                centerline_threshold: 40.0,
                snap_feature_source: SnapFeatureSource::LumaGradient,
                snap_threshold_kind: SnapThresholdKind::Gradient,
                snap_target_color: Color32::from_rgb(200, 60, 60),
                snap_color_tolerance: 30.0,
                snap_maps: None,
                pending_snap_job: None,
                snap_maps_dirty: true,
                snap_overlay_color: default_overlay_color,
                snap_overlay_choices: default_overlay_choices,
                snap_overlay_choice: 0,
            },
            export: ExportState {
                sample_count: 200,
                export_kind: ExportKind::Interpolated,
                interp_algorithm: InterpAlgorithm::Linear,
                raw_include_distances: false,
                raw_include_angles: false,
                polar_export_include_cartesian: false,
            },
            interaction: InteractionState {
                auto_place_cfg,
                auto_place_state: AutoPlaceState::default(),
                auto_trace_cfg: AutoTraceConfig::default(),
                primary_press: None,
                middle_pan_enabled: false,
            },
            ui: UiState {
                side_open: true,
                side_position: SidePanelPosition::Right,
                info_window_open: false,
                points_info_window_open: false,
                image_filters_window_open: false,
                auto_trace_window_open: false,
                last_status: None,
            },
        }
    }
}

impl CurcatApp {
    /// Create a new app and optionally queue an initial image load.
    pub fn new_with_initial_path(_ctx: &Context, initial_path: Option<&Path>) -> Self {
        let mut app = Self::default();
        if let Some(p) = initial_path {
            app.remember_image_dir_from_path(p);
            app.start_loading_image_from_path(p.to_owned());
        }
        app
    }

    const fn queue_value_focus(&mut self, field: AxisValueField) {
        self.calibration.pending_value_focus = Some(field);
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.ui.last_status = Some(msg.into());
    }

    fn begin_pick_mode(&mut self, mode: PickMode) {
        self.calibration.pick_mode = mode;
        if let Some(label) = Self::pick_mode_label(mode) {
            self.set_status(format!("{label}… (Esc to cancel)"));
        }
    }

    fn cancel_pick_mode(&mut self) {
        if self.calibration.pick_mode != PickMode::None {
            self.calibration.pick_mode = PickMode::None;
            self.set_status("Picking canceled.");
        }
    }

    const fn pick_mode_label(mode: PickMode) -> Option<&'static str> {
        match mode {
            PickMode::X1 => Some("Picking X1"),
            PickMode::X2 => Some("Picking X2"),
            PickMode::Y1 => Some("Picking Y1"),
            PickMode::Y2 => Some("Picking Y2"),
            PickMode::Origin => Some("Picking origin"),
            PickMode::R1 => Some("Picking R1"),
            PickMode::R2 => Some("Picking R2"),
            PickMode::A1 => Some("Picking A1"),
            PickMode::A2 => Some("Picking A2"),
            PickMode::CurveColor => Some("Pick curve color"),
            PickMode::AutoTrace => Some("Auto-trace: click start point"),
            PickMode::None => None,
        }
    }

    fn reset_calibrations(&mut self) {
        self.calibration.cal_x.p1 = None;
        self.calibration.cal_x.p2 = None;
        self.calibration.cal_x.v1_text.clear();
        self.calibration.cal_x.v2_text.clear();
        self.calibration.cal_y.p1 = None;
        self.calibration.cal_y.p2 = None;
        self.calibration.cal_y.v1_text.clear();
        self.calibration.cal_y.v2_text.clear();
        self.calibration.polar_cal.origin = None;
        self.calibration.polar_cal.radius.p1 = None;
        self.calibration.polar_cal.radius.p2 = None;
        self.calibration.polar_cal.radius.v1_text.clear();
        self.calibration.polar_cal.radius.v2_text.clear();
        self.calibration.polar_cal.angle.p1 = None;
        self.calibration.polar_cal.angle.p2 = None;
        self.calibration.polar_cal.angle.v1_text.clear();
        self.calibration.polar_cal.angle.v2_text.clear();
        self.calibration.pick_mode = PickMode::None;
        self.calibration.pending_value_focus = None;
        self.calibration.dragging_handle = None;
    }

    fn update_filtered_texture(&mut self) {
        let Some(base) = self.image.base_pixels.as_ref() else {
            return;
        };
        let Some(image) = self.image.image.as_mut() else {
            return;
        };
        let filtered = apply_image_filters(base, self.image.filters);
        image.replace_pixels(filtered);
    }

    fn after_image_pixels_changed(&mut self) {
        self.mark_snap_maps_dirty();
        self.refresh_snap_overlay_palette();
        self.interaction.auto_place_state = AutoPlaceState::default();
        self.interaction.primary_press = None;
    }

    fn apply_filters_to_loaded_image(&mut self) {
        self.update_filtered_texture();
        self.after_image_pixels_changed();
    }

    fn reset_after_new_image(&mut self) {
        self.reset_calibrations();
        self.points.points.clear();
        self.mark_points_dirty();
        self.calibration.dragging_handle = None;
        self.image.touch_pan_active = false;
        self.image.touch_pan_last = None;
        self.image.pan = Vec2::ZERO;
        self.after_image_pixels_changed();
        self.image.zoom_target = self.image.zoom;
        self.image.zoom_intent = ZoomIntent::TargetPan(self.image.pan);
    }

    fn reset_after_image_transform(&mut self) {
        self.calibration.dragging_handle = None;
        self.image.touch_pan_active = false;
        self.image.touch_pan_last = None;
        self.after_image_pixels_changed();
        self.image.zoom_target = self.image.zoom;
        self.image.zoom_intent = ZoomIntent::TargetPan(self.image.pan);
    }

    fn set_loaded_image(&mut self, mut image: LoadedImage, meta: Option<ImageMeta>) {
        self.image.base_pixels = Some(image.pixels.clone());
        if !self.image.filters.is_identity() {
            let filtered =
                apply_image_filters(self.image.base_pixels.as_ref().unwrap(), self.image.filters);
            image.replace_pixels(filtered);
        }
        self.image.image = Some(image);
        self.image.meta = meta;
        self.image.transform = ImageTransformRecord::identity();
        self.image.pan = Vec2::ZERO;
        self.project.title = None;
        self.project.description = None;
        self.image.zoom = 1.0;
        self.image.zoom_target = 1.0;
        self.image.zoom_intent = ZoomIntent::TargetPan(self.image.pan);
        self.reset_after_new_image();
    }

    fn apply_image_transform(&mut self, op: ImageTransformOp, status: Option<&str>) {
        let Some(base) = self.image.base_pixels.as_mut() else {
            return;
        };
        let old_size = base.size;
        match op {
            ImageTransformOp::RotateCw => rotate_color_image_cw(base),
            ImageTransformOp::RotateCcw => rotate_color_image_ccw(base),
            ImageTransformOp::FlipHorizontal => flip_color_image_horizontal(base),
            ImageTransformOp::FlipVertical => flip_color_image_vertical(base),
        }
        let new_size = base.size;
        self.transform_after_image_op(op, old_size, new_size);
        self.update_filtered_texture();
        self.image.transform.apply(op);
        if let Some(msg) = status {
            self.set_status(msg);
        }
        self.reset_after_image_transform();
    }

    fn transform_after_image_op(
        &mut self,
        op: ImageTransformOp,
        old_size: [usize; 2],
        new_size: [usize; 2],
    ) {
        let map_pos = |pos: Pos2| -> Pos2 {
            let max_x = old_size[0].saturating_sub(1) as f32;
            let max_y = old_size[1].saturating_sub(1) as f32;
            let clamped = pos2(pos.x.clamp(0.0, max_x), pos.y.clamp(0.0, max_y));
            let mapped = match op {
                ImageTransformOp::RotateCw => pos2(max_y - clamped.y, clamped.x),
                ImageTransformOp::RotateCcw => pos2(clamped.y, max_x - clamped.x),
                ImageTransformOp::FlipHorizontal => pos2(max_x - clamped.x, clamped.y),
                ImageTransformOp::FlipVertical => pos2(clamped.x, max_y - clamped.y),
            };
            let new_max_x = new_size[0].saturating_sub(1) as f32;
            let new_max_y = new_size[1].saturating_sub(1) as f32;
            pos2(
                mapped.x.clamp(0.0, new_max_x),
                mapped.y.clamp(0.0, new_max_y),
            )
        };

        self.calibration.cal_x.p1 = self.calibration.cal_x.p1.map(map_pos);
        self.calibration.cal_x.p2 = self.calibration.cal_x.p2.map(map_pos);
        self.calibration.cal_y.p1 = self.calibration.cal_y.p1.map(map_pos);
        self.calibration.cal_y.p2 = self.calibration.cal_y.p2.map(map_pos);
        self.calibration.polar_cal.origin = self.calibration.polar_cal.origin.map(map_pos);
        self.calibration.polar_cal.radius.p1 = self.calibration.polar_cal.radius.p1.map(map_pos);
        self.calibration.polar_cal.radius.p2 = self.calibration.polar_cal.radius.p2.map(map_pos);
        self.calibration.polar_cal.angle.p1 = self.calibration.polar_cal.angle.p1.map(map_pos);
        self.calibration.polar_cal.angle.p2 = self.calibration.polar_cal.angle.p2.map(map_pos);

        for point in &mut self.points.points {
            point.pixel = map_pos(point.pixel);
        }
        self.mark_points_dirty();
    }

    fn rotate_image(&mut self, clockwise: bool) {
        if clockwise {
            self.apply_image_transform(
                ImageTransformOp::RotateCw,
                Some("Rotated image 90° clockwise."),
            );
        } else {
            self.apply_image_transform(
                ImageTransformOp::RotateCcw,
                Some("Rotated image 90° counter-clockwise."),
            );
        }
    }

    fn flip_image(&mut self, horizontal: bool) {
        if horizontal {
            self.apply_image_transform(
                ImageTransformOp::FlipHorizontal,
                Some("Flipped image horizontally."),
            );
        } else {
            self.apply_image_transform(
                ImageTransformOp::FlipVertical,
                Some("Flipped image vertically."),
            );
        }
    }

    const fn set_zoom(&mut self, zoom: f32) {
        self.image.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn set_zoom_about_viewport_center(&mut self, zoom: f32) {
        self.request_zoom(zoom, ZoomIntent::Anchor(ZoomAnchor::ViewportCenter));
    }

    fn set_zoom_about_viewport_pos(&mut self, zoom: f32, anchor: Pos2) {
        self.request_zoom(zoom, ZoomIntent::Anchor(ZoomAnchor::ViewportPos(anchor)));
    }

    fn set_zoom_to_pan_target(&mut self, zoom: f32, pan: Vec2) {
        self.request_zoom(zoom, ZoomIntent::TargetPan(pan));
    }

    fn request_zoom(&mut self, zoom: f32, intent: ZoomIntent) {
        let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if !self.config.smooth_zoom {
            self.apply_zoom_instant(clamped, intent);
            self.image.zoom_target = self.image.zoom;
            self.image.zoom_intent = intent;
            return;
        }
        self.image.zoom_target = clamped;
        self.image.zoom_intent = intent;
    }

    fn apply_zoom_instant(&mut self, zoom: f32, intent: ZoomIntent) {
        match intent {
            ZoomIntent::Anchor(anchor) => self.set_zoom_about_anchor(zoom, anchor),
            ZoomIntent::TargetPan(pan) => {
                self.set_zoom(zoom);
                self.image.pan = pan;
                self.image.skip_pan_sync_once = true;
            }
        }
    }

    fn set_zoom_about_anchor(&mut self, zoom: f32, anchor: ZoomAnchor) {
        let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (clamped - self.image.zoom).abs() <= f32::EPSILON {
            return;
        }
        let Some(viewport) = self.image.last_viewport_size else {
            self.image.zoom = clamped;
            return;
        };
        let Some(image) = self.image.image.as_ref() else {
            self.image.zoom = clamped;
            return;
        };
        let [w, h] = image.size;
        if w == 0 || h == 0 {
            self.image.zoom = clamped;
            return;
        }
        let base_size = Vec2::new(safe_usize_to_f32(w), safe_usize_to_f32(h));
        let old_display = base_size * self.image.zoom;
        let new_display = base_size * clamped;
        let pad_old = Self::center_padding(viewport, old_display);
        let pad_new = Self::center_padding(viewport, new_display);
        let anchor = Self::zoom_anchor_pos(anchor, viewport);
        let zoom_ratio = clamped / self.image.zoom;
        self.image.pan = (self.image.pan + anchor - pad_old) * zoom_ratio - anchor + pad_new;
        self.image.zoom = clamped;
        self.image.skip_pan_sync_once = true;
    }

    fn zoom_anchor_pos(anchor: ZoomAnchor, viewport: Vec2) -> Vec2 {
        let anchor = match anchor {
            ZoomAnchor::ViewportCenter => viewport * 0.5,
            ZoomAnchor::ViewportPos(pos) => pos.to_vec2(),
        };
        Vec2::new(
            anchor.x.clamp(0.0, viewport.x),
            anchor.y.clamp(0.0, viewport.y),
        )
    }

    fn step_zoom_animation(&mut self, ctx: &Context) {
        if !self.config.smooth_zoom {
            return;
        }
        let target = self.image.zoom_target.clamp(MIN_ZOOM, MAX_ZOOM);
        let zoom_delta = target - self.image.zoom;
        let zoom_done = zoom_delta.abs() <= ZOOM_SNAP_EPS;
        let pan_done = match self.image.zoom_intent {
            ZoomIntent::TargetPan(pan) => (self.image.pan - pan).length() <= PAN_SNAP_EPS,
            ZoomIntent::Anchor(_) => true,
        };
        if zoom_done && pan_done {
            match self.image.zoom_intent {
                ZoomIntent::Anchor(anchor) => self.set_zoom_about_anchor(target, anchor),
                ZoomIntent::TargetPan(pan) => {
                    self.set_zoom(target);
                    self.image.pan = pan;
                    self.image.skip_pan_sync_once = true;
                }
            }
            return;
        }

        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let alpha = egui::emath::exponential_smooth_factor(0.90, ZOOM_SMOOTH_RESPONSE, dt);
        let next_zoom = zoom_delta.mul_add(alpha, self.image.zoom);
        match self.image.zoom_intent {
            ZoomIntent::Anchor(anchor) => self.set_zoom_about_anchor(next_zoom, anchor),
            ZoomIntent::TargetPan(pan) => {
                self.set_zoom(next_zoom);
                self.image.pan = self.image.pan + (pan - self.image.pan) * alpha;
                self.image.skip_pan_sync_once = true;
            }
        }
        ctx.request_repaint();
    }

    fn reset_view(&mut self) {
        self.set_zoom_to_pan_target(1.0, Vec2::ZERO);
        self.set_status("View reset to 100%.");
    }

    fn fit_image_to_viewport(&mut self) {
        self.fit_image_to_viewport_with_status(true);
    }

    fn center_padding(viewport: Vec2, display_size: Vec2) -> Vec2 {
        Vec2::new(
            ((viewport.x - display_size.x) * 0.5).max(0.0),
            ((viewport.y - display_size.y) * 0.5).max(0.0),
        )
    }

    fn fit_image_to_viewport_with_status(&mut self, report_status: bool) -> bool {
        let Some(image) = self.image.image.as_ref() else {
            if report_status {
                self.set_status("Load an image before fitting the view.");
            }
            return false;
        };
        let Some(viewport) = self.image.last_viewport_size else {
            if report_status {
                self.set_status("Fit view unavailable: viewport size not ready yet.");
            }
            return false;
        };
        let [w, h] = image.size;
        if w == 0 || h == 0 {
            if report_status {
                self.set_status("Cannot fit an empty image.");
            }
            return false;
        }
        let vw = viewport.x.max(1.0);
        let vh = viewport.y.max(1.0);
        let img_w = safe_usize_to_f32(w);
        let img_h = safe_usize_to_f32(h);
        let margin = 0.98;
        let fit_zoom = (vw / img_w).min(vh / img_h) * margin;
        let clamped = fit_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.set_zoom_to_pan_target(clamped, Vec2::ZERO);
        if report_status {
            self.set_status(format!("Fit view: {:.0}%", clamped * 100.0));
        }
        true
    }

    fn apply_pending_fit_on_load(&mut self) {
        if !self.image.pending_fit_on_load {
            return;
        }
        if self.fit_image_to_viewport_with_status(false) {
            self.image.pending_fit_on_load = false;
        }
    }

    fn format_zoom(zoom: f32) -> String {
        if (zoom - 1.0).abs() < 0.005 {
            "100%".to_string()
        } else {
            format!("{:.0}%", zoom * 100.0)
        }
    }

    fn cartesian_mappings(&self) -> (Option<AxisMapping>, Option<AxisMapping>) {
        (
            self.calibration.cal_x.mapping(),
            self.calibration.cal_y.mapping(),
        )
    }

    fn polar_mapping(&self) -> Option<PolarMapping> {
        self.calibration.polar_cal.mapping()
    }

    fn calibration_ready(&self) -> bool {
        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                let (x, y) = self.cartesian_mappings();
                x.is_some() && y.is_some()
            }
            CoordSystem::Polar => self.polar_mapping().is_some(),
        }
    }

    fn polar_needs_attention(&self) -> bool {
        let origin_missing = self.calibration.polar_cal.origin.is_none();
        let radius = &self.calibration.polar_cal.radius;
        let (r1_invalid, r2_invalid) = radius.value_invalid_flags();
        let mut angle = self.calibration.polar_cal.angle.clone();
        angle.scale = ScaleKind::Linear;
        let (a1_invalid, a2_invalid) = angle.value_invalid_flags();
        origin_missing
            || radius.p1.is_none()
            || radius.p2.is_none()
            || r1_invalid
            || r2_invalid
            || angle.p1.is_none()
            || angle.p2.is_none()
            || a1_invalid
            || a2_invalid
    }

    const fn axis_labels(&self) -> (&'static str, &'static str) {
        match self.calibration.coord_system {
            CoordSystem::Cartesian => ("x", "y"),
            CoordSystem::Polar => ("theta", "r"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragTarget {
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

impl CurcatApp {}

impl eframe::App for CurcatApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_image_loader(ctx);
        self.poll_project_save_job();
        self.poll_snap_build_job();
        // Global hotkeys (ignored while typing in text fields)
        let wants_kb = ctx.wants_keyboard_input();
        if !wants_kb {
            // Ctrl/Cmd + B: toggle side panel
            if ctx.input(|i| i.key_pressed(Key::B) && i.modifiers.command) {
                self.ui.side_open = !self.ui.side_open;
            }
            // Ctrl/Cmd + O: open image
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::O) && i.modifiers.command)
            {
                self.open_image_dialog();
            }
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::P) && i.modifiers.command && i.modifiers.shift)
            {
                self.open_project_dialog();
            }
            if self.project.active_dialog.is_none()
                && self.image.meta.as_ref().and_then(|m| m.path()).is_some()
                && ctx.input(|i| i.key_pressed(Key::S) && i.modifiers.command)
            {
                self.save_project_dialog();
            }
            // Ctrl/Cmd + V: paste image from clipboard
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::V) && i.modifiers.command)
            {
                self.paste_image_from_clipboard(ctx);
            }
            // Ctrl/Cmd + Shift + C: export CSV
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::C) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_csv();
            }
            // Ctrl/Cmd + Shift + J: export JSON
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::J) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_json();
            }
            // Ctrl/Cmd + Shift + R: export RON
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_ron();
            }
            // Ctrl/Cmd + Shift + E: export Excel
            if self.project.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::E) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_xlsx();
            }
            // Ctrl/Cmd + Shift + D: clear all points
            if ctx.input(|i| i.key_pressed(Key::D) && i.modifiers.command && i.modifiers.shift) {
                self.clear_all_points();
            }
            // Ctrl/Cmd + I: show image info
            if self.image.image.is_some()
                && ctx.input(|i| i.key_pressed(Key::I) && i.modifiers.command)
            {
                self.ui.info_window_open = true;
            }
            // Ctrl/Cmd + Shift + F: image filters
            if ctx.input(|i| i.key_pressed(Key::F) && i.modifiers.command && i.modifiers.shift) {
                self.ui.image_filters_window_open = true;
            }
            // Ctrl/Cmd + Shift + T: auto-trace
            if ctx.input(|i| i.key_pressed(Key::T) && i.modifiers.command && i.modifiers.shift) {
                self.ui.auto_trace_window_open = true;
            }
            // Ctrl/Cmd + F: fit view to viewport
            if self.image.image.is_some()
                && ctx.input(|i| i.key_pressed(Key::F) && i.modifiers.command && !i.modifiers.shift)
            {
                self.fit_image_to_viewport();
            }
            // Ctrl/Cmd + R: reset view (zoom 100%, pan origin)
            if self.image.image.is_some()
                && ctx.input(|i| i.key_pressed(Key::R) && i.modifiers.command && !i.modifiers.shift)
            {
                self.reset_view();
            }
            // Ctrl/Cmd + Z: undo
            if ctx.input(|i| i.key_pressed(Key::Z) && i.modifiers.command) {
                self.undo_last_point();
            }
        }

        // Esc: cancel active pick mode
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.cancel_pick_mode();
        }

        let needs_open_hint = self.image.image.is_none();
        let needs_cal_hint = match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                ui::common::axis_needs_attention(&self.calibration.cal_x)
                    || ui::common::axis_needs_attention(&self.calibration.cal_y)
            }
            CoordSystem::Polar => self.polar_needs_attention(),
        };
        if needs_open_hint || needs_cal_hint {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| self.ui_top(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.ui_status_bar(ui));
        let side_panel = match self.ui.side_position {
            SidePanelPosition::Left => egui::SidePanel::left("side"),
            SidePanelPosition::Right => egui::SidePanel::right("side"),
        };
        side_panel
            .resizable(true)
            .default_width(280.0)
            .show_animated(ctx, self.ui.side_open, |ui| self.ui_side_calibration(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.ui_central_image(ctx, ui));
        self.ui_image_info_window(ctx);
        self.ui_image_filters_window(ctx);
        self.ui_auto_trace_window(ctx);
        self.ui_points_info_window(ctx);
        self.ui_project_prompt(ctx);

        let mut close_dialog = false;
        let mut picked_export_path: Option<PathBuf> = None;

        if let Some(dialog_state) = self.project.active_dialog.as_mut() {
            match dialog_state {
                NativeDialog::Open(dialog) => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        self.start_loading_image_from_path(path);
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Open canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::OpenProject(dialog) => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        self.handle_project_load(path);
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Project open canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::SaveProject(dialog) => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        self.handle_project_save(&path);
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Project save canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::SaveCsv { dialog, payload } => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        picked_export_path = Some(path.clone());
                        match export::export_to_csv(&path, payload) {
                            Ok(()) => self.set_status("CSV exported."),
                            Err(e) => self.set_status(format!("CSV export failed: {e}")),
                        }
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Export canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::SaveXlsx { dialog, payload } => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        picked_export_path = Some(path.clone());
                        match export::export_to_xlsx(&path, payload) {
                            Ok(()) => self.set_status("Excel exported."),
                            Err(e) => self.set_status(format!("Excel export failed: {e}")),
                        }
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Export canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::SaveJson { dialog, payload } => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        picked_export_path = Some(path.clone());
                        match export::export_to_json(&path, payload) {
                            Ok(()) => self.set_status("JSON exported."),
                            Err(e) => self.set_status(format!("JSON export failed: {e}")),
                        }
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Export canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
                NativeDialog::SaveRon { dialog, payload } => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        picked_export_path = Some(path.clone());
                        match export::export_to_ron(&path, payload) {
                            Ok(()) => self.set_status("RON exported."),
                            Err(e) => self.set_status(format!("RON export failed: {e}")),
                        }
                        close_dialog = true;
                    } else {
                        match dialog.state() {
                            DialogState::Cancelled => {
                                self.set_status("Export canceled.");
                                close_dialog = true;
                            }
                            DialogState::Closed => close_dialog = true,
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some(path) = picked_export_path {
            self.remember_export_dir_from_path(&path);
        }

        if close_dialog {
            self.project.active_dialog = None;
        }
    }
}
