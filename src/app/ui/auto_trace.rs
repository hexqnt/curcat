use super::{common, icons, style};
use crate::app::widgets::{ActionButton, Choice};
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
        common::tool_window(self.t(TextKey::AutoTraceWindow))
            .open(&mut open)
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
                ActionButton::new(icons::ICON_AUTO_TRACE, i18n.text(TextKey::TraceFromClick)),
            )
            .on_hover_text(trace_hint)
            .clicked()
        {
            self.begin_pick_mode(PickMode::AutoTrace);
        }

        ui.add_space(style::SPACE_SMALL);
        ui.label(i18n.text(TextKey::DirectionShort));
        let language = self.ui.language;
        ui.add(Choice::new(
            "auto_trace_direction",
            &mut self.interaction.auto_trace_cfg.direction,
            &[
                AutoTraceDirection::Forward,
                AutoTraceDirection::Backward,
                AutoTraceDirection::Both,
            ],
            |direction| match (language, direction) {
                (UiLanguage::En, AutoTraceDirection::Forward) => "Forward (+X)",
                (UiLanguage::En, AutoTraceDirection::Backward) => "Backward (-X)",
                (UiLanguage::En, AutoTraceDirection::Both) => "Both",
                (UiLanguage::Ru, AutoTraceDirection::Forward) => "Вперёд (+X)",
                (UiLanguage::Ru, AutoTraceDirection::Backward) => "Назад (-X)",
                (UiLanguage::Ru, AutoTraceDirection::Both) => "В обе стороны",
            },
        ));

        ui.spacing_mut().slider_width = style::SLIDER_WIDTH;
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
