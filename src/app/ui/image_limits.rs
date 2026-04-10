use super::super::CurcatApp;
use crate::config::ImageLimits;
use crate::i18n::{TextKey, UiLanguage};
use crate::image::{ImageLimitInfo, ImageLoadPolicy, human_readable_bytes};

impl CurcatApp {
    pub(crate) fn ui_image_limits_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.project.pending_image_limit_prompt.as_ref() else {
            return;
        };

        let source_label = prompt.meta.description();
        let info = prompt.info.clone();
        let source_line = self.format_source_line(&source_label, &info);
        let config_line =
            self.format_limits_line(TextKey::ConfiguredLimitsInfo, &info.config_limits);
        let hard_line = self.format_limits_line(TextKey::HardLimitsInfo, &info.hard_limits);
        let autoscale_line = self.format_autoscale_line(&info);

        let mut reject = false;
        let mut selected_policy: Option<ImageLoadPolicy> = None;
        let mut open = true;

        egui::Window::new(self.t(TextKey::ImageLimitsWindow))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(self.t(TextKey::ImageLimitsIntro));
                ui.add_space(6.0);
                ui.label(source_line);
                ui.label(config_line);
                ui.label(hard_line);
                ui.label(autoscale_line);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(self.t(TextKey::RejectLoad)).clicked() {
                        reject = true;
                    }
                    if ui
                        .add_enabled(
                            info.can_autoscale,
                            egui::Button::new(self.t(TextKey::AutoscaleToFit)),
                        )
                        .clicked()
                    {
                        selected_policy = Some(ImageLoadPolicy::AutoscaleToConfig);
                    }
                    if ui
                        .add_enabled(
                            info.can_ignore_limits,
                            egui::Button::new(self.t(TextKey::IgnoreLimits)),
                        )
                        .clicked()
                    {
                        selected_policy = Some(ImageLoadPolicy::IgnoreConfigWithHardCap);
                    }
                });
                if !info.can_autoscale || !info.can_ignore_limits {
                    ui.add_space(4.0);
                    ui.weak(self.t(TextKey::ActionUnavailable));
                }
            });

        if !open {
            reject = true;
        }

        if let Some(policy) = selected_policy {
            self.retry_image_load_with_policy(policy);
        } else if reject {
            self.reject_image_load_due_to_limits();
        }
    }

    fn format_source_line(&self, source_label: &str, info: &ImageLimitInfo) -> String {
        let source_mp_major = info.source_total_pixels / 1_000_000;
        let source_mp_frac = (info.source_total_pixels % 1_000_000) / 10_000;
        match self.ui.language {
            UiLanguage::En => format!(
                "{}: {source_label} — {}x{} px (~{source_mp_major}.{source_mp_frac:02} MP, {})",
                self.t(TextKey::SourceImageInfo),
                info.source_width,
                info.source_height,
                human_readable_bytes(info.source_rgba_bytes)
            ),
            UiLanguage::Ru => format!(
                "{}: {source_label} — {}x{} px (~{source_mp_major}.{source_mp_frac:02} МП, {})",
                self.t(TextKey::SourceImageInfo),
                info.source_width,
                info.source_height,
                human_readable_bytes(info.source_rgba_bytes)
            ),
        }
    }

    fn format_limits_line(&self, key: TextKey, limits: &ImageLimits) -> String {
        match self.ui.language {
            UiLanguage::En => format!(
                "{}: side <= {} px, pixels <= {} MP, RGBA <= {}",
                self.t(key),
                limits.image_dim,
                limits.total_pixels / 1_000_000,
                human_readable_bytes(limits.alloc_bytes)
            ),
            UiLanguage::Ru => format!(
                "{}: сторона <= {} px, пиксели <= {} МП, RGBA <= {}",
                self.t(key),
                limits.image_dim,
                limits.total_pixels / 1_000_000,
                human_readable_bytes(limits.alloc_bytes)
            ),
        }
    }

    fn format_autoscale_line(&self, info: &ImageLimitInfo) -> String {
        match info.autoscale_size() {
            Some([w, h]) => format!("{}: {w}x{h} px", self.t(TextKey::SuggestedAutoscaleInfo)),
            None => format!("{}: -", self.t(TextKey::SuggestedAutoscaleInfo)),
        }
    }
}
