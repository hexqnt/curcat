//! Side panel UI: calibration, snapping, and export controls.

use super::super::snap_helpers::SNAP_SWATCH_SIZE;
use super::super::{AxisUnit, AxisValueField, CurcatApp, ExportKind, PickMode, PointInputMode};
use super::common::toggle_switch;
use super::icons;
use crate::interp::InterpAlgorithm;
use crate::snap::{SnapFeatureSource, SnapThresholdKind};
use crate::types::ScaleKind;
use egui::{
    Color32, CornerRadius, Response, RichText, StrokeKind, TextBuffer, TextEdit, Vec2,
    text::{CCursor, CCursorRange},
};

use std::any::TypeId;
use std::ops::Range;

/// Normalize axis input text by removing invalid characters and fixing decimals.
pub fn sanitize_axis_text(value: &mut String, unit: AxisUnit) {
    if value.is_empty() {
        return;
    }
    if matches!(unit, AxisUnit::Float) && value.contains(',') {
        *value = value.replace(',', ".");
    }
    value.retain(|ch| axis_char_allowed(unit, ch));
}

const fn axis_char_allowed(unit: AxisUnit, ch: char) -> bool {
    match unit {
        AxisUnit::Float => {
            ch.is_ascii_digit()
                || ch.is_ascii_whitespace()
                || matches!(ch, '+' | '-' | '.' | ',')
                || matches!(ch, 'e' | 'E')
                || matches!(ch, 'n' | 'N' | 'a' | 'A' | 'i' | 'I' | 'f' | 'F')
        }
        AxisUnit::DateTime => {
            ch.is_ascii_digit()
                || matches!(
                    ch,
                    '-' | '/' | '.' | ':' | ' ' | 'T' | 't' | '+' | 'Z' | 'z'
                )
        }
    }
}

struct AxisFilteredText<'a> {
    value: &'a mut String,
    unit: AxisUnit,
}

impl<'a> AxisFilteredText<'a> {
    const fn new(value: &'a mut String, unit: AxisUnit) -> Self {
        Self { value, unit }
    }
}

impl TextBuffer for AxisFilteredText<'_> {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.value.as_str()
    }

    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        let filtered: String = text
            .chars()
            .filter_map(|ch| {
                if !axis_char_allowed(self.unit, ch) {
                    return None;
                }
                let mapped = if matches!(self.unit, AxisUnit::Float) && ch == ',' {
                    '.'
                } else {
                    ch
                };
                Some(mapped)
            })
            .collect();
        if filtered.is_empty() {
            return 0;
        }
        let byte_idx = TextBuffer::byte_index_from_char_index(self, char_index);
        self.value.insert_str(byte_idx, &filtered);
        filtered.chars().count()
    }

    fn delete_char_range(&mut self, char_range: Range<usize>) {
        if char_range.start >= char_range.end {
            return;
        }
        let byte_start = TextBuffer::byte_index_from_char_index(self, char_range.start);
        let byte_end = TextBuffer::byte_index_from_char_index(self, char_range.end);
        self.value.drain(byte_start..byte_end);
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<AxisFilteredText<'static>>()
    }
}

impl CurcatApp {
    pub(crate) fn ui_side_calibration(&mut self, ui: &mut egui::Ui) {
        self.ui_point_input_section(ui);
        ui.separator();

        ui.heading("Calibration");
        ui.separator();
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.calibration_angle_snap)
                .on_hover_text("Snap calibration lines to 15° steps while picking or dragging");
            ui.add_space(4.0);
            ui.label("15° snap")
                .on_hover_text("Snap calibration lines to 15° steps while picking or dragging");
        });
        ui.separator();

        self.axis_cal_group(ui, true);
        ui.separator();
        self.axis_cal_group(ui, false);

        ui.separator();
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.show_calibration_segments)
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
            RichText::new(format!("Version {}", super::super::APP_VERSION))
                .small()
                .color(Color32::from_gray(160)),
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_point_input_section(&mut self, ui: &mut egui::Ui) {
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
            ui.radio_value(
                &mut self.point_input_mode,
                PointInputMode::CenterlineSnap,
                "Centerline snap",
            )
            .on_hover_text("Snap to the centerline of the color-matched curve");
        });
        match self.point_input_mode {
            PointInputMode::Free => {}
            PointInputMode::ContrastSnap => {
                self.ui_snap_radius_slider(ui);
                self.ui_snap_overlay_color_selector(ui);
                ui.add_space(4.0);
                ui.label("Feature source").on_hover_text(
                    "Choose what the snapper looks at when searching for a candidate",
                );
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
                    self.ui_curve_color_controls(ui);
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
                let threshold_range =
                    if matches!(self.snap_threshold_kind, SnapThresholdKind::Gradient) {
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
            }
            PointInputMode::CenterlineSnap => {
                self.ui_snap_radius_slider(ui);
                self.ui_snap_overlay_color_selector(ui);
                ui.label("Centerline detects flat color interiors")
                    .on_hover_text(
                        "Pick the curve color to help the detector focus on the intended line",
                    );
                self.ui_curve_color_controls(ui);
                ui.add_space(4.0);
                ui.label("Strength threshold")
                    .on_hover_text("Rejects weak centerline matches");
                ui.spacing_mut().slider_width = 150.0;
                ui.add(
                    egui::Slider::new(&mut self.centerline_threshold, 0.0..=255.0)
                        .text("threshold")
                        .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text("Higher = snap only to well-defined line centers");
                ui.scope(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.0;
                    ui.label(
                        RichText::new(
                            "Best results come from sampling the curve color before snapping.",
                        )
                        .small(),
                    );
                });
            }
        }
        if matches!(
            self.point_input_mode,
            PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
        ) {
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
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.show_curve_segments).on_hover_text(
                "Show or hide the lines that connect picked points (sorted by X value).",
            );
            ui.add_space(4.0);
            ui.label("Show point connections").on_hover_text(
                "Show or hide the lines between picked points (not calibration lines).",
            );
        });
    }

    pub(crate) fn ui_export_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Export points");
        let has_points = !self.points.is_empty();
        let x_ready = self.cal_x.mapping().is_some();
        let y_ready = self.cal_y.mapping().is_some();
        let can_export = has_points && x_ready && y_ready;
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
                ui.horizontal(|ui| {
                    let max_samples = self.config.export.samples_max_sanitized();
                    self.sample_count = self
                        .sample_count
                        .clamp(super::super::SAMPLE_COUNT_MIN, max_samples);
                    let sresp = ui.add(
                        egui::Slider::new(
                            &mut self.sample_count,
                            super::super::SAMPLE_COUNT_MIN..=max_samples,
                        )
                        .text("count"),
                    );
                    sresp.on_hover_text(format!(
                        "Higher values give a denser interpolated curve (max {max_samples})"
                    ));
                    if ui
                        .button("Auto")
                        .on_hover_text(
                            "Automatically choose a sample count based on curve smoothness",
                        )
                        .clicked()
                    {
                        self.auto_tune_sample_count();
                    }
                });
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
        let csv_hint = if !has_points {
            "Add points before exporting to CSV"
        } else if !x_ready || !y_ready {
            "Complete both axis calibrations before exporting to CSV"
        } else {
            "Export data to CSV (Ctrl+Shift+C)"
        };
        let resp_csv = ui
            .add_enabled(
                can_export,
                egui::Button::new(format!("{} Export CSV…", icons::ICON_EXPORT_CSV))
                    .shortcut_text("Ctrl+Shift+C"),
            )
            .on_hover_text(csv_hint);
        if resp_csv.clicked() {
            self.start_export_csv();
        }

        let json_hint = if !has_points {
            "Add points before exporting to JSON"
        } else if !x_ready || !y_ready {
            "Complete both axis calibrations before exporting to JSON"
        } else {
            "Export data to JSON (Ctrl+Shift+J)"
        };
        let resp_json = ui
            .add_enabled(
                can_export,
                egui::Button::new(format!("{} Export JSON…", icons::ICON_EXPORT_JSON))
                    .shortcut_text("Ctrl+Shift+J"),
            )
            .on_hover_text(json_hint);
        if resp_json.clicked() {
            self.start_export_json();
        }

        let xlsx_hint = if !has_points {
            "Add points before exporting to Excel"
        } else if !x_ready || !y_ready {
            "Complete both axis calibrations before exporting to Excel"
        } else {
            "Export data to Excel (Ctrl+Shift+E)"
        };
        let resp_xlsx = ui
            .add_enabled(
                can_export,
                egui::Button::new(format!("{} Export Excel…", icons::ICON_EXPORT_XLSX))
                    .shortcut_text("Ctrl+Shift+E"),
            )
            .on_hover_text(xlsx_hint);
        if resp_xlsx.clicked() {
            self.start_export_xlsx();
        }
    }

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
                    let mut highlight_jobs: Vec<(egui::Rect, bool)> = Vec::new();
                    let mut pending_focus = self.pending_value_focus;
                    let mut pending_pick: Option<PickMode> = None;
                    let mapping_ready;
                    {
                        let cal = if is_x {
                            &mut self.cal_x
                        } else {
                            &mut self.cal_y
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

                        let mut p1_value_rect = None;
                        let mut p2_value_rect = None;
                        let mut pick_p1_rect = None;
                        let mut pick_p2_rect = None;

                        ui.horizontal(|ui| {
                            ui.label(format!("{p1_name} value:")).on_hover_text(format!(
                                "Value of the calibration point ({p1_name})"
                            ));
                            let p1_resp = {
                                let mut buffer = AxisFilteredText::new(&mut cal.v1_text, cal.unit);
                                ui.add_sized(
                                    [100.0, ui.spacing().interact_size.y],
                                    TextEdit::singleline(&mut buffer),
                                )
                            };
                            let p1_resp = p1_resp.on_hover_text(match cal.unit {
                                AxisUnit::Float => "Enter a number (e.g., 1.23)",
                                AxisUnit::DateTime => "Enter date/time (e.g., 2024-10-31 12:30)",
                            });
                            let focus_target = if is_x {
                                AxisValueField::X1
                            } else {
                                AxisValueField::Y1
                            };
                            Self::apply_pending_focus(
                                &mut pending_focus,
                                focus_target,
                                &p1_resp,
                                &cal.v1_text,
                            );
                            p1_value_rect = Some(p1_resp.rect);
                            let pick_resp = ui
                                .button(format!("{} Pick {p1_name}", icons::ICON_PICK_POINT))
                                .on_hover_text(format!(
                                    "Click, then pick the {p1_name} point on the image"
                                ));
                            if pick_resp.clicked() {
                                pending_pick = Some(p1_mode);
                            }
                            pick_p1_rect = Some(pick_resp.rect);
                            if let Some(p) = cal.p1 {
                                ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(format!("{p2_name} value:")).on_hover_text(format!(
                                "Value of the calibration point ({p2_name})"
                            ));
                            let p2_resp = {
                                let mut buffer = AxisFilteredText::new(&mut cal.v2_text, cal.unit);
                                ui.add_sized(
                                    [100.0, ui.spacing().interact_size.y],
                                    TextEdit::singleline(&mut buffer),
                                )
                            };
                            let p2_resp = p2_resp.on_hover_text(match cal.unit {
                                AxisUnit::Float => "Enter a number (e.g., 4.56)",
                                AxisUnit::DateTime => "Enter date/time (e.g., 2024-10-31 12:45)",
                            });
                            let focus_target = if is_x {
                                AxisValueField::X2
                            } else {
                                AxisValueField::Y2
                            };
                            Self::apply_pending_focus(
                                &mut pending_focus,
                                focus_target,
                                &p2_resp,
                                &cal.v2_text,
                            );
                            p2_value_rect = Some(p2_resp.rect);
                            let pick_resp = ui
                                .button(format!("{} Pick {p2_name}", icons::ICON_PICK_POINT))
                                .on_hover_text(format!(
                                    "Click, then pick the {p2_name} point on the image"
                                ));
                            if pick_resp.clicked() {
                                pending_pick = Some(p2_mode);
                            }
                            pick_p2_rect = Some(pick_resp.rect);
                            if let Some(p) = cal.p2 {
                                ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
                            }
                        });

                        let (p1_value_invalid, p2_value_invalid) = cal.value_invalid_flags();
                        if let Some(rect) = p1_value_rect {
                            highlight_jobs.push((rect, p1_value_invalid));
                        }
                        if let Some(rect) = p2_value_rect {
                            highlight_jobs.push((rect, p2_value_invalid));
                        }
                        if let Some(rect) = pick_p1_rect {
                            highlight_jobs.push((rect, cal.p1.is_none()));
                        }
                        if let Some(rect) = pick_p2_rect {
                            highlight_jobs.push((rect, cal.p2.is_none()));
                        }

                        mapping_ready = cal.mapping().is_some();
                    }
                    if let Some(mode) = pending_pick {
                        self.begin_pick_mode(mode);
                    }
                    self.pending_value_focus = pending_focus;

                    for (rect, active) in highlight_jobs {
                        self.paint_attention_outline_if(ui, rect, active);
                    }

                    if mapping_ready {
                        ui.label(RichText::new("Mapping: OK").color(Color32::GREEN))
                            .on_hover_text("Calibration complete — you can pick points and export");
                    } else {
                        ui.label(
                            RichText::new("Mapping: incomplete or invalid").color(Color32::GRAY),
                        )
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

    fn apply_pending_focus(
        pending_focus: &mut Option<AxisValueField>,
        target: AxisValueField,
        response: &Response,
        text: &str,
    ) {
        if pending_focus.is_some_and(|pending| pending == target) {
            response.request_focus();
            if !text.is_empty() {
                Self::select_all_text(response, text);
            }
            *pending_focus = None;
        }
    }

    fn select_all_text(response: &Response, text: &str) {
        let mut state = TextEdit::load_state(&response.ctx, response.id).unwrap_or_default();
        let end = text.chars().count();
        let range = CCursorRange::two(CCursor::default(), CCursor::new(end));
        state.cursor.set_char_range(Some(range));
        TextEdit::store_state(&response.ctx, response.id, state);
    }

    fn ui_snap_radius_slider(&mut self, ui: &mut egui::Ui) {
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
        .on_hover_text("Radius used to look for snap candidates");
    }

    fn ui_curve_color_controls(&mut self, ui: &mut egui::Ui) {
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
                self.begin_pick_mode(PickMode::CurveColor);
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

    fn ui_snap_overlay_color_selector(&mut self, ui: &mut egui::Ui) {
        if self.snap_overlay_choices.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label("Snap overlay color")
            .on_hover_text("Choices are derived from the image to keep the snap preview visible");
        ui.horizontal_wrapped(|ui| {
            ui.style_mut().spacing.item_spacing.x = 6.0;
            for (idx, color) in self.snap_overlay_choices.iter().enumerate() {
                let selected = idx == self.snap_overlay_choice;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::splat(SNAP_SWATCH_SIZE), egui::Sense::click());
                if ui.is_rect_visible(rect) {
                    let stroke_color = if selected {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(90)
                    };
                    let stroke_width = if selected { 2.0 } else { 1.0 };
                    let rounding = CornerRadius::same(4);
                    ui.painter().rect_filled(rect, rounding, *color);
                    ui.painter().rect_stroke(
                        rect,
                        rounding,
                        egui::Stroke::new(stroke_width, stroke_color),
                        StrokeKind::Outside,
                    );
                }
                if response.clicked() {
                    self.snap_overlay_choice = idx;
                    self.snap_overlay_color = *color;
                }
                response.on_hover_ui(|ui| {
                    let [r, g, b, _] = color.to_array();
                    ui.label(format!("RGB {r}, {g}, {b}"));
                });
            }
        });
    }
}
