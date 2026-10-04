use super::{advance, click};
use curcat::testing::UiLanguage;
use curcat_test_support::{AppHarness, click_image, state};
use egui::{Key, Modifiers, Visuals, accesskit::Role, pos2, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

fn edit_axis_value(harness: &mut AppHarness, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .scroll_to_me();
    advance(harness);
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    advance(harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    advance(harness);
}

#[test]
fn narrow_russian_layout_keeps_fields_and_actions_reachable_on_either_side() {
    for right_side in [false, true] {
        let mut harness = curcat_test_support::harness(2, UiLanguage::Ru);
        harness.set_size(vec2(900.0, 650.0));
        advance(&mut harness);
        if right_side {
            click(&mut harness, "Скрыть панель Ctrl+B");
            click(&mut harness, "Справа");
        }
        edit_axis_value(&mut harness, "Значение X1:", "NaN");
        assert_eq!(state(&harness).x1_text, "NaN");
        let export = "Экспорт CSV… Ctrl+Shift+C";
        harness.get_by_label(export).scroll_to_me();
        advance(&mut harness);
        let viewport = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 650.0));
        assert!(viewport.contains_rect(harness.get_by_label(export).rect()));
        assert!(harness.get_by_label(export).accesskit_node().is_disabled());
        edit_axis_value(&mut harness, "Значение X1:", "0");
        assert!(
            viewport.contains_rect(
                harness
                    .get_by_role_and_label(Role::TextInput, "Значение X1:")
                    .rect()
            )
        );
        assert!(!harness.get_by_label(export).accesskit_node().is_disabled());
        for (label, remaining) in [("Отменить Ctrl+Z", 1), ("Очистить точки Ctrl+Shift+D", 0)]
        {
            harness.get_by_label(label).scroll_to_me();
            advance(&mut harness);
            assert!(viewport.contains_rect(harness.get_by_label(label).rect()));
            click(&mut harness, label);
            assert_eq!(state(&harness).points, remaining);
        }
    }
}

#[test]
fn missing_image_disables_transform_info_filters_and_trace_actions() {
    for (language, appearance, info, filters, brightness, reset, trace) in [
        (
            UiLanguage::En,
            "Appearance",
            "Image info",
            "Image filters",
            "brightness",
            "Reset filters",
            "Trace from click",
        ),
        (
            UiLanguage::Ru,
            "Вид",
            "Инфо об изображении",
            "Фильтры изображения",
            "яркость",
            "Сбросить фильтры",
            "Трассировать от клика",
        ),
    ] {
        let mut harness =
            curcat_test_support::harness_with(move |ctx| curcat::testing::empty(ctx, language));
        assert!(
            harness
                .get_all_by_role_and_label(Role::Button, "90°")
                .all(|node| node.accesskit_node().is_disabled())
        );
        click(&mut harness, appearance);
        let switch = harness.get_by_role_and_label(Role::CheckBox, info);
        assert!(switch.accesskit_node().is_disabled());
        switch.click();
        advance(&mut harness);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::CheckBox, info)
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::False)
        );
        harness.key_press(Key::Escape);
        advance(&mut harness);
        harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
        advance(&mut harness);
        let window = harness.get_by_role_and_label(Role::Window, filters);
        assert!(
            window
                .get_by_role_and_label(Role::Slider, brightness)
                .accesskit_node()
                .is_disabled()
        );
        assert!(window.get_by_label(reset).accesskit_node().is_disabled());
        close_window(&mut harness, filters);
        harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::T);
        advance(&mut harness);
        assert!(harness.get_by_label(trace).accesskit_node().is_disabled());
        click(&mut harness, trace);
        assert!(!state(&harness).picking);
        assert!(state(&harness).image_size.is_none());
    }
}

#[test]
fn invalid_calibration_blocks_export_and_trace_even_with_points_and_snapping() {
    let mut harness = curcat_test_support::harness(2, UiLanguage::En);
    edit_axis_value(&mut harness, "X1 value:", "NaN");
    assert!(
        harness
            .get_by_label("Export CSV… Ctrl+Shift+C")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_value("Free").click();
    advance(&mut harness);
    click(&mut harness, "Contrast snap");
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::T);
    advance(&mut harness);
    assert!(
        harness
            .get_by_label("Trace from click")
            .accesskit_node()
            .is_disabled()
    );
    click(&mut harness, "Trace from click");
    assert!(!state(&harness).picking);
    close_window(&mut harness, "Auto-trace");
    edit_axis_value(&mut harness, "X1 value:", "0");
    assert!(
        !harness
            .get_by_label("Export CSV… Ctrl+Shift+C")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(state(&harness).points, 2);
}

#[track_caller]
fn click_caption(harness: &mut AppHarness, caption: &str) {
    harness.get_by_role_and_label(Role::Label, caption).click();
    advance(harness);
}

#[track_caller]
fn close_window(harness: &mut AppHarness, title: &str) {
    harness
        .get_by_role_and_label(Role::Window, title)
        .get_by_label("Close window")
        .click();
    advance(harness);
    assert!(
        harness
            .query_by_role_and_label(Role::Window, title)
            .is_none()
    );
}

#[test]
fn point_actions_follow_point_availability() {
    for (language, clear, undo, export) in [
        (
            UiLanguage::En,
            "Clear points Ctrl+Shift+D",
            "Undo Ctrl+Z",
            "Export CSV… Ctrl+Shift+C",
        ),
        (
            UiLanguage::Ru,
            "Очистить точки Ctrl+Shift+D",
            "Отменить Ctrl+Z",
            "Экспорт CSV… Ctrl+Shift+C",
        ),
    ] {
        let mut harness = curcat_test_support::harness(0, language);
        harness.set_size(vec2(1500.0, 1100.0));
        advance(&mut harness);
        for label in [clear, undo, export] {
            assert!(harness.get_by_label(label).accesskit_node().is_disabled());
        }
        click_image(&mut harness, pos2(300.0, 200.0));
        click_image(&mut harness, pos2(350.0, 220.0));
        advance(&mut harness);
        for label in [clear, undo, export] {
            assert!(!harness.get_by_label(label).accesskit_node().is_disabled());
        }
        click(&mut harness, undo);
        assert_eq!(state(&harness).points, 1);
        click(&mut harness, clear);
        assert_eq!(state(&harness).points, 0);
        for label in [clear, undo, export] {
            assert!(harness.get_by_label(label).accesskit_node().is_disabled());
        }
    }
}

#[test]
fn appearance_captions_toggle_windows_and_close_buttons_keep_menu_state_in_sync() {
    for (language, appearance, windows) in [
        (
            UiLanguage::En,
            "Appearance",
            [
                ("Points stats", "Points info", true),
                ("Filters", "Image filters", false),
                ("Auto-trace", "Auto-trace", false),
                ("Image info", "Image info", false),
            ],
        ),
        (
            UiLanguage::Ru,
            "Вид",
            [
                ("Статистика точек", "Информация о точках", true),
                ("Фильтры", "Фильтры изображения", false),
                ("Авто-трассировка", "Авто-трассировка", false),
                ("Инфо об изображении", "Информация об изображении", false),
            ],
        ),
    ] {
        for visuals in [Visuals::dark(), Visuals::light()] {
            let mut harness = curcat_test_support::harness(2, language);
            harness.ctx.set_visuals(visuals);
            advance(&mut harness);
            for (caption, title, collapsible) in windows {
                click(&mut harness, appearance);
                click_caption(&mut harness, caption);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Window, title)
                        .is_some()
                );
                click_caption(&mut harness, caption);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Window, title)
                        .is_none()
                );
                click_caption(&mut harness, caption);
                harness.key_press(Key::Escape);
                advance(&mut harness);
                let window = harness.get_by_role_and_label(Role::Window, title);
                assert_eq!(window.query_by_label("Hide").is_some(), collapsible);
                if collapsible {
                    window.get_by_label("Hide").click();
                    advance(&mut harness);
                    let window = harness.get_by_role_and_label(Role::Window, title);
                    window.get_by_label("Show").click();
                    advance(&mut harness);
                }
                close_window(&mut harness, title);
                click(&mut harness, appearance);
                click_caption(&mut harness, caption);
                assert!(
                    harness
                        .query_by_role_and_label(Role::Window, title)
                        .is_some()
                );
                harness.key_press(Key::Escape);
                advance(&mut harness);
                close_window(&mut harness, title);
            }
        }
    }
}

#[test]
fn auto_trace_action_requires_snapping_and_still_starts_pick_mode() {
    let mut harness = super::harness();
    let shortcut = Modifiers::COMMAND | Modifiers::SHIFT;
    harness.key_press_modifiers(shortcut, Key::T);
    advance(&mut harness);
    assert!(
        harness
            .get_by_label("Trace from click")
            .accesskit_node()
            .is_disabled()
    );
    harness.get_by_label("Close window").click();
    advance(&mut harness);
    harness.get_by_value("Free").click();
    advance(&mut harness);
    click(&mut harness, "Contrast snap");
    harness.key_press_modifiers(shortcut, Key::T);
    advance(&mut harness);
    assert!(
        !harness
            .get_by_label("Trace from click")
            .accesskit_node()
            .is_disabled()
    );
    click(&mut harness, "Trace from click");
    assert!(state(&harness).picking);
    harness.key_press(Key::Escape);
    advance(&mut harness);
    assert!(!state(&harness).picking);
}

#[test]
fn polar_calibration_uses_filtered_inputs_and_focuses_after_picking() {
    for (language, cartesian, polar, pick, value) in [
        (UiLanguage::En, "Cartesian", "Polar", "Pick R1", "R1 value:"),
        (
            UiLanguage::Ru,
            "Декартова",
            "Полярная",
            "Выбрать R1",
            "Значение R1:",
        ),
    ] {
        let mut harness = curcat_test_support::harness(0, language);
        harness.set_size(vec2(1500.0, 1100.0));
        advance(&mut harness);
        harness.get_by_value(cartesian).click();
        advance(&mut harness);
        click(&mut harness, polar);
        click(&mut harness, pick);
        assert!(state(&harness).picking);
        click_image(&mut harness, pos2(200.0, 320.0));
        advance(&mut harness);
        assert!(!state(&harness).picking);
        let field = harness.get_by_role_and_label(Role::TextInput, value);
        assert!(field.is_focused());
        field.type_text("12,34");
        advance(&mut harness);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, value)
                .value()
                .as_deref(),
            Some("12.34")
        );
    }
}
