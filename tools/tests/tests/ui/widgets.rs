use super::{advance, click};
use curcat::testing::UiLanguage;
use curcat_test_support::{button, image_rect, move_pointer, state};
use egui::{
    Key, Modifiers, Visuals,
    accesskit::{Role, Toggled},
    vec2,
};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

#[test]
fn language_selection_preserves_collapsed_axis_identity() {
    let mut harness = curcat_test_support::harness(0, UiLanguage::En);
    click(&mut harness, "X axis");
    assert!(harness.query_by_label("X1 value:").is_none());
    click(&mut harness, "UI language");
    harness
        .get_by_role_and_label(Role::RadioButton, "Русский")
        .focus();
    advance(&mut harness);
    harness.key_press(Key::Enter);
    advance(&mut harness);
    assert!(harness.query_by_label("Значение X1:").is_none());
    assert!(harness.query_by_label("Значение Y1:").is_some());
    click(&mut harness, "Язык интерфейса");
    click(&mut harness, "English");
    assert!(harness.query_by_label("X1 value:").is_none());
    click(&mut harness, "X axis");
    assert!(harness.query_by_label("X1 value:").is_some());
}

#[test]
fn rotation_buttons_have_distinct_names_and_support_keyboard_activation() {
    for (language, clockwise, counterclockwise) in [
        (
            UiLanguage::En,
            "Rotate 90° clockwise.",
            "Rotate 90° counter-clockwise.",
        ),
        (
            UiLanguage::Ru,
            "Повернуть на 90° по часовой стрелке.",
            "Повернуть на 90° против часовой стрелки.",
        ),
    ] {
        let mut harness = curcat_test_support::harness(0, language);
        harness
            .get_by_role_and_label(Role::Button, clockwise)
            .focus();
        advance(&mut harness);
        harness.key_press(Key::Space);
        advance(&mut harness);
        assert_eq!(state(&harness).image_size, Some([480, 640]));
        click(&mut harness, counterclockwise);
        assert_eq!(state(&harness).image_size, Some([640, 480]));
        assert_eq!(state(&harness).points, 0);
    }
}

#[test]
fn choices_keep_axis_units_independent_and_hide_log_scale_for_dates() {
    let mut harness = curcat_test_support::harness(0, UiLanguage::En);
    harness.get_all_by_value("Float").next().unwrap().click();
    advance(&mut harness);
    click(&mut harness, "DateTime");
    assert_eq!(harness.get_all_by_value("Float").count(), 1);
    assert_eq!(harness.get_all_by_value("DateTime").count(), 1);
    harness.get_all_by_value("Linear").next().unwrap().click();
    advance(&mut harness);
    assert!(harness.query_by_label("Log10").is_none());
    harness.key_press(Key::Escape);
    advance(&mut harness);
    harness.get_by_value("DateTime").click();
    advance(&mut harness);
    click(&mut harness, "Float");
    let linear_count = harness.get_all_by_value("Linear").count();
    harness.get_all_by_value("Linear").next().unwrap().click();
    advance(&mut harness);
    click(&mut harness, "Log10");
    assert_eq!(harness.get_all_by_value("Log10").count(), 1);
    assert_eq!(harness.get_all_by_value("Linear").count(), linear_count - 1);
}

#[test]
fn palette_keyboard_selection_keeps_every_swatch_visible_and_reports_selection() {
    for visuals in [Visuals::dark(), Visuals::light()] {
        let mut harness = curcat_test_support::harness(0, UiLanguage::En);
        harness.ctx.set_visuals(visuals);
        advance(&mut harness);
        harness.get_by_value("Free").click();
        advance(&mut harness);
        click(&mut harness, "Contrast snap");
        let labels = [
            "RGB 236, 214, 96",
            "RGB 66, 123, 176",
            "RGB 184, 102, 128",
            "RGB 72, 138, 96",
        ];
        let choice = labels[1];
        harness
            .get_by_role_and_label(Role::RadioButton, choice)
            .focus();
        advance(&mut harness);
        assert!(
            harness
                .get_by_role_and_label(Role::RadioButton, choice)
                .is_focused()
        );
        harness.key_press(Key::Space);
        advance(&mut harness);
        harness.key_press(Key::Enter);
        advance(&mut harness);
        for label in labels {
            let node = harness.get_by_role_and_label(Role::RadioButton, label);
            assert_eq!(
                node.accesskit_node().toggled(),
                Some(if label == choice {
                    Toggled::True
                } else {
                    Toggled::False
                })
            );
        }
        assert_eq!(state(&harness).points, 0);
    }
}

#[test]
fn zoom_selector_and_navigator_pan_without_placing_points() {
    let mut harness = curcat_test_support::harness(0, UiLanguage::En);
    harness.get_by_value("100%").click();
    advance(&mut harness);
    click(&mut harness, "200%");
    assert!((state(&harness).zoom - 2.0).abs() < f32::EPSILON);
    let before = image_rect(&harness).min;
    let navigator = harness
        .get_by_label("Navigator: click or drag to pan the viewport")
        .rect();
    let start = navigator.center();
    button(&mut harness, start, true, Modifiers::NONE);
    move_pointer(&mut harness, start + vec2(35.0, 25.0));
    button(
        &mut harness,
        start + vec2(35.0, 25.0),
        false,
        Modifiers::NONE,
    );
    advance(&mut harness);
    let after = image_rect(&harness).min;
    assert!(after.x < before.x);
    assert!(after.y < before.y);
    assert_eq!(state(&harness).points, 0);
    harness.get_by_value("200%").click();
    advance(&mut harness);
    click(&mut harness, "Reset view Ctrl+R");
    assert!((state(&harness).zoom - 1.0).abs() < f32::EPSILON);
}
