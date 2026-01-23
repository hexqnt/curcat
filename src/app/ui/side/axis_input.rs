use super::super::icons;
use crate::app::{AxisValueField, CurcatApp, PickMode};
use crate::types::AxisUnit;
use egui::{
    Pos2, Rect, Response, TextBuffer, TextEdit,
    text::{CCursor, CCursorRange},
};
use std::any::TypeId;
use std::ops::Range;

/// Normalize axis input text by removing invalid characters and fixing decimals.
pub fn sanitize_axis_text(value: &mut String, unit: AxisUnit) {
    if value.is_empty() {
        return;
    }
    if matches!(unit, AxisUnit::Float) && value.contains(',') {
        *value = value.replace(',', ".");
    }
    value.retain(|ch| axis_char_allowed(unit, ch));
}

const fn axis_char_allowed(unit: AxisUnit, ch: char) -> bool {
    match unit {
        AxisUnit::Float => {
            ch.is_ascii_digit()
                || ch.is_ascii_whitespace()
                || matches!(ch, '+' | '-' | '.' | ',')
                || matches!(ch, 'e' | 'E')
                || matches!(ch, 'n' | 'N' | 'a' | 'A' | 'i' | 'I' | 'f' | 'F')
        }
        AxisUnit::DateTime => {
            ch.is_ascii_digit()
                || matches!(
                    ch,
                    '-' | '/' | '.' | ':' | ' ' | 'T' | 't' | '+' | 'Z' | 'z'
                )
        }
    }
}

struct AxisFilteredText<'a> {
    value: &'a mut String,
    unit: AxisUnit,
}

impl<'a> AxisFilteredText<'a> {
    const fn new(value: &'a mut String, unit: AxisUnit) -> Self {
        Self { value, unit }
    }
}

impl TextBuffer for AxisFilteredText<'_> {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.value.as_str()
    }

    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        let filtered: String = text
            .chars()
            .filter_map(|ch| {
                if !axis_char_allowed(self.unit, ch) {
                    return None;
                }
                let mapped = if matches!(self.unit, AxisUnit::Float) && ch == ',' {
                    '.'
                } else {
                    ch
                };
                Some(mapped)
            })
            .collect();
        if filtered.is_empty() {
            return 0;
        }
        let byte_idx = TextBuffer::byte_index_from_char_index(self, char_index);
        self.value.insert_str(byte_idx, &filtered);
        filtered.chars().count()
    }

    fn delete_char_range(&mut self, char_range: Range<usize>) {
        if char_range.start >= char_range.end {
            return;
        }
        let byte_start = TextBuffer::byte_index_from_char_index(self, char_range.start);
        let byte_end = TextBuffer::byte_index_from_char_index(self, char_range.end);
        self.value.drain(byte_start..byte_end);
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<AxisFilteredText<'static>>()
    }
}

pub(super) struct CalRowResult {
    pub(super) value_rect: Option<Rect>,
    pub(super) pick_rect: Option<Rect>,
    pub(super) requested_pick: Option<PickMode>,
}

impl CurcatApp {
    pub(super) fn render_calibration_row(
        ui: &mut egui::Ui,
        name: &str,
        unit: AxisUnit,
        value_text: &mut String,
        focus_target: AxisValueField,
        pending_focus: &mut Option<AxisValueField>,
        pick_mode: PickMode,
        point: Option<Pos2>,
    ) -> CalRowResult {
        let mut value_rect = None;
        let mut pick_rect = None;
        let mut requested_pick = None;

        ui.horizontal(|ui| {
            ui.label(format!("{name} value:"))
                .on_hover_text(format!("Value of the calibration point ({name})"));
            let value_resp = {
                let mut buffer = AxisFilteredText::new(value_text, unit);
                ui.add_sized(
                    [100.0, ui.spacing().interact_size.y],
                    TextEdit::singleline(&mut buffer),
                )
            };
            let value_resp = value_resp.on_hover_text(match unit {
                AxisUnit::Float => "Enter a number (e.g., 1.23)",
                AxisUnit::DateTime => "Enter date/time (e.g., 2024-10-31 12:30)",
            });
            Self::apply_pending_focus(pending_focus, focus_target, &value_resp, value_text);
            value_rect = Some(value_resp.rect);

            let pick_resp = ui
                .button(format!("{} Pick {name}", icons::ICON_PICK_POINT))
                .on_hover_text(format!("Click, then pick the {name} point on the image"));
            if pick_resp.clicked() {
                requested_pick = Some(pick_mode);
            }
            pick_rect = Some(pick_resp.rect);

            if let Some(p) = point {
                ui.label(format!("@ ({:.1},{:.1})", p.x, p.y));
            }
        });

        CalRowResult {
            value_rect,
            pick_rect,
            requested_pick,
        }
    }

    pub(super) fn apply_pending_focus(
        pending_focus: &mut Option<AxisValueField>,
        target: AxisValueField,
        response: &Response,
        text: &str,
    ) {
        if pending_focus.is_some_and(|pending| pending == target) {
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
