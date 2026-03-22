use super::super::common::{side_section_card, toggle_switch};
use super::super::icons;
use super::axis_input::sanitize_axis_text;
use crate::app::{AxisCalUi, AxisValueField, CurcatApp, PickMode, safe_usize_to_f32};
use crate::i18n::{TextKey, UiLanguage};
use crate::types::{AngleDirection, AngleUnit, AxisUnit, AxisValue, CoordSystem, ScaleKind};
use egui::containers::menu::MenuButton;
use egui::{Color32, Pos2, Rect, RichText};

#[derive(Clone, Copy)]
enum CalibrationPresetKind {
    Unit,
    Pixels,
}

impl CalibrationPresetKind {
    const fn label(self, lang: UiLanguage) -> &'static str {
        match (lang, self) {
            (UiLanguage::En, Self::Unit) => "Unit",
            (UiLanguage::En, Self::Pixels) => "Pixels",
            (UiLanguage::Ru, Self::Unit) => "Единицы",
            (UiLanguage::Ru, Self::Pixels) => "Пиксели",
        }
    }

    const fn icon(self) -> icons::Icon {
        match self {
            Self::Unit => icons::ICON_PRESET_UNIT,
            Self::Pixels => icons::ICON_PRESET_PIXELS,
        }
    }

    const fn hover_text(self, lang: UiLanguage) -> &'static str {
        match (lang, self) {
            (UiLanguage::En, Self::Unit) => "Quadrant preset (unit): set axes to 0..1 (signed).",
            (UiLanguage::En, Self::Pixels) => {
                "Quadrant preset (px): set axes to 0..size px (signed)."
            }
            (UiLanguage::Ru, Self::Unit) => {
                "Пресет квадранта (единицы): задать оси как 0..1 (со знаком)."
            }
            (UiLanguage::Ru, Self::Pixels) => {
                "Пресет квадранта (px): задать оси как 0..размер px (со знаком)."
            }
        }
    }
}

#[derive(Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
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

    const fn hint(self, lang: UiLanguage) -> &'static str {
        match (lang, self) {
            (UiLanguage::En, Self::I) => "Axes: bottom + left (x>=0, y>=0)",
            (UiLanguage::En, Self::II) => "Axes: bottom + right (x<=0, y>=0)",
            (UiLanguage::En, Self::III) => "Axes: top + right (x<=0, y<=0)",
            (UiLanguage::En, Self::IV) => "Axes: top + left (x>=0, y<=0)",
            (UiLanguage::Ru, Self::I) => "Оси: низ + лево (x>=0, y>=0)",
            (UiLanguage::Ru, Self::II) => "Оси: низ + право (x<=0, y>=0)",
            (UiLanguage::Ru, Self::III) => "Оси: верх + право (x<=0, y<=0)",
            (UiLanguage::Ru, Self::IV) => "Оси: верх + лево (x>=0, y<=0)",
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
    const fn label(self, lang: UiLanguage) -> &'static str {
        match (lang, self) {
            (UiLanguage::En, Self::Radius) => "Radius",
            (UiLanguage::En, Self::Angle) => "Angle",
            (UiLanguage::Ru, Self::Radius) => "Радиус",
            (UiLanguage::Ru, Self::Angle) => "Угол",
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

const fn axis_unit_label(lang: UiLanguage, unit: AxisUnit) -> &'static str {
    match (lang, unit) {
        (UiLanguage::En, AxisUnit::Float) => "Float",
        (UiLanguage::En, AxisUnit::DateTime) => "DateTime",
        (UiLanguage::Ru, AxisUnit::Float) => "Число",
        (UiLanguage::Ru, AxisUnit::DateTime) => "Дата/время",
    }
}

const fn scale_kind_label(lang: UiLanguage, scale: ScaleKind) -> &'static str {
    match (lang, scale) {
        (UiLanguage::En, ScaleKind::Linear) => "Linear",
        (UiLanguage::En, ScaleKind::Log10) => "Log10",
        (UiLanguage::Ru, ScaleKind::Linear) => "Линейная",
        (UiLanguage::Ru, ScaleKind::Log10) => "Лог10",
    }
}

const fn angle_unit_label(lang: UiLanguage, unit: AngleUnit) -> &'static str {
    match (lang, unit) {
        (UiLanguage::En, AngleUnit::Degrees) => "Degrees",
        (UiLanguage::En, AngleUnit::Radians) => "Radians",
        (UiLanguage::Ru, AngleUnit::Degrees) => "Градусы",
        (UiLanguage::Ru, AngleUnit::Radians) => "Радианы",
    }
}

const fn angle_direction_label(lang: UiLanguage, direction: AngleDirection) -> &'static str {
    match (lang, direction) {
        (UiLanguage::En, AngleDirection::Ccw) => "CCW",
        (UiLanguage::En, AngleDirection::Cw) => "CW",
        (UiLanguage::Ru, AngleDirection::Ccw) => "Против часовой",
        (UiLanguage::Ru, AngleDirection::Cw) => "По часовой",
    }
}

struct CalibrationUiState {
    highlight_jobs: Vec<(Rect, bool)>,
    pending_focus: Option<AxisValueField>,
    pending_pick: Option<PickMode>,
}

impl CalibrationUiState {
    const fn new(pending_focus: Option<AxisValueField>) -> Self {
        Self {
            highlight_jobs: Vec::new(),
            pending_focus,
            pending_pick: None,
        }
    }
}

impl CurcatApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_side_calibration(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.add_space(2.0);
        side_section_card(ui, |ui| {
            self.ui_point_input_section(ui);
        });
        ui.add_space(10.0);

        side_section_card(ui, |ui| {
            ui.heading(i18n.text(TextKey::Calibration));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(i18n.text(TextKey::CoordinateSystem))
                    .on_hover_text(i18n.text(TextKey::CoordinateSystemHover));
                let mut system = self.calibration.coord_system;
                let resp = egui::ComboBox::from_id_salt("coord_system_combo")
                    .selected_text(match system {
                        CoordSystem::Cartesian => i18n.text(TextKey::Cartesian),
                        CoordSystem::Polar => i18n.text(TextKey::Polar),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut system,
                            CoordSystem::Cartesian,
                            i18n.text(TextKey::Cartesian),
                        );
                        ui.selectable_value(
                            &mut system,
                            CoordSystem::Polar,
                            i18n.text(TextKey::Polar),
                        );
                    });
                resp.response
                    .on_hover_text(i18n.text(TextKey::CoordSystemForCalibrationExport));
                if system != self.calibration.coord_system {
                    self.calibration.coord_system = system;
                    self.mark_points_dirty();
                    self.calibration.pick_mode = PickMode::None;
                    self.calibration.pending_value_focus = None;
                    self.clear_calibration_snap_runtime();
                    self.set_status(match system {
                        CoordSystem::Cartesian => match self.ui.language {
                            UiLanguage::En => "Switched to Cartesian calibration.",
                            UiLanguage::Ru => "Переключено на декартову калибровку.",
                        },
                        CoordSystem::Polar => match self.ui.language {
                            UiLanguage::En => "Switched to Polar calibration.",
                            UiLanguage::Ru => "Переключено на полярную калибровку.",
                        },
                    });
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let cartesian = matches!(self.calibration.coord_system, CoordSystem::Cartesian);
                self.ui_calibration_snap_menu(ui, cartesian);
                ui.add_space(8.0);
                let has_image = self.image.image.is_some();
                if cartesian {
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
                    .on_hover_text(i18n.text(TextKey::ShowCalibrationOverlayHover));
                ui.add_space(4.0);
                ui.label(i18n.text(TextKey::ShowCalibrationOverlay))
                    .on_hover_text(i18n.text(TextKey::ShowCalibrationOverlayHover));
            });
        });
        ui.add_space(10.0);

        side_section_card(ui, |ui| {
            self.ui_export_section(ui);
        });

        let remaining = ui.available_height().max(0.0);
        if remaining > 24.0 {
            ui.add_space(remaining - 20.0);
        }
    }

    fn ui_quadrant_preset_menu(
        &mut self,
        ui: &mut egui::Ui,
        preset: CalibrationPresetKind,
        enabled: bool,
    ) {
        let lang = self.ui.language;
        ui.add_enabled_ui(enabled, |ui| {
            let button = egui::Button::image(icons::image(preset.icon(), icons::BUTTON_ICON_SIZE))
                .min_size(egui::vec2(24.0, 24.0))
                .image_tint_follows_text_color(true);
            let menu = MenuButton::from_button(button).ui(ui, |ui| {
                for quadrant in CalibrationQuadrant::ALL {
                    let resp = ui
                        .button(quadrant.label())
                        .on_hover_text(quadrant.hint(lang));
                    if resp.clicked() {
                        self.apply_calibration_preset(preset, quadrant);
                        ui.close();
                    }
                }
            });
            menu.0.on_hover_text(preset.hover_text(lang));
        });
    }

    fn ui_calibration_snap_toggle(ui: &mut egui::Ui, enabled: &mut bool, label: &str, hover: &str) {
        toggle_switch(ui, enabled).on_hover_text(hover);
        ui.add_space(2.0);
        ui.label(RichText::new(label).small().monospace())
            .on_hover_text(hover);
    }

    fn ui_calibration_snap_group_toggle(
        ui: &mut egui::Ui,
        group_enabled: &mut bool,
        label: &str,
        hover: &str,
    ) {
        toggle_switch(ui, group_enabled).on_hover_text(hover);
        ui.add_space(4.0);
        let label_resp = ui
            .add(egui::Label::new(RichText::new(label).strong()).sense(egui::Sense::click()))
            .on_hover_text(hover);
        if label_resp.clicked() {
            *group_enabled = !*group_enabled;
        }
    }

    #[allow(clippy::too_many_lines)]
    fn ui_calibration_snap_menu(&mut self, ui: &mut egui::Ui, cartesian: bool) {
        let i18n = self.i18n();
        let active_count = usize::from(self.calibration.calibration_angle_snap)
            + if cartesian {
                [
                    self.calibration.snap_ext,
                    self.calibration.snap_vh,
                    self.calibration.snap_end,
                    self.calibration.snap_int,
                ]
                .into_iter()
                .filter(|enabled| *enabled)
                .count()
            } else {
                0
            };
        let total = if cartesian { 5 } else { 1 };
        let button = egui::Button::image_and_text(
            icons::image(icons::ICON_MENU, icons::BUTTON_ICON_SIZE),
            format!(
                "{} ({active_count}/{total})",
                i18n.text(TextKey::CalSnapMenu)
            ),
        )
        .image_tint_follows_text_color(true);
        let menu_cfg = egui::containers::menu::MenuConfig::new()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let (response, _) = MenuButton::from_button(button)
            .config(menu_cfg)
            .ui(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::ui_calibration_snap_toggle(
                        ui,
                        &mut self.calibration.calibration_angle_snap,
                        i18n.text(TextKey::Snap15),
                        i18n.text(TextKey::Snap15Hover),
                    );
                });
                if !cartesian {
                    return;
                }

                ui.separator();

                let mut point_group = self.calibration.snap_end || self.calibration.snap_int;
                ui.horizontal(|ui| {
                    Self::ui_calibration_snap_group_toggle(
                        ui,
                        &mut point_group,
                        i18n.text(TextKey::CalSnapPointGroup),
                        i18n.text(TextKey::CalSnapPointGroupHover),
                    );
                });
                if point_group != (self.calibration.snap_end || self.calibration.snap_int) {
                    self.calibration.snap_end = point_group;
                    self.calibration.snap_int = point_group;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    Self::ui_calibration_snap_toggle(
                        ui,
                        &mut self.calibration.snap_end,
                        i18n.text(TextKey::CalSnapEnd),
                        i18n.text(TextKey::CalSnapEndHover),
                    );
                    ui.add_space(8.0);
                    Self::ui_calibration_snap_toggle(
                        ui,
                        &mut self.calibration.snap_int,
                        i18n.text(TextKey::CalSnapInt),
                        i18n.text(TextKey::CalSnapIntHover),
                    );
                });

                ui.separator();

                let mut line_group = self.calibration.snap_ext || self.calibration.snap_vh;
                ui.horizontal(|ui| {
                    Self::ui_calibration_snap_group_toggle(
                        ui,
                        &mut line_group,
                        i18n.text(TextKey::CalSnapLineGroup),
                        i18n.text(TextKey::CalSnapLineGroupHover),
                    );
                });
                if line_group != (self.calibration.snap_ext || self.calibration.snap_vh) {
                    self.calibration.snap_ext = line_group;
                    self.calibration.snap_vh = line_group;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    Self::ui_calibration_snap_toggle(
                        ui,
                        &mut self.calibration.snap_ext,
                        i18n.text(TextKey::CalSnapExt),
                        i18n.text(TextKey::CalSnapExtHover),
                    );
                    ui.add_space(8.0);
                    Self::ui_calibration_snap_toggle(
                        ui,
                        &mut self.calibration.snap_vh,
                        i18n.text(TextKey::CalSnapVh),
                        i18n.text(TextKey::CalSnapVhHover),
                    );
                });
            });
        response.on_hover_text(i18n.text(TextKey::CalSnapMenuHover));
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
            self.set_status(match self.ui.language {
                UiLanguage::En => "Load an image before applying calibration presets.",
                UiLanguage::Ru => "Загрузите изображение перед применением пресетов калибровки.",
            });
            return;
        };
        if width <= f32::EPSILON || height <= f32::EPSILON {
            self.set_status(match self.ui.language {
                UiLanguage::En => "Image dimensions are invalid for presets.",
                UiLanguage::Ru => "Размеры изображения некорректны для пресетов.",
            });
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
        self.clear_calibration_drag_runtime();
        self.mark_points_dirty();
        self.set_status(match self.ui.language {
            UiLanguage::En => format!(
                "Applied calibration preset: quadrant {} ({})",
                quadrant.label(),
                preset.label(self.ui.language)
            ),
            UiLanguage::Ru => format!(
                "Применён пресет калибровки: квадрант {} ({})",
                quadrant.label(),
                preset.label(self.ui.language)
            ),
        });
    }

    fn format_preset_value(value: f64) -> String {
        AxisValue::Float(value).format()
    }

    #[allow(clippy::too_many_arguments)]
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

    #[allow(clippy::too_many_arguments)]
    fn render_axis_rows(
        ui: &mut egui::Ui,
        language: UiLanguage,
        cal: &mut AxisCalUi,
        p1_label: &str,
        p2_label: &str,
        p1_field: AxisValueField,
        p2_field: AxisValueField,
        p1_mode: PickMode,
        p2_mode: PickMode,
        ui_state: &mut CalibrationUiState,
    ) -> (bool, bool) {
        let p1_row = Self::render_calibration_row(
            ui,
            p1_label,
            language,
            cal.unit,
            &mut cal.v1_text,
            p1_field,
            &mut ui_state.pending_focus,
            p1_mode,
            cal.p1,
        );
        ui.add_space(2.0);
        let p2_row = Self::render_calibration_row(
            ui,
            p2_label,
            language,
            cal.unit,
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

        (p1_invalid, p2_invalid)
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn axis_cal_group(&mut self, ui: &mut egui::Ui, is_x: bool) {
        let (label, p1_mode, p2_mode, p1_name, p2_name) = if is_x {
            (
                self.t(TextKey::XAxis),
                PickMode::X1,
                PickMode::X2,
                "X1",
                "X2",
            )
        } else {
            (
                self.t(TextKey::YAxis),
                PickMode::Y1,
                PickMode::Y2,
                "Y1",
                "Y2",
            )
        };

        let collapsing = egui::CollapsingHeader::new(label)
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id(label, |ui| {
                    let mut ui_state =
                        CalibrationUiState::new(self.calibration.pending_value_focus);
                    let mapping_ready;
                    {
                        let unit_label = self.t(TextKey::Unit);
                        let unit_hover = self.t(TextKey::UnitHover);
                        let axis_value_type_hover = self.t(TextKey::AxisValueTypeHover);
                        let scale_label = self.t(TextKey::Scale);
                        let scale_hover = self.t(TextKey::ScaleHover);
                        let axis_scale_hover = self.t(TextKey::AxisScaleHover);
                        let cal = if is_x {
                            &mut self.calibration.cal_x
                        } else {
                            &mut self.calibration.cal_y
                        };
                        let previous_unit = cal.unit;
                        ui.horizontal(|ui| {
                            ui.label(unit_label).on_hover_text(unit_hover);
                            let mut unit = cal.unit;
                            let unit_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_unit_combo"))
                                    .selected_text(axis_unit_label(self.ui.language, unit))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut unit,
                                            AxisUnit::Float,
                                            axis_unit_label(self.ui.language, AxisUnit::Float),
                                        );
                                        ui.selectable_value(
                                            &mut unit,
                                            AxisUnit::DateTime,
                                            axis_unit_label(self.ui.language, AxisUnit::DateTime),
                                        );
                                    });
                            unit_ir.response.on_hover_text(axis_value_type_hover);
                            cal.unit = unit;
                            ui.separator();

                            ui.label(scale_label).on_hover_text(scale_hover);
                            let mut scale = cal.scale;
                            let allow_log = matches!(cal.unit, AxisUnit::Float);
                            let scale_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_scale_combo"))
                                    .selected_text(scale_kind_label(self.ui.language, scale))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut scale,
                                            ScaleKind::Linear,
                                            scale_kind_label(self.ui.language, ScaleKind::Linear),
                                        );
                                        if allow_log {
                                            ui.selectable_value(
                                                &mut scale,
                                                ScaleKind::Log10,
                                                scale_kind_label(
                                                    self.ui.language,
                                                    ScaleKind::Log10,
                                                ),
                                            );
                                        }
                                    });
                            scale_ir.response.on_hover_text(axis_scale_hover);
                            if !allow_log && matches!(scale, ScaleKind::Log10) {
                                scale = ScaleKind::Linear;
                            }
                            cal.scale = scale;
                        });
                        if cal.unit != previous_unit {
                            sanitize_axis_text(&mut cal.v1_text, cal.unit);
                            sanitize_axis_text(&mut cal.v2_text, cal.unit);
                        }

                        let _ = Self::render_axis_rows(
                            ui,
                            self.ui.language,
                            cal,
                            p1_name,
                            p2_name,
                            if is_x {
                                AxisValueField::X1
                            } else {
                                AxisValueField::Y1
                            },
                            if is_x {
                                AxisValueField::X2
                            } else {
                                AxisValueField::Y2
                            },
                            p1_mode,
                            p2_mode,
                            &mut ui_state,
                        );

                        mapping_ready = cal.mapping().is_some();
                    }
                    self.finish_calibration_panel(
                        ui,
                        ui_state,
                        mapping_ready,
                        self.t(TextKey::MappingOk),
                        self.t(TextKey::MappingOkHover),
                        self.t(TextKey::MappingIncomplete),
                        self.t(TextKey::MappingIncompleteHover),
                    );
                });
            });
        collapsing.header_response.on_hover_text(if is_x {
            self.t(TextKey::XAxisCalibrationHover)
        } else {
            self.t(TextKey::YAxisCalibrationHover)
        });
    }

    fn ui_polar_origin_row(&mut self, ui: &mut egui::Ui) {
        let has_image = self.image.image.is_some();
        let row_height = ui.spacing().interact_size.y;
        let (label_width, pick_width, center_width) = match self.ui.language {
            UiLanguage::En => (56.0, 92.0, 64.0),
            UiLanguage::Ru => (70.0, 100.0, 72.0),
        };
        let mut pick_rect = None;
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 6.0;
            ui.add_sized(
                [label_width, row_height],
                egui::Label::new(format!("{}:", self.t(TextKey::Origin))),
            )
            .on_hover_text(match self.ui.language {
                UiLanguage::En => "Pick the pole (origin) for polar coordinates",
                UiLanguage::Ru => "Выберите полюс (начало координат) для полярной системы",
            });
            let pick_resp = ui
                .add_enabled(
                    has_image,
                    egui::Button::image_and_text(
                        icons::image(icons::ICON_PICK_POINT, icons::BUTTON_ICON_SIZE),
                        self.t(TextKey::PickOrigin),
                    )
                    .image_tint_follows_text_color(true)
                    .min_size(egui::vec2(pick_width, row_height)),
                )
                .on_hover_text(self.t(TextKey::PickOriginHover));
            if pick_resp.clicked() {
                self.begin_pick_mode(PickMode::Origin);
            }
            pick_rect = Some(pick_resp.rect);

            let center_resp = ui
                .add_enabled(
                    has_image,
                    egui::Button::new(self.t(TextKey::Center))
                        .min_size(egui::vec2(center_width, row_height)),
                )
                .on_hover_text(self.t(TextKey::CenterOriginHover));
            if center_resp.clicked()
                && let Some(image) = self.image.image.as_ref()
            {
                let cx = safe_usize_to_f32(image.size[0]) * 0.5;
                let cy = safe_usize_to_f32(image.size[1]) * 0.5;
                self.calibration.polar_cal.origin = Some(Pos2::new(cx, cy));
                self.mark_points_dirty();
                self.set_status(match self.ui.language {
                    UiLanguage::En => "Origin set to image center.",
                    UiLanguage::Ru => "Начало координат установлено в центр изображения.",
                });
            }
        });
        if let Some(p) = self.calibration.polar_cal.origin {
            ui.horizontal(|ui| {
                ui.add_space(label_width + 6.0);
                ui.label(
                    RichText::new(format!("@ ({:.1},{:.1})", p.x, p.y))
                        .small()
                        .weak(),
                );
            });
        }
        if let Some(rect) = pick_rect {
            self.paint_attention_outline_if(ui, rect, self.calibration.polar_cal.origin.is_none());
        }
    }

    #[allow(clippy::too_many_lines)]
    fn polar_axis_group(&mut self, ui: &mut egui::Ui, kind: PolarAxisKind) {
        let label = kind.label(self.ui.language);
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
                    {
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
                    }

                    if matches!(kind, PolarAxisKind::Radius) {
                        let scale_label = self.t(TextKey::Scale);
                        let scale_hover = self.t(TextKey::RadiusScaleHover);
                        let scale_choice_hover = self.t(TextKey::RadiusScaleChoiceHover);
                        ui.horizontal(|ui| {
                            ui.label(scale_label).on_hover_text(scale_hover);
                            let cal = &mut self.calibration.polar_cal.radius;
                            let mut scale = cal.scale;
                            let scale_ir =
                                egui::ComboBox::from_id_salt(format!("{label}_scale_combo"))
                                    .selected_text(scale_kind_label(self.ui.language, scale))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut scale,
                                            ScaleKind::Linear,
                                            scale_kind_label(self.ui.language, ScaleKind::Linear),
                                        );
                                        ui.selectable_value(
                                            &mut scale,
                                            ScaleKind::Log10,
                                            scale_kind_label(self.ui.language, ScaleKind::Log10),
                                        );
                                    });
                            scale_ir.response.on_hover_text(scale_choice_hover);
                            cal.scale = scale;
                        });
                    } else {
                        self.calibration.polar_cal.angle.scale = ScaleKind::Linear;
                        let angle_unit_caption = self.t(TextKey::AngleUnit);
                        let angle_unit_hover = self.t(TextKey::AngleUnitHover);
                        let direction_label = self.t(TextKey::Direction);
                        let direction_hover = self.t(TextKey::DirectionHover);
                        ui.horizontal(|ui| {
                            ui.label(angle_unit_caption).on_hover_text(angle_unit_hover);
                            let mut unit = self.calibration.polar_cal.angle_unit;
                            egui::ComboBox::from_id_salt(format!("{label}_unit_combo"))
                                .selected_text(angle_unit_label(self.ui.language, unit))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut unit,
                                        AngleUnit::Degrees,
                                        angle_unit_label(self.ui.language, AngleUnit::Degrees),
                                    );
                                    ui.selectable_value(
                                        &mut unit,
                                        AngleUnit::Radians,
                                        angle_unit_label(self.ui.language, AngleUnit::Radians),
                                    );
                                });
                            self.calibration.polar_cal.angle_unit = unit;
                            ui.separator();
                            ui.label(direction_label).on_hover_text(direction_hover);
                            let mut direction = self.calibration.polar_cal.angle_direction;
                            egui::ComboBox::from_id_salt(format!("{label}_dir_combo"))
                                .selected_text(angle_direction_label(self.ui.language, direction))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut direction,
                                        AngleDirection::Ccw,
                                        angle_direction_label(
                                            self.ui.language,
                                            AngleDirection::Ccw,
                                        ),
                                    );
                                    ui.selectable_value(
                                        &mut direction,
                                        AngleDirection::Cw,
                                        angle_direction_label(self.ui.language, AngleDirection::Cw),
                                    );
                                });
                            self.calibration.polar_cal.angle_direction = direction;
                        });
                    }

                    let (p1_invalid, p2_invalid) = {
                        let cal = match kind {
                            PolarAxisKind::Radius => &mut self.calibration.polar_cal.radius,
                            PolarAxisKind::Angle => &mut self.calibration.polar_cal.angle,
                        };
                        Self::render_axis_rows(
                            ui,
                            self.ui.language,
                            cal,
                            kind.p1_label(),
                            kind.p2_label(),
                            p1_field,
                            p2_field,
                            p1_mode,
                            p2_mode,
                            &mut ui_state,
                        )
                    };

                    let origin_ready = self.calibration.polar_cal.origin.is_some();
                    let values_ready = !p1_invalid && !p2_invalid;
                    let points_ready = match kind {
                        PolarAxisKind::Radius => {
                            self.calibration.polar_cal.radius.p1.is_some()
                                && self.calibration.polar_cal.radius.p2.is_some()
                        }
                        PolarAxisKind::Angle => {
                            self.calibration.polar_cal.angle.p1.is_some()
                                && self.calibration.polar_cal.angle.p2.is_some()
                        }
                    };
                    let mapping_ready = origin_ready && values_ready && points_ready;

                    self.finish_calibration_panel(
                        ui,
                        ui_state,
                        mapping_ready,
                        self.t(TextKey::MappingOk),
                        self.t(TextKey::MappingOkAxisHover),
                        self.t(TextKey::MappingIncomplete),
                        self.t(TextKey::MappingIncompleteAxisHover),
                    );
                });
            });
        collapsing.header_response.on_hover_text(match kind {
            PolarAxisKind::Radius => self.t(TextKey::RadiusCalibrationHover),
            PolarAxisKind::Angle => self.t(TextKey::AngleCalibrationHover),
        });
    }
}
