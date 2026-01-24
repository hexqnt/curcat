use super::super::common::toggle_switch;
use crate::app::snap_helpers::SNAP_SWATCH_SIZE;
use crate::app::{CurcatApp, PickMode, PointInputMode};
use crate::snap::{SnapFeatureSource, SnapThresholdKind};
use egui::{Color32, CornerRadius, RichText, StrokeKind, Vec2};

impl CurcatApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_point_input_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("Point input");
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::Free,
                "Free",
            )
            .on_hover_text("Place points exactly where you click");
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::ContrastSnap,
                "Contrast snap",
            )
            .on_hover_text("Snap to the nearest high-contrast area inside the search radius");
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::CenterlineSnap,
                "Centerline snap",
            )
            .on_hover_text("Snap to the centerline of the color-matched curve");
        });
        match self.snap.point_input_mode {
            PointInputMode::Free => {}
            PointInputMode::ContrastSnap => {
                self.ui_snap_radius_slider(ui);
                self.ui_snap_overlay_color_selector(ui);
                ui.add_space(4.0);
                ui.label("Feature source").on_hover_text(
                    "Choose what the snapper looks at when searching for a candidate",
                );
                egui::ComboBox::from_id_salt("snap_feature_source")
                    .selected_text(self.snap.snap_feature_source.label())
                    .show_ui(ui, |ui| {
                        for variant in SnapFeatureSource::ALL {
                            ui.selectable_value(
                                &mut self.snap.snap_feature_source,
                                variant,
                                variant.label(),
                            );
                        }
                    });
                if matches!(
                    self.snap.snap_feature_source,
                    SnapFeatureSource::ColorMatch | SnapFeatureSource::Hybrid
                ) {
                    self.ui_curve_color_controls(ui);
                }
                ui.add_space(4.0);
                ui.label("Threshold mode")
                    .on_hover_text("Select how the detector decides if a pixel is strong enough");
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.snap.snap_threshold_kind,
                        SnapThresholdKind::Gradient,
                        SnapThresholdKind::Gradient.label(),
                    )
                    .on_hover_text("Compare threshold against raw gradient strength");
                    ui.radio_value(
                        &mut self.snap.snap_threshold_kind,
                        SnapThresholdKind::Score,
                        SnapThresholdKind::Score.label(),
                    )
                    .on_hover_text("Compare threshold against combined feature score");
                });
                let threshold_range =
                    if matches!(self.snap.snap_threshold_kind, SnapThresholdKind::Gradient) {
                        0.0..=120.0
                    } else {
                        0.0..=255.0
                    };
                ui.add(
                    egui::Slider::new(&mut self.snap.contrast_threshold, threshold_range)
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
                    egui::Slider::new(&mut self.snap.centerline_threshold, 0.0..=255.0)
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
            self.snap.point_input_mode,
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
            toggle_switch(ui, &mut self.points.show_curve_segments).on_hover_text(
                "Show or hide the lines that connect picked points (sorted by X value).",
            );
            ui.add_space(4.0);
            ui.label("Show point connections").on_hover_text(
                "Show or hide the lines between picked points (not calibration lines).",
            );
        });
    }

    fn ui_snap_radius_slider(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label("Search radius (px)").on_hover_text(
            "Measured in image pixels; smaller values keep snapping near the cursor",
        );
        ui.spacing_mut().slider_width = 150.0;
        ui.add(
            egui::Slider::new(&mut self.snap.contrast_search_radius, 3.0..=60.0)
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
                .color_edit_button_srgba(&mut self.snap.snap_target_color)
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
                egui::Slider::new(&mut self.snap.snap_color_tolerance, 5.0..=150.0)
                    .text("tolerance")
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text("How far the pixel color may deviate from the picked color");
        if tol_resp.changed() {
            self.mark_snap_maps_dirty();
        }
    }

    fn ui_snap_overlay_color_selector(&mut self, ui: &mut egui::Ui) {
        if self.snap.snap_overlay_choices.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label("Snap overlay color")
            .on_hover_text("Choices are derived from the image to keep the snap preview visible");
        ui.horizontal_wrapped(|ui| {
            ui.style_mut().spacing.item_spacing.x = 6.0;
            for (idx, color) in self.snap.snap_overlay_choices.iter().enumerate() {
                let selected = idx == self.snap.snap_overlay_choice;
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
                    self.snap.snap_overlay_choice = idx;
                    self.snap.snap_overlay_color = *color;
                }
                response.on_hover_ui(|ui| {
                    let [r, g, b, _] = color.to_array();
                    ui.label(format!("RGB {r}, {g}, {b}"));
                });
            }
        });
    }
}
