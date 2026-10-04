use egui::{ComboBox, Response, Ui, Widget};

pub(in crate::app) trait ChoiceHint<T> {
    fn hint(&self, value: T) -> Option<&'static str>;
}

pub(in crate::app) struct NoHints;

impl<T> ChoiceHint<T> for NoHints {
    fn hint(&self, _value: T) -> Option<&'static str> {
        None
    }
}

impl<T, F: Fn(T) -> &'static str> ChoiceHint<T> for F {
    fn hint(&self, value: T) -> Option<&'static str> {
        Some(self(value))
    }
}

/// Labels and optional hints use static dispatch; options are borrowed.
#[must_use = "Add the choice to a UI with ui.add()."]
pub(in crate::app) struct Choice<'a, T, L, H = NoHints> {
    combo: ComboBox,
    selected: &'a mut T,
    values: &'a [T],
    label: L,
    hints: H,
}

impl<'a, T, L> Choice<'a, T, L> {
    pub(in crate::app) fn new(
        id: impl egui::AsIdSalt,
        selected: &'a mut T,
        values: &'a [T],
        label: L,
    ) -> Self {
        Self {
            combo: ComboBox::from_id_salt(id),
            selected,
            values,
            label,
            hints: NoHints,
        }
    }
}

impl<'a, T, L, H> Choice<'a, T, L, H> {
    pub(in crate::app) fn hints<F: Fn(T) -> &'static str>(self, hints: F) -> Choice<'a, T, L, F> {
        Choice {
            combo: self.combo,
            selected: self.selected,
            values: self.values,
            label: self.label,
            hints,
        }
    }
}

impl<T: Copy + PartialEq, L: Fn(T) -> &'static str, H: ChoiceHint<T>> Widget
    for Choice<'_, T, L, H>
{
    fn ui(self, ui: &mut Ui) -> Response {
        let before = *self.selected;
        let mut response = self
            .combo
            .selected_text((self.label)(before))
            .show_ui(ui, |ui| {
                for &value in self.values {
                    let response = ui.selectable_value(self.selected, value, (self.label)(value));
                    if let Some(hint) = self.hints.hint(value) {
                        response.on_hover_text(hint);
                    }
                }
            })
            .response;
        if before != *self.selected {
            response.mark_changed();
        }
        response
    }
}
