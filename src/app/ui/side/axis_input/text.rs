//! Character filtering for calibration text buffers.

use crate::types::AxisUnit;
use egui::{TextBuffer, text::CharIndex};
use std::{any::TypeId, borrow::Cow};

/// Normalize axis input text by removing invalid characters and fixing decimals.
pub(in crate::app::ui::side) fn sanitize_axis_text(value: &mut String, unit: AxisUnit) {
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

/// Borrow unchanged input; allocate a buffer only when characters need filtering.
fn filtered_axis_text(text: &str, unit: AxisUnit) -> Cow<'_, str> {
    let map_char = |ch| {
        axis_char_allowed(unit, ch).then_some(if unit == AxisUnit::Float && ch == ',' {
            '.'
        } else {
            ch
        })
    };
    let Some((first_changed, _)) = text
        .char_indices()
        .find(|&(_, ch)| map_char(ch) != Some(ch))
    else {
        return Cow::Borrowed(text);
    };
    let mut filtered = String::with_capacity(text.len());
    filtered.push_str(&text[..first_changed]);
    filtered.extend(text[first_changed..].chars().filter_map(map_char));
    Cow::Owned(filtered)
}

pub(super) struct FilteredAxisText<'a> {
    value: &'a mut String,
    unit: AxisUnit,
}

impl<'a> FilteredAxisText<'a> {
    pub(super) const fn new(value: &'a mut String, unit: AxisUnit) -> Self {
        Self { value, unit }
    }
}

impl TextBuffer for FilteredAxisText<'_> {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.value.as_str()
    }

    fn insert_text(&mut self, text: &str, char_index: CharIndex) -> usize {
        let filtered = filtered_axis_text(text, self.unit);
        if filtered.is_empty() {
            return 0;
        }
        let byte_idx = TextBuffer::byte_index_from_char_index(self, char_index);
        self.value.insert_str(byte_idx.into(), &filtered);
        filtered.chars().count()
    }

    fn delete_char_range(&mut self, char_range: std::ops::Range<CharIndex>) {
        if char_range.start >= char_range.end {
            return;
        }
        let byte_start = TextBuffer::byte_index_from_char_index(self, char_range.start);
        let byte_end = TextBuffer::byte_index_from_char_index(self, char_range.end);
        self.value
            .drain(usize::from(byte_start)..usize::from(byte_end));
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<FilteredAxisText<'static>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_axis_input_is_borrowed() {
        for (unit, text) in [
            (AxisUnit::Float, ""),
            (AxisUnit::Float, "-1.25e+3"),
            (AxisUnit::Float, " NaN inf\t"),
            (AxisUnit::DateTime, "2026-09-22T12:30:45+03:00"),
        ] {
            let filtered = filtered_axis_text(text, unit);
            assert!(matches!(filtered, Cow::Borrowed(_)));
            assert_eq!(filtered, text);
        }
    }

    #[test]
    fn pasted_axis_input_preserves_filtering_rules() {
        for (unit, input, expected) in [
            (AxisUnit::Float, "12,34", "12.34"),
            (AxisUnit::Float, "θ=−12,3 📈", "12.3 "),
            (AxisUnit::Float, "abc123", "a123"),
            (
                AxisUnit::DateTime,
                "2026/09/22\t12:30\nZ",
                "2026/09/2212:30Z",
            ),
            (AxisUnit::DateTime, "1,5", "15"),
            (AxisUnit::DateTime, "текст📈", ""),
        ] {
            assert_eq!(filtered_axis_text(input, unit), expected);
        }
    }
}
