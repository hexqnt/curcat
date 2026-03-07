use super::icons;
use crate::app::{AutoTraceDirection, CurcatApp, PickMode, PointInputMode};
use crate::i18n::{TextKey, UiLanguage};
use crate::types::CoordSystem;
use egui::RichText;

impl CurcatApp {
    pub(crate) fn ui_auto_trace_window(&mut self, ctx: &egui::Context) {
        if !self.ui.auto_trace_window_open {
            return;
        }

        let mut open = self.ui.auto_trace_window_open;
        egui::Window::new(self.t(TextKey::AutoTraceWindow))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui_auto_trace_section(ui);
            });
        self.ui.auto_trace_window_open = open;
    }

    fn ui_auto_trace_section(&mut self, ui: &mut egui::Ui) {
        let i18n = self.i18n();
        ui.label(RichText::new(i18n.text(TextKey::AutoTraceIntro)).small());

        let has_image = self.image.image.is_some();
        let calibrated = self.calibration_ready();
        let cartesian = matches!(self.calibration.coord_system, CoordSystem::Cartesian);
        let snap_ok = matches!(
            self.snap.point_input_mode,
            PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
        );
        let can_trace = has_image && calibrated && cartesian && snap_ok;
        let trace_hint = if !has_image {
            i18n.text(TextKey::LoadImageFirst)
        } else if !calibrated {
            i18n.text(TextKey::CompleteCalibrationBeforeTracing)
        } else if !cartesian {
            i18n.text(TextKey::AutoTraceCartesianOnly)
        } else if !snap_ok {
            i18n.text(TextKey::SelectSnapBeforeTracing)
        } else {
            i18n.text(TextKey::ClickStartPoint)
        };

        if ui
            .add_enabled(
                can_trace,
                egui::Button::new(format!(
                    "{} {}",
                    icons::ICON_AUTO_TRACE,
                    i18n.text(TextKey::TraceFromClick)
                )),
            )
            .on_hover_text(trace_hint)
            .clicked()
        {
            self.begin_pick_mode(PickMode::AutoTrace);
        }

        ui.add_space(4.0);
        ui.label(i18n.text(TextKey::DirectionShort));
        egui::ComboBox::from_id_salt("auto_trace_direction")
            .selected_text(
                match (self.ui.language, self.interaction.auto_trace_cfg.direction) {
                    (UiLanguage::En, AutoTraceDirection::Forward) => "Forward (+X)",
                    (UiLanguage::En, AutoTraceDirection::Backward) => "Backward (-X)",
                    (UiLanguage::En, AutoTraceDirection::Both) => "Both",
                    (UiLanguage::Ru, AutoTraceDirection::Forward) => "Вперёд (+X)",
                    (UiLanguage::Ru, AutoTraceDirection::Backward) => "Назад (-X)",
                    (UiLanguage::Ru, AutoTraceDirection::Both) => "В обе стороны",
                },
            )
            .show_ui(ui, |ui| {
                for dir in [
                    AutoTraceDirection::Forward,
                    AutoTraceDirection::Backward,
                    AutoTraceDirection::Both,
                ] {
                    ui.selectable_value(
                        &mut self.interaction.auto_trace_cfg.direction,
                        dir,
                        match (self.ui.language, dir) {
                            (UiLanguage::En, AutoTraceDirection::Forward) => "Forward (+X)",
                            (UiLanguage::En, AutoTraceDirection::Backward) => "Backward (-X)",
                            (UiLanguage::En, AutoTraceDirection::Both) => "Both",
                            (UiLanguage::Ru, AutoTraceDirection::Forward) => "Вперёд (+X)",
                            (UiLanguage::Ru, AutoTraceDirection::Backward) => "Назад (-X)",
                            (UiLanguage::Ru, AutoTraceDirection::Both) => "В обе стороны",
                        },
                    );
                }
            });

        ui.spacing_mut().slider_width = 150.0;
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.step_px, 2.0..=40.0)
                .text(i18n.text(TextKey::StepPx))
                .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(
                &mut self.interaction.auto_trace_cfg.search_radius,
                3.0..=80.0,
            )
            .text(i18n.text(TextKey::SearchRadiusShort))
            .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.max_points, 50..=5000)
                .text(i18n.text(TextKey::MaxPoints))
                .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.max_misses, 0..=20)
                .text(i18n.text(TextKey::GapTolerance))
                .clamping(egui::SliderClamping::Always),
        )
        .on_hover_text(i18n.text(TextKey::GapToleranceHover));
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.dedup_radius, 0.5..=8.0)
                .text(i18n.text(TextKey::MinSpacingPx))
                .clamping(egui::SliderClamping::Always),
        );
    }
}
