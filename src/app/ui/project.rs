use super::super::CurcatApp;
use super::{common, style};
use crate::i18n::{TextKey, UiLanguage};

impl CurcatApp {
    pub(crate) fn ui_project_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.project.project_prompt.as_ref() else {
            return;
        };
        let mut continue_load = false;
        let mut cancel_load = false;
        let mut open = true;
        common::tool_window(self.t(TextKey::ProjectWarningsWindow))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(self.t(TextKey::ProjectWarningsIntro));
                ui.add_space(style::SPACE_SMALL);
                for warn in &prompt.warnings {
                    ui.label(format!("• {}", self.project_warning_text(warn)));
                }
                ui.add_space(style::SPACE_GROUP);
                ui.horizontal(|ui| {
                    if ui.button(self.t(TextKey::ContinueAnyway)).clicked() {
                        continue_load = true;
                    }
                    if ui.button(self.t(TextKey::Cancel)).clicked() {
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
            self.set_status(match self.ui.language {
                UiLanguage::En => "Project load canceled.",
                UiLanguage::Ru => "Загрузка проекта отменена.",
            });
        }
    }
}
