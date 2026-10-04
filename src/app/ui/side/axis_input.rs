//! Calibration value inputs and point-picking controls.

mod text;

use super::super::{
    common, icons,
    style::{self, CalibrationRowWidths},
};
use crate::app::{AxisValueField, PickMode};
use crate::i18n::UiLanguage;
use crate::types::AxisUnit;
use egui::{
    Pos2, Rect, Response, TextEdit,
    text::{CCursor, CCursorRange},
};

use text::FilteredAxisText;
pub(super) use text::sanitize_axis_text;

pub(super) struct CalibrationRowResponse {
    pub(super) value_rect: Rect,
    pub(super) pick_rect: Rect,
    pub(super) requested_pick: Option<PickMode>,
}

impl AxisValueField {
    const fn name(self) -> &'static str {
        match self {
            Self::X1 => "X1",
            Self::X2 => "X2",
            Self::Y1 => "Y1",
            Self::Y2 => "Y2",
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::A1 => "A1",
            Self::A2 => "A2",
        }
    }

    const fn pick_mode(self) -> PickMode {
        match self {
            Self::X1 => PickMode::X1,
            Self::X2 => PickMode::X2,
            Self::Y1 => PickMode::Y1,
            Self::Y2 => PickMode::Y2,
            Self::R1 => PickMode::R1,
            Self::R2 => PickMode::R2,
            Self::A1 => PickMode::A1,
            Self::A2 => PickMode::A2,
        }
    }
}

/// A calibration input and pick button whose identity determines its label and action.
#[must_use = "Render the calibration row with show()."]
pub(super) struct CalibrationRow<'a> {
    field: AxisValueField,
    value_text: &'a mut String,
    language: UiLanguage,
    unit: AxisUnit,
    point: Option<Pos2>,
}

impl<'a> CalibrationRow<'a> {
    pub(super) const fn new(
        field: AxisValueField,
        value_text: &'a mut String,
        language: UiLanguage,
    ) -> Self {
        Self {
            field,
            value_text,
            language,
            unit: AxisUnit::Float,
            point: None,
        }
    }

    pub(super) const fn unit(mut self, unit: AxisUnit) -> Self {
        self.unit = unit;
        self
    }

    pub(super) const fn point(mut self, point: Option<Pos2>) -> Self {
        self.point = point;
        self
    }

    pub(super) fn show(
        self,
        ui: &mut egui::Ui,
        pending_focus: &mut Option<AxisValueField>,
    ) -> CalibrationRowResponse {
        let Self {
            field,
            value_text,
            language,
            unit,
            point,
        } = self;
        let name = field.name();
        let row_height = ui.spacing().interact_size.y;
        let row_spacing_x = style::SPACE_ROW;
        let CalibrationRowWidths {
            label: label_width,
            pick: pick_width,
            value: value_width,
        } = CalibrationRowWidths::new(language, ui.available_width());

        let row = ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = row_spacing_x;
            let value_label = match language {
                UiLanguage::En => format!("{name} value:"),
                UiLanguage::Ru => format!("Значение {name}:"),
            };
            let value_hover = match language {
                UiLanguage::En => format!("Value of the calibration point ({name})"),
                UiLanguage::Ru => format!("Значение калибровочной точки ({name})"),
            };
            let value_label = ui
                .add_sized([label_width, row_height], egui::Label::new(value_label))
                .on_hover_text(value_hover);
            let value_resp = {
                let mut buffer = FilteredAxisText::new(value_text, unit);
                ui.add_sized([value_width, row_height], TextEdit::singleline(&mut buffer))
            };
            let value_resp = value_resp
                .labelled_by(value_label.id)
                .on_hover_text(match unit {
                    AxisUnit::Float => match language {
                        UiLanguage::En => "Enter a number (e.g., 1.23)",
                        UiLanguage::Ru => "Введите число (например, 1.23)",
                    },
                    AxisUnit::DateTime => match language {
                        UiLanguage::En => "Enter date/time (e.g., 2024-10-31 12:30)",
                        UiLanguage::Ru => "Введите дату/время (например, 2024-10-31 12:30)",
                    },
                });
            Self::apply_pending_focus(pending_focus, field, &value_resp, value_text);

            let pick_button = match language {
                UiLanguage::En => format!("Pick {name}"),
                UiLanguage::Ru => format!("Выбрать {name}"),
            };
            let pick_hover = match language {
                UiLanguage::En => format!("Click, then pick the {name} point on the image"),
                UiLanguage::Ru => format!("Нажмите и выберите точку {name} на изображении"),
            };
            let pick_resp = ui
                .add_sized(
                    [pick_width, row_height],
                    common::icon_button(icons::ICON_PICK_POINT, pick_button)
                        .min_size(egui::vec2(pick_width, row_height)),
                )
                .on_hover_text(pick_hover);
            CalibrationRowResponse {
                value_rect: value_resp.rect,
                pick_rect: pick_resp.rect,
                requested_pick: pick_resp.clicked().then_some(field.pick_mode()),
            }
        });
        if let Some(point) = point {
            common::point_coordinates(ui, point, label_width + row_spacing_x);
        }

        row.inner
    }

    fn apply_pending_focus(
        pending_focus: &mut Option<AxisValueField>,
        target: AxisValueField,
        response: &Response,
        text: &str,
    ) {
        if *pending_focus == Some(target) {
            response.request_focus();
            if !text.is_empty() {
                Self::select_all_text(response, text);
            }
            *pending_focus = None;
        }
    }

    fn select_all_text(response: &Response, text: &str) {
        let mut state = TextEdit::load_state(&response.ctx, response.id).unwrap_or_default();
        let end = text.chars().count();
        let range = CCursorRange::two(CCursor::default(), CCursor::new(end));
        state.cursor.set_char_range(Some(range));
        TextEdit::store_state(&response.ctx, response.id, state);
    }
}
