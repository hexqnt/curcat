use super::super::common::toggle_switch;
use crate::app::snap_helpers::SNAP_SWATCH_SIZE;
use crate::app::{CurcatApp, PickMode, PointInputMode};
use crate::i18n::TextKey;
use crate::snap::{SnapFeatureSource, SnapThresholdKind};
use egui::{Color32, CornerRadius, RichText, StrokeKind, Vec2};

impl CurcatApp {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn ui_point_input_section(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.heading(i18n.text(TextKey::PointInput));
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::Free,
                i18n.text(TextKey::Free),
            )
            .on_hover_text(i18n.text(TextKey::FreeHover));
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::ContrastSnap,
                i18n.text(TextKey::ContrastSnap),
            )
            .on_hover_text(i18n.text(TextKey::ContrastSnapHover));
            ui.radio_value(
                &mut self.snap.point_input_mode,
                PointInputMode::CenterlineSnap,
                i18n.text(TextKey::CenterlineSnap),
            )
            .on_hover_text(i18n.text(TextKey::CenterlineSnapHover));
        });

        match self.snap.point_input_mode {
            PointInputMode::Free => {}
            PointInputMode::ContrastSnap => {
                self.ui_snap_radius_slider(ui);
                self.ui_snap_overlay_color_selector(ui);
                ui.add_space(4.0);
                ui.label(i18n.text(TextKey::FeatureSource))
                    .on_hover_text(i18n.text(TextKey::FeatureSourceHover));
                egui::ComboBox::from_id_salt("snap_feature_source")
                    .selected_text(i18n.snap_feature_source_label(self.snap.snap_feature_source))
                    .show_ui(ui, |ui| {
                        for variant in SnapFeatureSource::ALL {
                            ui.selectable_value(
                                &mut self.snap.snap_feature_source,
                                variant,
                                i18n.snap_feature_source_label(variant),
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
                ui.label(i18n.text(TextKey::ThresholdMode))
                    .on_hover_text(i18n.text(TextKey::ThresholdModeHover));
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.snap.snap_threshold_kind,
                        SnapThresholdKind::Gradient,
                        i18n.snap_threshold_kind_label(SnapThresholdKind::Gradient),
                    )
                    .on_hover_text(i18n.text(TextKey::GradientOnlyHover));
                    ui.radio_value(
                        &mut self.snap.snap_threshold_kind,
                        SnapThresholdKind::Score,
                        i18n.snap_threshold_kind_label(SnapThresholdKind::Score),
                    )
                    .on_hover_text(i18n.text(TextKey::FeatureScoreHover));
                });
                let threshold_range =
                    if matches!(self.snap.snap_threshold_kind, SnapThresholdKind::Gradient) {
                        0.0..=120.0
                    } else {
                        0.0..=255.0
                    };
                ui.add(
                    egui::Slider::new(&mut self.snap.contrast_threshold, threshold_range)
                        .text(i18n.text(TextKey::Threshold))
                        .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text(i18n.text(TextKey::ThresholdHigherHint));
            }
            PointInputMode::CenterlineSnap => {
                self.ui_snap_radius_slider(ui);
                self.ui_snap_overlay_color_selector(ui);
                ui.label(i18n.text(TextKey::CenterlineDetectsFlat))
                    .on_hover_text(i18n.text(TextKey::CenterlineDetectsFlatHover));
                self.ui_curve_color_controls(ui);
                ui.add_space(4.0);
                ui.label(i18n.text(TextKey::StrengthThreshold))
                    .on_hover_text(i18n.text(TextKey::StrengthThresholdHover));
                ui.spacing_mut().slider_width = 150.0;
                ui.add(
                    egui::Slider::new(&mut self.snap.centerline_threshold, 0.0..=255.0)
                        .text(i18n.text(TextKey::Threshold))
                        .clamping(egui::SliderClamping::Always),
                )
                .on_hover_text(i18n.text(TextKey::HigherOnlyWellDefined));
                ui.scope(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 4.0;
                    ui.label(RichText::new(i18n.text(TextKey::BestResultsColorSample)).small());
                });
            }
        }
        if matches!(
            self.snap.point_input_mode,
            PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
        ) {
            ui.scope(|ui| {
                ui.style_mut().spacing.item_spacing.x = 4.0;
                ui.label(RichText::new(i18n.text(TextKey::PreviewCircleHint)).small());
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            toggle_switch(ui, &mut self.points.show_curve_segments)
                .on_hover_text(i18n.text(TextKey::ShowPointConnectionsHover));
            ui.add_space(4.0);
            ui.label(i18n.text(TextKey::ShowPointConnections))
                .on_hover_text(i18n.text(TextKey::ShowPointConnectionsHover));
        });
    }

    fn ui_snap_radius_slider(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.add_space(4.0);
        ui.label(i18n.text(TextKey::SearchRadiusPx))
            .on_hover_text(i18n.text(TextKey::SearchRadiusHover));
        ui.spacing_mut().slider_width = 150.0;
        ui.add(
            egui::Slider::new(&mut self.snap.contrast_search_radius, 3.0..=60.0)
                .logarithmic(false)
                .clamping(egui::SliderClamping::Always)
                .text("px"),
        )
        .on_hover_text(i18n.text(TextKey::RadiusUsedToLookForCandidates));
    }

    fn ui_curve_color_controls(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(i18n.text(TextKey::CurveColor));
            let color_button = ui
                .color_edit_button_srgba(&mut self.snap.snap_target_color)
                .on_hover_text(i18n.text(TextKey::CurveColorHover));
            if color_button.changed() {
                self.mark_snap_maps_dirty();
            }
            if ui
                .button(i18n.text(TextKey::PickFromImage))
                .on_hover_text(i18n.text(TextKey::PickFromImageHover))
                .clicked()
            {
                self.begin_pick_mode(PickMode::CurveColor);
            }
        });
        let tol_resp = ui
            .add(
                egui::Slider::new(&mut self.snap.snap_color_tolerance, 5.0..=150.0)
                    .text(i18n.text(TextKey::Tolerance))
                    .clamping(egui::SliderClamping::Always),
            )
            .on_hover_text(i18n.text(TextKey::ToleranceHover));
        if tol_resp.changed() {
            self.mark_snap_maps_dirty();
        }
    }

    fn ui_snap_overlay_color_selector(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        if self.snap.snap_overlay_choices.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(i18n.text(TextKey::SnapOverlayColor))
            .on_hover_text(i18n.text(TextKey::SnapOverlayColorHover));
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
