use crate::config::{AppConfig, AutoPlaceConfig};
use crate::export::{self, ExportPayload};
use crate::image_info::{
    ImageMeta, describe_aspect_ratio, format_system_time, human_readable_bytes, total_pixel_count,
};

use crate::image_util::LoadedImage;
use crate::interp::{InterpAlgorithm, XYPoint};
use crate::snap::{SnapFeatureSource, SnapMapCache, SnapThresholdKind};
use crate::types::{AxisMapping, AxisUnit, AxisValue, ScaleKind, parse_axis_value};
use egui::{Color32, ColorImage, Context, Key, Pos2};

use egui_file_dialog::{DialogState, FileDialog};
use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
    sync::mpsc::Receiver,
    time::{Duration, Instant, SystemTime},
};

mod clipboard;
mod export_helpers;
mod image_loader;
mod points;
mod snap_helpers;
mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickMode {
    None,
    X1,
    X2,
    Y1,
    Y2,
    CurveColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisValueField {
    X1,
    X2,
    Y1,
    Y2,
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

fn safe_usize_to_f32(value: usize) -> f32 {
    let clamped = value.min(u32::MAX as usize);
    let as_u32 = u32::try_from(clamped).unwrap_or(u32::MAX);
    #[allow(clippy::cast_precision_loss)]
    {
        as_u32 as f32
    }
}

fn rounded_u8(value: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value.round().clamp(0.0, f32::from(u8::MAX)) as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    Interpolated,
    RawPoints,
}

const ZOOM_PRESETS: &[f32] = &[0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0];
const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 8.0;
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

#[allow(clippy::struct_excessive_bools)]
pub struct CurcatApp {
    image: Option<LoadedImage>,
    image_meta: Option<ImageMeta>,
    pending_image_task: Option<PendingImageTask>,
    last_status: Option<String>,
    pick_mode: PickMode,
    pending_value_focus: Option<AxisValueField>,
    cal_x: AxisCalUi,
    cal_y: AxisCalUi,
    points: Vec<PickedPoint>,
    points_numeric_dirty: bool,
    cached_sorted_preview: Vec<(f64, Pos2)>,
    cached_sorted_numeric: Vec<XYPoint>,
    sorted_preview_dirty: bool,
    sorted_numeric_dirty: bool,
    last_x_mapping: Option<AxisMapping>,
    last_y_mapping: Option<AxisMapping>,
    calibration_angle_snap: bool,
    show_calibration_segments: bool,
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
    sample_count: usize,
    active_dialog: Option<NativeDialog>,
    last_image_dir: Option<PathBuf>,
    last_export_dir: Option<PathBuf>,
    config: AppConfig,
    auto_place_cfg: AutoPlaceConfig,
    auto_place_state: AutoPlaceState,
    image_zoom: f32,
    dragging_handle: Option<DragTarget>,
    middle_pan_enabled: bool,
    touch_pan_active: bool,
    touch_pan_last: Option<Pos2>,
    side_open: bool,
    info_window_open: bool,
    points_info_window_open: bool,
    export_kind: ExportKind,
    interp_algorithm: InterpAlgorithm,
    raw_include_distances: bool,
    raw_include_angles: bool,
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
            image: None,
            image_meta: None,
            pending_image_task: None,
            last_status: None,
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
            points: Vec::new(),
            points_numeric_dirty: true,
            cached_sorted_preview: Vec::new(),
            cached_sorted_numeric: Vec::new(),
            sorted_preview_dirty: true,
            sorted_numeric_dirty: true,
            last_x_mapping: None,
            last_y_mapping: None,
            calibration_angle_snap: false,
            show_calibration_segments: true,
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
            sample_count: 200,
            active_dialog: None,
            last_image_dir: None,
            last_export_dir: None,
            config,
            auto_place_cfg,
            image_zoom: 1.0,
            dragging_handle: None,
            middle_pan_enabled: true,
            touch_pan_active: false,
            touch_pan_last: None,
            side_open: true,
            info_window_open: false,
            points_info_window_open: false,
            export_kind: ExportKind::Interpolated,
            interp_algorithm: InterpAlgorithm::Linear,
            raw_include_distances: false,
            raw_include_angles: false,
            auto_place_state: AutoPlaceState::default(),
        }
    }
}

impl CurcatApp {
    pub fn new_with_initial_path(_ctx: &Context, initial_path: Option<&Path>) -> Self {
        let mut app = Self::default();
        if let Some(p) = initial_path {
            app.remember_image_dir_from_path(p);
            app.start_loading_image_from_path(p.to_owned());
        }
        app
    }

    const fn queue_value_focus(&mut self, field: AxisValueField) {
        self.pending_value_focus = Some(field);
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.last_status = Some(msg.into());
    }

    fn reset_calibrations(&mut self) {
        self.cal_x.p1 = None;
        self.cal_x.p2 = None;
        self.cal_x.v1_text.clear();
        self.cal_x.v2_text.clear();
        self.cal_y.p1 = None;
        self.cal_y.p2 = None;
        self.cal_y.v1_text.clear();
        self.cal_y.v2_text.clear();
        self.pick_mode = PickMode::None;
        self.pending_value_focus = None;
        self.dragging_handle = None;
    }

    fn reset_after_image_transform(&mut self) {
        self.reset_calibrations();
        self.points.clear();
        self.mark_points_dirty();
        self.dragging_handle = None;
        self.touch_pan_active = false;
        self.touch_pan_last = None;
        self.mark_snap_maps_dirty();
        self.refresh_snap_overlay_palette();
        self.auto_place_state = AutoPlaceState::default();
    }

    fn set_loaded_image(&mut self, image: LoadedImage, meta: Option<ImageMeta>) {
        self.image = Some(image);
        self.image_meta = meta;
        self.image_zoom = 1.0;
        self.reset_after_image_transform();
    }

    fn rotate_image(&mut self, clockwise: bool) {
        if let Some(img) = self.image.as_mut() {
            if clockwise {
                img.rotate_90_cw();
                self.set_status("Rotated image 90° clockwise.");
            } else {
                img.rotate_90_ccw();
                self.set_status("Rotated image 90° counter-clockwise.");
            }
            self.reset_after_image_transform();
        }
    }

    fn flip_image(&mut self, horizontal: bool) {
        if let Some(img) = self.image.as_mut() {
            if horizontal {
                img.flip_horizontal();
                self.set_status("Flipped image horizontally.");
            } else {
                img.flip_vertical();
                self.set_status("Flipped image vertically.");
            }
            self.reset_after_image_transform();
        }
    }

    const fn set_zoom(&mut self, zoom: f32) {
        self.image_zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn format_zoom(zoom: f32) -> String {
        if (zoom - 1.0).abs() < 0.005 {
            "100%".to_string()
        } else {
            format!("{:.0}%", zoom * 100.0)
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
}

impl eframe::App for CurcatApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.poll_image_loader(ctx);
        self.poll_snap_build_job();
        // Global hotkeys (ignored while typing in text fields)
        let wants_kb = ctx.wants_keyboard_input();
        if !wants_kb {
            // Ctrl/Cmd + B: toggle side panel
            if ctx.input(|i| i.key_pressed(Key::B) && i.modifiers.command) {
                self.side_open = !self.side_open;
            }
            // Ctrl/Cmd + O: open image
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::O) && i.modifiers.command)
            {
                self.open_image_dialog();
            }
            // Ctrl/Cmd + V: paste image from clipboard
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::V) && i.modifiers.command)
            {
                self.paste_image_from_clipboard(ctx);
            }
            // Ctrl/Cmd + Shift + C: export CSV
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::C) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_csv();
            }
            // Ctrl/Cmd + Shift + J: export JSON
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::J) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_json();
            }
            // Ctrl/Cmd + Shift + E: export Excel
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::E) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_xlsx();
            }
            // Ctrl/Cmd + Shift + D: clear all points
            if ctx.input(|i| i.key_pressed(Key::D) && i.modifiers.command && i.modifiers.shift) {
                self.clear_all_points();
            }
            // Ctrl/Cmd + I: show image info
            if self.image.is_some() && ctx.input(|i| i.key_pressed(Key::I) && i.modifiers.command) {
                self.info_window_open = true;
            }
            // Ctrl/Cmd + Z: undo
            if ctx.input(|i| i.key_pressed(Key::Z) && i.modifiers.command) {
                self.undo_last_point();
            }
        }

        let needs_open_hint = self.image.is_none();
        let needs_cal_hint = ui::common::axis_needs_attention(&self.cal_x)
            || ui::common::axis_needs_attention(&self.cal_y);
        if needs_open_hint || needs_cal_hint {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| self.ui_top(ui));
        egui::SidePanel::right("side")
            .resizable(true)
            .default_width(280.0)
            .show_animated(ctx, self.side_open, |ui| self.ui_side_calibration(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.ui_central_image(ctx, ui));
        self.ui_image_info_window(ctx);
        self.ui_points_info_window(ctx);

        let mut close_dialog = false;
        let mut picked_export_path: Option<PathBuf> = None;

        if let Some(dialog_state) = self.active_dialog.as_mut() {
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
            }
        }

        if let Some(path) = picked_export_path {
            self.remember_export_dir_from_path(&path);
        }

        if close_dialog {
            self.active_dialog = None;
        }
    }
}
