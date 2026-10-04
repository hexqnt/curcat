//! Shared presentation values; widget interaction still follows the active egui theme.

use crate::app::{PickMode, StatusLevel};
use crate::i18n::UiLanguage;
use egui::{Color32, Margin, Visuals};

pub(super) const SPACE_TIGHT: f32 = 2.0;
pub(super) const SPACE_SMALL: f32 = 4.0;
pub(super) const SPACE_ROW: f32 = 6.0;
pub(super) const SPACE_GROUP: f32 = 8.0;
pub(super) const SPACE_SECTION: f32 = 10.0;
pub(super) const CARD_MARGIN: Margin = Margin::symmetric(10, 8);
pub(super) const SLIDER_WIDTH: f32 = 150.0;
pub(super) const ICON_BUTTON_SIZE: f32 = 24.0;
pub(super) const STATUS_FONT_SIZE: f32 = 13.0;
pub(super) const COLLAPSING_ICON_SCALE: f32 = 1.5;
pub(super) const COLLAPSING_TEXT_OFFSET: f32 = 4.0;

pub(super) const MODE_NORMAL: Color32 = Color32::from_gray(210);
pub(super) const MODE_AUTO_PLACE: Color32 = Color32::from_rgb(170, 220, 255);
pub(super) const MODE_DELETE: Color32 = Color32::from_rgb(255, 150, 150);
pub(super) const MODE_PAN: Color32 = Color32::from_rgb(190, 225, 255);
pub(super) const MODE_ZOOM: Color32 = Color32::from_rgb(186, 235, 186);

pub(super) fn secondary_text_color(visuals: &Visuals) -> Color32 {
    // Keep labels opaque: alpha-based dimming loses too much contrast on light panels.
    visuals.text_color().lerp_to_gamma(visuals.panel_fill, 0.05)
}

pub(super) fn status_color(level: StatusLevel, visuals: &Visuals) -> Color32 {
    match level {
        StatusLevel::Info => visuals.text_color(),
        StatusLevel::Warn if visuals.dark_mode => Color32::from_rgb(242, 194, 102),
        StatusLevel::Warn => Color32::from_rgb(128, 75, 0),
        StatusLevel::Error if visuals.dark_mode => Color32::from_rgb(240, 128, 128),
        StatusLevel::Error => Color32::from_rgb(180, 40, 40),
    }
}

pub(super) fn mapping_color(ready: bool, visuals: &Visuals) -> Color32 {
    if ready {
        if visuals.dark_mode {
            Color32::GREEN
        } else {
            Color32::from_rgb(0, 100, 0)
        }
    } else {
        visuals.text_color()
    }
}

/// Keep cursor colors unchanged while making chip text readable on light panels.
pub(super) const fn mode_text_color(base: Color32, visuals: &Visuals) -> Color32 {
    if visuals.dark_mode {
        base
    } else {
        let [r, g, b, _] = base.to_array();
        Color32::from_rgb(r / 3, g / 3, b / 3)
    }
}

/// Use the same mode colors in the status bar and on the image cursor.
pub(super) const fn pick_mode_color(mode: PickMode) -> Color32 {
    match mode {
        PickMode::None => MODE_NORMAL,
        PickMode::X1 | PickMode::X2 => MODE_PAN,
        PickMode::Y1 | PickMode::Y2 => Color32::from_rgb(200, 255, 200),
        PickMode::Origin => Color32::from_rgb(255, 230, 180),
        PickMode::R1 | PickMode::R2 | PickMode::CurveColor => Color32::from_rgb(255, 210, 160),
        PickMode::A1 | PickMode::A2 => Color32::from_rgb(200, 210, 255),
        PickMode::AutoTrace => Color32::from_rgb(215, 215, 255),
    }
}

pub(super) struct CalibrationRowWidths {
    pub(super) label: f32,
    pub(super) pick: f32,
    pub(super) value: f32,
}

impl CalibrationRowWidths {
    pub(super) fn new(language: UiLanguage, available_width: f32) -> Self {
        let (label_width, pick_width) = match language {
            UiLanguage::En => (70.0, 82.0),
            UiLanguage::Ru => (82.0, 90.0),
        };
        let value_width = SPACE_ROW
            .mul_add(-2.0, available_width.max(220.0) - label_width - pick_width)
            .clamp(64.0, 110.0);
        Self {
            label: label_width,
            pick: pick_width,
            value: value_width,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f32 {
        let [r, g, b, _] = color.to_array();
        let linear = |component| {
            let value = f32::from(component) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        linear(r).mul_add(0.2126, linear(g).mul_add(0.7152, linear(b) * 0.0722))
    }

    fn assert_readable(foreground: Color32, background: Color32) {
        let foreground = luminance(background.blend(foreground));
        let background = luminance(background);
        let contrast = (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05);
        assert!(
            contrast >= 4.5,
            "Small UI text contrast is too low: {contrast:.2}"
        );
    }

    #[test]
    fn status_and_mapping_text_are_readable_in_both_themes() {
        for visuals in [Visuals::dark(), Visuals::light()] {
            for level in [StatusLevel::Info, StatusLevel::Warn, StatusLevel::Error] {
                assert_readable(status_color(level, &visuals), visuals.panel_fill);
            }
            let card = visuals.panel_fill.blend(visuals.faint_bg_color);
            for ready in [false, true] {
                assert_readable(mapping_color(ready, &visuals), card);
            }
            assert_readable(secondary_text_color(&visuals), visuals.panel_fill);
        }
    }

    #[test]
    fn mode_chip_text_is_readable_in_both_themes() {
        for visuals in [Visuals::dark(), Visuals::light()] {
            let pick_colors = [
                PickMode::X1,
                PickMode::X2,
                PickMode::Y1,
                PickMode::Y2,
                PickMode::Origin,
                PickMode::R1,
                PickMode::R2,
                PickMode::A1,
                PickMode::A2,
                PickMode::CurveColor,
                PickMode::AutoTrace,
            ]
            .map(pick_mode_color);
            for base in [
                MODE_NORMAL,
                MODE_AUTO_PLACE,
                MODE_DELETE,
                MODE_PAN,
                MODE_ZOOM,
            ]
            .into_iter()
            .chain(pick_colors)
            {
                let [r, g, b, _] = base.to_array();
                let chip = visuals
                    .panel_fill
                    .blend(Color32::from_rgba_unmultiplied(r, g, b, 44));
                assert_readable(mode_text_color(base, &visuals), chip);
            }
        }
    }
}
