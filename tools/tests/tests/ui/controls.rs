use super::{advance, click, harness};
use curcat::testing::UiLanguage;
use curcat_test_support::{app, button, click_image, image_rect, state};
use egui::{Key, Modifiers, accesskit::Role, pos2, vec2};
use egui_kittest::kittest::Queryable as _;

#[test]
fn picking_calibration_point_focuses_value_and_escape_cancels_next_pick() {
    for (language, pick_x1, value_label, pick_x2) in [
        (UiLanguage::En, "Pick X1", "X1 value:", "Pick X2"),
        (UiLanguage::Ru, "Выбрать X1", "Значение X1:", "Выбрать X2"),
    ] {
        let mut harness = curcat_test_support::harness(0, language);
        click(&mut harness, pick_x1);
        assert!(state(&harness).picking);
        let pixel = pos2(200.0, 320.0);
        click_image(&mut harness, pixel);
        advance(&mut harness);
        assert!(!state(&harness).picking);
        assert_eq!(state(&harness).x1, Some(pixel));
        let field = harness.get_by_role_and_label(Role::TextInput, value_label);
        assert!(field.is_focused());
        field.type_text("12,34");
        advance(&mut harness);
        assert_eq!(state(&harness).x1_text, "12.34");
        click(&mut harness, pick_x2);
        assert!(state(&harness).picking);
        harness.key_press(Key::Escape);
        advance(&mut harness);
        assert!(!state(&harness).picking);
    }
}

#[test]
fn undo_shortcut_removes_clicked_point_and_resize_preserves_points() {
    let mut harness = harness();
    let first = pos2(300.0, 200.0);
    let second = pos2(350.0, 220.0);
    click_image(&mut harness, first);
    click_image(&mut harness, second);
    assert_eq!(state(&harness).points, 2);
    harness.set_size(vec2(900.0, 650.0));
    advance(&mut harness);
    assert_eq!(state(&harness).points, 2);
    assert_eq!(app(&harness).point_pixel(0), Some(first));
    assert_eq!(app(&harness).point_pixel(1), Some(second));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    advance(&mut harness);
    assert_eq!(state(&harness).points, 1);
    assert_eq!(app(&harness).point_pixel(0), Some(first));
}

#[test]
fn filter_window_blocks_clicks_on_the_image_underneath() {
    let mut harness = harness();
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    advance(&mut harness);
    let window = harness
        .get_by_role_and_label(Role::Window, "Image filters")
        .rect();
    let overlap = window.intersect(image_rect(&harness)).shrink(8.0);
    assert!(overlap.is_positive());
    let pos = overlap.center();
    button(&mut harness, pos, true, Modifiers::NONE);
    button(&mut harness, pos, false, Modifiers::NONE);
    assert_eq!(state(&harness).points, 0);
    assert!(!state(&harness).holding);
}
