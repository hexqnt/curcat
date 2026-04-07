use super::super::{
    AutoPlaceState, AxisValueField, CalIntSnapSticky, CalSnapEndpoint, CalSnapGuide, CurcatApp,
    DragTarget, PickMode, PointInputMode, PrimaryPressInfo, safe_usize_to_f32,
};
use super::icons;

use crate::i18n::TextKey;
use crate::types::{AxisMapping, AxisValue, CoordSystem, PolarMapping};
use egui::{Color32, CornerRadius, Key, PointerButton, Pos2, Sense, Vec2, pos2};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LIGHT_DRAG_CLICK_DIST: f32 = 20.0;
const LIGHT_DRAG_CLICK_MAX_DURATION: Duration = Duration::from_millis(400);

fn is_soft_primary_click(
    press: &PrimaryPressInfo,
    release_pos: Option<Pos2>,
    rect: egui::Rect,
    response_hovered: bool,
) -> bool {
    if !response_hovered || press.shift_down || !press.in_rect {
        return false;
    }
    let Some(release_pos) = release_pos else {
        return false;
    };
    if !rect.contains(release_pos) {
        return false;
    }
    let dist = press.pos.distance(release_pos);
    let elapsed = press.time.elapsed();
    dist <= LIGHT_DRAG_CLICK_DIST && elapsed <= LIGHT_DRAG_CLICK_MAX_DURATION
}

fn format_overlay_value(value: &AxisValue) -> String {
    match value {
        AxisValue::Float(v) => format!("{v:.3}"),
        AxisValue::DateTime(_) => value.format(),
    }
}

fn line_drag_hit_distance(pointer: Pos2, start: Pos2, end: Pos2) -> Option<f32> {
    let segment = end - start;
    let segment_len_sq = segment.length_sq();
    if segment_len_sq <= f32::EPSILON {
        return None;
    }
    let segment_len = segment_len_sq.sqrt();
    let end_gap = super::super::CAL_LINE_DRAG_END_GAP;
    if segment_len <= 2.0 * end_gap {
        return None;
    }

    let t = (pointer - start).dot(segment) / segment_len_sq;
    let along = t * segment_len;
    if along < end_gap || along > segment_len - end_gap {
        return None;
    }

    let closest = start + segment * t;
    let dist = pointer.distance(closest);
    (dist <= super::super::CAL_LINE_DRAG_HIT_RADIUS).then_some(dist)
}

fn clamp_line_drag_delta(delta: Vec2, p1: Pos2, p2: Pos2, image_size: Vec2) -> Vec2 {
    let min_delta_x = -p1.x.min(p2.x);
    let max_delta_x = image_size.x - p1.x.max(p2.x);
    let min_delta_y = -p1.y.min(p2.y);
    let max_delta_y = image_size.y - p1.y.max(p2.y);
    Vec2::new(
        delta.x.clamp(min_delta_x, max_delta_x),
        delta.y.clamp(min_delta_y, max_delta_y),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalibrationSnapKind {
    None,
    End,
    Intersection,
    Extension,
    VerticalHorizontal,
    Angle15,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SnapCriterion {
    End,
    Intersection,
    Extension,
    VerticalHorizontal,
}

#[derive(Clone, Copy, Default)]
struct SnapCriteriaHits(u8);

impl SnapCriteriaHits {
    const END: u8 = 1 << 0;
    const INTERSECTION: u8 = 1 << 1;
    const EXTENSION: u8 = 1 << 2;
    const VERTICAL_HORIZONTAL: u8 = 1 << 3;

    const fn bit(criterion: SnapCriterion) -> u8 {
        match criterion {
            SnapCriterion::End => Self::END,
            SnapCriterion::Intersection => Self::INTERSECTION,
            SnapCriterion::Extension => Self::EXTENSION,
            SnapCriterion::VerticalHorizontal => Self::VERTICAL_HORIZONTAL,
        }
    }

    const fn mark(&mut self, criterion: SnapCriterion) {
        self.0 |= Self::bit(criterion);
    }

    const fn has_other_than(self, criterion: SnapCriterion) -> bool {
        self.0 & !Self::bit(criterion) != 0
    }
}

#[derive(Clone, Copy)]
struct CalibrationSnapResult {
    snapped_pixel: Pos2,
    snap_kind: CalibrationSnapKind,
    guides: [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS],
    sticky_armed: bool,
}

#[derive(Clone, Copy)]
struct CalibrationSnapCandidate {
    snapped_pixel: Pos2,
    snap_kind: CalibrationSnapKind,
    criterion: SnapCriterion,
    distance_screen_sq: f32,
    priority: u8,
    guides: [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CartesianEndpointId {
    X1,
    X2,
    Y1,
    Y2,
}

impl CartesianEndpointId {
    const fn to_snap_endpoint(self) -> CalSnapEndpoint {
        match self {
            Self::X1 => CalSnapEndpoint::X1,
            Self::X2 => CalSnapEndpoint::X2,
            Self::Y1 => CalSnapEndpoint::Y1,
            Self::Y2 => CalSnapEndpoint::Y2,
        }
    }
}

#[derive(Clone, Copy)]
struct CartesianEndpoint {
    id: CartesianEndpointId,
    pixel: Pos2,
}

#[derive(Clone, Copy)]
struct LineIntersection {
    point: Pos2,
    lhs_t: f32,
    rhs_t: f32,
}

#[derive(Clone, Copy)]
struct EndpointSnapContext {
    target: CartesianEndpointId,
    pointer: Pos2,
    image_size: Vec2,
    zoom_sq: f32,
    end_radius_sq: f32,
    int_radius_sq: f32,
    ext_radius_sq: f32,
    vh_radius_sq: f32,
    sticky_int_radius_sq: f32,
    endpoints: [Option<CartesianEndpoint>; 4],
    x_line: Option<(Pos2, Pos2)>,
    y_line: Option<(Pos2, Pos2)>,
    intersection: Option<(LineIntersection, Pos2, Pos2, Pos2, Pos2)>,
}

const fn empty_calibration_guides() -> [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS] {
    [None; super::super::CAL_SNAP_GUIDE_SLOTS]
}

fn push_calibration_guide(
    guides: &mut [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS],
    segment: CalSnapGuide,
) {
    for slot in guides {
        if slot.is_none() {
            *slot = Some(segment);
            break;
        }
    }
}

fn try_push_calibration_guide(
    guides: &mut [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS],
    start: Pos2,
    end: Pos2,
) {
    if start.distance(end) <= f32::EPSILON {
        return;
    }
    push_calibration_guide(guides, CalSnapGuide { start, end });
}

fn point_in_image_bounds(point: Pos2, image_size: Vec2) -> bool {
    point.x >= 0.0 && point.x <= image_size.x && point.y >= 0.0 && point.y <= image_size.y
}

fn point_matches_any_endpoint(
    point: Pos2,
    endpoints: &[Option<CartesianEndpoint>; 4],
    zoom_sq: f32,
) -> bool {
    let duplicate_eps_sq = super::super::CAL_ENDPOINT_DUPLICATE_EPS_SCREEN
        * super::super::CAL_ENDPOINT_DUPLICATE_EPS_SCREEN;
    endpoints
        .iter()
        .flatten()
        .any(|endpoint| endpoint.pixel.distance_sq(point) * zoom_sq <= duplicate_eps_sq)
}

fn screen_distance_sq_if_within(
    a: Pos2,
    b: Pos2,
    zoom_sq: f32,
    snap_radius_sq: f32,
) -> Option<f32> {
    let distance_screen_sq = a.distance_sq(b) * zoom_sq;
    (distance_screen_sq <= snap_radius_sq).then_some(distance_screen_sq)
}

fn projection_to_line(point: Pos2, start: Pos2, end: Pos2) -> Option<(Pos2, f32)> {
    let dir = end - start;
    let denom = dir.length_sq();
    if denom <= f32::EPSILON {
        return None;
    }
    let t = (point - start).dot(dir) / denom;
    Some((start + dir * t, t))
}

fn normalized_direction(start: Pos2, end: Pos2) -> Option<Vec2> {
    let dir = end - start;
    let len = dir.length();
    if len <= f32::EPSILON {
        return None;
    }
    Some(dir / len)
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x.mul_add(b.y, -a.y * b.x)
}

fn line_intersection(
    lhs_start: Pos2,
    lhs_end: Pos2,
    rhs_start: Pos2,
    rhs_end: Pos2,
) -> Option<LineIntersection> {
    let lhs = lhs_end - lhs_start;
    let rhs = rhs_end - rhs_start;
    let denom = cross(lhs, rhs);
    if denom.abs() <= f32::EPSILON {
        return None;
    }
    let delta = rhs_start - lhs_start;
    let lhs_t = cross(delta, rhs) / denom;
    let rhs_t = cross(delta, lhs) / denom;
    Some(LineIntersection {
        point: lhs_start + lhs * lhs_t,
        lhs_t,
        rhs_t,
    })
}

fn build_intersection_guides(
    x1: Pos2,
    x2: Pos2,
    y1: Pos2,
    y2: Pos2,
    intersection: LineIntersection,
) -> [Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS] {
    let mut guides = empty_calibration_guides();
    if intersection.lhs_t < 0.0 || intersection.lhs_t > 1.0 {
        let start = if intersection.lhs_t < 0.0 { x1 } else { x2 };
        push_calibration_guide(
            &mut guides,
            CalSnapGuide {
                start,
                end: intersection.point,
            },
        );
    }
    if intersection.rhs_t < 0.0 || intersection.rhs_t > 1.0 {
        let start = if intersection.rhs_t < 0.0 { y1 } else { y2 };
        push_calibration_guide(
            &mut guides,
            CalSnapGuide {
                start,
                end: intersection.point,
            },
        );
    }
    guides
}

fn update_best_calibration_candidate(
    best: &mut Option<CalibrationSnapCandidate>,
    candidate: CalibrationSnapCandidate,
) {
    const GUIDE_MERGE_DIST_EPS_SQ: f32 = 0.25;
    const GUIDE_MERGE_POINT_EPS_SQ: f32 = 0.25;

    fn has_guides(guides: &[Option<CalSnapGuide>; super::super::CAL_SNAP_GUIDE_SLOTS]) -> bool {
        guides.iter().any(Option::is_some)
    }

    let tie_eps_sq = super::super::CAL_SNAP_TIE_EPS_SCREEN * super::super::CAL_SNAP_TIE_EPS_SCREEN;

    if let Some(current) = best {
        let replace = candidate.distance_screen_sq + tie_eps_sq < current.distance_screen_sq
            || ((candidate.distance_screen_sq - current.distance_screen_sq).abs() <= tie_eps_sq
                && candidate.priority < current.priority);
        if replace {
            *best = Some(candidate);
            return;
        }

        let same_snap_point =
            candidate.snapped_pixel.distance_sq(current.snapped_pixel) <= GUIDE_MERGE_POINT_EPS_SQ;
        let same_pointer_distance = (candidate.distance_screen_sq - current.distance_screen_sq)
            .abs()
            <= GUIDE_MERGE_DIST_EPS_SQ;
        if same_snap_point
            && same_pointer_distance
            && !has_guides(&current.guides)
            && has_guides(&candidate.guides)
        {
            let mut merged = *current;
            merged.guides = candidate.guides;
            *best = Some(merged);
        }
    } else {
        *best = Some(candidate);
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
struct PointerState {
    shift_pressed: bool,
    primary_down: bool,
    primary_pressed: bool,
    primary_released: bool,
    delete_down: bool,
    ctrl_pressed: bool,
    press_origin: Option<Pos2>,
    latest_pos: Option<Pos2>,
}

impl PointerState {
    fn read(ctx: &egui::Context) -> Self {
        ctx.input(|i| Self {
            shift_pressed: i.modifiers.shift,
            primary_down: i.pointer.button_down(PointerButton::Primary),
            primary_pressed: i.pointer.button_pressed(PointerButton::Primary),
            primary_released: i.pointer.button_released(PointerButton::Primary),
            delete_down: i.key_down(Key::Delete),
            ctrl_pressed: i.modifiers.ctrl,
            press_origin: i.pointer.press_origin(),
            latest_pos: i.pointer.latest_pos(),
        })
    }
}

#[derive(Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
struct PrimaryImageGesture {
    down: bool,
    pressed: bool,
    started_in_image: bool,
    pointer_over_image: bool,
    clicked: bool,
    click_pos: Option<Pos2>,
}

#[derive(Clone, Copy)]
enum CalTarget {
    X1,
    X2,
    Y1,
    Y2,
    Origin,
    R1,
    R2,
    A1,
    A2,
}

impl CalTarget {
    const fn label(self) -> &'static str {
        match self {
            Self::X1 => "X1",
            Self::X2 => "X2",
            Self::Y1 => "Y1",
            Self::Y2 => "Y2",
            Self::Origin => "Origin",
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::A1 => "A1",
            Self::A2 => "A2",
        }
    }

    const fn value_field(self) -> Option<AxisValueField> {
        match self {
            Self::X1 => Some(AxisValueField::X1),
            Self::X2 => Some(AxisValueField::X2),
            Self::Y1 => Some(AxisValueField::Y1),
            Self::Y2 => Some(AxisValueField::Y2),
            Self::R1 => Some(AxisValueField::R1),
            Self::R2 => Some(AxisValueField::R2),
            Self::A1 => Some(AxisValueField::A1),
            Self::A2 => Some(AxisValueField::A2),
            Self::Origin => None,
        }
    }

    const fn from_drag(target: DragTarget) -> Option<Self> {
        match target {
            DragTarget::CalX1 => Some(Self::X1),
            DragTarget::CalX2 => Some(Self::X2),
            DragTarget::CalY1 => Some(Self::Y1),
            DragTarget::CalY2 => Some(Self::Y2),
            DragTarget::PolarOrigin => Some(Self::Origin),
            DragTarget::PolarR1 => Some(Self::R1),
            DragTarget::PolarR2 => Some(Self::R2),
            DragTarget::PolarA1 => Some(Self::A1),
            DragTarget::PolarA2 => Some(Self::A2),
            DragTarget::CalXLine | DragTarget::CalYLine | DragTarget::CurvePoint(_) => None,
        }
    }

    const fn from_pick_mode(mode: PickMode) -> Option<Self> {
        match mode {
            PickMode::X1 => Some(Self::X1),
            PickMode::X2 => Some(Self::X2),
            PickMode::Y1 => Some(Self::Y1),
            PickMode::Y2 => Some(Self::Y2),
            PickMode::Origin => Some(Self::Origin),
            PickMode::R1 => Some(Self::R1),
            PickMode::R2 => Some(Self::R2),
            PickMode::A1 => Some(Self::A1),
            PickMode::A2 => Some(Self::A2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalUpdateMode {
    Drag,
    Pick,
}

struct CalOverlayStyle {
    outline: egui::Stroke,
    stroke: egui::Stroke,
    point_outer_radius: f32,
    point_inner_radius: f32,
    label_font: egui::FontId,
    label_shadow: Color32,
}

impl CurcatApp {
    pub(crate) fn handle_middle_pan(&mut self, response: &egui::Response, ui: &egui::Ui) {
        // When the MMB pan toggle is off, treat middle drag like direct touch pan.
        let touch_style = !self.interaction.middle_pan_enabled;
        let factor = if touch_style {
            1.0
        } else {
            self.config.pan_speed_factor()
        };

        if response.drag_started_by(PointerButton::Middle)
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.image.touch_pan_active = true;
            self.image.touch_pan_last = Some(pos);
        }

        if self.image.touch_pan_active {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(last) = self.image.touch_pan_last {
                    let delta = (pos - last) * factor;
                    if delta.length_sq() > 0.0 {
                        let scroll_delta = if touch_style { delta } else { -delta };
                        ui.scroll_with_delta_animation(
                            scroll_delta,
                            egui::style::ScrollAnimation::none(),
                        );
                    }
                }
                self.image.touch_pan_last = Some(pos);
            }

            let middle_down = ui
                .ctx()
                .input(|i| i.pointer.button_down(PointerButton::Middle));
            if !middle_down {
                self.image.touch_pan_active = false;
                self.image.touch_pan_last = None;
            }
        } else if touch_style {
            self.image.touch_pan_last = None;
        }
    }

    fn handle_drag_and_drop(&mut self, ui: &egui::Ui) {
        enum DropAction {
            None,
            LoadPath(PathBuf),
            LoadBytes {
                name: Option<String>,
                bytes: Vec<u8>,
                last_modified: Option<std::time::SystemTime>,
            },
            FailNoReadable,
        }

        let action = ui.input(|i| {
            if (!i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty())
                && cfg!(debug_assertions)
            {
                eprintln!(
                    "[DnD] hovered={} dropped={}",
                    i.raw.hovered_files.len(),
                    i.raw.dropped_files.len()
                );
                for (idx, h) in i.raw.hovered_files.iter().enumerate() {
                    eprintln!("[DnD] hover[{idx}] path={:?} mime={}", h.path, h.mime);
                }
                for (idx, f) in i.raw.dropped_files.iter().enumerate() {
                    let blen = f.bytes.as_ref().map_or(0, |b| b.len());
                    eprintln!(
                        "[DnD] drop[{idx}] name='{}' mime={} path={:?} bytes={} last_modified={:?}",
                        f.name, f.mime, f.path, blen, f.last_modified
                    );
                }
            }

            if i.raw.dropped_files.is_empty() {
                return DropAction::None;
            }

            for f in &i.raw.dropped_files {
                if let Some(path) = f.path.as_ref() {
                    return DropAction::LoadPath(path.clone());
                }
                if let Some(bytes) = f.bytes.as_ref() {
                    return DropAction::LoadBytes {
                        name: (!f.name.is_empty()).then(|| f.name.clone()),
                        bytes: bytes.to_vec(),
                        last_modified: f.last_modified,
                    };
                }
            }

            DropAction::FailNoReadable
        });

        match action {
            DropAction::None => {}
            DropAction::LoadPath(path) => {
                if cfg!(debug_assertions) {
                    eprintln!("[DnD] Loading from path: {}", path.display());
                }
                self.start_loading_image_from_path(path);
            }
            DropAction::LoadBytes {
                name,
                bytes,
                last_modified,
            } => {
                if cfg!(debug_assertions) {
                    let debug_name = name.as_deref().unwrap_or("<unnamed>");
                    eprintln!("[DnD] Loading from dropped bytes: name='{debug_name}'");
                }
                self.start_loading_image_from_bytes(name, bytes, last_modified);
            }
            DropAction::FailNoReadable => {
                self.set_status_error(match self.ui.language {
                    crate::i18n::UiLanguage::En => "Drop failed: no readable bytes/path",
                    crate::i18n::UiLanguage::Ru => {
                        "Не удалось обработать перетаскивание: нет читаемого пути/байтов"
                    }
                });
                if cfg!(debug_assertions) {
                    eprintln!("[DnD] Drop failed: no readable bytes/path");
                }
            }
        }
    }

    #[allow(clippy::unused_self)]
    fn add_centered_image(
        &self,
        ui: &mut egui::Ui,
        image: egui::Image,
        display_size: Vec2,
    ) -> egui::Response {
        let viewport = ui.available_size();
        let pad = Self::center_padding(viewport, display_size);
        if pad.x > 0.0 || pad.y > 0.0 {
            ui.vertical(|ui| {
                if pad.y > 0.0 {
                    ui.add_space(pad.y);
                }
                let response = ui
                    .horizontal(|ui| {
                        if pad.x > 0.0 {
                            ui.add_space(pad.x);
                        }
                        let response = ui.add(image.sense(Sense::click_and_drag()));
                        if pad.x > 0.0 {
                            ui.add_space(pad.x);
                        }
                        response
                    })
                    .inner;
                if pad.y > 0.0 {
                    ui.add_space(pad.y);
                }
                response
            })
            .inner
        } else {
            ui.add(image.sense(Sense::click_and_drag()))
        }
    }

    fn compute_scroll_zoom(
        &self,
        response: &egui::Response,
        ui: &egui::Ui,
    ) -> Option<(f32, Option<Pos2>)> {
        if !response.hovered() {
            return None;
        }
        let mut scroll = 0.0_f32;
        let mut hover_pos: Option<Pos2> = None;
        ui.ctx().input(|i| {
            scroll = i.smooth_scroll_delta().y;
            hover_pos = i.pointer.latest_pos();
        });
        if scroll.abs() <= f32::EPSILON {
            return None;
        }
        // В egui 0.34 wheel delta чаще приходит дробными порциями.
        // Квантизация через round() гасит эти значения в ноль, поэтому используем непрерывный шаг.
        let steps = scroll / super::super::WHEEL_ZOOM_STEP_POINTS;
        if steps.abs() < 0.01 {
            return None;
        }
        let factor = 1.1_f32.powf(steps);
        Some((self.image.zoom * factor, response.hover_pos().or(hover_pos)))
    }

    fn resolve_primary_gesture(
        &mut self,
        response: &egui::Response,
        rect: egui::Rect,
        pointer: PointerState,
        pointer_pos: Option<Pos2>,
        hover_pos: Option<Pos2>,
    ) -> PrimaryImageGesture {
        let mut soft_primary_click = false;
        let pointer_over_image = response.contains_pointer();
        let mut started_in_image = self
            .interaction
            .primary_press
            .as_ref()
            .is_some_and(|press| press.in_rect);

        if pointer.primary_pressed {
            if let Some(pos) = pointer.press_origin.or(pointer.latest_pos) {
                let press_in_image = rect.contains(pos);
                self.interaction.primary_press = Some(PrimaryPressInfo {
                    pos,
                    time: Instant::now(),
                    in_rect: press_in_image,
                    shift_down: pointer.shift_pressed,
                });
                started_in_image = press_in_image;
            } else {
                self.interaction.primary_press = None;
                started_in_image = false;
            }
        }
        if pointer.primary_released {
            if let Some(info) = self.interaction.primary_press.take() {
                started_in_image = info.in_rect;
                if is_soft_primary_click(&info, pointer.latest_pos, rect, pointer_over_image) {
                    soft_primary_click = true;
                }
            }
        } else if !pointer.primary_down {
            self.interaction.primary_press = None;
            started_in_image = false;
        }

        let response_clicked = response.clicked_by(PointerButton::Primary);
        if response_clicked {
            started_in_image = true;
        }
        let primary_clicked = response_clicked || soft_primary_click;
        let click_pos = if response_clicked {
            pointer_pos.or(hover_pos)
        } else if soft_primary_click {
            pointer.latest_pos.or(pointer_pos)
        } else {
            None
        };
        let click_pos = Self::pos_in_rect(click_pos, rect);
        let clicked = primary_clicked && click_pos.is_some() && started_in_image;

        PrimaryImageGesture {
            down: pointer.primary_down,
            pressed: pointer.primary_pressed,
            started_in_image,
            pointer_over_image,
            clicked,
            click_pos,
        }
    }

    fn compute_snap_preview(&mut self, pointer_pixel: Option<Pos2>) -> Option<Pos2> {
        if !matches!(self.snap.point_input_mode, PointInputMode::Free)
            && !matches!(self.calibration.pick_mode, PickMode::CurveColor)
            && let Some(pixel) = pointer_pixel
        {
            self.compute_snap_candidate(pixel)
        } else {
            None
        }
    }

    const fn cartesian_endpoint_target_id(target: CalTarget) -> Option<CartesianEndpointId> {
        match target {
            CalTarget::X1 => Some(CartesianEndpointId::X1),
            CalTarget::X2 => Some(CartesianEndpointId::X2),
            CalTarget::Y1 => Some(CartesianEndpointId::Y1),
            CalTarget::Y2 => Some(CartesianEndpointId::Y2),
            CalTarget::Origin | CalTarget::R1 | CalTarget::R2 | CalTarget::A1 | CalTarget::A2 => {
                None
            }
        }
    }

    fn pos_in_rect(pos: Option<Pos2>, rect: egui::Rect) -> Option<Pos2> {
        pos.filter(|candidate| rect.contains(*candidate))
    }

    fn cartesian_calibration_endpoints(&self) -> [Option<CartesianEndpoint>; 4] {
        [
            self.calibration.cal_x.p1.map(|pixel| CartesianEndpoint {
                id: CartesianEndpointId::X1,
                pixel,
            }),
            self.calibration.cal_x.p2.map(|pixel| CartesianEndpoint {
                id: CartesianEndpointId::X2,
                pixel,
            }),
            self.calibration.cal_y.p1.map(|pixel| CartesianEndpoint {
                id: CartesianEndpointId::Y1,
                pixel,
            }),
            self.calibration.cal_y.p2.map(|pixel| CartesianEndpoint {
                id: CartesianEndpointId::Y2,
                pixel,
            }),
        ]
    }

    fn endpoint_snap_context(
        &self,
        target: CartesianEndpointId,
        pointer: Pos2,
        image_size: Vec2,
    ) -> EndpointSnapContext {
        let endpoints = self.cartesian_calibration_endpoints();
        let x_line = self.calibration.cal_x.p1.zip(self.calibration.cal_x.p2);
        let y_line = self.calibration.cal_y.p1.zip(self.calibration.cal_y.p2);
        let intersection = if let (Some((x1, x2)), Some((y1, y2))) = (x_line, y_line) {
            line_intersection(x1, x2, y1, y2)
                .filter(|value| point_in_image_bounds(value.point, image_size))
                .map(|value| (value, x1, x2, y1, y2))
        } else {
            None
        };
        let zoom_sq = self.image.zoom * self.image.zoom;
        let end_radius_sq =
            super::super::CAL_ENDPOINT_SNAP_RADIUS_END * super::super::CAL_ENDPOINT_SNAP_RADIUS_END;
        let int_radius_sq =
            super::super::CAL_ENDPOINT_SNAP_RADIUS_INT * super::super::CAL_ENDPOINT_SNAP_RADIUS_INT;
        let ext_radius_sq =
            super::super::CAL_ENDPOINT_SNAP_RADIUS_EXT * super::super::CAL_ENDPOINT_SNAP_RADIUS_EXT;
        let vh_radius_sq =
            super::super::CAL_ENDPOINT_SNAP_RADIUS_VH * super::super::CAL_ENDPOINT_SNAP_RADIUS_VH;
        let sticky_int_radius =
            super::super::CAL_ENDPOINT_SNAP_RADIUS_INT * super::super::CAL_INT_SNAP_STICKY_FACTOR;
        EndpointSnapContext {
            target,
            pointer,
            image_size,
            zoom_sq,
            end_radius_sq,
            int_radius_sq,
            ext_radius_sq,
            vh_radius_sq,
            sticky_int_radius_sq: sticky_int_radius * sticky_int_radius,
            endpoints,
            x_line,
            y_line,
            intersection,
        }
    }

    fn add_snap_candidate(
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
        candidate: CalibrationSnapCandidate,
    ) {
        criteria_hits.mark(candidate.criterion);
        update_best_calibration_candidate(best, candidate);
    }

    fn collect_end_snap_candidates(
        ctx: &EndpointSnapContext,
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        for endpoint in ctx.endpoints.iter().flatten() {
            if endpoint.id == ctx.target {
                continue;
            }
            if let Some(distance_screen_sq) = screen_distance_sq_if_within(
                ctx.pointer,
                endpoint.pixel,
                ctx.zoom_sq,
                ctx.end_radius_sq,
            ) {
                Self::add_snap_candidate(
                    best,
                    criteria_hits,
                    CalibrationSnapCandidate {
                        snapped_pixel: endpoint.pixel,
                        snap_kind: CalibrationSnapKind::End,
                        criterion: SnapCriterion::End,
                        distance_screen_sq,
                        priority: 0,
                        guides: empty_calibration_guides(),
                    },
                );
            }
        }
    }

    fn collect_intersection_snap_candidate(
        ctx: &EndpointSnapContext,
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        if let Some((intersection, x1, x2, y1, y2)) = ctx.intersection
            && let Some(distance_screen_sq) = screen_distance_sq_if_within(
                ctx.pointer,
                intersection.point,
                ctx.zoom_sq,
                ctx.int_radius_sq,
            )
        {
            Self::add_snap_candidate(
                best,
                criteria_hits,
                CalibrationSnapCandidate {
                    snapped_pixel: intersection.point,
                    snap_kind: CalibrationSnapKind::Intersection,
                    criterion: SnapCriterion::Intersection,
                    distance_screen_sq,
                    priority: 1,
                    guides: build_intersection_guides(x1, x2, y1, y2, intersection),
                },
            );
        }
    }

    fn collect_extension_line_projection_candidates(
        ctx: &EndpointSnapContext,
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        for (line_start, line_end) in [ctx.x_line, ctx.y_line].into_iter().flatten() {
            if let Some((projected, t)) = projection_to_line(ctx.pointer, line_start, line_end) {
                if (0.0..=1.0).contains(&t) || !point_in_image_bounds(projected, ctx.image_size) {
                    continue;
                }
                if let Some(distance_screen_sq) = screen_distance_sq_if_within(
                    ctx.pointer,
                    projected,
                    ctx.zoom_sq,
                    ctx.ext_radius_sq,
                ) {
                    let start = if t < 0.0 { line_start } else { line_end };
                    let mut guides = empty_calibration_guides();
                    push_calibration_guide(
                        &mut guides,
                        CalSnapGuide {
                            start,
                            end: projected,
                        },
                    );
                    Self::add_snap_candidate(
                        best,
                        criteria_hits,
                        CalibrationSnapCandidate {
                            snapped_pixel: projected,
                            snap_kind: CalibrationSnapKind::Extension,
                            criterion: SnapCriterion::Extension,
                            distance_screen_sq,
                            priority: 2,
                            guides,
                        },
                    );
                }
            }
        }
    }

    fn extension_dirs(ctx: &EndpointSnapContext) -> [Option<Vec2>; 2] {
        [
            ctx.y_line
                .and_then(|(start, end)| normalized_direction(start, end)),
            ctx.x_line
                .and_then(|(start, end)| normalized_direction(start, end)),
        ]
    }

    fn collect_extension_rotated_projection_candidates(
        ctx: &EndpointSnapContext,
        ext_dirs: [Option<Vec2>; 2],
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        // Поворотный аналог V/H: локальные направляющие по углам калибровочных осей.
        // Это остаётся частью EXT и использует те же пороги/приоритеты.
        for dir in ext_dirs.iter().flatten().copied() {
            for endpoint in ctx.endpoints.iter().flatten() {
                if endpoint.id == ctx.target {
                    continue;
                }
                let projected = endpoint.pixel + dir * (ctx.pointer - endpoint.pixel).dot(dir);
                if !point_in_image_bounds(projected, ctx.image_size)
                    || point_matches_any_endpoint(projected, &ctx.endpoints, ctx.zoom_sq)
                {
                    continue;
                }
                if let Some(distance_screen_sq) = screen_distance_sq_if_within(
                    ctx.pointer,
                    projected,
                    ctx.zoom_sq,
                    ctx.ext_radius_sq,
                ) {
                    let mut guides = empty_calibration_guides();
                    try_push_calibration_guide(&mut guides, endpoint.pixel, projected);
                    Self::add_snap_candidate(
                        best,
                        criteria_hits,
                        CalibrationSnapCandidate {
                            snapped_pixel: projected,
                            snap_kind: CalibrationSnapKind::Extension,
                            criterion: SnapCriterion::Extension,
                            distance_screen_sq,
                            priority: 2,
                            guides,
                        },
                    );
                }
            }
        }
    }

    fn collect_extension_rotated_intersection_candidates(
        ctx: &EndpointSnapContext,
        ext_dirs: [Option<Vec2>; 2],
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        if let [Some(first_dir), Some(second_dir)] = ext_dirs {
            for first_endpoint in ctx.endpoints.iter().flatten() {
                if first_endpoint.id == ctx.target {
                    continue;
                }
                for second_endpoint in ctx.endpoints.iter().flatten() {
                    if second_endpoint.id == ctx.target {
                        continue;
                    }
                    if second_endpoint.id == first_endpoint.id {
                        continue;
                    }
                    let Some(intersection) = line_intersection(
                        first_endpoint.pixel,
                        first_endpoint.pixel + first_dir,
                        second_endpoint.pixel,
                        second_endpoint.pixel + second_dir,
                    ) else {
                        continue;
                    };
                    if !point_in_image_bounds(intersection.point, ctx.image_size)
                        || point_matches_any_endpoint(
                            intersection.point,
                            &ctx.endpoints,
                            ctx.zoom_sq,
                        )
                    {
                        continue;
                    }
                    if let Some(distance_screen_sq) = screen_distance_sq_if_within(
                        ctx.pointer,
                        intersection.point,
                        ctx.zoom_sq,
                        ctx.ext_radius_sq,
                    ) {
                        let mut guides = empty_calibration_guides();
                        try_push_calibration_guide(
                            &mut guides,
                            first_endpoint.pixel,
                            intersection.point,
                        );
                        try_push_calibration_guide(
                            &mut guides,
                            second_endpoint.pixel,
                            intersection.point,
                        );
                        Self::add_snap_candidate(
                            best,
                            criteria_hits,
                            CalibrationSnapCandidate {
                                snapped_pixel: intersection.point,
                                snap_kind: CalibrationSnapKind::Extension,
                                criterion: SnapCriterion::Extension,
                                distance_screen_sq,
                                priority: 2,
                                guides,
                            },
                        );
                    }
                }
            }
        }
    }

    fn collect_extension_snap_candidates(
        ctx: &EndpointSnapContext,
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        Self::collect_extension_line_projection_candidates(ctx, best, criteria_hits);
        let ext_dirs = Self::extension_dirs(ctx);
        Self::collect_extension_rotated_projection_candidates(ctx, ext_dirs, best, criteria_hits);
        Self::collect_extension_rotated_intersection_candidates(ctx, ext_dirs, best, criteria_hits);
    }

    fn collect_vh_snap_candidates(
        ctx: &EndpointSnapContext,
        best: &mut Option<CalibrationSnapCandidate>,
        criteria_hits: &mut SnapCriteriaHits,
    ) {
        for endpoint_x in ctx.endpoints.iter().flatten() {
            if endpoint_x.id == ctx.target {
                continue;
            }
            let vertical = pos2(endpoint_x.pixel.x, ctx.pointer.y);
            if point_matches_any_endpoint(vertical, &ctx.endpoints, ctx.zoom_sq) {
                continue;
            }
            if let Some(distance_screen_sq) =
                screen_distance_sq_if_within(ctx.pointer, vertical, ctx.zoom_sq, ctx.vh_radius_sq)
            {
                let mut guides = empty_calibration_guides();
                try_push_calibration_guide(&mut guides, endpoint_x.pixel, vertical);
                Self::add_snap_candidate(
                    best,
                    criteria_hits,
                    CalibrationSnapCandidate {
                        snapped_pixel: vertical,
                        snap_kind: CalibrationSnapKind::VerticalHorizontal,
                        criterion: SnapCriterion::VerticalHorizontal,
                        distance_screen_sq,
                        priority: 3,
                        guides,
                    },
                );
            }
        }
        for endpoint_y in ctx.endpoints.iter().flatten() {
            if endpoint_y.id == ctx.target {
                continue;
            }
            let horizontal = pos2(ctx.pointer.x, endpoint_y.pixel.y);
            if point_matches_any_endpoint(horizontal, &ctx.endpoints, ctx.zoom_sq) {
                continue;
            }
            if let Some(distance_screen_sq) =
                screen_distance_sq_if_within(ctx.pointer, horizontal, ctx.zoom_sq, ctx.vh_radius_sq)
            {
                let mut guides = empty_calibration_guides();
                try_push_calibration_guide(&mut guides, endpoint_y.pixel, horizontal);
                Self::add_snap_candidate(
                    best,
                    criteria_hits,
                    CalibrationSnapCandidate {
                        snapped_pixel: horizontal,
                        snap_kind: CalibrationSnapKind::VerticalHorizontal,
                        criterion: SnapCriterion::VerticalHorizontal,
                        distance_screen_sq,
                        priority: 3,
                        guides,
                    },
                );
            }
        }
        for endpoint_x in ctx.endpoints.iter().flatten() {
            if endpoint_x.id == ctx.target {
                continue;
            }
            for endpoint_y in ctx.endpoints.iter().flatten() {
                if endpoint_y.id == ctx.target {
                    continue;
                }
                if endpoint_y.id == endpoint_x.id {
                    continue;
                }
                let both = pos2(endpoint_x.pixel.x, endpoint_y.pixel.y);
                if point_matches_any_endpoint(both, &ctx.endpoints, ctx.zoom_sq) {
                    continue;
                }
                if let Some(distance_screen_sq) =
                    screen_distance_sq_if_within(ctx.pointer, both, ctx.zoom_sq, ctx.vh_radius_sq)
                {
                    let mut guides = empty_calibration_guides();
                    try_push_calibration_guide(&mut guides, endpoint_x.pixel, both);
                    try_push_calibration_guide(&mut guides, endpoint_y.pixel, both);
                    Self::add_snap_candidate(
                        best,
                        criteria_hits,
                        CalibrationSnapCandidate {
                            snapped_pixel: both,
                            snap_kind: CalibrationSnapKind::VerticalHorizontal,
                            criterion: SnapCriterion::VerticalHorizontal,
                            distance_screen_sq,
                            priority: 3,
                            guides,
                        },
                    );
                }
            }
        }
    }

    fn try_sticky_intersection_snap(
        &mut self,
        ctx: &EndpointSnapContext,
        target_snap_endpoint: CalSnapEndpoint,
    ) -> Option<CalibrationSnapResult> {
        if !self.calibration.snap_int {
            self.calibration.int_snap_sticky = None;
            return None;
        }
        let sticky = self.calibration.int_snap_sticky?;
        if sticky.endpoint != target_snap_endpoint {
            self.calibration.int_snap_sticky = None;
            return None;
        }
        if screen_distance_sq_if_within(
            ctx.pointer,
            sticky.point,
            ctx.zoom_sq,
            ctx.sticky_int_radius_sq,
        )
        .is_none()
        {
            self.calibration.int_snap_sticky = None;
            return None;
        }
        if let Some((intersection, x1, x2, y1, y2)) = ctx.intersection {
            return Some(CalibrationSnapResult {
                snapped_pixel: intersection.point,
                snap_kind: CalibrationSnapKind::Intersection,
                guides: build_intersection_guides(x1, x2, y1, y2, intersection),
                sticky_armed: true,
            });
        }
        Some(CalibrationSnapResult {
            snapped_pixel: sticky.point,
            snap_kind: CalibrationSnapKind::Intersection,
            guides: empty_calibration_guides(),
            sticky_armed: true,
        })
    }

    fn endpoint_snap_reference(
        &mut self,
        target: CartesianEndpointId,
        pointer: Pos2,
        image_size: Vec2,
    ) -> Option<CalibrationSnapResult> {
        let ctx = self.endpoint_snap_context(target, pointer, image_size);
        let target_snap_endpoint = target.to_snap_endpoint();
        if let Some(result) = self.try_sticky_intersection_snap(&ctx, target_snap_endpoint) {
            return Some(result);
        }

        let mut best: Option<CalibrationSnapCandidate> = None;
        let mut criteria_hits = SnapCriteriaHits::default();
        if self.calibration.snap_end {
            Self::collect_end_snap_candidates(&ctx, &mut best, &mut criteria_hits);
        }
        if self.calibration.snap_int {
            Self::collect_intersection_snap_candidate(&ctx, &mut best, &mut criteria_hits);
        }
        if self.calibration.snap_ext {
            Self::collect_extension_snap_candidates(&ctx, &mut best, &mut criteria_hits);
        }
        if self.calibration.snap_vh {
            Self::collect_vh_snap_candidates(&ctx, &mut best, &mut criteria_hits);
        }
        best.map(|candidate| CalibrationSnapResult {
            snapped_pixel: candidate.snapped_pixel,
            snap_kind: candidate.snap_kind,
            guides: candidate.guides,
            sticky_armed: matches!(candidate.criterion, SnapCriterion::Intersection)
                && criteria_hits.has_other_than(SnapCriterion::Intersection),
        })
    }

    const fn snap_angle_anchor(&self, target: CalTarget) -> Option<Pos2> {
        match target {
            CalTarget::X1 => self.calibration.cal_x.p2,
            CalTarget::X2 => self.calibration.cal_x.p1,
            CalTarget::Y1 => self.calibration.cal_y.p2,
            CalTarget::Y2 => self.calibration.cal_y.p1,
            CalTarget::R1 | CalTarget::R2 | CalTarget::A1 | CalTarget::A2 => {
                self.calibration.polar_cal.origin
            }
            CalTarget::Origin => None,
        }
    }

    fn calibration_snap_result(
        &mut self,
        target: CalTarget,
        candidate: Pos2,
        image_size: Vec2,
    ) -> CalibrationSnapResult {
        if !self.calibration.snap_int {
            self.calibration.int_snap_sticky = None;
        }
        if matches!(target, CalTarget::Origin) {
            self.calibration.int_snap_sticky = None;
            return CalibrationSnapResult {
                snapped_pixel: candidate,
                snap_kind: CalibrationSnapKind::None,
                guides: empty_calibration_guides(),
                sticky_armed: false,
            };
        }

        if matches!(self.calibration.coord_system, CoordSystem::Cartesian)
            && let Some(endpoint_target) = Self::cartesian_endpoint_target_id(target)
            && let Some(result) =
                self.endpoint_snap_reference(endpoint_target, candidate, image_size)
        {
            if matches!(result.snap_kind, CalibrationSnapKind::Intersection) && result.sticky_armed
            {
                self.calibration.int_snap_sticky = Some(CalIntSnapSticky {
                    endpoint: endpoint_target.to_snap_endpoint(),
                    point: result.snapped_pixel,
                });
            } else {
                self.calibration.int_snap_sticky = None;
            }
            return result;
        }

        let snapped =
            self.snap_calibration_angle(candidate, self.snap_angle_anchor(target), image_size);
        let snap_kind = if snapped.distance(candidate) > f32::EPSILON {
            CalibrationSnapKind::Angle15
        } else {
            CalibrationSnapKind::None
        };
        self.calibration.int_snap_sticky = None;
        CalibrationSnapResult {
            snapped_pixel: snapped,
            snap_kind,
            guides: empty_calibration_guides(),
            sticky_armed: false,
        }
    }

    fn update_calibration_pick_preview_guides(
        &mut self,
        hover_pixel: Option<Pos2>,
        image_size: Vec2,
    ) {
        let Some(hover_pixel) = hover_pixel else {
            return;
        };
        let Some(target) = CalTarget::from_pick_mode(self.calibration.pick_mode) else {
            return;
        };
        if Self::cartesian_endpoint_target_id(target).is_none() {
            return;
        }
        let result = self.calibration_snap_result(target, hover_pixel, image_size);
        let has_visual_snap = !matches!(
            result.snap_kind,
            CalibrationSnapKind::None | CalibrationSnapKind::Angle15
        );
        if has_visual_snap && result.guides.iter().any(Option::is_some) {
            self.calibration.snap_guides = result.guides;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_calibration_point(
        &mut self,
        target: CalTarget,
        pixel: Pos2,
        base_size: Vec2,
        mode: CalUpdateMode,
        x_mapping: &mut Option<AxisMapping>,
        y_mapping: &mut Option<AxisMapping>,
        polar_mapping: &mut Option<PolarMapping>,
    ) {
        let pixel = if mode == CalUpdateMode::Pick {
            self.snap_pixel_if_requested(pixel)
        } else {
            pixel
        };
        let snap_result = self.calibration_snap_result(target, pixel, base_size);
        let snapped = snap_result.snapped_pixel;
        let has_visual_snap = !matches!(
            snap_result.snap_kind,
            CalibrationSnapKind::None | CalibrationSnapKind::Angle15
        );
        self.calibration.snap_guides =
            if has_visual_snap && snap_result.guides.iter().any(Option::is_some) {
                snap_result.guides
            } else {
                empty_calibration_guides()
            };

        match target {
            CalTarget::X1 => {
                self.calibration.cal_x.p1 = Some(snapped);
                *x_mapping = self.calibration.cal_x.mapping();
            }
            CalTarget::X2 => {
                self.calibration.cal_x.p2 = Some(snapped);
                *x_mapping = self.calibration.cal_x.mapping();
            }
            CalTarget::Y1 => {
                self.calibration.cal_y.p1 = Some(snapped);
                *y_mapping = self.calibration.cal_y.mapping();
            }
            CalTarget::Y2 => {
                self.calibration.cal_y.p2 = Some(snapped);
                *y_mapping = self.calibration.cal_y.mapping();
            }
            CalTarget::Origin => {
                self.calibration.polar_cal.origin = Some(snapped);
                *polar_mapping = self.calibration.polar_cal.mapping();
            }
            CalTarget::R1 => {
                self.calibration.polar_cal.radius.p1 = Some(snapped);
                *polar_mapping = self.calibration.polar_cal.mapping();
            }
            CalTarget::R2 => {
                self.calibration.polar_cal.radius.p2 = Some(snapped);
                *polar_mapping = self.calibration.polar_cal.mapping();
            }
            CalTarget::A1 => {
                self.calibration.polar_cal.angle.p1 = Some(snapped);
                *polar_mapping = self.calibration.polar_cal.mapping();
            }
            CalTarget::A2 => {
                self.calibration.polar_cal.angle.p2 = Some(snapped);
                *polar_mapping = self.calibration.polar_cal.mapping();
            }
        }

        if mode == CalUpdateMode::Pick {
            self.calibration.pick_mode = PickMode::None;
            if let Some(field) = target.value_field() {
                self.queue_value_focus(field);
            }
            self.set_status(self.i18n().format_picked(target.label()));
        }
    }

    fn draw_calibration_overlay(&self, painter: &egui::Painter, rect: egui::Rect) {
        if !self.calibration.show_calibration_segments {
            return;
        }
        match self.calibration.coord_system {
            CoordSystem::Cartesian => self.draw_cartesian_calibration_overlay(painter, rect),
            CoordSystem::Polar => self.draw_polar_calibration_overlay(painter, rect),
        }
    }

    fn calibration_style() -> CalOverlayStyle {
        let stroke = egui::Stroke {
            width: super::super::CAL_LINE_WIDTH,
            color: Color32::LIGHT_BLUE,
        };
        CalOverlayStyle {
            outline: egui::Stroke {
                width: super::super::CAL_LINE_OUTLINE_WIDTH,
                color: Color32::from_black_alpha(super::super::CAL_OUTLINE_ALPHA),
            },
            stroke,
            point_outer_radius: super::super::CAL_POINT_DRAW_RADIUS
                + super::super::CAL_POINT_OUTLINE_PAD,
            point_inner_radius: super::super::CAL_POINT_DRAW_RADIUS,
            label_font: egui::FontId::monospace(11.0),
            label_shadow: Color32::from_black_alpha(160),
        }
    }

    fn draw_cal_line(
        painter: &egui::Painter,
        rect: egui::Rect,
        zoom: f32,
        style: &CalOverlayStyle,
        p1: Pos2,
        p2: Pos2,
    ) {
        let line = [
            rect.min + p1.to_vec2() * zoom,
            rect.min + p2.to_vec2() * zoom,
        ];
        painter.line_segment(line, style.outline);
        painter.line_segment(line, style.stroke);
    }

    fn draw_dashed_segment(painter: &egui::Painter, start: Pos2, end: Pos2, stroke: egui::Stroke) {
        let dir = end - start;
        let len = dir.length();
        if len <= f32::EPSILON {
            return;
        }
        let unit = dir / len;
        let dash_and_gap = super::super::CAL_GUIDE_DASH_LENGTH + super::super::CAL_GUIDE_GAP_LENGTH;
        let mut offset = 0.0;
        loop {
            if offset >= len {
                break;
            }
            let dash_end = (offset + super::super::CAL_GUIDE_DASH_LENGTH).min(len);
            let dash_start_pos = start + unit * offset;
            let dash_end_pos = start + unit * dash_end;
            painter.line_segment([dash_start_pos, dash_end_pos], stroke);
            offset += dash_and_gap;
        }
    }

    fn draw_calibration_snap_guides(&self, painter: &egui::Painter, rect: egui::Rect) {
        let [r, g, b, _] = Color32::LIGHT_BLUE.to_array();
        let stroke = egui::Stroke::new(
            (super::super::CAL_LINE_WIDTH - 0.2).max(1.0),
            Color32::from_rgba_unmultiplied(r, g, b, 220),
        );
        for guide in self.calibration.snap_guides.iter().flatten() {
            let start = rect.min + guide.start.to_vec2() * self.image.zoom;
            let end = rect.min + guide.end.to_vec2() * self.image.zoom;
            Self::draw_dashed_segment(painter, start, end, stroke);
        }
    }

    fn draw_cal_point_base(
        painter: &egui::Painter,
        rect: egui::Rect,
        zoom: f32,
        style: &CalOverlayStyle,
        point: Pos2,
    ) -> Pos2 {
        let screen = rect.min + point.to_vec2() * zoom;
        painter.circle_filled(screen, style.point_outer_radius, style.outline.color);
        painter.circle_filled(screen, style.point_inner_radius, style.stroke.color);
        screen
    }

    #[allow(clippy::too_many_lines)]
    fn draw_cartesian_calibration_overlay(&self, painter: &egui::Painter, rect: egui::Rect) {
        let style = Self::calibration_style();
        let cal_point_color = style.stroke.color;
        let cal_radius = style.point_outer_radius;
        let cal_label_shadow = style.label_shadow;
        let cal_label_font = style.label_font.clone();
        let cal_length_bg = Color32::from_rgba_unmultiplied(0, 0, 0, 140);
        let cal_length_padding = Vec2::new(6.0, 3.0);
        let label_gap_px = 6.0;
        let default_label_offset = Vec2::new(8.0, -8.0);
        let default_dir = {
            let len = default_label_offset.length();
            if len > f32::EPSILON {
                default_label_offset / len
            } else {
                Vec2::new(0.0, -1.0)
            }
        };
        let calc_label_normal = |a: Option<Pos2>, b: Option<Pos2>| -> Option<Vec2> {
            let p1 = a?;
            let p2 = b?;
            let dir_screen = (p2 - p1) * self.image.zoom;
            if dir_screen.length_sq() <= f32::EPSILON {
                return None;
            }
            Some(Vec2::new(-dir_screen.y, dir_screen.x).normalized())
        };
        let x_normal = calc_label_normal(self.calibration.cal_x.p1, self.calibration.cal_x.p2);
        let y_normal = calc_label_normal(self.calibration.cal_y.p1, self.calibration.cal_y.p2);
        let draw_cal_point = |point: Pos2, label: &str, normal: Option<Vec2>, flip_side: bool| {
            let screen = Self::draw_cal_point_base(painter, rect, self.image.zoom, &style, point);
            let dir = normal.unwrap_or(default_dir);
            let dir = if flip_side { -dir } else { dir };
            let galley =
                painter.layout_no_wrap(label.to_owned(), cal_label_font.clone(), cal_point_color);
            let offset = galley.size().y.mul_add(0.5, cal_radius + label_gap_px);
            let label_center = screen + dir * offset;
            let label_pos = label_center - galley.size() * 0.5;
            let shadow_pos = label_pos + Vec2::splat(1.0);
            painter.galley(shadow_pos, galley.clone(), cal_label_shadow);
            painter.galley(label_pos, galley, cal_point_color);
        };
        let draw_cal_length_label = |p1: Pos2, p2: Pos2| {
            let len = (p2 - p1).length();
            if len <= f32::EPSILON {
                return;
            }
            let screen_p1 = rect.min + p1.to_vec2() * self.image.zoom;
            let screen_p2 = rect.min + p2.to_vec2() * self.image.zoom;
            let center = screen_p1 + (screen_p2 - screen_p1) * 0.5;
            let mut angle = (screen_p2 - screen_p1).angle();
            if angle.abs() > std::f32::consts::FRAC_PI_2 {
                angle += std::f32::consts::PI;
            }
            let label = format!("{len:.1} px");
            let galley = painter.layout_no_wrap(label, cal_label_font.clone(), cal_point_color);
            let size = galley.size();
            let half = size * 0.5 + cal_length_padding;
            let rot = egui::emath::Rot2::from_angle(angle);
            let points = vec![
                center + rot * Vec2::new(-half.x, -half.y),
                center + rot * Vec2::new(half.x, -half.y),
                center + rot * Vec2::new(half.x, half.y),
                center + rot * Vec2::new(-half.x, half.y),
            ];
            painter.add(egui::Shape::convex_polygon(
                points,
                cal_length_bg,
                egui::Stroke::NONE,
            ));
            let mut text_shape =
                egui::epaint::TextShape::new(center - size * 0.5, galley, cal_point_color);
            text_shape = text_shape.with_angle_and_anchor(angle, egui::Align2::CENTER_CENTER);
            painter.add(text_shape);
        };
        let draw_cal_line = |p1: Pos2, p2: Pos2| {
            Self::draw_cal_line(painter, rect, self.image.zoom, &style, p1, p2);
        };
        if let Some(p1) = self.calibration.cal_x.p1
            && let Some(p2) = self.calibration.cal_x.p2
        {
            draw_cal_line(p1, p2);
            draw_cal_length_label(p1, p2);
        }
        if let Some(p1) = self.calibration.cal_y.p1
            && let Some(p2) = self.calibration.cal_y.p2
        {
            draw_cal_line(p1, p2);
            draw_cal_length_label(p1, p2);
        }
        self.draw_calibration_snap_guides(painter, rect);
        if let Some(p) = self.calibration.cal_x.p1 {
            draw_cal_point(p, "X1", x_normal, false);
        }
        if let Some(p) = self.calibration.cal_x.p2 {
            draw_cal_point(p, "X2", x_normal, true);
        }
        if let Some(p) = self.calibration.cal_y.p1 {
            draw_cal_point(p, "Y1", y_normal, false);
        }
        if let Some(p) = self.calibration.cal_y.p2 {
            draw_cal_point(p, "Y2", y_normal, true);
        }
    }

    fn draw_polar_calibration_overlay(&self, painter: &egui::Painter, rect: egui::Rect) {
        let style = Self::calibration_style();
        let cal_point_color = style.stroke.color;
        let cal_label_shadow = style.label_shadow;
        let cal_label_font = style.label_font.clone();
        let label_offset = Vec2::new(8.0, -8.0);
        let draw_label = |screen: Pos2, label: &str| {
            let galley =
                painter.layout_no_wrap(label.to_owned(), cal_label_font.clone(), cal_point_color);
            let label_pos = screen + label_offset;
            let shadow_pos = label_pos + Vec2::splat(1.0);
            painter.galley(shadow_pos, galley.clone(), cal_label_shadow);
            painter.galley(label_pos, galley, cal_point_color);
        };
        let draw_point = |point: Pos2, label: &str| {
            let screen = Self::draw_cal_point_base(painter, rect, self.image.zoom, &style, point);
            draw_label(screen, label);
        };
        let draw_line = |p1: Pos2, p2: Pos2| {
            Self::draw_cal_line(painter, rect, self.image.zoom, &style, p1, p2);
        };

        let Some(origin) = self.calibration.polar_cal.origin else {
            return;
        };

        // Draw origin.
        draw_point(origin, "O");

        // Radius calibration lines and points.
        if let Some(p) = self.calibration.polar_cal.radius.p1 {
            draw_line(origin, p);
            draw_point(p, "R1");
        }
        if let Some(p) = self.calibration.polar_cal.radius.p2 {
            draw_line(origin, p);
            draw_point(p, "R2");
        }

        // Angle calibration rays and points.
        if let Some(p) = self.calibration.polar_cal.angle.p1 {
            draw_line(origin, p);
            draw_point(p, "A1");
        }
        if let Some(p) = self.calibration.polar_cal.angle.p2 {
            draw_line(origin, p);
            draw_point(p, "A2");
        }
    }

    fn draw_points_overlay(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        point_radius: f32,
        point_color: Color32,
    ) {
        for (idx, p) in self.points.points.iter().enumerate() {
            let screen = rect.min + p.pixel.to_vec2() * self.image.zoom;
            painter.circle_filled(screen, point_radius, point_color);
            painter.text(
                screen + Vec2::new(6.0, -6.0),
                egui::Align2::LEFT_TOP,
                format!("{}", idx + 1),
                egui::FontId::monospace(10.0),
                Color32::WHITE,
            );
        }
    }

    fn draw_snap_overlay(
        &self,
        painter: &egui::Painter,
        rect: egui::Rect,
        pointer_pixel: Option<Pos2>,
        snap_preview: Option<Pos2>,
        point_radius: f32,
    ) {
        if !matches!(
            self.snap.point_input_mode,
            PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
        ) || matches!(self.calibration.pick_mode, PickMode::CurveColor)
        {
            return;
        }

        if let Some(pixel) = pointer_pixel {
            let screen = rect.min + pixel.to_vec2() * self.image.zoom;
            let radius = (self.snap.contrast_search_radius * self.image.zoom).max(4.0);
            painter.circle_stroke(
                screen,
                radius,
                egui::Stroke::new(1.2, self.snap.snap_overlay_color),
            );
        }
        if let Some(preview) = snap_preview {
            let screen = rect.min + preview.to_vec2() * self.image.zoom;
            painter.circle_stroke(
                screen,
                (point_radius + 4.0).max(6.0),
                egui::Stroke::new(1.2, self.snap.snap_overlay_color),
            );
            painter.circle_filled(screen, 3.0, self.snap.snap_overlay_color);
        }
    }

    fn draw_curve_preview(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        if !self.points.show_curve_segments {
            return;
        }
        let stroke_curve = self.config.curve_line.stroke();
        let zoom = self.image.zoom;
        let preview_segments = self.sorted_preview_segments();
        if preview_segments.len() >= 2 {
            for win in preview_segments.windows(2) {
                let a = rect.min + win[0].1.to_vec2() * zoom;
                let b = rect.min + win[1].1.to_vec2() * zoom;
                painter.line_segment([a, b], stroke_curve);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn draw_navigator_minimap(
        &mut self,
        ui: &egui::Ui,
        texture_id: egui::TextureId,
        image_rect: egui::Rect,
        image_size: Vec2,
        viewport_rect: egui::Rect,
    ) {
        if image_size.x <= f32::EPSILON || image_size.y <= f32::EPSILON {
            return;
        }

        let display_size = image_size * self.image.zoom;
        let is_large = display_size.x > viewport_rect.width() * 1.10
            || display_size.y > viewport_rect.height() * 1.10;
        if !is_large {
            return;
        }

        let minimap_max = Vec2::new(190.0, 145.0);
        let scale = (minimap_max.x / image_size.x).min(minimap_max.y / image_size.y);
        if !scale.is_finite() || scale <= f32::EPSILON {
            return;
        }
        let thumb_size = image_size * scale;
        let frame_padding = Vec2::new(8.0, 8.0);
        let panel_size = thumb_size + frame_padding * 2.0;
        let outer_rect = ui.max_rect();
        let panel_rect = egui::Rect::from_min_size(
            pos2(
                outer_rect.right() - panel_size.x - 12.0,
                outer_rect.top() + 12.0,
            ),
            panel_size,
        );
        let thumb_rect = egui::Rect::from_min_size(panel_rect.min + frame_padding, thumb_size);

        let id = ui.make_persistent_id("navigator_minimap");
        let response = ui.interact(panel_rect, id, Sense::click_and_drag());
        let hover_text = match self.ui.language {
            crate::i18n::UiLanguage::En => "Navigator: click or drag to pan the viewport",
            crate::i18n::UiLanguage::Ru => {
                "Навигатор: кликните или тяните, чтобы панорамировать вид"
            }
        };
        let response = response.on_hover_text(hover_text);
        if (response.clicked() || response.dragged())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let clamped = pos2(
                pointer.x.clamp(thumb_rect.left(), thumb_rect.right()),
                pointer.y.clamp(thumb_rect.top(), thumb_rect.bottom()),
            );
            let local = clamped - thumb_rect.min;
            let image_target = pos2(
                (local.x / scale).clamp(0.0, image_size.x),
                (local.y / scale).clamp(0.0, image_size.y),
            );
            let viewport = viewport_rect.size();
            let max_pan = Vec2::new(
                (display_size.x - viewport.x).max(0.0),
                (display_size.y - viewport.y).max(0.0),
            );
            let pan_target = Vec2::new(
                viewport
                    .x
                    .mul_add(-0.5, image_target.x * self.image.zoom)
                    .clamp(0.0, max_pan.x),
                viewport
                    .y
                    .mul_add(-0.5, image_target.y * self.image.zoom)
                    .clamp(0.0, max_pan.y),
            );
            self.set_zoom_to_pan_target(self.image.zoom, pan_target);
        }

        let visible = image_rect.intersect(viewport_rect);
        let visible_min_display = Vec2::new(
            (visible.min.x - image_rect.min.x).clamp(0.0, display_size.x),
            (visible.min.y - image_rect.min.y).clamp(0.0, display_size.y),
        );
        let visible_max_display = Vec2::new(
            (visible.max.x - image_rect.min.x).clamp(0.0, display_size.x),
            (visible.max.y - image_rect.min.y).clamp(0.0, display_size.y),
        );
        let inv_zoom = self.image.zoom.recip();
        let view_min_image = pos2(
            (visible_min_display.x * inv_zoom).clamp(0.0, image_size.x),
            (visible_min_display.y * inv_zoom).clamp(0.0, image_size.y),
        );
        let view_max_image = pos2(
            (visible_max_display.x * inv_zoom).clamp(0.0, image_size.x),
            (visible_max_display.y * inv_zoom).clamp(0.0, image_size.y),
        );

        let [r, g, b, _] = Color32::from_rgb(120, 185, 255).to_array();
        let painter = ui.painter();
        painter.rect_filled(
            panel_rect,
            CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(20, 24, 28, 190),
        );
        painter.rect_stroke(
            panel_rect,
            CornerRadius::same(6),
            egui::Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Outside,
        );
        painter.image(
            texture_id,
            thumb_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(
            thumb_rect,
            CornerRadius::same(3),
            egui::Stroke::new(1.0, Color32::from_gray(130)),
            egui::StrokeKind::Outside,
        );

        let mut viewport_min = thumb_rect.min + view_min_image.to_vec2() * scale;
        let mut viewport_max = thumb_rect.min + view_max_image.to_vec2() * scale;
        if viewport_max.x < viewport_min.x {
            std::mem::swap(&mut viewport_min.x, &mut viewport_max.x);
        }
        if viewport_max.y < viewport_min.y {
            std::mem::swap(&mut viewport_min.y, &mut viewport_max.y);
        }
        let mut viewport_marker = egui::Rect::from_min_max(viewport_min, viewport_max);
        if viewport_marker.width() < 4.0 {
            let cx = viewport_marker.center().x;
            viewport_marker.min.x = (cx - 2.0).max(thumb_rect.left());
            viewport_marker.max.x = (cx + 2.0).min(thumb_rect.right());
        }
        if viewport_marker.height() < 4.0 {
            let cy = viewport_marker.center().y;
            viewport_marker.min.y = (cy - 2.0).max(thumb_rect.top());
            viewport_marker.max.y = (cy + 2.0).min(thumb_rect.bottom());
        }
        painter.rect_filled(
            viewport_marker,
            CornerRadius::same(2),
            Color32::from_rgba_unmultiplied(r, g, b, 30),
        );
        painter.rect_stroke(
            viewport_marker,
            CornerRadius::same(2),
            egui::Stroke::new(1.4, Color32::from_rgba_unmultiplied(r, g, b, 235)),
            egui::StrokeKind::Outside,
        );
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn draw_crosshair_overlay(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: egui::Rect,
        hover_pos: Option<Pos2>,
        pointer_pixel: Option<Pos2>,
        x_mapping: Option<&AxisMapping>,
        y_mapping: Option<&AxisMapping>,
        polar_mapping: Option<&PolarMapping>,
        delete_down: bool,
        shift_pressed: bool,
        ctrl_pressed: bool,
    ) {
        let Some(pos) = hover_pos else {
            return;
        };
        let Some(pixel) = pointer_pixel else {
            return;
        };

        let crosshair_color = self.config.crosshair.color32();
        let stroke = egui::Stroke::new(1.0, crosshair_color);
        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                painter.line_segment(
                    [pos2(rect.left(), pos.y), pos2(rect.right(), pos.y)],
                    stroke,
                );
                painter.line_segment(
                    [pos2(pos.x, rect.top()), pos2(pos.x, rect.bottom())],
                    stroke,
                );
            }
            CoordSystem::Polar => {
                if let Some(origin) = self.calibration.polar_cal.origin {
                    let origin_screen = rect.min + origin.to_vec2() * self.image.zoom;
                    let pointer_screen = rect.min + pixel.to_vec2() * self.image.zoom;
                    let radius = origin_screen.distance(pointer_screen);
                    if radius > f32::EPSILON {
                        painter.circle_stroke(origin_screen, radius, stroke);
                        painter.line_segment([origin_screen, pointer_screen], stroke);
                    }
                }
            }
        }

        let font = egui::FontId::proportional(12.0);
        let text_color = Color32::BLACK;
        let bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
        let padding = Vec2::new(4.0, 2.0);

        let clip = painter.clip_rect();
        let draw_label_centered = |center: Pos2, text: String, font: egui::FontId| {
            let galley = painter.layout_no_wrap(text, font, text_color);
            let size = galley.size();
            let total = size + padding * 2.0;
            let min_x = clip.left() + 2.0;
            let max_x = clip.right() - total.x - 2.0;
            let min_y = clip.top() + 2.0;
            let max_y = clip.bottom() - total.y - 2.0;
            let label_pos = pos2(
                if max_x < min_x {
                    min_x
                } else {
                    total.x.mul_add(-0.5, center.x).clamp(min_x, max_x)
                },
                if max_y < min_y {
                    min_y
                } else {
                    total.y.mul_add(-0.5, center.y).clamp(min_y, max_y)
                },
            );
            let bg_rect = egui::Rect::from_min_size(label_pos, total);
            painter.rect_filled(bg_rect, 3.0, bg_color);
            painter.galley(label_pos + padding, galley, text_color);
        };

        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                if let Some(xmap) = x_mapping
                    && let Some(value) = xmap.value_at(pixel)
                {
                    let text = format_overlay_value(&value);
                    let galley = painter.layout_no_wrap(text, font.clone(), text_color);
                    let size = galley.size();
                    let total = size + padding * 2.0;
                    let min_x = clip.left() + 2.0;
                    let max_x = clip.right() - total.x - 2.0;
                    let label_pos = pos2(
                        if max_x < min_x {
                            min_x
                        } else {
                            total.x.mul_add(-0.5, pos.x).clamp(min_x, max_x)
                        },
                        clip.top() + 4.0,
                    );
                    let bg_rect = egui::Rect::from_min_size(label_pos, total);
                    painter.rect_filled(bg_rect, 3.0, bg_color);
                    painter.galley(label_pos + padding, galley, text_color);
                }
                if let Some(ymap) = y_mapping
                    && let Some(value) = ymap.value_at(pixel)
                {
                    let text = format_overlay_value(&value);
                    let galley = painter.layout_no_wrap(text, font, text_color);
                    let size = galley.size();
                    let total = size + padding * 2.0;
                    let min_y = clip.top() + 2.0;
                    let max_y = clip.bottom() - total.y - 2.0;
                    let label_pos = pos2(
                        clip.left() + 4.0,
                        if max_y < min_y {
                            min_y
                        } else {
                            total.y.mul_add(-0.5, pos.y).clamp(min_y, max_y)
                        },
                    );
                    let bg_rect = egui::Rect::from_min_size(label_pos, total);
                    painter.rect_filled(bg_rect, 3.0, bg_color);
                    painter.galley(label_pos + padding, galley, text_color);
                }
            }
            CoordSystem::Polar => {
                if let (Some(mapping), Some(origin)) =
                    (polar_mapping, self.calibration.polar_cal.origin)
                {
                    let origin_screen = rect.min + origin.to_vec2() * self.image.zoom;
                    let pointer_screen = rect.min + pixel.to_vec2() * self.image.zoom;
                    let radial_vec = pointer_screen - origin_screen;
                    let radial_len = radial_vec.length();
                    if radial_len > f32::EPSILON {
                        let dir = radial_vec / radial_len;
                        let angle_offset = 6.0_f32.to_radians();
                        let signed_offset = match self.calibration.polar_cal.angle_direction {
                            crate::types::AngleDirection::Cw => angle_offset,
                            crate::types::AngleDirection::Ccw => -angle_offset,
                        };
                        let rot = egui::emath::Rot2::from_angle(signed_offset);
                        let theta_dir = rot * dir;
                        let theta_center = origin_screen + theta_dir * radial_len;
                        let r_center = origin_screen + dir * (radial_len * 0.5);

                        if let Some(angle) = mapping.angle_at(pixel) {
                            let text = format!("{angle:.3}");
                            draw_label_centered(theta_center, text, font.clone());
                        }
                        if let Some(radius) = mapping.radius_at(pixel) {
                            let text = format!("{radius:.3}");
                            draw_label_centered(r_center, text, font);
                        }
                    }
                }
            }
        }

        let badge_offset = Vec2::new(18.0, -18.0);
        let badge_anchor = pos + badge_offset;
        let badge_radius = 12.0;
        let showed_color_badge = {
            if matches!(self.calibration.pick_mode, PickMode::CurveColor)
                && let Some(sampled) = self.sample_image_color(pixel)
            {
                let [r, g, b, _] = sampled.to_array();
                let badge_color = Color32::from_rgb(r, g, b);
                painter.circle_filled(badge_anchor, badge_radius, badge_color);
                painter.circle_stroke(
                    badge_anchor,
                    badge_radius,
                    egui::Stroke::new(1.0, Color32::from_gray(30)),
                );
                true
            } else {
                false
            }
        };

        if !showed_color_badge
            && let Some(icon_badge) = self.cursor_badge(delete_down, shift_pressed, ctrl_pressed)
        {
            let icon_bg = Color32::from_rgba_unmultiplied(0, 0, 0, 160);
            painter.circle_filled(badge_anchor, badge_radius, icon_bg);
            match icon_badge {
                CursorBadge::Text(icon_text, icon_color) => {
                    let icon_font = egui::FontId::proportional(15.0);
                    let icon_galley =
                        painter.layout_no_wrap(icon_text.to_string(), icon_font, icon_color);
                    let icon_size = icon_galley.size();
                    let icon_pos = pos2(
                        icon_size.x.mul_add(-0.5, badge_anchor.x),
                        icon_size.y.mul_add(-0.5, badge_anchor.y),
                    );
                    painter.galley(icon_pos, icon_galley, icon_color);
                }
                CursorBadge::Icon(icon, icon_color) => {
                    let icon_rect = egui::Rect::from_center_size(
                        badge_anchor,
                        Vec2::splat(icons::BADGE_ICON_SIZE),
                    );
                    icons::image(icon, icons::BADGE_ICON_SIZE)
                        .tint(icon_color)
                        .paint_at(ui, icon_rect);
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_central_image(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        self.image.last_viewport_size = Some(ui.available_size());
        self.apply_pending_fit_on_load();
        self.handle_drag_and_drop(ui);

        if let Some(img) = self.image.image.as_ref() {
            let mut x_mapping = self.calibration.cal_x.mapping();
            let mut y_mapping = self.calibration.cal_y.mapping();
            let mut polar_mapping = self.polar_mapping();
            let mut pending_zoom: Option<f32> = None;
            let mut pending_zoom_anchor: Option<Pos2> = None;
            let mut image_screen_rect: Option<egui::Rect> = None;
            let mut image_base_size: Option<Vec2> = None;
            // Take a snapshot of the texture handle and size to avoid borrowing self.image in the UI closure
            let (tex_id, img_size) = (img.texture.id(), img.size);
            let scroll_out = egui::ScrollArea::both()
                .id_salt("image_scroll")
                .scroll_offset(self.image.pan)
                .scroll_source(egui::scroll_area::ScrollSource {
                    mouse_wheel: false,
                    ..egui::scroll_area::ScrollSource::ALL
                })
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                let base_size = egui::vec2(
                    safe_usize_to_f32(img_size[0]),
                    safe_usize_to_f32(img_size[1]),
                );
                let display_size = base_size * self.image.zoom;
                let image = egui::Image::new((tex_id, display_size));
                let response = self.add_centered_image(ui, image, display_size);
                let rect = response.rect;
                image_screen_rect = Some(rect);
                image_base_size = Some(base_size);
                let painter = ui.painter_at(rect);

                self.handle_middle_pan(&response, ui);

                if let Some((next_zoom, anchor)) = self.compute_scroll_zoom(&response, ui) {
                    pending_zoom = Some(next_zoom);
                    pending_zoom_anchor = anchor;
                }

                let zoom = self.image.zoom;
                let to_pixel = |pos: Pos2| {
                    let local = pos - rect.min;
                    pos2(
                        (local.x / zoom).clamp(0.0, base_size.x),
                        (local.y / zoom).clamp(0.0, base_size.y),
                    )
                };

                let pointer_pos = response.interact_pointer_pos();
                let pointer_state = PointerState::read(ui.ctx());
                if !pointer_state.shift_pressed
                    && matches!(self.calibration.pick_mode, PickMode::None)
                    && self.calibration.dragging_handle.is_none()
                {
                    self.calibration.int_snap_sticky = None;
                }
                let hover_pos = response
                    .hover_pos()
                    .or(pointer_pos)
                    .or_else(|| Self::pos_in_rect(pointer_state.latest_pos, rect));
                let pointer_pixel = hover_pos.map(&to_pixel);
                let hover_pos_only = response.hover_pos();
                let hover_pixel = hover_pos_only.map(&to_pixel);
                self.calibration.snap_guides = empty_calibration_guides();
                if self.calibration.dragging_handle.is_none() {
                    self.update_calibration_pick_preview_guides(hover_pixel, base_size);
                }
                let primary_gesture = self.resolve_primary_gesture(
                    &response,
                    rect,
                    pointer_state,
                    pointer_pos,
                    hover_pos,
                );
                let snap_preview = self.compute_snap_preview(pointer_pixel);
                let calibrated = match self.calibration.coord_system {
                    CoordSystem::Cartesian => {
                        x_mapping.is_some() && y_mapping.is_some()
                    }
                    CoordSystem::Polar => polar_mapping.is_some(),
                };
                let auto_place_pointer_pixel = if primary_gesture.pointer_over_image {
                    pointer_pixel
                } else {
                    None
                };
                let suppress_primary_click = self.auto_place_tick(
                    auto_place_pointer_pixel,
                    primary_gesture,
                    pointer_state.shift_pressed,
                    pointer_state.delete_down,
                    calibrated,
                );

                if primary_gesture.down && primary_gesture.started_in_image {
                    ui.ctx().request_repaint_after(Duration::from_millis(16));
                }

                if pointer_state.shift_pressed
                    && response.drag_started_by(PointerButton::Primary)
                    && let Some(pos) = pointer_pos
                {
                    let mut best: Option<(DragTarget, f32)> = None;
                    let mut consider = |target: DragTarget, screen: Pos2| {
                        let dist = pos.distance(screen);
                        if dist <= super::super::POINT_HIT_RADIUS
                            && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
                        {
                            best = Some((target, dist));
                        }
                    };

                    for (idx, point) in self.points.points.iter().enumerate() {
                        let screen = rect.min + point.pixel.to_vec2() * self.image.zoom;
                        consider(DragTarget::CurvePoint(idx), screen);
                    }

                    match self.calibration.coord_system {
                        CoordSystem::Cartesian => {
                            for (target, maybe_pixel) in [
                                (DragTarget::CalX1, self.calibration.cal_x.p1),
                                (DragTarget::CalX2, self.calibration.cal_x.p2),
                                (DragTarget::CalY1, self.calibration.cal_y.p1),
                                (DragTarget::CalY2, self.calibration.cal_y.p2),
                            ] {
                                if let Some(pixel) = maybe_pixel {
                                    let screen = rect.min + pixel.to_vec2() * self.image.zoom;
                                    consider(target, screen);
                                }
                            }
                            if let (Some(p1), Some(p2)) =
                                (self.calibration.cal_x.p1, self.calibration.cal_x.p2)
                            {
                                let a = rect.min + p1.to_vec2() * self.image.zoom;
                                let b = rect.min + p2.to_vec2() * self.image.zoom;
                                if let Some(dist) = line_drag_hit_distance(pos, a, b)
                                    && best
                                        .as_ref()
                                        .is_none_or(|(_, best_dist)| dist < *best_dist)
                                {
                                    best = Some((DragTarget::CalXLine, dist));
                                }
                            }
                            if let (Some(p1), Some(p2)) =
                                (self.calibration.cal_y.p1, self.calibration.cal_y.p2)
                            {
                                let a = rect.min + p1.to_vec2() * self.image.zoom;
                                let b = rect.min + p2.to_vec2() * self.image.zoom;
                                if let Some(dist) = line_drag_hit_distance(pos, a, b)
                                    && best
                                        .as_ref()
                                        .is_none_or(|(_, best_dist)| dist < *best_dist)
                                {
                                    best = Some((DragTarget::CalYLine, dist));
                                }
                            }
                        }
                        CoordSystem::Polar => {
                            for (target, maybe_pixel) in [
                                (DragTarget::PolarOrigin, self.calibration.polar_cal.origin),
                                (DragTarget::PolarR1, self.calibration.polar_cal.radius.p1),
                                (DragTarget::PolarR2, self.calibration.polar_cal.radius.p2),
                                (DragTarget::PolarA1, self.calibration.polar_cal.angle.p1),
                                (DragTarget::PolarA2, self.calibration.polar_cal.angle.p2),
                            ] {
                                if let Some(pixel) = maybe_pixel {
                                    let screen = rect.min + pixel.to_vec2() * self.image.zoom;
                                    consider(target, screen);
                                }
                            }
                        }
                    }

                    let picked = best.map(|(target, _)| target);
                    self.calibration.dragging_handle = picked;
                    self.calibration.drag_last_pixel = picked.map(|_| to_pixel(pos));
                }

                if let Some(target) = self.calibration.dragging_handle {
                    if let Some(pos) = pointer_pos {
                        let pixel = to_pixel(pos);
                        match target {
                            DragTarget::CurvePoint(idx) => {
                                if let Some(point) = self.points.points.get_mut(idx) {
                                    point.pixel = pixel;
                                    self.mark_points_dirty();
                                }
                            }
                            DragTarget::CalXLine => {
                                if let (Some(p1), Some(p2)) =
                                    (self.calibration.cal_x.p1, self.calibration.cal_x.p2)
                                {
                                    let prev = self.calibration.drag_last_pixel.unwrap_or(pixel);
                                    let delta = clamp_line_drag_delta(pixel - prev, p1, p2, base_size);
                                    if delta.length_sq() > f32::EPSILON {
                                        self.calibration.cal_x.p1 = Some(p1 + delta);
                                        self.calibration.cal_x.p2 = Some(p2 + delta);
                                        x_mapping = self.calibration.cal_x.mapping();
                                    }
                                }
                            }
                            DragTarget::CalYLine => {
                                if let (Some(p1), Some(p2)) =
                                    (self.calibration.cal_y.p1, self.calibration.cal_y.p2)
                                {
                                    let prev = self.calibration.drag_last_pixel.unwrap_or(pixel);
                                    let delta = clamp_line_drag_delta(pixel - prev, p1, p2, base_size);
                                    if delta.length_sq() > f32::EPSILON {
                                        self.calibration.cal_y.p1 = Some(p1 + delta);
                                        self.calibration.cal_y.p2 = Some(p2 + delta);
                                        y_mapping = self.calibration.cal_y.mapping();
                                    }
                                }
                            }
                            _ => {
                                if let Some(cal_target) = CalTarget::from_drag(target) {
                                    self.apply_calibration_point(
                                        cal_target,
                                        pixel,
                                        base_size,
                                        CalUpdateMode::Drag,
                                        &mut x_mapping,
                                        &mut y_mapping,
                                        &mut polar_mapping,
                                    );
                                }
                            }
                        }
                        self.calibration.drag_last_pixel = Some(pixel);
                    }
                    if !pointer_state.shift_pressed || !pointer_state.primary_down {
                        self.clear_calibration_drag_runtime();
                    }
                } else if response.clicked_by(PointerButton::Secondary)
                    && matches!(self.calibration.pick_mode, PickMode::None)
                    && let Some(pos) = pointer_pos
                {
                    let image_origin = rect.min;
                    self.remove_point_near_screen(pos, image_origin);
                } else if primary_gesture.clicked
                    && !suppress_primary_click
                    && !pointer_state.shift_pressed
                    && let Some(pos) = primary_gesture.click_pos
                {
                    if pointer_state.delete_down {
                        let image_origin = rect.min;
                        self.remove_point_near_screen(pos, image_origin);
                    } else {
                        let pixel = to_pixel(pos);
                        let pick_mode = self.calibration.pick_mode;
                        match pick_mode {
                            PickMode::None => {
                                if calibrated {
                                    self.push_curve_point(pixel);
                                } else {
                                    self.set_status_warn(match self.calibration.coord_system {
                                        CoordSystem::Cartesian => {
                                            match self.ui.language {
                                                crate::i18n::UiLanguage::En =>
                                                    "Calibration incomplete: set both X and Y axes before picking points.",
                                                crate::i18n::UiLanguage::Ru =>
                                                    "Калибровка неполная: задайте обе оси X и Y перед установкой точек.",
                                            }
                                        }
                                        CoordSystem::Polar => {
                                            match self.ui.language {
                                                crate::i18n::UiLanguage::En =>
                                                    "Calibration incomplete: set origin, radius, and angle before picking points.",
                                                crate::i18n::UiLanguage::Ru =>
                                                    "Калибровка неполная: задайте начало, радиус и угол перед установкой точек.",
                                            }
                                        }
                                    });
                                }
                            }
                            PickMode::CurveColor => {
                                self.pick_curve_color_at(pixel);
                                self.calibration.pick_mode = PickMode::None;
                            }
                            PickMode::AutoTrace => {
                                self.auto_trace_from(pixel);
                                self.calibration.pick_mode = PickMode::None;
                            }
                            _ => {
                                if let Some(cal_target) = CalTarget::from_pick_mode(pick_mode) {
                                    self.apply_calibration_point(
                                        cal_target,
                                        pixel,
                                        base_size,
                                        CalUpdateMode::Pick,
                                        &mut x_mapping,
                                        &mut y_mapping,
                                        &mut polar_mapping,
                                    );
                                }
                            }
                        }
                    }
                }

                self.ensure_point_numeric_cache(
                    self.calibration.coord_system,
                    x_mapping.as_ref(),
                    y_mapping.as_ref(),
                    polar_mapping.as_ref(),
                );
                self.draw_calibration_overlay(&painter, rect);

                let point_style = &self.config.curve_points;
                let point_color = point_style.color32();
                let point_radius = point_style.radius();
                self.draw_points_overlay(&painter, rect, point_radius, point_color);
                self.draw_snap_overlay(&painter, rect, pointer_pixel, snap_preview, point_radius);
                self.draw_curve_preview(&painter, rect);
                self.draw_crosshair_overlay(
                    ui,
                    &painter,
                    rect,
                    hover_pos_only,
                    hover_pixel,
                    x_mapping.as_ref(),
                    y_mapping.as_ref(),
                    polar_mapping.as_ref(),
                    pointer_state.delete_down,
                    pointer_state.shift_pressed,
                    pointer_state.ctrl_pressed,
                );
            });
            if self.image.skip_pan_sync_once {
                self.image.skip_pan_sync_once = false;
            } else {
                self.image.pan = scroll_out.state.offset;
            }
            self.image.last_viewport_size = Some(scroll_out.inner_rect.size());
            if let Some(next_zoom) = pending_zoom {
                if let Some(anchor_screen) = pending_zoom_anchor {
                    let anchor = anchor_screen - scroll_out.inner_rect.min;
                    self.set_zoom_about_viewport_pos(next_zoom, pos2(anchor.x, anchor.y));
                } else {
                    self.set_zoom_about_viewport_center(next_zoom);
                }
            }
            if let (Some(image_rect), Some(image_size)) = (image_screen_rect, image_base_size) {
                self.draw_navigator_minimap(
                    ui,
                    tex_id,
                    image_rect,
                    image_size,
                    scroll_out.inner_rect,
                );
            }
            self.step_zoom_animation(ui.ctx());
        } else if self.project.pending_image_task.is_some() {
            ui.centered_and_justified(|ui| {
                if let Some(task) = self.project.pending_image_task.as_ref() {
                    ui.label(
                        self.i18n()
                            .format_loading_image_row(&task.meta.description()),
                    );
                } else {
                    ui.label(self.t(TextKey::LoadingImage));
                }
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(self.t(TextKey::DropHint));
            });
        }
    }
}

#[derive(Clone, Copy)]
enum CursorBadge {
    Text(&'static str, Color32),
    Icon(icons::Icon, Color32),
}

impl CurcatApp {
    const fn cursor_badge(
        &self,
        delete_down: bool,
        shift_pressed: bool,
        ctrl_pressed: bool,
    ) -> Option<CursorBadge> {
        if let Some(badge) = self.calibration_cursor_badge() {
            return Some(badge);
        }
        if matches!(self.calibration.pick_mode, PickMode::AutoTrace) {
            return Some(CursorBadge::Icon(icons::ICON_AUTO_TRACE, Color32::WHITE));
        }
        if self.interaction.auto_place_state.active {
            return Some(CursorBadge::Icon(icons::ICON_AUTO_PLACE, Color32::WHITE));
        }
        if matches!(self.calibration.pick_mode, PickMode::CurveColor) {
            return Some(CursorBadge::Icon(icons::ICON_PICK_COLOR, Color32::WHITE));
        }
        if delete_down {
            return Some(CursorBadge::Icon(icons::ICON_DELETE_POINT, Color32::WHITE));
        }
        if shift_pressed {
            return Some(CursorBadge::Icon(icons::ICON_PAN, Color32::WHITE));
        }
        if ctrl_pressed {
            return Some(CursorBadge::Icon(icons::ICON_ZOOM, Color32::WHITE));
        }
        None
    }

    const fn calibration_cursor_badge(&self) -> Option<CursorBadge> {
        match self.calibration.pick_mode {
            PickMode::X1 => Some(CursorBadge::Text("X1", Color32::from_rgb(190, 225, 255))),
            PickMode::X2 => Some(CursorBadge::Text("X2", Color32::from_rgb(190, 225, 255))),
            PickMode::Y1 => Some(CursorBadge::Text("Y1", Color32::from_rgb(200, 255, 200))),
            PickMode::Y2 => Some(CursorBadge::Text("Y2", Color32::from_rgb(200, 255, 200))),
            PickMode::Origin => Some(CursorBadge::Text("O", Color32::from_rgb(255, 230, 180))),
            PickMode::R1 => Some(CursorBadge::Text("R1", Color32::from_rgb(255, 210, 160))),
            PickMode::R2 => Some(CursorBadge::Text("R2", Color32::from_rgb(255, 210, 160))),
            PickMode::A1 => Some(CursorBadge::Text("A1", Color32::from_rgb(200, 210, 255))),
            PickMode::A2 => Some(CursorBadge::Text("A2", Color32::from_rgb(200, 210, 255))),
            _ => None,
        }
    }

    fn remove_point_near_screen(&mut self, pointer: Pos2, image_origin: Pos2) -> bool {
        let mut best: Option<(usize, f32)> = None;
        for (idx, point) in self.points.points.iter().enumerate() {
            let screen = image_origin + point.pixel.to_vec2() * self.image.zoom;
            let dist = pointer.distance(screen);
            if dist <= super::super::POINT_HIT_RADIUS
                && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
            {
                best = Some((idx, dist));
            }
        }
        if let Some((idx, _)) = best {
            self.points.points.remove(idx);
            self.mark_points_dirty();
            self.set_status(match self.ui.language {
                crate::i18n::UiLanguage::En => "Point removed.",
                crate::i18n::UiLanguage::Ru => "Точка удалена.",
            });
            true
        } else {
            false
        }
    }
}

impl CurcatApp {
    fn reset_auto_place_runtime(&mut self, keep_suppress: bool) {
        let suppress_click = self.interaction.auto_place_state.suppress_click && keep_suppress;
        self.interaction.auto_place_state = AutoPlaceState {
            suppress_click,
            ..AutoPlaceState::default()
        };
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::fn_params_excessive_bools)]
    fn auto_place_tick(
        &mut self,
        pointer_pixel: Option<Pos2>,
        primary: PrimaryImageGesture,
        shift_pressed: bool,
        delete_down: bool,
        calibrated: bool,
    ) -> bool {
        if primary.pressed {
            self.reset_auto_place_runtime(false);
        }

        let mut suppress_click = self.interaction.auto_place_state.suppress_click;

        if !primary.down {
            suppress_click = self.interaction.auto_place_state.suppress_click;
            self.reset_auto_place_runtime(false);
            return suppress_click;
        }

        if !primary.started_in_image {
            self.reset_auto_place_runtime(false);
            return self.interaction.auto_place_state.suppress_click;
        }

        if shift_pressed || delete_down || !matches!(self.calibration.pick_mode, PickMode::None) {
            self.reset_auto_place_runtime(true);
            return suppress_click;
        }

        if !calibrated {
            return suppress_click;
        }

        let Some(pixel) = pointer_pixel else {
            self.reset_auto_place_runtime(true);
            return suppress_click;
        };

        let now = Instant::now();
        let cfg = self.interaction.auto_place_cfg;

        if self.interaction.auto_place_state.hold_started_at.is_none() {
            self.interaction.auto_place_state.hold_started_at = Some(now);
            self.interaction.auto_place_state.last_pointer = Some((pixel, now));
            self.interaction.auto_place_state.pause_started_at = None;
            self.interaction.auto_place_state.speed_ewma = 0.0;
        }

        if !self.interaction.auto_place_state.active {
            let hold_started_at =
                if let Some(started_at) = self.interaction.auto_place_state.hold_started_at {
                    started_at
                } else {
                    eprintln!("auto-place: missing hold start; resetting timer");
                    self.interaction.auto_place_state.hold_started_at = Some(now);
                    now
                };
            let hold_elapsed = now.saturating_duration_since(hold_started_at).as_secs_f32();
            if hold_elapsed >= cfg.hold_activation_secs {
                self.interaction.auto_place_state.active = true;
                self.interaction.auto_place_state.suppress_click = true;
                suppress_click = true;
                self.update_auto_place_speed(pixel, now);
                self.try_auto_place_point(pixel, now);
            }
            return suppress_click;
        }

        self.update_auto_place_speed(pixel, now);
        self.interaction.auto_place_state.suppress_click = true;
        let _ = self.try_auto_place_point(pixel, now);
        true
    }

    fn update_auto_place_speed(&mut self, pixel: Pos2, now: Instant) {
        if let Some((prev, prev_time)) = self.interaction.auto_place_state.last_pointer {
            let dt = now
                .saturating_duration_since(prev_time)
                .as_secs_f32()
                .max(f32::EPSILON);
            let dist = (pixel - prev).length();
            let inst_speed = dist / dt;
            let alpha = self
                .interaction
                .auto_place_cfg
                .speed_smoothing
                .clamp(0.0, 1.0);
            let prev_speed = self.interaction.auto_place_state.speed_ewma;
            self.interaction.auto_place_state.speed_ewma =
                if alpha <= f32::EPSILON || !prev_speed.is_finite() || prev_speed <= f32::EPSILON {
                    inst_speed
                } else {
                    prev_speed + alpha * (inst_speed - prev_speed)
                };
        } else {
            self.interaction.auto_place_state.speed_ewma = 0.0;
        }
        self.interaction.auto_place_state.last_pointer = Some((pixel, now));
    }

    fn try_auto_place_point(&mut self, pointer_pixel: Pos2, now: Instant) -> bool {
        let cfg = self.interaction.auto_place_cfg;
        let speed = self.interaction.auto_place_state.speed_ewma.max(0.0);
        let distance_threshold =
            (speed * cfg.distance_per_speed).clamp(cfg.distance_min, cfg.distance_max);
        let time_threshold = if speed <= f32::EPSILON {
            cfg.time_max_secs
        } else {
            (cfg.time_per_speed / speed).clamp(cfg.time_min_secs, cfg.time_max_secs)
        };

        let paused = if speed < cfg.pause_speed_threshold {
            let start = self
                .interaction
                .auto_place_state
                .pause_started_at
                .get_or_insert(now);
            now.saturating_duration_since(*start).as_millis() >= u128::from(cfg.pause_timeout_ms)
        } else {
            self.interaction.auto_place_state.pause_started_at = None;
            false
        };
        if paused {
            return false;
        }

        let snapped = self.resolve_curve_pick(pointer_pixel);

        if let Some((last_pos, last_time)) = self.interaction.auto_place_state.last_snapped_point {
            let dist = (snapped - last_pos).length();
            if dist < cfg.dedup_radius {
                return false;
            }
            let elapsed = now.saturating_duration_since(last_time).as_secs_f32();
            if dist < distance_threshold || elapsed < time_threshold {
                return false;
            }
        }

        self.push_curve_point_snapped(snapped);
        self.interaction.auto_place_state.last_snapped_point = Some((snapped, now));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CalTarget, CalibrationSnapKind, CartesianEndpointId, CurcatApp, PrimaryImageGesture,
        PrimaryPressInfo, clamp_line_drag_delta, is_soft_primary_click, line_drag_hit_distance,
    };
    use crate::types::CoordSystem;
    use egui::{Pos2, Rect, Vec2, pos2, vec2};
    use std::time::{Duration, Instant};

    fn assert_vec2_close(actual: Vec2, expected: Vec2) {
        assert!((actual.x - expected.x).abs() <= f32::EPSILON);
        assert!((actual.y - expected.y).abs() <= f32::EPSILON);
    }

    fn assert_point_in_bounds(point: Pos2, bounds: Vec2) {
        assert!(point.x >= 0.0 && point.x <= bounds.x);
        assert!(point.y >= 0.0 && point.y <= bounds.y);
    }

    fn assert_point_close(actual: Pos2, expected: Pos2) {
        assert!((actual.x - expected.x).abs() <= f32::EPSILON);
        assert!((actual.y - expected.y).abs() <= f32::EPSILON);
    }

    #[test]
    fn pos_in_rect_accepts_inside_position() {
        let rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(30.0, 40.0));
        let pos = CurcatApp::pos_in_rect(Some(pos2(15.0, 25.0)), rect);
        assert_eq!(pos, Some(pos2(15.0, 25.0)));
    }

    #[test]
    fn pos_in_rect_rejects_outside_position() {
        let rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(30.0, 40.0));
        let pos = CurcatApp::pos_in_rect(Some(pos2(35.0, 25.0)), rect);
        assert!(pos.is_none());
    }

    #[test]
    fn soft_primary_click_requires_hovered_response() {
        let rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(50.0, 50.0));
        let now = Instant::now();
        let press = PrimaryPressInfo {
            pos: pos2(20.0, 20.0),
            time: now.checked_sub(Duration::from_millis(20)).unwrap_or(now),
            in_rect: true,
            shift_down: false,
        };
        let clicked = is_soft_primary_click(&press, Some(pos2(21.0, 20.5)), rect, false);
        assert!(!clicked);
    }

    #[test]
    fn soft_primary_click_accepts_short_release_inside_hovered_image() {
        let rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(50.0, 50.0));
        let now = Instant::now();
        let press = PrimaryPressInfo {
            pos: pos2(20.0, 20.0),
            time: now.checked_sub(Duration::from_millis(20)).unwrap_or(now),
            in_rect: true,
            shift_down: false,
        };
        let clicked = is_soft_primary_click(&press, Some(pos2(22.0, 21.0)), rect, true);
        assert!(clicked);
    }

    fn primary_gesture_for_test(
        down: bool,
        pressed: bool,
        started_in_image: bool,
    ) -> PrimaryImageGesture {
        PrimaryImageGesture {
            down,
            pressed,
            started_in_image,
            pointer_over_image: started_in_image,
            clicked: false,
            click_pos: None,
        }
    }

    #[test]
    fn auto_place_tick_ignores_presses_started_outside_image() {
        let mut app = CurcatApp::default();
        let suppressed = app.auto_place_tick(
            Some(pos2(100.0, 100.0)),
            primary_gesture_for_test(true, true, false),
            false,
            false,
            true,
        );
        assert!(!suppressed);
        assert!(!app.interaction.auto_place_state.active);
        assert!(app.interaction.auto_place_state.hold_started_at.is_none());
    }

    #[test]
    fn auto_place_tick_starts_hold_only_for_image_origin_press() {
        let mut app = CurcatApp::default();
        let _ = app.auto_place_tick(
            Some(pos2(100.0, 100.0)),
            primary_gesture_for_test(true, true, true),
            false,
            false,
            true,
        );
        assert!(app.interaction.auto_place_state.hold_started_at.is_some());
    }

    #[test]
    fn auto_place_tick_resets_hold_when_pointer_leaves_image() {
        let mut app = CurcatApp::default();
        let _ = app.auto_place_tick(
            Some(pos2(100.0, 100.0)),
            primary_gesture_for_test(true, true, true),
            false,
            false,
            true,
        );
        assert!(app.interaction.auto_place_state.hold_started_at.is_some());

        let _ = app.auto_place_tick(
            None,
            primary_gesture_for_test(true, false, true),
            false,
            false,
            true,
        );
        assert!(app.interaction.auto_place_state.hold_started_at.is_none());
        assert!(!app.interaction.auto_place_state.active);
    }

    fn cartesian_app(x1: Pos2, x2: Pos2, y1: Pos2, y2: Pos2) -> CurcatApp {
        let mut app = CurcatApp::default();
        app.calibration.coord_system = CoordSystem::Cartesian;
        app.image.zoom = 1.0;
        app.calibration.cal_x.p1 = Some(x1);
        app.calibration.cal_x.p2 = Some(x2);
        app.calibration.cal_y.p1 = Some(y1);
        app.calibration.cal_y.p2 = Some(y2);
        app
    }

    fn guide_count(app: &CurcatApp) -> usize {
        app.calibration
            .snap_guides
            .iter()
            .filter(|guide| guide.is_some())
            .count()
    }

    #[test]
    fn line_drag_hit_accepts_middle_segment() {
        let hit = line_drag_hit_distance(pos2(50.0, 3.0), pos2(0.0, 0.0), pos2(100.0, 0.0));
        assert!(hit.is_some());
    }

    #[test]
    fn line_drag_hit_rejects_end_gap_zone() {
        let hit = line_drag_hit_distance(pos2(4.0, 0.0), pos2(0.0, 0.0), pos2(100.0, 0.0));
        assert!(hit.is_none());
    }

    #[test]
    fn line_drag_hit_rejects_short_segment() {
        let hit = line_drag_hit_distance(pos2(8.0, 0.0), pos2(0.0, 0.0), pos2(16.0, 0.0));
        assert!(hit.is_none());
    }

    #[test]
    fn line_drag_hit_rejects_far_pointer() {
        let hit = line_drag_hit_distance(pos2(50.0, 20.0), pos2(0.0, 0.0), pos2(100.0, 0.0));
        assert!(hit.is_none());
    }

    #[test]
    fn clamp_line_drag_delta_keeps_delta_when_inside_bounds() {
        let delta = clamp_line_drag_delta(
            vec2(5.0, -8.0),
            pos2(10.0, 20.0),
            pos2(30.0, 60.0),
            vec2(100.0, 100.0),
        );
        assert_vec2_close(delta, vec2(5.0, -8.0));
    }

    #[test]
    fn clamp_line_drag_delta_limits_shift_to_image_bounds() {
        let p1 = pos2(2.0, 5.0);
        let p2 = pos2(10.0, 15.0);
        let bounds = vec2(20.0, 40.0);
        let delta = clamp_line_drag_delta(vec2(-10.0, 50.0), p1, p2, bounds);
        assert_vec2_close(delta, vec2(-2.0, 25.0));
        assert_point_in_bounds(p1 + delta, bounds);
        assert_point_in_bounds(p2 + delta, bounds);
    }

    #[test]
    fn endpoint_snap_extension_works_on_imaginary_continuation() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = true;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(40.0, 102.0),
                vec2(200.0, 200.0),
            )
            .expect("extension snap");
        assert!(matches!(result.snap_kind, CalibrationSnapKind::Extension));
        assert_point_close(result.snapped_pixel, pos2(40.0, 100.0));
        let guide = result.guides[0].expect("extension guide");
        assert_point_close(guide.start, pos2(50.0, 100.0));
        assert_point_close(guide.end, pos2(40.0, 100.0));
    }

    #[test]
    fn endpoint_snap_extension_ignores_real_segment_body() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = true;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let result = app.endpoint_snap_reference(
            CartesianEndpointId::X1,
            pos2(65.0, 115.0),
            vec2(200.0, 200.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn endpoint_snap_extension_supports_rotated_vh_guides() {
        let mut app = cartesian_app(
            pos2(50.0, 50.0),
            pos2(100.0, 100.0),
            pos2(50.0, 100.0),
            pos2(100.0, 50.0),
        );
        app.calibration.snap_ext = true;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::Y2,
                pos2(70.0, 62.0),
                vec2(200.0, 200.0),
            )
            .expect("rotated ext");
        assert!(matches!(result.snap_kind, CalibrationSnapKind::Extension));
        assert!((result.snapped_pixel.x - 66.0).abs() <= 1.0e-3);
        assert!((result.snapped_pixel.y - 66.0).abs() <= 1.0e-3);
        assert!(result.guides.iter().any(Option::is_some));
    }

    #[test]
    fn endpoint_snap_vh_supports_combined_xy_alignment() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = true;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(100.0, 140.0),
                vec2(200.0, 200.0),
            )
            .expect("vh snap");
        assert!(matches!(
            result.snap_kind,
            CalibrationSnapKind::VerticalHorizontal
        ));
        assert_point_close(result.snapped_pixel, pos2(100.0, 140.0));
    }

    #[test]
    fn endpoint_snap_vh_does_not_duplicate_endpoints_when_end_disabled() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = true;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let result = app.endpoint_snap_reference(
            CartesianEndpointId::X1,
            pos2(80.0, 140.0),
            vec2(200.0, 200.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn endpoint_snap_end_respects_radius() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = true;
        app.calibration.snap_int = false;

        let near = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(82.0, 138.0),
                vec2(200.0, 200.0),
            )
            .expect("end snap");
        assert!(matches!(near.snap_kind, CalibrationSnapKind::End));
        assert_point_close(near.snapped_pixel, pos2(80.0, 140.0));

        let far = app.endpoint_snap_reference(
            CartesianEndpointId::X1,
            pos2(10.0, 10.0),
            vec2(200.0, 200.0),
        );
        assert!(far.is_none());
    }

    #[test]
    fn endpoint_snap_end_uses_narrow_radius() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(160.0, 60.0),
            pos2(160.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = true;
        app.calibration.snap_int = false;
        let result = app.endpoint_snap_reference(
            CartesianEndpointId::X1,
            pos2(108.0, 100.0),
            vec2(220.0, 220.0),
        );
        assert!(result.is_none());
    }

    #[test]
    fn endpoint_snap_int_uses_wider_radius() {
        let mut app = cartesian_app(
            pos2(50.0, 120.0),
            pos2(100.0, 120.0),
            pos2(130.0, 80.0),
            pos2(130.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = true;
        app.calibration.snap_int = true;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(118.0, 120.0),
                vec2(220.0, 220.0),
            )
            .expect("int snap");
        assert!(matches!(
            result.snap_kind,
            CalibrationSnapKind::Intersection
        ));
    }

    #[test]
    fn endpoint_snap_int_without_competition_is_not_sticky() {
        let mut app = cartesian_app(
            pos2(50.0, 120.0),
            pos2(100.0, 120.0),
            pos2(130.0, 80.0),
            pos2(130.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = true;
        app.calibration.calibration_angle_snap = false;
        let bounds = vec2(220.0, 220.0);

        let first = app.calibration_snap_result(CalTarget::X1, pos2(129.0, 119.0), bounds);
        assert!(matches!(first.snap_kind, CalibrationSnapKind::Intersection));

        let second = app.calibration_snap_result(CalTarget::X1, pos2(147.0, 120.0), bounds);
        assert!(matches!(second.snap_kind, CalibrationSnapKind::None));
    }

    #[test]
    fn endpoint_snap_int_is_sticky_with_competition() {
        let mut app = cartesian_app(
            pos2(50.0, 120.0),
            pos2(100.0, 120.0),
            pos2(130.0, 80.0),
            pos2(130.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = true;
        app.calibration.snap_end = false;
        app.calibration.snap_int = true;
        app.calibration.calibration_angle_snap = false;
        let bounds = vec2(220.0, 220.0);

        let first = app.calibration_snap_result(CalTarget::X1, pos2(130.0, 120.0), bounds);
        assert!(matches!(first.snap_kind, CalibrationSnapKind::Intersection));

        let sticky = app.calibration_snap_result(CalTarget::X1, pos2(147.0, 120.0), bounds);
        assert!(matches!(
            sticky.snap_kind,
            CalibrationSnapKind::Intersection
        ));

        let released = app.calibration_snap_result(CalTarget::X1, pos2(151.0, 170.0), bounds);
        assert!(matches!(released.snap_kind, CalibrationSnapKind::None));
    }

    #[test]
    fn endpoint_snap_intersection_and_guides_work() {
        let mut app = cartesian_app(
            pos2(50.0, 120.0),
            pos2(100.0, 120.0),
            pos2(130.0, 80.0),
            pos2(130.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = true;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(129.0, 119.0),
                vec2(220.0, 220.0),
            )
            .expect("intersection snap");
        assert!(matches!(
            result.snap_kind,
            CalibrationSnapKind::Intersection
        ));
        assert_point_close(result.snapped_pixel, pos2(130.0, 120.0));
        let guide = result.guides[0].expect("intersection guide");
        assert_point_close(guide.start, pos2(100.0, 120.0));
        assert_point_close(guide.end, pos2(130.0, 120.0));
        assert!(result.guides[1].is_none());
    }

    #[test]
    fn endpoint_snap_tie_prefers_end_over_vh() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = true;
        app.calibration.snap_end = true;
        app.calibration.snap_int = false;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(80.0, 80.0),
                vec2(200.0, 200.0),
            )
            .expect("tie");
        assert!(matches!(result.snap_kind, CalibrationSnapKind::End));
        assert_point_close(result.snapped_pixel, pos2(80.0, 80.0));
    }

    #[test]
    fn endpoint_snap_end_keeps_intersection_guide_on_same_point() {
        let mut app = cartesian_app(
            pos2(50.0, 120.0),
            pos2(130.0, 120.0),
            pos2(130.0, 80.0),
            pos2(130.0, 100.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = true;
        app.calibration.snap_int = true;
        let result = app
            .endpoint_snap_reference(
                CartesianEndpointId::X1,
                pos2(129.0, 119.0),
                vec2(220.0, 220.0),
            )
            .expect("end/int tie");
        assert!(matches!(result.snap_kind, CalibrationSnapKind::End));
        assert_point_close(result.snapped_pixel, pos2(130.0, 120.0));
        let guide = result.guides[0].expect("intersection guide kept");
        assert_point_close(guide.start, pos2(130.0, 100.0));
        assert_point_close(guide.end, pos2(130.0, 120.0));
    }

    #[test]
    fn endpoint_snap_falls_back_to_15_deg_when_no_candidate() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        app.calibration.calibration_angle_snap = true;
        let pointer = pos2(77.0, 117.0);
        let expected =
            app.snap_calibration_angle(pointer, app.calibration.cal_x.p2, vec2(200.0, 200.0));
        let result = app.calibration_snap_result(CalTarget::X1, pointer, vec2(200.0, 200.0));
        assert!(matches!(result.snap_kind, CalibrationSnapKind::Angle15));
        assert_point_close(result.snapped_pixel, expected);
        assert!(result.guides.iter().all(Option::is_none));
    }

    #[test]
    fn apply_calibration_point_keeps_guides_for_snaps_with_guides() {
        let mut app = cartesian_app(
            pos2(50.0, 100.0),
            pos2(100.0, 100.0),
            pos2(80.0, 80.0),
            pos2(80.0, 140.0),
        );
        app.calibration.snap_ext = true;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        let mut x_mapping = app.calibration.cal_x.mapping();
        let mut y_mapping = app.calibration.cal_y.mapping();
        let mut polar_mapping = None;
        app.apply_calibration_point(
            CalTarget::X1,
            pos2(40.0, 101.0),
            vec2(200.0, 200.0),
            super::CalUpdateMode::Drag,
            &mut x_mapping,
            &mut y_mapping,
            &mut polar_mapping,
        );
        assert!(guide_count(&app) > 0);

        app.calibration.snap_ext = false;
        app.calibration.snap_vh = true;
        app.apply_calibration_point(
            CalTarget::X1,
            pos2(98.0, 138.0),
            vec2(200.0, 200.0),
            super::CalUpdateMode::Drag,
            &mut x_mapping,
            &mut y_mapping,
            &mut polar_mapping,
        );
        assert!(guide_count(&app) > 0);

        app.calibration.snap_ext = false;
        app.calibration.snap_vh = false;
        app.calibration.snap_end = false;
        app.calibration.snap_int = false;
        app.calibration.calibration_angle_snap = false;
        app.apply_calibration_point(
            CalTarget::X1,
            pos2(97.0, 137.0),
            vec2(200.0, 200.0),
            super::CalUpdateMode::Drag,
            &mut x_mapping,
            &mut y_mapping,
            &mut polar_mapping,
        );
        assert_eq!(guide_count(&app), 0);
    }
}
