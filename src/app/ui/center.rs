use super::super::export_helpers::format_overlay_value;
use super::super::{
    AxisValueField, CurcatApp, DragTarget, PickMode, PointInputMode, safe_usize_to_f32,
};
use egui::{Color32, Key, PointerButton, Pos2, Sense, Vec2, pos2};

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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_central_image(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
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
                let (shift_pressed, primary_down, delete_down, ctrl_pressed) =
                    ui.ctx().input(|i| {
                        (
                            i.modifiers.shift,
                            i.pointer.button_down(PointerButton::Primary),
                            i.key_down(Key::Delete),
                            i.modifiers.ctrl,
                        )
                    });
                let pointer_pixel = hover_pos.map(&to_pixel);
                let snap_preview = if !matches!(self.point_input_mode, PointInputMode::Free)
                    && !matches!(self.pick_mode, PickMode::CurveColor)
                    && let Some(pixel) = pointer_pixel
                {
                    self.compute_snap_candidate(pixel)
                } else {
                    None
                };

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
                } else if response.clicked_by(PointerButton::Primary)
                    && !shift_pressed
                    && let Some(pos) = pointer_pos
                {
                    if delete_down {
                        let mut best: Option<(usize, f32)> = None;
                        for (idx, point) in self.points.iter().enumerate() {
                            let screen = rect.min + point.pixel.to_vec2() * self.image_zoom;
                            let dist = pos.distance(screen);
                            if dist <= super::super::POINT_HIT_RADIUS
                                && best.as_ref().is_none_or(|(_, best_dist)| dist < *best_dist)
                            {
                                best = Some((idx, dist));
                            }
                        }
                        if let Some((idx, _)) = best {
                            self.points.remove(idx);
                            self.mark_points_dirty();
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
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_x.p2, base_size);
                                self.cal_x.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                                self.queue_value_focus(AxisValueField::X1);
                            }
                            PickMode::X2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_x.p1, base_size);
                                self.cal_x.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                x_mapping = self.cal_x.mapping();
                                self.queue_value_focus(AxisValueField::X2);
                            }
                            PickMode::Y1 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_y.p2, base_size);
                                self.cal_y.p1 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                                self.queue_value_focus(AxisValueField::Y1);
                            }
                            PickMode::Y2 => {
                                let snapped = self.snap_pixel_if_requested(pixel);
                                let snapped =
                                    self.snap_calibration_angle(snapped, self.cal_y.p1, base_size);
                                self.cal_y.p2 = Some(snapped);
                                self.pick_mode = PickMode::None;
                                y_mapping = self.cal_y.mapping();
                                self.queue_value_focus(AxisValueField::Y2);
                            }
                            PickMode::CurveColor => {
                                self.pick_curve_color_at(pixel);
                                self.pick_mode = PickMode::None;
                            }
                        }
                    }
                }

                self.ensure_point_numeric_cache(x_mapping.as_ref(), y_mapping.as_ref());

                // Draw picked calibration points lines
                let stroke_cal_outline = egui::Stroke {
                    width: super::super::CAL_LINE_OUTLINE_WIDTH,
                    color: Color32::from_black_alpha(super::super::CAL_OUTLINE_ALPHA),
                };
                let stroke_cal = egui::Stroke {
                    width: super::super::CAL_LINE_WIDTH,
                    color: Color32::LIGHT_BLUE,
                };
                let cal_point_color = stroke_cal.color;
                let draw_cal_point = |point: Pos2| {
                    let screen = rect.min + point.to_vec2() * self.image_zoom;
                    painter.circle_filled(
                        screen,
                        super::super::CAL_POINT_DRAW_RADIUS + super::super::CAL_POINT_OUTLINE_PAD,
                        stroke_cal_outline.color,
                    );
                    painter.circle_filled(
                        screen,
                        super::super::CAL_POINT_DRAW_RADIUS,
                        cal_point_color,
                    );
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
                        let mut label_pos = pos2(total.x.mul_add(-0.5, pos.x), clip.top() + 4.0);
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
                        let galley = painter.layout_no_wrap(text, font, text_color);
                        let size = galley.size();
                        let total = size + padding * 2.0;
                        let mut label_pos = pos2(clip.left() + 4.0, total.y.mul_add(-0.5, pos.y));
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

                    if let Some(icon) = if matches!(self.pick_mode, PickMode::CurveColor) {
                        Some("🧪")
                    } else if delete_down {
                        Some("🗑")
                    } else if shift_pressed {
                        Some("✋")
                    } else if ctrl_pressed {
                        Some("🔍")
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
                        let icon_pos = pos2(
                            icon_size.x.mul_add(-0.5, anchor.x),
                            icon_size.y.mul_add(-0.5, anchor.y),
                        );
                        painter.galley(icon_pos, icon_galley, Color32::WHITE);
                    }
                }
            });
        } else if self.pending_image_task.is_some() {
            ui.centered_and_justified(|ui| {
                ui.label("Loading image…");
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Drop an image here or use Open image…");
            });
        }
    }
}
