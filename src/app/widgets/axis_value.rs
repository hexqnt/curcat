//! Text input behavior; parsing and calibration validity belong to the model.

mod text;

use egui::{
    Response, TextEdit, Ui, Widget,
    text::{CCursor, CCursorRange},
};

use crate::{app::AxisValueField, i18n::UiLanguage, types::AxisUnit};
use text::FilteredAxisText;

pub(in crate::app) use text::sanitize_axis_text;

#[must_use = "Add the axis input to a UI with ui.add()."]
pub(in crate::app) struct AxisValueInput<'a> {
    field: AxisValueField,
    text: &'a mut String,
    unit: AxisUnit,
    language: UiLanguage,
    pending_focus: Option<&'a mut Option<AxisValueField>>,
}

impl<'a> AxisValueInput<'a> {
    pub(in crate::app) const fn new(
        field: AxisValueField,
        text: &'a mut String,
        unit: AxisUnit,
        language: UiLanguage,
    ) -> Self {
        Self {
            field,
            text,
            unit,
            language,
            pending_focus: None,
        }
    }

    pub(in crate::app) const fn pending_focus(
        mut self,
        focus: &'a mut Option<AxisValueField>,
    ) -> Self {
        self.pending_focus = Some(focus);
        self
    }
}

impl Widget for AxisValueInput<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let mut buffer = FilteredAxisText::new(self.text, self.unit);
        let mut output = TextEdit::singleline(&mut buffer)
            .id_salt(("axis_value", self.field))
            .show(ui);
        if let Some(pending) = self.pending_focus
            && *pending == Some(self.field)
        {
            output.response.request_focus();
            if !self.text.is_empty() {
                output.state.cursor.set_char_range(Some(CCursorRange::two(
                    CCursor::default(),
                    CCursor::new(self.text.chars().count()),
                )));
                output.state.store(ui.ctx(), output.response.id);
            }
            *pending = None;
        }
        output
            .response
            .response
            .on_hover_text(match (self.unit, self.language) {
                (AxisUnit::Float, UiLanguage::En) => "Enter a number (e.g., 1.23)",
                (AxisUnit::Float, UiLanguage::Ru) => "Введите число (например, 1.23)",
                (AxisUnit::DateTime, UiLanguage::En) => "Enter date/time (e.g., 2024-10-31 12:30)",
                (AxisUnit::DateTime, UiLanguage::Ru) => {
                    "Введите дату/время (например, 2024-10-31 12:30)"
                }
            })
    }
}
