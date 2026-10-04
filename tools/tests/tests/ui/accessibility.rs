use super::{advance, click};
use curcat::testing::UiLanguage;
use curcat_test_support::{AppHarness, harness};
use egui::{
    Key, Modifiers,
    accesskit::{Role, Toggled},
};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

fn checked(harness: &AppHarness, label: &str) -> bool {
    match harness
        .get_by_role_and_label(Role::CheckBox, label)
        .accesskit_node()
        .toggled()
    {
        Some(Toggled::True) => true,
        Some(Toggled::False) => false,
        other => panic!("Expected a binary switch state, got {other:?}"),
    }
}

#[test]
fn switches_report_state_and_support_keyboard_focus_and_activation() {
    for (language, labels) in [
        (
            UiLanguage::En,
            [
                "MMB pan",
                "Show point connections",
                "Show calibration overlay",
            ],
        ),
        (
            UiLanguage::Ru,
            [
                "Панорам. СКМ",
                "Показывать соединения точек",
                "Показывать калибровочный оверлей",
            ],
        ),
    ] {
        let mut harness = harness(0, language);
        for label in labels {
            let original = checked(&harness, label);
            let switch = harness.get_by_role_and_label(Role::CheckBox, label);
            switch.scroll_to_me();
            advance(&mut harness);
            harness.get_by_role_and_label(Role::CheckBox, label).focus();
            advance(&mut harness);
            assert!(
                harness
                    .get_by_role_and_label(Role::CheckBox, label)
                    .is_focused()
            );
            harness.key_press(Key::Space);
            advance(&mut harness);
            assert_eq!(checked(&harness, label), !original);
            harness.key_press(Key::Enter);
            advance(&mut harness);
            assert_eq!(checked(&harness, label), original);
            harness.key_press(Key::Tab);
            advance(&mut harness);
            assert!(
                !harness
                    .get_by_role_and_label(Role::CheckBox, label)
                    .is_focused()
            );
            harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
            advance(&mut harness);
            assert!(
                harness
                    .get_by_role_and_label(Role::CheckBox, label)
                    .is_focused()
            );
            harness.get_by_role_and_label(Role::CheckBox, label).click();
            advance(&mut harness);
            assert_eq!(checked(&harness, label), !original);
        }
    }
}

#[test]
fn appearance_switch_and_caption_control_the_same_window() {
    let mut harness = harness(0, UiLanguage::En);
    click(&mut harness, "Appearance");
    assert!(!checked(&harness, "Filters"));
    harness
        .get_by_role_and_label(Role::CheckBox, "Filters")
        .focus();
    advance(&mut harness);
    harness.key_press(Key::Space);
    advance(&mut harness);
    assert!(checked(&harness, "Filters"));
    assert!(
        harness
            .query_by_role_and_label(Role::Window, "Image filters")
            .is_some()
    );
    harness
        .get_by_role_and_label(Role::Label, "Filters")
        .click();
    advance(&mut harness);
    assert!(!checked(&harness, "Filters"));
    assert!(
        harness
            .query_by_role_and_label(Role::Window, "Image filters")
            .is_none()
    );
}

#[test]
fn cad_snap_group_switch_updates_both_child_switches() {
    let mut harness = harness(0, UiLanguage::En);
    harness.get_by_label_contains("CAD snap (").click();
    advance(&mut harness);
    assert!(checked(&harness, "Point snaps"));
    harness
        .get_by_role_and_label(Role::CheckBox, "Point snaps")
        .click();
    advance(&mut harness);
    assert!(!checked(&harness, "END"));
    assert!(!checked(&harness, "INT"));
    harness
        .get_by_role_and_label(Role::CheckBox, "Point snaps")
        .focus();
    advance(&mut harness);
    harness.key_press(Key::Space);
    advance(&mut harness);
    assert!(checked(&harness, "Point snaps"));
    assert!(checked(&harness, "END"));
    assert!(checked(&harness, "INT"));
}
