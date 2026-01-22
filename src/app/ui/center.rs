use super::super::export_helpers::format_overlay_value;
use super::super::{
    AutoPlaceState, AxisValueField, CurcatApp, DragTarget, PickMode, PointInputMode,
    PrimaryPressInfo, safe_usize_to_f32,
};

use crate::types::AxisMapping;
use egui::{Color32, Key, PointerButton, Pos2, Sense, Vec2, pos2};
use std::time::{Duration, Instant};

const LIGHT_DRAG_CLICK_DIST: f32 = 20.0;
const LIGHT_DRAG_CLICK_MAX_DURATION: Duration = Duration::from_millis(400);

impl CurcatApp {
    pub(crate) fn handle_middle_pan(&mut self, response: &egui::Response, ui: &egui::Ui) {
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
                        ui.scroll_with_delta_animation(
                            scroll_delta,
                            egui::style::ScrollAnimation::none(),
                        );
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_central_image(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        self.last_viewport_size = Some(ui.available_size());
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
                if let Some(path) = &f.path {
                    self.start_loading_image_from_path(path.clone());
                    loaded = true;
                    if cfg!(debug_assertions) {
                        eprintln!("[DnD] Loading from path: {}", path.display());
                    }
                    break;
                }
                if let Some(bytes) = &f.bytes {
                    self.start_loading_image_from_bytes(
                        (!f.name.is_empty()).then(|| f.name.clone()),
                        bytes.to_vec(),
                        f.last_modified,
                    );
                    loaded = true;
                    if cfg!(debug_assertions) {
                        eprintln!("[DnD] Loading from dropped bytes: name='{}'", f.name);
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

        if let Some(img) = self.image.as_ref() {
            let mut x_mapping = self.cal_x.mapping();
            let mut y_mapping = self.cal_y.mapping();
            // Take a snapshot of the texture handle and size to avoid borrowing self.image in the UI closure
            let (tex_id, img_size) = (img.texture.id(), img.size);
            let scroll_out = egui::ScrollArea::both()
                .id_salt("image_scroll")
                .scroll_offset(self.image_pan)
                .show(ui, |ui| {
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
                let (
                    shift_pressed,
                    primary_down,
                    primary_pressed,
                    primary_released,
                    delete_down,
                    ctrl_pressed,
                    press_origin,
                    latest_pos,
                ) = ui.ctx().input(|i| {
                    (
                        i.modifiers.shift,
                        i.pointer.button_down(PointerButton::Primary),
                        i.pointer.button_pressed(PointerButton::Primary),
                        i.pointer.button_released(PointerButton::Primary),
                        i.key_down(Key::Delete),
                        i.modifiers.ctrl,
                        i.pointer.press_origin(),
                        i.pointer.latest_pos(),
                    )
                });
                let hover_pos = response.hover_pos().or(latest_pos);
                let pointer_pixel = hover_pos.map(&to_pixel);
                let mut soft_primary_click = false;
                if primary_pressed {
                    if let Some(pos) = press_origin.or(latest_pos) {
                        self.primary_press = Some(PrimaryPressInfo {
                            pos,
                            time: Instant::now(),
                            in_rect: rect.contains(pos),
                            shift_down: shift_pressed,
                        });
                    } else {
                        self.primary_press = None;
                    }
                }
                if primary_released {
                    if let Some(info) = self.primary_press.take()
                        && !info.shift_down
                            && info.in_rect
                            && let Some(release_pos) = latest_pos
                            && rect.contains(release_pos)
                        {
                            let dist = info.pos.distance(release_pos);
                            let elapsed = info.time.elapsed();
                            if dist <= LIGHT_DRAG_CLICK_DIST
                                && elapsed <= LIGHT_DRAG_CLICK_MAX_DURATION
                            {
                                soft_primary_click = true;
                            }
                        }
                } else if !primary_down {
                    self.primary_press = None;
                }
                let response_clicked = response.clicked_by(PointerButton::Primary);
                let primary_clicked = response_clicked || soft_primary_click;
                let click_pos = if response_clicked {
                    pointer_pos.or(hover_pos)
                } else if soft_primary_click {
                    latest_pos.or(pointer_pos)
                } else {
                    None
                };
                let snap_preview = if !matches!(self.point_input_mode, PointInputMode::Free)
                    && !matches!(self.pick_mode, PickMode::CurveColor)
                    && let Some(pixel) = pointer_pixel
                {
                    self.compute_snap_candidate(pixel)
                } else {
                    None
                };

                let suppress_primary_click = self.auto_place_tick(
                    pointer_pixel,
                    primary_down,
                    primary_pressed,
                    shift_pressed,
                    delete_down,
                    x_mapping.as_ref(),
                    y_mapping.as_ref(),
                );

                if primary_down {
                    ui.ctx().request_repaint_after(Duration::from_millis(16));
                }

                if shift_pressed
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
                        let pixel = match target {
                            DragTarget::CurvePoint(_) => pixel,
                            DragTarget::CalX1 => {
                                self.snap_calibration_angle(pixel, self.cal_x.p2, base_size)
                            }
                            DragTarget::CalX2 => {
                                self.snap_calibration_angle(pixel, self.cal_x.p1, base_size)
                            }
                            DragTarget::CalY1 => {
                                self.snap_calibration_angle(pixel, self.cal_y.p2, base_size)
                            }
                            DragTarget::CalY2 => {
                                self.snap_calibration_angle(pixel, self.cal_y.p1, base_size)
                            }
                        };
                        match target {
                            DragTarget::CurvePoint(idx) => {
                                if let Some(point) = self.points.get_mut(idx) {
                                    point.pixel = pixel;
                                    self.mark_points_dirty();
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
                } else if response.clicked_by(PointerButton::Secondary)
                    && matches!(self.pick_mode, PickMode::None)
                    && let Some(pos) = pointer_pos
                {
                    let image_origin = rect.min;
                    self.remove_point_near_screen(pos, image_origin);
                } else if primary_clicked
                    && !suppress_primary_click
                    && !shift_pressed
                    && let Some(pos) = click_pos
                {
                    if delete_down {
                        let image_origin = rect.min;
                        self.remove_point_near_screen(pos, image_origin);
                    } else {
                        let pixel = to_pixel(pos);
                        match self.pick_mode {
                            PickMode::None => {
                                if x_mapping.is_some() && y_mapping.is_some() {
                                    self.push_curve_point(pixel);
                                } else {
                                    self.set_status(
                                        "Calibration incomplete: set both X and Y axes before picking points.",
                                    );
                                }
                            }
                            PickMode::X1 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_x.p2, base_size);
                                self.cal_x.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                                self.queue_value_focus(AxisValueField::X1);
                                self.set_status("Picked X1.");
                            }
                            PickMode::X2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_x.p1, base_size);
                                self.cal_x.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                                self.queue_value_focus(AxisValueField::X2);
                                self.set_status("Picked X2.");
                            }
                            PickMode::Y1 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_y.p2, base_size);
                                self.cal_y.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                                self.queue_value_focus(AxisValueField::Y1);
                                self.set_status("Picked Y1.");
                            }
                            PickMode::Y2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_y.p1, base_size);
                                self.cal_y.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                                self.queue_value_focus(AxisValueField::Y2);
                                self.set_status("Picked Y2.");
                            }
                            PickMode::CurveColor => {
                                self.pick_curve_color_at(pixel);
                                self.pick_mode = PickMode::None;
                            }
                        }
                    }
                }

                self.ensure_point_numeric_cache(x_mapping.as_ref(), y_mapping.as_ref());

                // Draw picked calibration overlays (lines, points, labels)
                if self.show_calibration_segments {
                    let stroke_cal_outline = egui::Stroke {
                        width: super::super::CAL_LINE_OUTLINE_WIDTH,
                        color: Color32::from_black_alpha(super::super::CAL_OUTLINE_ALPHA),
                    };
                    let stroke_cal = egui::Stroke {
                        width: super::super::CAL_LINE_WIDTH,
                        color: Color32::LIGHT_BLUE,
                    };
                    let cal_point_color = stroke_cal.color;
                    let cal_radius =
                        super::super::CAL_POINT_DRAW_RADIUS + super::super::CAL_POINT_OUTLINE_PAD;
                    let cal_label_shadow = Color32::from_black_alpha(160);
                    let cal_label_font = egui::FontId::monospace(11.0);
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
                        let dir_screen = (p2 - p1) * self.image_zoom;
                        if dir_screen.length_sq() <= f32::EPSILON {
                            return None;
                        }
                        Some(Vec2::new(-dir_screen.y, dir_screen.x).normalized())
                    };
                    let x_normal = calc_label_normal(self.cal_x.p1, self.cal_x.p2);
                    let y_normal = calc_label_normal(self.cal_y.p1, self.cal_y.p2);
                    let draw_cal_point =
                        |point: Pos2, label: &str, normal: Option<Vec2>, flip_side: bool| {
                            let screen = rect.min + point.to_vec2() * self.image_zoom;
                            let dir = normal.unwrap_or(default_dir);
                            let dir = if flip_side { -dir } else { dir };
                            let galley = painter.layout_no_wrap(
                                label.to_owned(),
                                cal_label_font.clone(),
                                cal_point_color,
                            );
                            let offset = galley.size().y.mul_add(0.5, cal_radius + label_gap_px);
                            let label_center = screen + dir * offset;
                            let label_pos = label_center - galley.size() * 0.5;
                            painter.circle_filled(screen, cal_radius, stroke_cal_outline.color);
                            painter.circle_filled(
                                screen,
                                super::super::CAL_POINT_DRAW_RADIUS,
                                cal_point_color,
                            );
                            let shadow_pos = label_pos + Vec2::splat(1.0);
                            painter.galley(shadow_pos, galley.clone(), cal_label_shadow);
                            painter.galley(label_pos, galley, cal_point_color);
                        };
                    let draw_cal_line = |p1: Pos2, p2: Pos2| {
                        let line = [
                            rect.min + p1.to_vec2() * self.image_zoom,
                            rect.min + p2.to_vec2() * self.image_zoom,
                        ];
                        painter.line_segment(line, stroke_cal_outline);
                        painter.line_segment(line, stroke_cal);
                    };
                    if let Some(p1) = self.cal_x.p1
                        && let Some(p2) = self.cal_x.p2
                    {
                        draw_cal_line(p1, p2);
                    }
                    if let Some(p1) = self.cal_y.p1
                        && let Some(p2) = self.cal_y.p2
                    {
                        draw_cal_line(p1, p2);
                    }
                    if let Some(p) = self.cal_x.p1 {
                        draw_cal_point(p, "X1", x_normal, false);
                    }
                    if let Some(p) = self.cal_x.p2 {
                        draw_cal_point(p, "X2", x_normal, true);
                    }
                    if let Some(p) = self.cal_y.p1 {
                        draw_cal_point(p, "Y1", y_normal, false);
                    }
                    if let Some(p) = self.cal_y.p2 {
                        draw_cal_point(p, "Y2", y_normal, true);
                    }
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

                if matches!(
                    self.point_input_mode,
                    PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
                ) && !matches!(self.pick_mode, PickMode::CurveColor)
                {
                    if let Some(pixel) = pointer_pixel {
                        let screen = rect.min + pixel.to_vec2() * self.image_zoom;
                        let radius = (self.contrast_search_radius * self.image_zoom).max(4.0);
                        painter.circle_stroke(
                            screen,
                            radius,
                            egui::Stroke::new(1.2, self.snap_overlay_color),
                        );
                    }
                    if let Some(preview) = snap_preview {
                        let screen = rect.min + preview.to_vec2() * self.image_zoom;
                        painter.circle_stroke(
                            screen,
                            (point_radius + 4.0).max(6.0),
                            egui::Stroke::new(1.2, self.snap_overlay_color),
                        );
                        painter.circle_filled(screen, 3.0, self.snap_overlay_color);
                    }
                }

                // Draw interpolation preview: connect points sorted by X numeric
                let stroke_curve = self.config.curve_line.stroke();
                let zoom = self.image_zoom;
                let preview_segments = self.sorted_preview_segments();
                if preview_segments.len() >= 2 {
                    for win in preview_segments.windows(2) {
                        let a = rect.min + win[0].1.to_vec2() * zoom;
                        let b = rect.min + win[1].1.to_vec2() * zoom;
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
                    if let Some(ymap) = y_mapping.as_ref()
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

                    let badge_offset = Vec2::new(18.0, -18.0);
                    let badge_anchor = pos + badge_offset;
                    let badge_radius = 12.0;
                    let showed_color_badge = {
                        if matches!(self.pick_mode, PickMode::CurveColor)
                            && let Some(sampled) = self.sample_image_color(pixel) {
                                let [r, g, b, _] = sampled.to_array();
                                let badge_color = Color32::from_rgb(r, g, b);
                                painter.circle_filled(badge_anchor, badge_radius, badge_color);
                                painter.circle_stroke(
                                    badge_anchor,
                                    badge_radius,
                                    egui::Stroke::new(1.0, Color32::from_gray(30)),
                                );
                                true
                            }
                            else{
                                false
                            }
                    };

                    if !showed_color_badge
                        && let Some((icon_text, icon_color)) =
                            self.cursor_badge(delete_down, shift_pressed, ctrl_pressed)
                        {
                            let icon_font = egui::FontId::proportional(15.0);
                            let icon_galley =
                                painter.layout_no_wrap(icon_text.to_string(), icon_font, icon_color);
                            let icon_size = icon_galley.size();
                            let icon_bg = Color32::from_rgba_unmultiplied(0, 0, 0, 160);
                            painter.circle_filled(badge_anchor, badge_radius, icon_bg);
                            let icon_pos = pos2(
                                icon_size.x.mul_add(-0.5, badge_anchor.x),
                                icon_size.y.mul_add(-0.5, badge_anchor.y),
                            );
                            painter.galley(icon_pos, icon_galley, icon_color);
                        }
                }
            });
            self.image_pan = scroll_out.state.offset;
            self.last_viewport_size = Some(scroll_out.inner_rect.size());
        } else if self.pending_image_task.is_some() {
            ui.centered_and_justified(|ui| {
                if let Some(task) = self.pending_image_task.as_ref() {
                    ui.label(format!("Loading image: {}…", task.meta.description()));
                } else {
                    ui.label("Loading image…");
                }
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Drop an image here, open a file, or paste from clipboard (Ctrl+V).");
            });
        }
    }
}

impl CurcatApp {
    const fn cursor_badge(
        &self,
        delete_down: bool,
        shift_pressed: bool,
        ctrl_pressed: bool,
    ) -> Option<(&'static str, Color32)> {
        if let Some(badge) = self.calibration_cursor_badge() {
            return Some(badge);
        }
        if self.auto_place_state.active {
            return Some(("✚", Color32::WHITE));
        }
        if matches!(self.pick_mode, PickMode::CurveColor) {
            return Some(("🧪", Color32::WHITE));
        }
        if delete_down {
            return Some(("🗑", Color32::WHITE));
        }
        if shift_pressed {
            return Some(("✋", Color32::WHITE));
        }
        if ctrl_pressed {
            return Some(("🔍", Color32::WHITE));
        }
        None
    }

    const fn calibration_cursor_badge(&self) -> Option<(&'static str, Color32)> {
        match self.pick_mode {
            PickMode::X1 => Some(("X1", Color32::from_rgb(190, 225, 255))),
            PickMode::X2 => Some(("X2", Color32::from_rgb(190, 225, 255))),
            PickMode::Y1 => Some(("Y1", Color32::from_rgb(200, 255, 200))),
            PickMode::Y2 => Some(("Y2", Color32::from_rgb(200, 255, 200))),
            _ => None,
        }
    }

    fn remove_point_near_screen(&mut self, pointer: Pos2, image_origin: Pos2) -> bool {
        let mut best: Option<(usize, f32)> = None;
        for (idx, point) in self.points.iter().enumerate() {
            let screen = image_origin + point.pixel.to_vec2() * self.image_zoom;
            let dist = pointer.distance(screen);
            if dist <= super::super::POINT_HIT_RADIUS
                && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
            {
                best = Some((idx, dist));
            }
        }
        if let Some((idx, _)) = best {
            self.points.remove(idx);
            self.mark_points_dirty();
            self.set_status("Point removed.");
            true
        } else {
            false
        }
    }
}

impl CurcatApp {
    fn reset_auto_place_runtime(&mut self, keep_suppress: bool) {
        let suppress_click = self.auto_place_state.suppress_click && keep_suppress;
        self.auto_place_state = AutoPlaceState {
            suppress_click,
            ..AutoPlaceState::default()
        };
    }

    #[allow(clippy::too_many_arguments)]
    fn auto_place_tick(
        &mut self,
        pointer_pixel: Option<Pos2>,
        primary_down: bool,
        primary_pressed: bool,
        shift_pressed: bool,
        delete_down: bool,
        x_mapping: Option<&AxisMapping>,
        y_mapping: Option<&AxisMapping>,
    ) -> bool {
        if primary_pressed {
            self.reset_auto_place_runtime(false);
        }

        let mut suppress_click = self.auto_place_state.suppress_click;

        if !primary_down {
            suppress_click = self.auto_place_state.suppress_click;
            self.reset_auto_place_runtime(false);
            return suppress_click;
        }

        if shift_pressed || delete_down || !matches!(self.pick_mode, PickMode::None) {
            self.reset_auto_place_runtime(true);
            return suppress_click;
        }

        if x_mapping.is_none() || y_mapping.is_none() {
            return suppress_click;
        }

        let Some(pixel) = pointer_pixel else {
            self.reset_auto_place_runtime(true);
            return suppress_click;
        };

        let now = Instant::now();
        let cfg = self.auto_place_cfg;

        if self.auto_place_state.hold_started_at.is_none() {
            self.auto_place_state.hold_started_at = Some(now);
            self.auto_place_state.last_pointer = Some((pixel, now));
            self.auto_place_state.pause_started_at = None;
            self.auto_place_state.speed_ewma = 0.0;
        }

        if !self.auto_place_state.active {
            let hold_elapsed = now
                .saturating_duration_since(self.auto_place_state.hold_started_at.unwrap())
                .as_secs_f32();
            if hold_elapsed >= cfg.hold_activation_secs {
                self.auto_place_state.active = true;
                self.auto_place_state.suppress_click = true;
                suppress_click = true;
                self.update_auto_place_speed(pixel, now);
                self.try_auto_place_point(pixel, now);
            }
            return suppress_click;
        }

        self.update_auto_place_speed(pixel, now);
        self.auto_place_state.suppress_click = true;
        let _ = self.try_auto_place_point(pixel, now);
        true
    }

    fn update_auto_place_speed(&mut self, pixel: Pos2, now: Instant) {
        if let Some((prev, prev_time)) = self.auto_place_state.last_pointer {
            let dt = now
                .saturating_duration_since(prev_time)
                .as_secs_f32()
                .max(f32::EPSILON);
            let dist = (pixel - prev).length();
            let inst_speed = dist / dt;
            let alpha = self.auto_place_cfg.speed_smoothing.clamp(0.0, 1.0);
            let prev_speed = self.auto_place_state.speed_ewma;
            self.auto_place_state.speed_ewma =
                if alpha <= f32::EPSILON || !prev_speed.is_finite() || prev_speed <= f32::EPSILON {
                    inst_speed
                } else {
                    prev_speed + alpha * (inst_speed - prev_speed)
                };
        } else {
            self.auto_place_state.speed_ewma = 0.0;
        }
        self.auto_place_state.last_pointer = Some((pixel, now));
    }

    fn try_auto_place_point(&mut self, pointer_pixel: Pos2, now: Instant) -> bool {
        let cfg = self.auto_place_cfg;
        let speed = self.auto_place_state.speed_ewma.max(0.0);
        let distance_threshold =
            (speed * cfg.distance_per_speed).clamp(cfg.distance_min, cfg.distance_max);
        let time_threshold = if speed <= f32::EPSILON {
            cfg.time_max_secs
        } else {
            (cfg.time_per_speed / speed).clamp(cfg.time_min_secs, cfg.time_max_secs)
        };

        let paused = if speed < cfg.pause_speed_threshold {
            let start = self.auto_place_state.pause_started_at.get_or_insert(now);
            now.saturating_duration_since(*start).as_millis() >= u128::from(cfg.pause_timeout_ms)
        } else {
            self.auto_place_state.pause_started_at = None;
            false
        };
        if paused {
            return false;
        }

        let snapped = self.resolve_curve_pick(pointer_pixel);

        if let Some((last_pos, last_time)) = self.auto_place_state.last_snapped_point {
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
        self.auto_place_state.last_snapped_point = Some((snapped, now));
        true
    }
}
