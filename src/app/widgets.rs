//! Small UI components; application actions remain with their callers.

mod action_button;
mod axis_value;
mod calibration_row;
mod choice;
mod color_palette;
mod language;
mod mode_chip;
mod navigator;
mod toggle;
mod zoom;

pub(super) use action_button::ActionButton;
pub(super) use axis_value::sanitize_axis_text;
pub(super) use calibration_row::CalibrationRow;
pub(super) use choice::Choice;
pub(super) use color_palette::ColorPalette;
pub(super) use language::LanguageSelector;
pub(super) use mode_chip::ModeChip;
pub(super) use navigator::Navigator;
pub(super) use toggle::ToggleRow;
pub(super) use zoom::{ZoomAction, ZoomSelector, format_zoom};
