use super::super::common::toggle_switch;
use super::super::icons;
use super::axis_input::sanitize_axis_text;
use crate::app::{APP_VERSION, AxisValueField, CurcatApp, PickMode, safe_usize_to_f32};
use crate::types::{AngleDirection, AngleUnit, AxisUnit, AxisValue, CoordSystem, ScaleKind};
use egui::{Color32, Pos2, Rect, RichText};

#[derive(Clone, Copy)]
enum CalibrationPresetKind {
    Unit,
    Pixels,
}

impl CalibrationPresetKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Unit => "Unit",
            Self::Pixels => "Pixels",
        }
    }

    const fn icon(self) -> &'static str {
        match self {
            Self::Unit => icons::ICON_PRESET_UNIT,
            Self::Pixels => icons::ICON_PRESET_PIXELS,
        }
    }

    const fn hover_text(self) -> &'static str {
        match self {
            Self::Unit => "Quadrant preset (unit): set axes to 0..1 (signed).",
            Self::Pixels => "Quadrant preset (px): set axes to 0..size px (signed).",
        }
    }
}

#[derive(Clone, Copy)]
enum CalibrationQuadrant {
    I,
    II,
    III,
    IV,
}

impl CalibrationQuadrant {
    const ALL: [Self; 4] = [Self::I, Self::II, Self::III, Self::IV];

    const fn label(self) -> &'static str {
        match self {
            Self::I => "I",
            Self::II => "II",
            Self::III => "III",
            Self::IV => "IV",
        }
    }

    const fn hint(self) -> &'static str {
        match self {
            Self::I => "Axes: bottom + left (x>=0, y>=0)",
            Self::II => "Axes: bottom + right (x<=0, y>=0)",
            Self::III => "Axes: top + right (x<=0, y<=0)",
            Self::IV => "Axes: top + left (x>=0, y<=0)",
        }
    }

    const fn axis_on_bottom(self) -> bool {
        matches!(self, Self::I | Self::II)
    }

    const fn axis_on_left(self) -> bool {
        matches!(self, Self::I | Self::IV)
    }
}

#[derive(Clone, Copy)]
enum PolarAxisKind {
    Radius,
    Angle,
}

impl PolarAxisKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Radius => "Radius",
            Self::Angle => "Angle",
        }
    }

    const fn p1_label(self) -> &'static str {
        match self {
            Self::Radius => "R1",
            Self::Angle => "A1",
        }
    }

    const fn p2_label(self) -> &'static str {
        match self {
            Self::Radius => "R2",
            Self::Angle => "A2",
        }
    }
}

struct CalibrationUiState {
    highlight_jobs: Vec<(Rect, bool)>,
    pending_focus: Option<AxisValueField>,
    pending_pick: Option<PickMode>,
}

impl CalibrationUiState {
    fn new(pending_focus: Option<AxisValueField>) -> Self {
        Self {
            highlight_jobs: Vec::new(),
            pending_focus,
            pending_pick: None,
        }
    }
}

impl CurcatApp {
    pub(crate) fn ui_side_calibration(&mut self, ui: &mut egui::Ui) {
        self.ui_point_input_section(ui);
        ui.separator();

        ui.heading("Calibration");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Coordinate system:")
                .on_hover_text("Choose between Cartesian (X/Y) or Polar (angle/radius)");
            let mut system = self.calibration.coord_system;
            let resp = egui::ComboBox::from_id_salt("coord_system_combo")
                .selected_text(match system {
                    CoordSystem::Cartesian => "Cartesian",
                    CoordSystem::Polar => "Polar",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut system, CoordSystem::Cartesian, "Cartesian");
                    ui.selectable_value(&mut system, CoordSystem::Polar, "Polar");
                });
            resp.response
                .on_hover_text("Coordinate system for calibration and export");
            if system != self.calibration.coord_system {
                self.calibration.coord_system = system;
                self.mark_points_dirty();
                self.calibration.pick_mode = PickMode::None;
                self.calibration.pending_value_focus = None;
                self.set_status(match system {
                    CoordSystem::Cartesian => "Switched to Cartesian calibration.",
                    CoordSystem::Polar => "Switched to Polar calibration.",
                });
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.calibration.calibration_angle_snap)
                .on_hover_text("Snap calibration lines to 15° steps while picking or dragging");
            ui.add_space(4.0);
            ui.label("15° snap")
                .on_hover_text("Snap calibration lines to 15° steps while picking or dragging");
            ui.add_space(8.0);
            let has_image = self.image.image.is_some();
            if matches!(self.calibration.coord_system, CoordSystem::Cartesian) {
                self.ui_quadrant_preset_menu(ui, CalibrationPresetKind::Unit, has_image);
                self.ui_quadrant_preset_menu(ui, CalibrationPresetKind::Pixels, has_image);
            }
        });
        ui.separator();

        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                self.axis_cal_group(ui, true);
                ui.separator();
                self.axis_cal_group(ui, false);
            }
            CoordSystem::Polar => {
                self.ui_polar_origin_row(ui);
                ui.separator();
                self.polar_axis_group(ui, PolarAxisKind::Radius);
                ui.separator();
                self.polar_axis_group(ui, PolarAxisKind::Angle);
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.calibration.show_calibration_segments)
                .on_hover_text("Show calibration lines and point labels on the image");
            ui.add_space(4.0);
            ui.label("Show calibration overlay")
                .on_hover_text("Show or hide calibration lines and point labels on the image");
        });
        ui.separator();
        self.ui_export_section(ui);

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

    fn ui_quadrant_preset_menu(
        &mut self,
        ui: &mut egui::Ui,
        preset: CalibrationPresetKind,
        enabled: bool,
    ) {
        ui.add_enabled_ui(enabled, |ui| {
            let menu = ui.menu_button(preset.icon(), |ui| {
                for quadrant in CalibrationQuadrant::ALL {
                    let resp = ui.button(quadrant.label()).on_hover_text(quadrant.hint());
                    if resp.clicked() {
                        self.apply_calibration_preset(preset, quadrant);
                        ui.close();
                    }
                }
            });
            menu.response.on_hover_text(preset.hover_text());
        });
    }

    fn apply_calibration_preset(
        &mut self,
        preset: CalibrationPresetKind,
        quadrant: CalibrationQuadrant,
    ) {
        let (width, height) = if let Some(image) = self.image.image.as_ref() {
            (
                safe_usize_to_f32(image.size[0]),
                safe_usize_to_f32(image.size[1]),
            )
        } else {
            self.set_status("Load an image before applying calibration presets.");
            return;
        };
        if width <= f32::EPSILON || height <= f32::EPSILON {
            self.set_status("Image dimensions are invalid for presets.");
            return;
        }

        let axis_on_bottom = quadrant.axis_on_bottom();
        let axis_on_left = quadrant.axis_on_left();

        let x_axis_y = if axis_on_bottom { height } else { 0.0 };
        let y_axis_x = if axis_on_left { 0.0 } else { width };

        let x_end = if axis_on_left { width } else { 0.0 };
        let y_end = if axis_on_bottom { 0.0 } else { height };

        let x_sign = if axis_on_left { 1.0 } else { -1.0 };
        let y_sign = if axis_on_bottom { 1.0 } else { -1.0 };

        let (span_x, span_y) = match preset {
            CalibrationPresetKind::Unit => (1.0, 1.0),
            CalibrationPresetKind::Pixels => (f64::from(width), f64::from(height)),
        };

        let origin = Pos2::new(y_axis_x, x_axis_y);

        self.calibration.cal_x.unit = AxisUnit::Float;
        self.calibration.cal_x.scale = ScaleKind::Linear;
        self.calibration.cal_x.p1 = Some(origin);
        self.calibration.cal_x.p2 = Some(Pos2::new(x_end, x_axis_y));
        self.calibration.cal_x.v1_text = Self::format_preset_value(0.0);
        self.calibration.cal_x.v2_text = Self::format_preset_value(span_x * x_sign);

        self.calibration.cal_y.unit = AxisUnit::Float;
        self.calibration.cal_y.scale = ScaleKind::Linear;
        self.calibration.cal_y.p1 = Some(origin);
        self.calibration.cal_y.p2 = Some(Pos2::new(y_axis_x, y_end));
        self.calibration.cal_y.v1_text = Self::format_preset_value(0.0);
        self.calibration.cal_y.v2_text = Self::format_preset_value(span_y * y_sign);

        self.calibration.pick_mode = PickMode::None;
        self.calibration.pending_value_focus = None;
        self.calibration.dragging_handle = None;
        self.mark_points_dirty();
        self.set_status(format!(
            "Applied calibration preset: quadrant {} ({})",
            quadrant.label(),
            preset.label()
        ));
    }

    fn format_preset_value(value: f64) -> String {
        AxisValue::Float(value).format()
    }

    fn finish_calibration_panel(
        &mut self,
        ui: &mut egui::Ui,
        state: CalibrationUiState,
        mapping_ready: bool,
        ok_label: &str,
        ok_hover: &str,
        warn_label: &str,
        warn_hover: &str,
    ) {
        if let Some(mode) = state.pending_pick {
            self.begin_pick_mode(mode);
        }
        self.calibration.pending_value_focus = state.pending_focus;

        for (rect, active) in state.highlight_jobs {
            self.paint_attention_outline_if(ui, rect, active);
        }

        if mapping_ready {
            ui.label(RichText::new(ok_label).color(Color32::GREEN))
                .on_hover_text(ok_hover);
        } else {
            ui.label(RichText::new(warn_label).color(Color32::GRAY))
                .on_hover_text(warn_hover);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn axis_cal_group(&mut self, ui: &mut egui::Ui, is_x: bool) {
        let (label, p1_mode, p2_mode, p1_name, p2_name) = if is_x {
            ("X axis", PickMode::X1, PickMode::X2, "X1", "X2")
        } else {
            ("Y axis", PickMode::Y1, PickMode::Y2, "Y1", "Y2")
        };

        let collapsing = egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id(label, |ui| {
                    let mut ui_state =
                        CalibrationUiState::new(self.calibration.pending_value_focus);
                    let mapping_ready;
                    {
                        let cal = if is_x {
                            &mut self.calibration.cal_x
                        } else {
                            &mut self.calibration.cal_y
                        };
                        let previous_unit = cal.unit;
                        ui.horizontal(|ui| {
                            ui.label("Unit:")
                                .on_hover_text("Value type for the axis (Float/DateTime)");
                            let mut unit = cal.unit;
                            let unit_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_unit_combo"))
                                    .selected_text(match unit {
                                        AxisUnit::Float => "Float",
                                        AxisUnit::DateTime => "DateTime",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut unit, AxisUnit::Float, "Float");
                                        ui.selectable_value(
                                            &mut unit,
                                            AxisUnit::DateTime,
                                            "DateTime",
                                        );
                                    });
                            unit_ir.response.on_hover_text("Choose the axis value type");
                            cal.unit = unit;
                            ui.separator();

                            ui.label("Scale:")
                                .on_hover_text("Axis scale (Linear/Log10)");
                            let mut scale = cal.scale;
                            let allow_log = matches!(cal.unit, AxisUnit::Float);
                            let scale_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_scale_combo"))
                                    .selected_text(match scale {
                                        ScaleKind::Linear => "Linear",
                                        ScaleKind::Log10 => "Log10",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut scale,
                                            ScaleKind::Linear,
                                            "Linear",
                                        );
                                        if allow_log {
                                            ui.selectable_value(
                                                &mut scale,
                                                ScaleKind::Log10,
                                                "Log10",
                                            );
                                        }
                                    });
                            scale_ir.response.on_hover_text("Choose the axis scale");
                            if !allow_log && matches!(scale, ScaleKind::Log10) {
                                scale = ScaleKind::Linear;
                            }
                            cal.scale = scale;
                        });
                        if cal.unit != previous_unit {
                            sanitize_axis_text(&mut cal.v1_text, cal.unit);
                            sanitize_axis_text(&mut cal.v2_text, cal.unit);
                        }

                        let p1_row = Self::render_calibration_row(
                            ui,
                            p1_name,
                            cal.unit,
                            &mut cal.v1_text,
                            if is_x {
                                AxisValueField::X1
                            } else {
                                AxisValueField::Y1
                            },
                            &mut ui_state.pending_focus,
                            p1_mode,
                            cal.p1,
                        );
                        let p2_row = Self::render_calibration_row(
                            ui,
                            p2_name,
                            cal.unit,
                            &mut cal.v2_text,
                            if is_x {
                                AxisValueField::X2
                            } else {
                                AxisValueField::Y2
                            },
                            &mut ui_state.pending_focus,
                            p2_mode,
                            cal.p2,
                        );
                        if let Some(mode) = p1_row.requested_pick.or(p2_row.requested_pick) {
                            ui_state.pending_pick = Some(mode);
                        }

                        let (p1_value_invalid, p2_value_invalid) = cal.value_invalid_flags();
                        if let Some(rect) = p1_row.value_rect {
                            ui_state.highlight_jobs.push((rect, p1_value_invalid));
                        }
                        if let Some(rect) = p2_row.value_rect {
                            ui_state.highlight_jobs.push((rect, p2_value_invalid));
                        }
                        if let Some(rect) = p1_row.pick_rect {
                            ui_state.highlight_jobs.push((rect, cal.p1.is_none()));
                        }
                        if let Some(rect) = p2_row.pick_rect {
                            ui_state.highlight_jobs.push((rect, cal.p2.is_none()));
                        }

                        mapping_ready = cal.mapping().is_some();
                    }
                    self.finish_calibration_panel(
                        ui,
                        ui_state,
                        mapping_ready,
                        "Mapping: OK",
                        "Calibration complete — you can pick points and export",
                        "Mapping: incomplete or invalid",
                        "Provide two points and valid values to calibrate",
                    );
                });
            });
        collapsing.header_response.on_hover_text(if is_x {
            "X axis calibration"
        } else {
            "Y axis calibration"
        });
    }

    fn ui_polar_origin_row(&mut self, ui: &mut egui::Ui) {
        let has_image = self.image.image.is_some();
        let mut pick_rect = None;
        ui.horizontal(|ui| {
            ui.label("Origin:")
                .on_hover_text("Pick the pole (origin) for polar coordinates");
            let pick_resp = ui
                .add_enabled(
                    has_image,
                    egui::Button::new(format!("{} Pick Origin", icons::ICON_PICK_POINT)),
                )
                .on_hover_text("Click, then pick the origin on the image");
            if pick_resp.clicked() {
                self.begin_pick_mode(PickMode::Origin);
            }
            pick_rect = Some(pick_resp.rect);

            let center_resp = ui
                .add_enabled(has_image, egui::Button::new("Center"))
                .on_hover_text("Set origin to image center");
            if center_resp.clicked()
                && let Some(image) = self.image.image.as_ref()
            {
                let cx = safe_usize_to_f32(image.size[0]) * 0.5;
                let cy = safe_usize_to_f32(image.size[1]) * 0.5;
                self.calibration.polar_cal.origin = Some(Pos2::new(cx, cy));
                self.mark_points_dirty();
                self.set_status("Origin set to image center.");
            }

            if let Some(p) = self.calibration.polar_cal.origin {
                ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
            }
        });
        if let Some(rect) = pick_rect {
            self.paint_attention_outline_if(ui, rect, self.calibration.polar_cal.origin.is_none());
        }
    }

    #[allow(clippy::too_many_lines)]
    fn polar_axis_group(&mut self, ui: &mut egui::Ui, kind: PolarAxisKind) {
        let label = kind.label();
        let (p1_mode, p2_mode, p1_field, p2_field) = match kind {
            PolarAxisKind::Radius => (
                PickMode::R1,
                PickMode::R2,
                AxisValueField::R1,
                AxisValueField::R2,
            ),
            PolarAxisKind::Angle => (
                PickMode::A1,
                PickMode::A2,
                AxisValueField::A1,
                AxisValueField::A2,
            ),
        };
        let collapsing = egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id(label, |ui| {
                    let mut ui_state =
                        CalibrationUiState::new(self.calibration.pending_value_focus);

                    let cal = match kind {
                        PolarAxisKind::Radius => &mut self.calibration.polar_cal.radius,
                        PolarAxisKind::Angle => &mut self.calibration.polar_cal.angle,
                    };
                    let previous_unit = cal.unit;
                    cal.unit = AxisUnit::Float;
                    if cal.unit != previous_unit {
                        sanitize_axis_text(&mut cal.v1_text, cal.unit);
                        sanitize_axis_text(&mut cal.v2_text, cal.unit);
                    }

                    if matches!(kind, PolarAxisKind::Radius) {
                        ui.horizontal(|ui| {
                            ui.label("Scale:")
                                .on_hover_text("Radius scale (Linear/Log10)");
                            let mut scale = cal.scale;
                            let scale_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_scale_combo"))
                                    .selected_text(match scale {
                                        ScaleKind::Linear => "Linear",
                                        ScaleKind::Log10 => "Log10",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut scale,
                                            ScaleKind::Linear,
                                            "Linear",
                                        );
                                        ui.selectable_value(&mut scale, ScaleKind::Log10, "Log10");
                                    });
                            scale_ir.response.on_hover_text("Choose the radius scale");
                            cal.scale = scale;
                        });
                    } else {
                        cal.scale = ScaleKind::Linear;
                        ui.horizontal(|ui| {
                            ui.label("Angle unit:")
                                .on_hover_text("Units for angle values (degrees or radians)");
                            let mut unit = self.calibration.polar_cal.angle_unit;
                            egui::ComboBox::from_id_salt(format!("{label}_unit_combo"))
                                .selected_text(match unit {
                                    AngleUnit::Degrees => "Degrees",
                                    AngleUnit::Radians => "Radians",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut unit, AngleUnit::Degrees, "Degrees");
                                    ui.selectable_value(&mut unit, AngleUnit::Radians, "Radians");
                                });
                            self.calibration.polar_cal.angle_unit = unit;
                            ui.separator();
                            ui.label("Direction:")
                                .on_hover_text("Direction of increasing angle");
                            let mut direction = self.calibration.polar_cal.angle_direction;
                            egui::ComboBox::from_id_salt(format!("{label}_dir_combo"))
                                .selected_text(direction.label())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut direction, AngleDirection::Ccw, "CCW");
                                    ui.selectable_value(&mut direction, AngleDirection::Cw, "CW");
                                });
                            self.calibration.polar_cal.angle_direction = direction;
                        });
                    }

                    let p1_row = Self::render_calibration_row(
                        ui,
                        kind.p1_label(),
                        AxisUnit::Float,
                        &mut cal.v1_text,
                        p1_field,
                        &mut ui_state.pending_focus,
                        p1_mode,
                        cal.p1,
                    );
                    let p2_row = Self::render_calibration_row(
                        ui,
                        kind.p2_label(),
                        AxisUnit::Float,
                        &mut cal.v2_text,
                        p2_field,
                        &mut ui_state.pending_focus,
                        p2_mode,
                        cal.p2,
                    );
                    if let Some(mode) = p1_row.requested_pick.or(p2_row.requested_pick) {
                        ui_state.pending_pick = Some(mode);
                    }

                    let (p1_invalid, p2_invalid) = cal.value_invalid_flags();
                    if let Some(rect) = p1_row.value_rect {
                        ui_state.highlight_jobs.push((rect, p1_invalid));
                    }
                    if let Some(rect) = p2_row.value_rect {
                        ui_state.highlight_jobs.push((rect, p2_invalid));
                    }
                    if let Some(rect) = p1_row.pick_rect {
                        ui_state.highlight_jobs.push((rect, cal.p1.is_none()));
                    }
                    if let Some(rect) = p2_row.pick_rect {
                        ui_state.highlight_jobs.push((rect, cal.p2.is_none()));
                    }

                    let origin_ready = self.calibration.polar_cal.origin.is_some();
                    let values_ready = !p1_invalid && !p2_invalid;
                    let points_ready = cal.p1.is_some() && cal.p2.is_some();
                    let mapping_ready = origin_ready && values_ready && points_ready;

                    self.finish_calibration_panel(
                        ui,
                        ui_state,
                        mapping_ready,
                        "Mapping: OK",
                        "Calibration complete for this axis",
                        "Mapping: incomplete or invalid",
                        "Provide origin, two points, and valid values",
                    );
                });
            });
        collapsing.header_response.on_hover_text(match kind {
            PolarAxisKind::Radius => "Radius calibration",
            PolarAxisKind::Angle => "Angle calibration",
        });
    }
}
