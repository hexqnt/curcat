use super::icons;
use crate::app::{AutoTraceDirection, CurcatApp, PickMode, PointInputMode};
use crate::types::CoordSystem;
use egui::RichText;

impl CurcatApp {
    pub(crate) fn ui_auto_trace_window(&mut self, ctx: &egui::Context) {
        if !self.ui.auto_trace_window_open {
            return;
        }

        let mut open = self.ui.auto_trace_window_open;
        egui::Window::new("Auto-trace")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                self.ui_auto_trace_section(ui);
            });
        self.ui.auto_trace_window_open = open;
    }

    fn ui_auto_trace_section(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Click once to trace a curve segment automatically.").small());

        let has_image = self.image.image.is_some();
        let calibrated = self.calibration_ready();
        let cartesian = matches!(self.calibration.coord_system, CoordSystem::Cartesian);
        let snap_ok = matches!(
            self.snap.point_input_mode,
            PointInputMode::ContrastSnap | PointInputMode::CenterlineSnap
        );
        let can_trace = has_image && calibrated && cartesian && snap_ok;
        let trace_hint = if !has_image {
            "Load an image first."
        } else if !calibrated {
            "Complete calibration before tracing."
        } else if !cartesian {
            "Auto-trace currently supports Cartesian mode only."
        } else if !snap_ok {
            "Select Contrast snap or Centerline snap before tracing."
        } else {
            "Click, then pick a start point on the image."
        };

        if ui
            .add_enabled(
                can_trace,
                egui::Button::new(format!("{} Trace from click", icons::ICON_AUTO_TRACE)),
            )
            .on_hover_text(trace_hint)
            .clicked()
        {
            self.begin_pick_mode(PickMode::AutoTrace);
        }

        ui.add_space(4.0);
        ui.label("Direction");
        egui::ComboBox::from_id_salt("auto_trace_direction")
            .selected_text(self.interaction.auto_trace_cfg.direction.label())
            .show_ui(ui, |ui| {
                for dir in [
                    AutoTraceDirection::Forward,
                    AutoTraceDirection::Backward,
                    AutoTraceDirection::Both,
                ] {
                    ui.selectable_value(
                        &mut self.interaction.auto_trace_cfg.direction,
                        dir,
                        dir.label(),
                    );
                }
            });

        ui.spacing_mut().slider_width = 150.0;
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.step_px, 2.0..=40.0)
                .text("step (px)")
                .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(
                &mut self.interaction.auto_trace_cfg.search_radius,
                3.0..=80.0,
            )
            .text("search radius (px)")
            .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.max_points, 50..=5000)
                .text("max points")
                .clamping(egui::SliderClamping::Always),
        );
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.max_misses, 0..=20)
                .text("gap tolerance")
                .clamping(egui::SliderClamping::Always),
        )
        .on_hover_text("How many missed steps to tolerate before stopping.");
        ui.add(
            egui::Slider::new(&mut self.interaction.auto_trace_cfg.dedup_radius, 0.5..=8.0)
                .text("min spacing (px)")
                .clamping(egui::SliderClamping::Always),
        );
    }
}
