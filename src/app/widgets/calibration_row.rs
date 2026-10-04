//! Calibration value inputs and point-picking controls.

use crate::app::ui::{common, icons, style};
use crate::app::{AxisValueField, PickMode};
use crate::i18n::UiLanguage;
use crate::types::AxisUnit;
use egui::{Pos2, Rect};

use super::{ActionButton, axis_value::AxisValueInput};

struct CalibrationRowWidths {
    label: f32,
    pick: f32,
    value: f32,
}

impl CalibrationRowWidths {
    fn new(language: UiLanguage, available_width: f32) -> Self {
        let (label_width, pick_width) = match language {
            UiLanguage::En => (70.0, 82.0),
            UiLanguage::Ru => (82.0, 90.0),
        };
        let value_width = style::SPACE_ROW
            .mul_add(-2.0, available_width.max(220.0) - label_width - pick_width)
            .clamp(64.0, 110.0);
        Self {
            label: label_width,
            pick: pick_width,
            value: value_width,
        }
    }
}

pub(in crate::app) struct CalibrationRowResponse {
    pub(in crate::app) value_rect: Rect,
    pub(in crate::app) pick_rect: Rect,
    pub(in crate::app) requested_pick: Option<PickMode>,
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
pub(in crate::app) struct CalibrationRow<'a> {
    field: AxisValueField,
    value_text: &'a mut String,
    language: UiLanguage,
    unit: AxisUnit,
    point: Option<Pos2>,
}

impl<'a> CalibrationRow<'a> {
    pub(in crate::app) const fn new(
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

    pub(in crate::app) const fn unit(mut self, unit: AxisUnit) -> Self {
        self.unit = unit;
        self
    }

    pub(in crate::app) const fn point(mut self, point: Option<Pos2>) -> Self {
        self.point = point;
        self
    }

    pub(in crate::app) fn show(
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
            let value_label = ui
                .add_sized([label_width, row_height], egui::Label::new(value_label))
                .on_hover_ui(|ui| {
                    ui.label(match language {
                        UiLanguage::En => format!("Value of the calibration point ({name})"),
                        UiLanguage::Ru => format!("Значение калибровочной точки ({name})"),
                    });
                });
            let value_resp = ui.add_sized(
                [value_width, row_height],
                AxisValueInput::new(field, value_text, unit, language).pending_focus(pending_focus),
            );
            let value_resp = value_resp.labelled_by(value_label.id);

            let pick_button = match language {
                UiLanguage::En => format!("Pick {name}"),
                UiLanguage::Ru => format!("Выбрать {name}"),
            };
            let pick_resp = ui
                .add_sized(
                    [pick_width, row_height],
                    ActionButton::new(icons::ICON_PICK_POINT, pick_button)
                        .min_size(egui::vec2(pick_width, row_height)),
                )
                .on_hover_ui(|ui| {
                    ui.label(match language {
                        UiLanguage::En => format!("Click, then pick the {name} point on the image"),
                        UiLanguage::Ru => format!("Нажмите и выберите точку {name} на изображении"),
                    });
                });
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
}
