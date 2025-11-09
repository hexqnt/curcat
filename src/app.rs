use crate::config::AppConfig;
use crate::export::{self, ExportExtraColumn, ExportPayload};
use crate::image_util::{LoadedImage, load_image_from_bytes};
use crate::interp::{InterpAlgorithm, XYPoint, interpolate_sorted};
use crate::types::{AxisMapping, AxisUnit, AxisValue, ScaleKind, parse_axis_value};
use egui::{
    Color32, ColorImage, Context, CornerRadius, Key, PointerButton, Pos2, RichText, Sense,
    StrokeKind, TextEdit, Vec2, lerp, pos2,
};
use egui_file_dialog::{DialogState, FileDialog};
use std::cmp::Ordering;
use std::path::Path;

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
enum SnapFeatureSource {
    LumaGradient,
    ColorMatch,
    Hybrid,
}

impl SnapFeatureSource {
    const ALL: [Self; 3] = [Self::LumaGradient, Self::ColorMatch, Self::Hybrid];

    fn label(self) -> &'static str {
        match self {
            Self::LumaGradient => "Luma gradient",
            Self::ColorMatch => "Color mask",
            Self::Hybrid => "Gradient + color",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapThresholdKind {
    Gradient,
    Score,
}

impl SnapThresholdKind {
    fn label(self) -> &'static str {
        match self {
            Self::Gradient => "Gradient only",
            Self::Score => "Feature score",
        }
    }
}

#[derive(Debug, Clone)]
struct SnapMapLevel {
    size: [usize; 2],
    scale: u32,
    gradient: Vec<f32>,
    color_similarity: Vec<f32>,
}

impl SnapMapLevel {
    fn base_from_image(image: &ColorImage, target: Color32, tolerance: f32) -> Self {
        let size = image.size;
        let len = size[0] * size[1];
        let mut luminance = vec![0.0_f32; len];
        for (idx, color) in image.pixels.iter().enumerate() {
            luminance[idx] = color_luminance(*color);
        }
        let mut gradient = vec![0.0_f32; len];
        let width = size[0];
        let height = size[1];
        if width >= 3 && height >= 3 {
            for y in 1..(height - 1) {
                for x in 1..(width - 1) {
                    let idx = y * width + x;
                    let gx = luminance[idx + 1] - luminance[idx - 1];
                    let gy = luminance[idx + width] - luminance[idx - width];
                    gradient[idx] = (gx * gx + gy * gy).sqrt().min(255.0);
                }
            }
        }
        let mut color_similarity = vec![0.0_f32; len];
        for (idx, color) in image.pixels.iter().enumerate() {
            color_similarity[idx] = color_similarity_value(*color, target, tolerance);
        }
        Self {
            size,
            scale: 1,
            gradient,
            color_similarity,
        }
    }

    fn downsample(prev: &Self) -> Option<Self> {
        let [w, h] = prev.size;
        if w < 2 || h < 2 {
            return None;
        }
        let new_w = w.div_ceil(2);
        let new_h = h.div_ceil(2);
        if new_w < 2 || new_h < 2 {
            return None;
        }
        let mut gradient = vec![0.0; new_w * new_h];
        let mut color_similarity = vec![0.0; new_w * new_h];
        for y in 0..new_h {
            for x in 0..new_w {
                let mut g_sum = 0.0;
                let mut c_sum = 0.0;
                let mut count = 0.0;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let sx = x * 2 + dx;
                        let sy = y * 2 + dy;
                        if sx < w && sy < h {
                            let idx = sy * w + sx;
                            g_sum += prev.gradient[idx];
                            c_sum += prev.color_similarity[idx];
                            count += 1.0;
                        }
                    }
                }
                let idx = y * new_w + x;
                gradient[idx] = if count > 0.0 { g_sum / count } else { 0.0 };
                color_similarity[idx] = if count > 0.0 { c_sum / count } else { 0.0 };
            }
        }
        Some(Self {
            size: [new_w, new_h],
            scale: prev.scale * 2,
            gradient,
            color_similarity,
        })
    }

    fn gradient_at(&self, x: i32, y: i32) -> f32 {
        if self.gradient.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.gradient[yi * self.size[0] + xi]
    }

    fn color_similarity_at(&self, x: i32, y: i32) -> f32 {
        if self.color_similarity.is_empty() {
            return 0.0;
        }
        let xi = clamp_index(x, self.size[0]);
        let yi = clamp_index(y, self.size[1]);
        self.color_similarity[yi * self.size[0] + xi]
    }
}

#[derive(Debug, Clone)]
struct SnapMapCache {
    levels: Vec<SnapMapLevel>,
}

impl SnapMapCache {
    fn build(image: &ColorImage, target: Color32, tolerance: f32) -> Option<Self> {
        if image.size[0] == 0 || image.size[1] == 0 {
            return None;
        }
        let mut levels = Vec::new();
        levels.push(SnapMapLevel::base_from_image(image, target, tolerance));
        while let Some(prev) = levels.last() {
            if prev.size[0] < 4 || prev.size[1] < 4 {
                break;
            }
            if let Some(next) = SnapMapLevel::downsample(prev) {
                levels.push(next);
            } else {
                break;
            }
        }
        Some(Self { levels })
    }

    fn level_for_radius(&self, radius: f32) -> (usize, &SnapMapLevel) {
        assert!(!self.levels.is_empty(), "SnapMapCache without levels");
        let mut chosen = 0;
        for (idx, level) in self.levels.iter().enumerate() {
            let level_scale = u32_to_f32(level.scale);
            if radius / level_scale <= 12.0 || idx == self.levels.len() - 1 {
                chosen = idx;
                break;
            }
        }
        (chosen, &self.levels[chosen])
    }
}

#[derive(Debug, Clone, Copy)]
struct SnapCandidate {
    pos: Pos2,
    score: f32,
    dist: f32,
}

fn safe_usize_to_f32(value: usize) -> f32 {
    let clamped = value.min(u32::MAX as usize);
    let as_u32 = u32::try_from(clamped).unwrap_or(u32::MAX);
    #[allow(clippy::cast_precision_loss)]
    {
        as_u32 as f32
    }
}

fn u32_to_f32(value: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

fn i32_to_f32(value: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

fn clamp_index(value: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    let Ok(last_i32) = i32::try_from(last) else {
        return last;
    };
    let clamped = value.clamp(0, last_i32);
    usize::try_from(clamped).unwrap_or(last)
}

fn clamp_pixel_coord(coord: f32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let max = safe_usize_to_f32(len - 1);
    let rounded = coord.round().clamp(0.0, max);
    let rounded_i32 = saturating_f32_to_i32(rounded);
    clamp_index(rounded_i32, len)
}

fn saturating_f32_to_i32(value: f32) -> i32 {
    #[allow(clippy::cast_precision_loss)]
    const MAX: f32 = i32::MAX as f32;
    #[allow(clippy::cast_precision_loss)]
    const MIN: f32 = i32::MIN as f32;
    #[allow(clippy::cast_possible_truncation)]
    {
        if value.is_nan() {
            0
        } else {
            value.clamp(MIN, MAX).round() as i32
        }
    }
}

fn rounded_u8(value: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value.round().clamp(0.0, f32::from(u8::MAX)) as u8
    }
}

fn color_luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

fn color_similarity_value(color: Color32, target: Color32, tolerance: f32) -> f32 {
    let [tr, tg, tb, _] = target.to_array();
    let [r, g, b, _] = color.to_array();
    let dr = f32::from(r) - f32::from(tr);
    let dg = f32::from(g) - f32::from(tg);
    let db = f32::from(b) - f32::from(tb);
    let diff = (dr * dr + dg * dg + db * db).sqrt();
    let tol = tolerance.max(1.0);
    ((tol - diff).max(0.0) / tol).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointInputMode {
    Free,
    ContrastSnap,
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
const CAL_POINT_DRAW_RADIUS: f32 = 4.0;

fn format_overlay_value(value: &AxisValue) -> String {
    match value {
        AxisValue::Float(v) => format!("{v:.3}"),
        AxisValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

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
}

#[derive(Debug, Clone)]
struct PickedPoint {
    pixel: Pos2,
    x_numeric: Option<f64>,
    y_numeric: Option<f64>,
}

impl PickedPoint {
    fn new(pixel: Pos2) -> Self {
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
    last_status: Option<String>,
    pick_mode: PickMode,
    cal_x: AxisCalUi,
    cal_y: AxisCalUi,
    points: Vec<PickedPoint>,
    point_input_mode: PointInputMode,
    contrast_search_radius: f32,
    contrast_threshold: f32,
    snap_feature_source: SnapFeatureSource,
    snap_threshold_kind: SnapThresholdKind,
    snap_target_color: Color32,
    snap_color_tolerance: f32,
    snap_maps: Option<SnapMapCache>,
    snap_maps_dirty: bool,
    sample_count: usize,
    active_dialog: Option<NativeDialog>,
    config: AppConfig,
    image_zoom: f32,
    dragging_handle: Option<DragTarget>,
    middle_pan_enabled: bool,
    touch_pan_active: bool,
    touch_pan_last: Option<Pos2>,
    side_open: bool,
    export_kind: ExportKind,
    interp_algorithm: InterpAlgorithm,
    raw_include_distances: bool,
    raw_include_angles: bool,
}

impl Default for CurcatApp {
    fn default() -> Self {
        Self {
            image: None,
            last_status: None,
            pick_mode: PickMode::None,
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
            point_input_mode: PointInputMode::Free,
            contrast_search_radius: 12.0,
            contrast_threshold: 12.0,
            snap_feature_source: SnapFeatureSource::LumaGradient,
            snap_threshold_kind: SnapThresholdKind::Gradient,
            snap_target_color: Color32::from_rgb(200, 60, 60),
            snap_color_tolerance: 30.0,
            snap_maps: None,
            snap_maps_dirty: true,
            sample_count: 200,
            active_dialog: None,
            config: AppConfig::load(),
            image_zoom: 1.0,
            dragging_handle: None,
            middle_pan_enabled: true,
            touch_pan_active: false,
            touch_pan_last: None,
            side_open: true,
            export_kind: ExportKind::Interpolated,
            interp_algorithm: InterpAlgorithm::Linear,
            raw_include_distances: false,
            raw_include_angles: false,
        }
    }
}

impl CurcatApp {
    fn open_image_dialog(&mut self) {
        let mut dialog = Self::make_open_dialog();
        dialog.pick_file();
        self.active_dialog = Some(NativeDialog::Open(dialog));
    }

    fn start_export_csv(&mut self) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog("Export CSV", "curve.csv", &["csv"]);
                dialog.save_file();
                self.active_dialog = Some(NativeDialog::SaveCsv { dialog, payload });
            }
            Err(msg) => self.set_status(msg),
        }
    }

    fn start_export_xlsx(&mut self) {
        match self.build_export_payload() {
            Ok(payload) => {
                let mut dialog = Self::make_save_dialog("Export Excel", "curve.xlsx", &["xlsx"]);
                dialog.save_file();
                self.active_dialog = Some(NativeDialog::SaveXlsx { dialog, payload });
            }
            Err(msg) => self.set_status(msg),
        }
    }

    fn clear_all_points(&mut self) {
        self.points.clear();
    }

    fn undo_last_point(&mut self) {
        self.points.pop();
    }

    fn push_curve_point(&mut self, pixel_hint: Pos2) {
        let resolved = self.resolve_curve_pick(pixel_hint);
        self.points.push(PickedPoint::new(resolved));
    }

    fn resolve_curve_pick(&mut self, pixel_hint: Pos2) -> Pos2 {
        self.snap_pixel_if_requested(pixel_hint)
    }

    fn snap_pixel_if_requested(&mut self, pixel_hint: Pos2) -> Pos2 {
        match self.point_input_mode {
            PointInputMode::Free => pixel_hint,
            PointInputMode::ContrastSnap => {
                self.find_contrast_point(pixel_hint).unwrap_or(pixel_hint)
            }
        }
    }

    fn mark_snap_maps_dirty(&mut self) {
        self.snap_maps_dirty = true;
        self.snap_maps = None;
    }

    fn ensure_snap_maps(&mut self) {
        if !self.snap_maps_dirty {
            return;
        }
        if let Some(image) = &self.image {
            self.snap_maps = SnapMapCache::build(
                &image.pixels,
                self.snap_target_color,
                self.snap_color_tolerance,
            );
        } else {
            self.snap_maps = None;
        }
        self.snap_maps_dirty = false;
    }

    fn find_contrast_point(&mut self, pixel_hint: Pos2) -> Option<Pos2> {
        self.ensure_snap_maps();
        let cache = self.snap_maps.as_ref()?;
        if cache.levels.is_empty() {
            return None;
        }
        let radius = self.contrast_search_radius.max(1.0);
        let (_, level) = cache.level_for_radius(radius);
        let scale = u32_to_f32(level.scale);
        let coarse_center = pos2(pixel_hint.x / scale, pixel_hint.y / scale);
        let coarse_radius = (radius / scale).max(1.0);
        let coarse_candidate = self.search_in_level(level, coarse_center, coarse_radius)?;
        let coarse_base_pos = pos2(
            coarse_candidate.pos.x * scale,
            coarse_candidate.pos.y * scale,
        );
        let base_level = cache.levels.first().unwrap();
        let refine_radius = (scale * 2.5).max(3.0);
        let refined_candidate = self
            .search_in_level(base_level, coarse_base_pos, refine_radius)
            .map_or(coarse_base_pos, |cand| cand.pos);
        Some(self.refine_snap_position(refined_candidate))
    }

    fn search_in_level(
        &self,
        level: &SnapMapLevel,
        center: Pos2,
        radius: f32,
    ) -> Option<SnapCandidate> {
        if radius <= 0.0 || level.size[0] < 3 || level.size[1] < 3 {
            return None;
        }
        let width = i32::try_from(level.size[0]).ok()?;
        let height = i32::try_from(level.size[1]).ok()?;
        let radius = radius.max(1.0);
        let radius_sq = radius * radius;
        let reach = saturating_f32_to_i32(radius.ceil());
        let center_x = center.x.clamp(1.0, i32_to_f32(width - 2));
        let center_y = center.y.clamp(1.0, i32_to_f32(height - 2));
        let cx = saturating_f32_to_i32(center_x.round());
        let cy = saturating_f32_to_i32(center_y.round());
        let min_x = (cx - reach).max(1);
        let max_x = (cx + reach).min(width - 2);
        let min_y = (cy - reach).max(1);
        let max_y = (cy + reach).min(height - 2);
        let mut best: Option<SnapCandidate> = None;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let xf = i32_to_f32(x);
                let yf = i32_to_f32(y);
                let dx = xf - center_x;
                let dy = yf - center_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > radius_sq {
                    continue;
                }
                let gradient = level.gradient_at(x, y);
                let color_similarity = level.color_similarity_at(x, y);
                let feature_strength = self.feature_strength(gradient, color_similarity);
                if feature_strength <= 0.0 {
                    continue;
                }
                if !self.threshold_passes(gradient, feature_strength) {
                    continue;
                }
                let dist = dist_sq.sqrt();
                let closeness = (1.0 - dist / radius).max(0.05);
                let score = feature_strength * closeness;
                let candidate = SnapCandidate {
                    pos: pos2(xf, yf),
                    score,
                    dist,
                };
                let update = match &best {
                    Some(existing) => {
                        score > existing.score + 0.1
                            || ((score - existing.score).abs() <= 0.1 && dist < existing.dist)
                    }
                    None => true,
                };
                if update {
                    best = Some(candidate);
                }
            }
        }

        best
    }

    fn refine_snap_position(&self, approx: Pos2) -> Pos2 {
        let Some(cache) = &self.snap_maps else {
            return approx;
        };
        let Some(level) = cache.levels.first() else {
            return approx;
        };
        if level.size[0] < 3 || level.size[1] < 3 {
            return approx;
        }
        let Ok(width) = i32::try_from(level.size[0]) else {
            return approx;
        };
        let Ok(height) = i32::try_from(level.size[1]) else {
            return approx;
        };
        let ax = saturating_f32_to_i32(approx.x.clamp(1.0, i32_to_f32(width - 2)).round());
        let ay = saturating_f32_to_i32(approx.y.clamp(1.0, i32_to_f32(height - 2)).round());

        let mut sum = 0.0;
        let mut sx = 0.0;
        let mut sy = 0.0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let px = (ax + dx).clamp(0, width - 1);
                let py = (ay + dy).clamp(0, height - 1);
                let strength = self
                    .feature_strength(level.gradient_at(px, py), level.color_similarity_at(px, py));
                if strength <= 0.0 {
                    continue;
                }
                sum += strength;
                sx += strength * i32_to_f32(px);
                sy += strength * i32_to_f32(py);
            }
        }
        if sum > 0.0 {
            pos2(
                (sx / sum).clamp(0.0, i32_to_f32(width - 1)),
                (sy / sum).clamp(0.0, i32_to_f32(height - 1)),
            )
        } else {
            approx
        }
    }

    fn feature_strength(&self, gradient: f32, color_similarity: f32) -> f32 {
        let grad_strength = gradient.clamp(0.0, 255.0);
        let color_strength = (color_similarity * 255.0).clamp(0.0, 255.0);
        match self.snap_feature_source {
            SnapFeatureSource::LumaGradient => grad_strength,
            SnapFeatureSource::ColorMatch => color_strength,
            SnapFeatureSource::Hybrid => 0.6 * grad_strength + 0.4 * color_strength,
        }
    }

    fn threshold_passes(&self, gradient: f32, feature_strength: f32) -> bool {
        match self.snap_threshold_kind {
            SnapThresholdKind::Gradient => gradient >= self.contrast_threshold,
            SnapThresholdKind::Score => feature_strength >= self.contrast_threshold,
        }
    }

    fn sample_image_color(&self, pixel: Pos2) -> Option<Color32> {
        let image = self.image.as_ref()?;
        let [w, h] = image.pixels.size;
        if w == 0 || h == 0 {
            return None;
        }
        let x = clamp_pixel_coord(pixel.x, w);
        let y = clamp_pixel_coord(pixel.y, h);
        let idx = y * w + x;
        image.pixels.pixels.get(idx).copied()
    }

    fn pick_curve_color_at(&mut self, pixel: Pos2) {
        if let Some(color) = self.sample_image_color(pixel) {
            self.snap_target_color = color;
            self.mark_snap_maps_dirty();
            self.set_status(format!(
                "Picked curve color #{:02X}{:02X}{:02X}",
                color[0], color[1], color[2]
            ));
        } else {
            self.set_status("Unable to pick color at cursor.");
        }
    }

    pub fn new_with_initial_path(ctx: &Context, initial_path: Option<&Path>) -> Self {
        let mut app = Self::default();
        if let Some(p) = initial_path {
            app.handle_open_path(ctx, p);
        }
        app
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
        self.dragging_handle = None;
    }

    fn reset_after_image_transform(&mut self) {
        self.reset_calibrations();
        self.points.clear();
        self.dragging_handle = None;
        self.touch_pan_active = false;
        self.touch_pan_last = None;
        self.mark_snap_maps_dirty();
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

    fn make_open_dialog() -> FileDialog {
        // Keep in sync with enabled `image` crate features.
        // Add separate presets for frequent formats.
        FileDialog::new()
            .title("Open image")
            // Combined filter
            .add_file_filter_extensions(
                "All images",
                vec![
                    "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tga", "tiff", "tif", "pnm",
                    "pbm", "pgm", "ppm", "hdr", "dds",
                ],
            )
            // Individual format presets
            .add_file_filter_extensions("PNG", vec!["png"])
            .add_file_filter_extensions("JPEG/JPG", vec!["jpg", "jpeg"])
            .add_file_filter_extensions("BMP", vec!["bmp"])
            .add_file_filter_extensions("TIFF", vec!["tiff", "tif"])
            .default_file_filter("All images")
    }

    fn make_save_dialog(title: &str, default_name: &str, extensions: &[&str]) -> FileDialog {
        let mut dialog = FileDialog::new()
            .title(title)
            .default_file_name(default_name);
        let mut first_label: Option<String> = None;
        for ext in extensions {
            let label = format!("*.{ext}");
            if first_label.is_none() {
                first_label = Some(label.clone());
            }
            dialog = dialog.add_save_extension(&label, ext);
        }
        if let Some(label) = first_label.as_deref() {
            dialog = dialog.default_save_extension(label);
        }
        dialog
    }

    fn handle_open_path(&mut self, ctx: &Context, path: &Path) {
        match std::fs::read(path) {
            Ok(bytes) => match load_image_from_bytes(ctx, &self.config, &bytes) {
                Ok(img) => {
                    self.image = Some(img);
                    self.image_zoom = 1.0;
                    self.reset_after_image_transform();
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("image");
                    self.set_status(format!("Loaded {name}"));
                }
                Err(e) => self.set_status(format!("Failed to decode image: {e}")),
            },
            Err(e) => self.set_status(format!("Failed to read file: {e}")),
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        self.image_zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    fn format_zoom(zoom: f32) -> String {
        if (zoom - 1.0).abs() < 0.005 {
            "100%".to_string()
        } else {
            format!("{:.0}%", zoom * 100.0)
        }
    }

    fn handle_middle_pan(&mut self, response: &egui::Response, ui: &mut egui::Ui) {
        // When the MMB pan toggle is off, treat middle drag like direct touch pan.
        let touch_style = !self.middle_pan_enabled;
        let factor = if touch_style {
            1.0
        } else {
            self.config.pan_speed_factor()
        };

        if response.drag_started_by(PointerButton::Middle)
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.touch_pan_active = true;
            self.touch_pan_last = Some(pos);
        }

        if self.touch_pan_active {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(last) = self.touch_pan_last {
                    let delta = (pos - last) * factor;
                    if delta.length_sq() > 0.0 {
                        let scroll_delta = if touch_style { delta } else { -delta };
                        ui.scroll_with_delta(scroll_delta);
                    }
                }
                self.touch_pan_last = Some(pos);
            }

            let middle_down = ui
                .ctx()
                .input(|i| i.pointer.button_down(PointerButton::Middle));
            if !middle_down {
                self.touch_pan_active = false;
                self.touch_pan_last = None;
            }
        } else if touch_style {
            self.touch_pan_last = None;
        }
    }

    fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Use egui's built-in theme toggle so icon matches current mode.
            egui::widgets::global_theme_preference_switch(ui);
            ui.separator();

            let side_label = if self.side_open {
                "Hide side"
            } else {
                "Show side"
            };
            if ui
                .add(egui::Button::new(format!("⟷ {side_label}")).shortcut_text("Ctrl+B"))
                .on_hover_text("Toggle side panel (Ctrl+B)")
                .clicked()
            {
                self.side_open = !self.side_open;
            }
            ui.separator();

            if ui
                .add(egui::Button::new("📂 Open image…").shortcut_text("Ctrl+O"))
                .on_hover_text("Open an image (Ctrl+O). You can also drag & drop into the center.")
                .clicked()
            {
                self.open_image_dialog();
            }

            let has_image = self.image.is_some();
            {
                let resp = ui.add_enabled(has_image, egui::Button::new("↺ 90°"));
                let resp = resp.on_hover_text("Rotate 90° counter-clockwise");
                if resp.clicked() {
                    self.rotate_image(false);
                }
            }
            {
                let resp = ui.add_enabled(has_image, egui::Button::new("↻ 90°"));
                let resp = resp.on_hover_text("Rotate 90° clockwise");
                if resp.clicked() {
                    self.rotate_image(true);
                }
            }

            ui.label("Zoom:")
                .on_hover_text("Choose a preset zoom level");
            let zoom_ir = egui::ComboBox::from_id_salt("zoom_combo")
                .selected_text(Self::format_zoom(self.image_zoom))
                .show_ui(ui, |ui| {
                    for &preset in ZOOM_PRESETS {
                        let label = Self::format_zoom(preset);
                        let selected = (self.image_zoom - preset).abs() < 0.0001;
                        if ui.selectable_label(selected, label).clicked() {
                            self.set_zoom(preset);
                        }
                    }
                });
            zoom_ir.response.on_hover_text("Zoom presets (percent)");

            ui.separator();

            let toggle_response = toggle_switch(ui, &mut self.middle_pan_enabled)
                .on_hover_text("Pan with middle mouse button");
            ui.add_space(4.0);
            ui.label("MMB pan")
                .on_hover_text("Enable/disable middle-button panning");
            if toggle_response.changed() && !self.middle_pan_enabled {
                self.touch_pan_active = false;
                self.touch_pan_last = None;
            }

            ui.separator();

            let resp_clear = ui
                .add(egui::Button::new("🗑 Clear points").shortcut_text("Ctrl+Shift+D"))
                .on_hover_text("Clear all points (Ctrl+Shift+D)");
            if resp_clear.clicked() {
                self.clear_all_points();
            }
            let resp_undo = ui
                .add(egui::Button::new("↶ Undo").shortcut_text("Ctrl+Z"))
                .on_hover_text("Undo last point (Ctrl+Z)");
            if resp_undo.clicked() {
                self.undo_last_point();
            }
        });
    }

    #[allow(clippy::too_many_lines)]
    fn ui_point_input_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Point input");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.point_input_mode, PointInputMode::Free, "Free")
                .on_hover_text("Place points exactly where you click");
            ui.radio_value(
                &mut self.point_input_mode,
                PointInputMode::ContrastSnap,
                "Contrast snap",
            )
            .on_hover_text("Snap to the nearest high-contrast area inside the search radius");
        });
        if self.point_input_mode == PointInputMode::ContrastSnap {
            ui.add_space(4.0);
            ui.label("Search radius (px)").on_hover_text(
                "Measured in image pixels; smaller values keep snapping near the cursor",
            );
            ui.spacing_mut().slider_width = 150.0;
            ui.add(
                egui::Slider::new(&mut self.contrast_search_radius, 3.0..=60.0)
                    .logarithmic(false)
                    .clamping(egui::SliderClamping::Always)
                    .text("px"),
            )
            .on_hover_text("Radius used to look for contrast");
            ui.add_space(4.0);
            ui.label("Feature source")
                .on_hover_text("Choose what the snapper looks at when searching for a candidate");
            egui::ComboBox::from_id_salt("snap_feature_source")
                .selected_text(self.snap_feature_source.label())
                .show_ui(ui, |ui| {
                    for variant in SnapFeatureSource::ALL {
                        ui.selectable_value(
                            &mut self.snap_feature_source,
                            variant,
                            variant.label(),
                        );
                    }
                });
            if matches!(
                self.snap_feature_source,
                SnapFeatureSource::ColorMatch | SnapFeatureSource::Hybrid
            ) {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Curve color:");
                    let color_button = ui
                        .color_edit_button_srgba(&mut self.snap_target_color)
                        .on_hover_text("Target color for the curve");
                    if color_button.changed() {
                        self.mark_snap_maps_dirty();
                    }
                    if ui
                        .button("Pick from image")
                        .on_hover_text("Click, then select a pixel on the image")
                        .clicked()
                    {
                        self.pick_mode = PickMode::CurveColor;
                        self.set_status("Click on the image to sample the curve color.");
                    }
                });
                let tol_resp = ui
                    .add(
                        egui::Slider::new(&mut self.snap_color_tolerance, 5.0..=150.0)
                            .text("tolerance")
                            .clamping(egui::SliderClamping::Always),
                    )
                    .on_hover_text("How far the pixel color may deviate from the picked color");
                if tol_resp.changed() {
                    self.mark_snap_maps_dirty();
                }
            }
            ui.add_space(4.0);
            ui.label("Threshold mode")
                .on_hover_text("Select how the detector decides if a pixel is strong enough");
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut self.snap_threshold_kind,
                    SnapThresholdKind::Gradient,
                    SnapThresholdKind::Gradient.label(),
                )
                .on_hover_text("Compare threshold against raw gradient strength");
                ui.radio_value(
                    &mut self.snap_threshold_kind,
                    SnapThresholdKind::Score,
                    SnapThresholdKind::Score.label(),
                )
                .on_hover_text("Compare threshold against combined feature score");
            });
            let threshold_range = if matches!(self.snap_threshold_kind, SnapThresholdKind::Gradient)
            {
                0.0..=120.0
            } else {
                0.0..=255.0
            };
            ui.add(
                egui::Slider::new(&mut self.contrast_threshold, threshold_range)
                    .text("threshold")
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text("Higher = snap only to strong candidates");
            ui.scope(|ui| {
                ui.style_mut().spacing.item_spacing.x = 4.0;
                ui.label(
                    RichText::new(
                        "The preview circle in the image shows the area that will be scanned.",
                    )
                    .small(),
                );
            });
        }
    }

    fn ui_side_calibration(&mut self, ui: &mut egui::Ui) {
        self.ui_point_input_section(ui);
        ui.separator();

        ui.heading("Calibration");
        ui.separator();

        self.axis_cal_group(ui, true);
        ui.separator();
        self.axis_cal_group(ui, false);

        ui.separator();
        self.ui_export_section(ui);

        if let Some(msg) = &self.last_status {
            ui.separator();
            ui.label(RichText::new(msg).small());
        }

        let remaining = ui.available_height().max(0.0);
        if remaining > 24.0 {
            ui.add_space(remaining - 20.0);
        }
        ui.separator();
        ui.label(
            RichText::new(format!("Version {APP_VERSION}"))
                .small()
                .color(Color32::from_gray(160)),
        );
    }

    fn ui_export_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Export points");
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.export_kind,
                ExportKind::Interpolated,
                "Interpolated curve",
            )
            .on_hover_text("Export evenly spaced samples of the curve");
            ui.radio_value(
                &mut self.export_kind,
                ExportKind::RawPoints,
                "Raw picked points",
            )
            .on_hover_text("Export only the points you clicked, in order");
        });
        ui.add_space(4.0);

        match self.export_kind {
            ExportKind::Interpolated => {
                ui.label("Interpolation:")
                    .on_hover_text("Choose how to interpolate between control points");
                let combo = egui::ComboBox::from_id_salt("interp_algo_combo")
                    .selected_text(self.interp_algorithm.label())
                    .show_ui(ui, |ui| {
                        for algo in InterpAlgorithm::ALL.iter().copied() {
                            ui.selectable_value(&mut self.interp_algorithm, algo, algo.label());
                        }
                    });
                combo
                    .response
                    .on_hover_text("Algorithm used to generate the interpolated samples");

                ui.label("Samples:")
                    .on_hover_text("Number of evenly spaced samples to export");
                ui.spacing_mut().slider_width = 150.0;
                let sresp =
                    ui.add(egui::Slider::new(&mut self.sample_count, 10..=10000).text("count"));
                sresp.on_hover_text("Higher values give a denser interpolated curve (max 5000)");
            }
            ExportKind::RawPoints => {
                ui.label("Extra columns:")
                    .on_hover_text("Optional metrics for the picked points");
                let dist = ui.checkbox(
                    &mut self.raw_include_distances,
                    "Include distance to previous point",
                );
                dist.on_hover_text(
                    "Adds a column with distances between consecutive picked points",
                );
                let ang = ui.checkbox(&mut self.raw_include_angles, "Include angle (deg)");
                ang.on_hover_text(
                    "Adds a column with angles at each interior point (first/last stay empty)",
                );
            }
        }

        ui.separator();
        let resp_csv = ui
            .add(egui::Button::new("📄 Export CSV…").shortcut_text("Ctrl+Shift+C"))
            .on_hover_text("Export data to CSV (Ctrl+Shift+C)");
        if resp_csv.clicked() {
            self.start_export_csv();
        }
        let resp_xlsx = ui
            .add(egui::Button::new("📊 Export Excel…").shortcut_text("Ctrl+Shift+E"))
            .on_hover_text("Export data to Excel (Ctrl+Shift+E)");
        if resp_xlsx.clicked() {
            self.start_export_xlsx();
        }
    }

    fn axis_cal_group(&mut self, ui: &mut egui::Ui, is_x: bool) {
        let (label, cal, p1_mode, p2_mode) = if is_x {
            ("X axis", &mut self.cal_x, PickMode::X1, PickMode::X2)
        } else {
            ("Y axis", &mut self.cal_y, PickMode::Y1, PickMode::Y2)
        };

        let collapsing = egui::CollapsingHeader::new(label).show(ui, |ui| {
            ui.push_id(label, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Unit:")
                        .on_hover_text("Value type for the axis (Float/DateTime)");
                    let mut unit = cal.unit;
                    let unit_ir = egui::ComboBox::from_id_salt(format!("{label}_unit_combo"))
                        .selected_text(match unit {
                            AxisUnit::Float => "Float",
                            AxisUnit::DateTime => "DateTime",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut unit, AxisUnit::Float, "Float");
                            ui.selectable_value(&mut unit, AxisUnit::DateTime, "DateTime");
                        });
                    unit_ir.response.on_hover_text("Choose the axis value type");
                    cal.unit = unit;
                    ui.separator();

                    ui.label("Scale:")
                        .on_hover_text("Axis scale (Linear/Log10)");
                    let mut scale = cal.scale;
                    let scale_ir = egui::ComboBox::from_id_salt(format!("{label}_scale_combo"))
                        .selected_text(match scale {
                            ScaleKind::Linear => "Linear",
                            ScaleKind::Log10 => "Log10",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut scale, ScaleKind::Linear, "Linear");
                            ui.selectable_value(&mut scale, ScaleKind::Log10, "Log10");
                        });
                    scale_ir.response.on_hover_text("Choose the axis scale");
                    cal.scale = scale;
                });

                if cal.unit == AxisUnit::DateTime && cal.scale == ScaleKind::Log10 {
                    ui.label(
                        RichText::new("Log scale is not supported for DateTime")
                            .color(Color32::YELLOW),
                    );
                }

                ui.horizontal(|ui| {
                    ui.label("P1 value:")
                        .on_hover_text("Value of the first calibration point (P1)");
                    let p1_resp = ui.add_sized(
                        [100.0, ui.spacing().interact_size.y],
                        TextEdit::singleline(&mut cal.v1_text),
                    );
                    p1_resp.on_hover_text(match cal.unit {
                        AxisUnit::Float => "Enter a number (e.g., 1.23)",
                        AxisUnit::DateTime => "Enter date/time (e.g., 2024-10-31 12:30)",
                    });
                    let btn = ui.button("📍 Pick P1");
                    let btn =
                        btn.on_hover_text("Click, then pick the corresponding point on the image");
                    if btn.clicked() {
                        self.pick_mode = p1_mode;
                    }
                    if let Some(p) = cal.p1 {
                        ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("P2 value:")
                        .on_hover_text("Value of the second calibration point (P2)");
                    let p2_resp = ui.add_sized(
                        [100.0, ui.spacing().interact_size.y],
                        TextEdit::singleline(&mut cal.v2_text),
                    );
                    p2_resp.on_hover_text(match cal.unit {
                        AxisUnit::Float => "Enter a number (e.g., 4.56)",
                        AxisUnit::DateTime => "Enter date/time (e.g., 2024-10-31 12:45)",
                    });
                    let btn = ui.button("📍 Pick P2");
                    let btn =
                        btn.on_hover_text("Click, then pick the corresponding point on the image");
                    if btn.clicked() {
                        self.pick_mode = p2_mode;
                    }
                    if let Some(p) = cal.p2 {
                        ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
                    }
                });

                if cal.mapping().is_some() {
                    ui.label(RichText::new("Mapping: OK").color(Color32::GREEN))
                        .on_hover_text("Calibration complete — you can pick points and export");
                } else {
                    ui.label(RichText::new("Mapping: incomplete or invalid").color(Color32::GRAY))
                        .on_hover_text("Provide two points and valid values to calibrate");
                }
            });
        });
        collapsing.header_response.on_hover_text(if is_x {
            "X axis calibration"
        } else {
            "Y axis calibration"
        });
    }

    #[allow(clippy::too_many_lines)]
    fn ui_central_image(&mut self, ctx: &Context, ui: &mut egui::Ui) {
        // Handle drag & drop regardless of whether an image is already loaded
        let (hovered_files, dropped_files) =
            ui.input(|i| (i.raw.hovered_files.clone(), i.raw.dropped_files.clone()));
        if (!hovered_files.is_empty() || !dropped_files.is_empty()) && cfg!(debug_assertions) {
            eprintln!(
                "[DnD] hovered={} dropped={}",
                hovered_files.len(),
                dropped_files.len()
            );
            for (idx, h) in hovered_files.iter().enumerate() {
                eprintln!("[DnD] hover[{idx}] path={:?} mime={}", h.path, h.mime);
            }
            for (idx, f) in dropped_files.iter().enumerate() {
                let blen = f.bytes.as_ref().map_or(0, |b| b.len());
                eprintln!(
                    "[DnD] drop[{idx}] name='{}' mime={} path={:?} bytes={} last_modified={:?}",
                    f.name, f.mime, f.path, blen, f.last_modified
                );
            }
        }
        if !dropped_files.is_empty() {
            let mut loaded = false;
            for f in &dropped_files {
                if let Some(bytes) = &f.bytes
                    && let Ok(new_img) = load_image_from_bytes(ctx, &self.config, bytes)
                {
                    self.image = Some(new_img);
                    self.image_zoom = 1.0;
                    self.reset_after_image_transform();
                    loaded = true;
                    self.set_status(format!("Loaded from drop (bytes): {}", f.name));
                    if cfg!(debug_assertions) {
                        eprintln!("[DnD] Loaded from bytes: name='{}'", f.name);
                    }
                    break;
                }
                if let Some(path) = &f.path
                    && let Ok(bytes) = std::fs::read(path)
                    && let Ok(new_img) = load_image_from_bytes(ctx, &self.config, &bytes)
                {
                    self.image = Some(new_img);
                    self.image_zoom = 1.0;
                    self.reset_after_image_transform();
                    loaded = true;
                    self.set_status(format!("Loaded from drop (path): {}", path.display()));
                    if cfg!(debug_assertions) {
                        eprintln!("[DnD] Loaded from path: {}", path.display());
                    }
                    break;
                }
            }
            if !loaded {
                self.set_status("Drop failed: no readable bytes/path");
                if cfg!(debug_assertions) {
                    eprintln!("[DnD] Drop failed: no readable bytes/path");
                }
            }
        }

        if self.image.is_some() {
            let mut x_mapping = self.cal_x.mapping();
            let mut y_mapping = self.cal_y.mapping();
            // Take a snapshot of the texture handle and size to avoid borrowing self.image in the UI closure
            let (tex_id, img_size) = {
                let img = self.image.as_ref().unwrap();
                (img.texture.id(), img.size)
            };
            egui::ScrollArea::both().show(ui, |ui| {
                let base_size = egui::vec2(
                    safe_usize_to_f32(img_size[0]),
                    safe_usize_to_f32(img_size[1]),
                );
                let display_size = base_size * self.image_zoom;
                let image = egui::Image::new((tex_id, display_size));
                let response = ui.add(image.sense(Sense::click_and_drag()));
                let rect = response.rect;
                let painter = ui.painter_at(rect);

                self.handle_middle_pan(&response, ui);

                if response.hovered() {
                    let mut scroll = 0.0_f32;
                    let mut ctrl = false;
                    ui.ctx().input(|i| {
                        scroll = i.raw_scroll_delta.y;
                        ctrl = i.modifiers.ctrl;
                    });
                    if ctrl && scroll.abs() > f32::EPSILON {
                        let steps = (scroll / 40.0).round();
                        if steps.abs() > f32::EPSILON {
                            let base: f32 = if steps > 0.0 { 1.1 } else { 0.9 };
                            let factor = base.powf(steps.abs());
                            self.set_zoom(self.image_zoom * factor);
                        }
                    }
                }

                let zoom = self.image_zoom;
                let to_pixel = |pos: Pos2| {
                    let local = pos - rect.min;
                    pos2(
                        (local.x / zoom).clamp(0.0, base_size.x),
                        (local.y / zoom).clamp(0.0, base_size.y),
                    )
                };

                let pointer_pos = response.interact_pointer_pos();
                let hover_pos = response.hover_pos();
                let (shift_pressed, primary_down, delete_down) = ui.ctx().input(|i| {
                    (
                        i.modifiers.shift,
                        i.pointer.button_down(PointerButton::Primary),
                        i.key_down(Key::Delete),
                    )
                });
                let pointer_pixel = hover_pos.map(&to_pixel);
                let mut snap_preview: Option<Pos2> = None;
                if self.point_input_mode == PointInputMode::ContrastSnap
                    && let Some(pixel) = pointer_pixel
                {
                    snap_preview = self.find_contrast_point(pixel);
                }

                if shift_pressed
                    && response.drag_started_by(PointerButton::Primary)
                    && let Some(pos) = pointer_pos
                {
                    let mut best: Option<(DragTarget, f32)> = None;
                    let mut consider = |target: DragTarget, screen: Pos2| {
                        let dist = pos.distance(screen);
                        if dist <= POINT_HIT_RADIUS
                            && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
                        {
                            best = Some((target, dist));
                        }
                    };

                    for (idx, point) in self.points.iter().enumerate() {
                        let screen = rect.min + point.pixel.to_vec2() * self.image_zoom;
                        consider(DragTarget::CurvePoint(idx), screen);
                    }

                    for (target, maybe_pixel) in [
                        (DragTarget::CalX1, self.cal_x.p1),
                        (DragTarget::CalX2, self.cal_x.p2),
                        (DragTarget::CalY1, self.cal_y.p1),
                        (DragTarget::CalY2, self.cal_y.p2),
                    ] {
                        if let Some(pixel) = maybe_pixel {
                            let screen = rect.min + pixel.to_vec2() * self.image_zoom;
                            consider(target, screen);
                        }
                    }

                    self.dragging_handle = best.map(|(target, _)| target);
                }

                if let Some(target) = self.dragging_handle {
                    if let Some(pos) = pointer_pos {
                        let pixel = to_pixel(pos);
                        match target {
                            DragTarget::CurvePoint(idx) => {
                                if let Some(point) = self.points.get_mut(idx) {
                                    point.pixel = pixel;
                                }
                            }
                            DragTarget::CalX1 => {
                                self.cal_x.p1 = Some(pixel);
                                x_mapping = self.cal_x.mapping();
                            }
                            DragTarget::CalX2 => {
                                self.cal_x.p2 = Some(pixel);
                                x_mapping = self.cal_x.mapping();
                            }
                            DragTarget::CalY1 => {
                                self.cal_y.p1 = Some(pixel);
                                y_mapping = self.cal_y.mapping();
                            }
                            DragTarget::CalY2 => {
                                self.cal_y.p2 = Some(pixel);
                                y_mapping = self.cal_y.mapping();
                            }
                        }
                    }
                    if !shift_pressed || !primary_down {
                        self.dragging_handle = None;
                    }
                } else if response.clicked_by(PointerButton::Primary)
                    && !shift_pressed
                    && let Some(pos) = pointer_pos
                {
                    if delete_down {
                        let mut best: Option<(usize, f32)> = None;
                        for (idx, point) in self.points.iter().enumerate() {
                            let screen = rect.min + point.pixel.to_vec2() * self.image_zoom;
                            let dist = pos.distance(screen);
                            if dist <= POINT_HIT_RADIUS
                                && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
                            {
                                best = Some((idx, dist));
                            }
                        }
                        if let Some((idx, _)) = best {
                            self.points.remove(idx);
                        }
                    } else {
                        let pixel = to_pixel(pos);
                        match self.pick_mode {
                            PickMode::None => {
                                if x_mapping.is_some() && y_mapping.is_some() {
                                    self.push_curve_point(pixel);
                                }
                            }
                            PickMode::X1 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                self.cal_x.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                            }
                            PickMode::X2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                self.cal_x.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                            }
                            PickMode::Y1 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                self.cal_y.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                            }
                            PickMode::Y2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                self.cal_y.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                            }
                            PickMode::CurveColor => {
                                self.pick_curve_color_at(pixel);
                                self.pick_mode = PickMode::None;
                            }
                        }
                    }
                }

                // Update numeric values for points if mappings are ready
                if let (Some(xm), Some(ym)) = (x_mapping.as_ref(), y_mapping.as_ref()) {
                    for p in &mut self.points {
                        p.x_numeric = xm.numeric_at(p.pixel);
                        p.y_numeric = ym.numeric_at(p.pixel);
                    }
                }

                // Draw picked calibration points lines
                let stroke_cal = egui::Stroke {
                    width: 1.0,
                    color: Color32::LIGHT_BLUE,
                };
                let cal_point_color = stroke_cal.color;
                let draw_cal_point = |point: Pos2| {
                    let screen = rect.min + point.to_vec2() * self.image_zoom;
                    painter.circle_filled(screen, CAL_POINT_DRAW_RADIUS, cal_point_color);
                };
                if let Some(p1) = self.cal_x.p1
                    && let Some(p2) = self.cal_x.p2
                {
                    painter.line_segment(
                        [
                            rect.min + p1.to_vec2() * self.image_zoom,
                            rect.min + p2.to_vec2() * self.image_zoom,
                        ],
                        stroke_cal,
                    );
                }
                if let Some(p) = self.cal_x.p1 {
                    draw_cal_point(p);
                }
                if let Some(p) = self.cal_x.p2 {
                    draw_cal_point(p);
                }
                if let Some(p) = self.cal_y.p1 {
                    draw_cal_point(p);
                }
                if let Some(p) = self.cal_y.p2 {
                    draw_cal_point(p);
                }
                if let Some(p1) = self.cal_y.p1
                    && let Some(p2) = self.cal_y.p2
                {
                    painter.line_segment(
                        [
                            rect.min + p1.to_vec2() * self.image_zoom,
                            rect.min + p2.to_vec2() * self.image_zoom,
                        ],
                        stroke_cal,
                    );
                }

                // Draw picked points
                let point_style = &self.config.curve_points;
                let point_color = point_style.color32();
                let point_radius = point_style.radius();
                for (idx, p) in self.points.iter().enumerate() {
                    let screen = rect.min + p.pixel.to_vec2() * self.image_zoom;
                    painter.circle_filled(screen, point_radius, point_color);
                    painter.text(
                        screen + Vec2::new(6.0, -6.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", idx + 1),
                        egui::FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                }

                if self.point_input_mode == PointInputMode::ContrastSnap {
                    if let Some(pixel) = pointer_pixel {
                        let screen = rect.min + pixel.to_vec2() * self.image_zoom;
                        let radius = (self.contrast_search_radius * self.image_zoom).max(4.0);
                        painter.circle_stroke(
                            screen,
                            radius,
                            egui::Stroke::new(1.0, Color32::YELLOW),
                        );
                    }
                    if let Some(preview) = snap_preview {
                        let screen = rect.min + preview.to_vec2() * self.image_zoom;
                        painter.circle_stroke(
                            screen,
                            (point_radius + 4.0).max(6.0),
                            egui::Stroke::new(1.0, Color32::YELLOW),
                        );
                        painter.circle_filled(screen, 3.0, Color32::YELLOW);
                    }
                }

                // Draw interpolation preview: connect points sorted by X numeric
                let mut valid: Vec<(f64, Pos2)> = Vec::with_capacity(self.points.len());
                for p in &self.points {
                    if let Some(xn) = p.x_numeric {
                        valid.push((xn, p.pixel));
                    }
                }
                valid.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
                if valid.len() >= 2 {
                    let stroke_curve = self.config.curve_line.stroke();
                    for win in valid.windows(2) {
                        let a = rect.min + win[0].1.to_vec2() * self.image_zoom;
                        let b = rect.min + win[1].1.to_vec2() * self.image_zoom;
                        painter.line_segment([a, b], stroke_curve);
                    }
                }

                // Hover crosshair
                if let Some(pos) = response.hover_pos() {
                    let crosshair_color = self.config.crosshair.color32();
                    let stroke = egui::Stroke::new(1.0, crosshair_color);
                    painter.line_segment(
                        [pos2(rect.left(), pos.y), pos2(rect.right(), pos.y)],
                        stroke,
                    );
                    painter.line_segment(
                        [pos2(pos.x, rect.top()), pos2(pos.x, rect.bottom())],
                        stroke,
                    );

                    let pixel = to_pixel(pos);
                    let font = egui::FontId::proportional(12.0);
                    let text_color = Color32::BLACK;
                    let bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
                    let padding = Vec2::new(4.0, 2.0);

                    let clip = painter.clip_rect();

                    if let Some(xmap) = x_mapping.as_ref()
                        && let Some(value) = xmap.value_at(pixel)
                    {
                        let text = format_overlay_value(&value);
                        let galley = painter.layout_no_wrap(text, font.clone(), text_color);
                        let size = galley.size();
                        let total = size + padding * 2.0;
                        let mut label_pos = pos2(pos.x - total.x * 0.5, clip.top() + 4.0);
                        let min_x = clip.left() + 2.0;
                        let max_x = clip.right() - total.x - 2.0;
                        label_pos.x = if max_x < min_x {
                            min_x
                        } else {
                            label_pos.x.clamp(min_x, max_x)
                        };
                        label_pos.y = clip.top() + 4.0;
                        let bg_rect = egui::Rect::from_min_size(label_pos, total);
                        painter.rect_filled(bg_rect, 3.0, bg_color);
                        painter.galley(label_pos + padding, galley, text_color);
                    }
                    if let Some(ymap) = y_mapping.as_ref()
                        && let Some(value) = ymap.value_at(pixel)
                    {
                        let text = format_overlay_value(&value);
                        let galley = painter.layout_no_wrap(text, font.clone(), text_color);
                        let size = galley.size();
                        let total = size + padding * 2.0;
                        let mut label_pos = pos2(clip.left() + 4.0, pos.y - total.y * 0.5);
                        let min_y = clip.top() + 2.0;
                        let max_y = clip.bottom() - total.y - 2.0;
                        label_pos.x = clip.left() + 4.0;
                        label_pos.y = if max_y < min_y {
                            min_y
                        } else {
                            label_pos.y.clamp(min_y, max_y)
                        };
                        let bg_rect = egui::Rect::from_min_size(label_pos, total);
                        painter.rect_filled(bg_rect, 3.0, bg_color);
                        painter.galley(label_pos + padding, galley, text_color);
                    }

                    if let Some(icon) = if delete_down {
                        Some("🗑")
                    } else if shift_pressed {
                        Some("✋")
                    } else {
                        None
                    } {
                        let icon_font = egui::FontId::proportional(24.0);
                        let icon_galley =
                            painter.layout_no_wrap(icon.to_string(), icon_font, Color32::WHITE);
                        let icon_size = icon_galley.size();
                        let backdrop_offset = Vec2::new(22.0, -22.0);
                        let anchor = pos + backdrop_offset;
                        let radius = (icon_size.x.max(icon_size.y) * 0.6).max(14.0);
                        let icon_bg = Color32::from_rgba_unmultiplied(0, 0, 0, 160);
                        painter.circle_filled(anchor, radius, icon_bg);
                        let icon_pos =
                            pos2(anchor.x - icon_size.x * 0.5, anchor.y - icon_size.y * 0.5);
                        painter.galley(icon_pos, icon_galley, Color32::WHITE);
                    }
                }
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Drop an image here or use Open image…");
            });
        }
    }

    fn collect_numeric_points_in_order(&self) -> Vec<XYPoint> {
        self.points
            .iter()
            .filter_map(|p| match (p.x_numeric, p.y_numeric) {
                (Some(x), Some(y)) => Some(XYPoint { x, y }),
                _ => None,
            })
            .collect()
    }

    fn collect_numeric_points_sorted(&self) -> Vec<XYPoint> {
        let mut pts = self.collect_numeric_points_in_order();
        pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
        pts
    }

    fn build_interpolated_samples(&self) -> Vec<XYPoint> {
        let nums = self.collect_numeric_points_sorted();
        if nums.len() < 2 {
            return Vec::new();
        }
        interpolate_sorted(&nums, self.sample_count, self.interp_algorithm)
    }

    fn build_export_payload(&self) -> Result<ExportPayload, &'static str> {
        let x_unit = match self.cal_x.mapping() {
            Some(mapping) => mapping.unit,
            None => return Err("Complete both axis calibrations before export."),
        };
        let y_unit = match self.cal_y.mapping() {
            Some(mapping) => mapping.unit,
            None => return Err("Complete both axis calibrations before export."),
        };

        match self.export_kind {
            ExportKind::Interpolated => {
                let data = self.build_interpolated_samples();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        extra_columns: Vec::new(),
                    })
                }
            }
            ExportKind::RawPoints => {
                let data = self.collect_numeric_points_in_order();
                if data.is_empty() {
                    Err("Nothing to export. Add data points first.")
                } else {
                    let extras = self.build_raw_extra_columns(&data);
                    Ok(ExportPayload {
                        points: data,
                        x_unit,
                        y_unit,
                        extra_columns: extras,
                    })
                }
            }
        }
    }

    fn build_raw_extra_columns(&self, raw_points: &[XYPoint]) -> Vec<ExportExtraColumn> {
        let mut extras = Vec::new();
        if self.raw_include_distances {
            extras.push(ExportExtraColumn::new(
                "distance",
                Self::sequential_distances(raw_points),
            ));
        }
        if self.raw_include_angles {
            extras.push(ExportExtraColumn::new(
                "angle_deg",
                Self::turning_angles(raw_points),
            ));
        }
        extras
    }

    fn sequential_distances(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
        let len = raw_points.len();
        let mut values = vec![None; len];
        for i in 1..len {
            let prev = &raw_points[i - 1];
            let curr = &raw_points[i];
            let dx = curr.x - prev.x;
            let dy = curr.y - prev.y;
            values[i] = Some((dx * dx + dy * dy).sqrt());
        }
        values
    }

    fn turning_angles(raw_points: &[XYPoint]) -> Vec<Option<f64>> {
        let len = raw_points.len();
        let mut values = vec![None; len];
        if len < 3 {
            return values;
        }
        for i in 1..(len - 1) {
            let prev = &raw_points[i - 1];
            let curr = &raw_points[i];
            let next = &raw_points[i + 1];
            let v1 = (curr.x - prev.x, curr.y - prev.y);
            let v2 = (next.x - curr.x, next.y - curr.y);
            let mag1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt();
            let mag2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt();
            if mag1 <= f64::EPSILON || mag2 <= f64::EPSILON {
                continue;
            }
            let dot = v1.0 * v2.0 + v1.1 * v2.1;
            let cos_theta = (dot / (mag1 * mag2)).clamp(-1.0, 1.0);
            values[i] = Some(cos_theta.acos().to_degrees());
        }
        values
    }
}

impl eframe::App for CurcatApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
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
            // Ctrl/Cmd + Shift + C: export CSV
            if self.active_dialog.is_none()
                && ctx.input(|i| i.key_pressed(Key::C) && i.modifiers.command && i.modifiers.shift)
            {
                self.start_export_csv();
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
            // Ctrl/Cmd + Z: undo
            if ctx.input(|i| i.key_pressed(Key::Z) && i.modifiers.command) {
                self.undo_last_point();
            }
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| self.ui_top(ui));
        egui::SidePanel::right("side")
            .resizable(true)
            .default_width(280.0)
            .show_animated(ctx, self.side_open, |ui| self.ui_side_calibration(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.ui_central_image(ctx, ui));

        let mut close_dialog = false;

        if let Some(dialog_state) = self.active_dialog.as_mut() {
            match dialog_state {
                NativeDialog::Open(dialog) => {
                    dialog.update(ctx);
                    if let Some(path) = dialog.take_picked() {
                        self.handle_open_path(ctx, &path);
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
            }
        }

        if close_dialog {
            self.active_dialog = None;
        }
    }
}

fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(
        ui.spacing().interact_size.y * 1.8,
        ui.spacing().interact_size.y,
    );
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, *on);
        let radius = rect.height() / 2.0;
        ui.painter().rect(
            rect,
            CornerRadius::same(rounded_u8(radius)),
            visuals.bg_fill,
            visuals.bg_stroke,
            StrokeKind::Middle,
        );

        let knob_radius = radius - 2.0;
        let knob_x = lerp(
            (rect.left() + knob_radius + 2.0)..=(rect.right() - knob_radius - 2.0),
            if *on { 1.0 } else { 0.0 },
        );
        let knob_center = pos2(knob_x, rect.center().y);
        ui.painter()
            .circle_filled(knob_center, knob_radius, visuals.fg_stroke.color);
    }

    response
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragTarget {
    CurvePoint(usize),
    CalX1,
    CalX2,
    CalY1,
    CalY2,
}
