use egui::Ui;

use super::ActionButton;
use crate::{
    app::{ZOOM_PRESETS, ui::icons},
    i18n::{I18n, TextKey},
};

pub(in crate::app) enum ZoomAction {
    Fit,
    Reset,
    Preset(f32),
}

#[must_use = "Show the zoom selector with show()."]
pub(in crate::app) struct ZoomSelector {
    zoom: f32,
    i18n: I18n,
}

impl ZoomSelector {
    pub(in crate::app) const fn new(zoom: f32, i18n: I18n) -> Self {
        Self { zoom, i18n }
    }

    pub(in crate::app) fn show(self, ui: &mut Ui) -> egui::InnerResponse<Option<ZoomAction>> {
        let mut action = None;
        let mut response = egui::ComboBox::from_id_salt("zoom_combo")
            .width(86.0)
            .selected_text(format_zoom(self.zoom))
            .show_ui(ui, |ui| {
                for (icon, label, hint, shortcut, next) in [
                    (
                        icons::ICON_FIT,
                        TextKey::Fit,
                        TextKey::FitHover,
                        "Ctrl+F",
                        ZoomAction::Fit,
                    ),
                    (
                        icons::ICON_RESET_VIEW,
                        TextKey::ResetView,
                        TextKey::ResetViewHover,
                        "Ctrl+R",
                        ZoomAction::Reset,
                    ),
                ] {
                    if ui
                        .add(ActionButton::new(icon, self.i18n.text(label)).shortcut_text(shortcut))
                        .on_hover_text(self.i18n.text(hint))
                        .clicked()
                    {
                        action = Some(next);
                        ui.close();
                    }
                }
                ui.separator();
                for &preset in ZOOM_PRESETS {
                    let selected = (self.zoom - preset).abs() < 0.0001;
                    if ui.selectable_label(selected, format_zoom(preset)).clicked() {
                        action = Some(ZoomAction::Preset(preset));
                    }
                }
            })
            .response
            .on_hover_text(self.i18n.text(TextKey::ZoomPresetsHover));
        if action.is_some() {
            response.mark_changed();
        }
        egui::InnerResponse {
            response,
            inner: action,
        }
    }
}

pub(in crate::app) fn format_zoom(zoom: f32) -> String {
    if (zoom - 1.0).abs() < 0.005 {
        "100%".to_string()
    } else {
        format!("{:.0}%", zoom * 100.0)
    }
}
