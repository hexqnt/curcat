use super::super::CurcatApp;

impl CurcatApp {
    pub(crate) fn ui_project_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.project.project_prompt.as_ref() else {
            return;
        };
        let mut continue_load = false;
        let mut cancel_load = false;
        let mut open = true;
        egui::Window::new("Project warnings")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Issues detected while loading the project:");
                ui.add_space(4.0);
                for warn in &prompt.warnings {
                    ui.label(format!("• {}", Self::project_warning_text(warn)));
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Continue anyway").clicked() {
                        continue_load = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel_load = true;
                    }
                });
            });

        if !open {
            cancel_load = true;
        }

        if continue_load {
            if let Some(prompt) = self.project.project_prompt.take() {
                self.begin_applying_project(prompt.plan);
            }
        } else if cancel_load {
            self.project.project_prompt = None;
            self.project.pending_project_apply = None;
            self.set_status("Project load canceled.");
        }
    }
}
