use super::harness;
use curcat_test_support::{app, button, image_rect, move_pointer, screen_pixel, state};
use egui::{Modifiers, pos2, vec2};

#[test]
fn auto_place_ignores_presses_started_outside_image() {
    let mut harness = harness();
    let outside = image_rect(&harness).min - vec2(20.0, 0.0);
    button(&mut harness, outside, true, Modifiers::NONE);
    assert!(!state(&harness).holding);
    let inside = image_rect(&harness).center();
    move_pointer(&mut harness, inside);
    assert!(!state(&harness).holding);
    assert!(!state(&harness).auto_placing);
    button(&mut harness, inside, false, Modifiers::NONE);
    assert_eq!(state(&harness).points, 0);
}

#[test]
fn auto_place_starts_hold_for_image_press_and_resets_on_release() {
    let mut harness = harness();
    let inside = image_rect(&harness).center();
    button(&mut harness, inside, true, Modifiers::NONE);
    assert!(state(&harness).holding);
    button(&mut harness, inside, false, Modifiers::NONE);
    assert!(!state(&harness).holding);
    assert!(!state(&harness).auto_placing);
    assert_eq!(state(&harness).points, 1);
}

#[test]
fn auto_place_resets_hold_when_pointer_leaves_image() {
    let mut harness = harness();
    let inside = image_rect(&harness).center();
    button(&mut harness, inside, true, Modifiers::NONE);
    assert!(state(&harness).holding);
    let outside = image_rect(&harness).min - vec2(20.0, 0.0);
    move_pointer(&mut harness, outside);
    assert!(!state(&harness).holding);
    assert!(!state(&harness).auto_placing);
    button(&mut harness, outside, false, Modifiers::NONE);
    assert_eq!(state(&harness).points, 0);
}

#[test]
fn short_release_inside_image_accepts_small_pointer_movement() {
    let mut harness = harness();
    let start = image_rect(&harness).center();
    let end = start + vec2(2.0, 1.0);
    button(&mut harness, start, true, Modifiers::NONE);
    button(&mut harness, end, false, Modifiers::NONE);
    assert_eq!(state(&harness).points, 1);
    let pixel = app(&harness).point_pixel(0).unwrap();
    assert!((screen_pixel(&harness, pixel) - end).length() < 0.01);
}

#[test]
fn release_outside_image_does_not_add_point() {
    let mut harness = harness();
    let rect = image_rect(&harness);
    button(
        &mut harness,
        rect.min + vec2(2.0, 20.0),
        true,
        Modifiers::NONE,
    );
    button(
        &mut harness,
        rect.min + vec2(-2.0, 20.0),
        false,
        Modifiers::NONE,
    );
    assert_eq!(state(&harness).points, 0);
}

#[test]
fn calibration_drag_keeps_snap_guides_and_clears_them_away_from_targets() {
    let mut harness = harness();
    let start = screen_pixel(&harness, state(&harness).x1.unwrap());
    button(&mut harness, start, true, Modifiers::SHIFT);
    move_pointer(&mut harness, start - vec2(8.0, 0.0));
    let extension = screen_pixel(&harness, pos2(40.0, 433.0));
    move_pointer(&mut harness, extension);
    assert!(state(&harness).guides > 0);
    let snapped = state(&harness).x1.unwrap();
    assert!((snapped - pos2(40.0, 432.0)).length() < 0.01);
    let free = screen_pixel(&harness, pos2(200.0, 300.0));
    move_pointer(&mut harness, free);
    assert_eq!(state(&harness).guides, 0);
    assert!((state(&harness).x1.unwrap() - pos2(200.0, 300.0)).length() < 0.01);
    button(&mut harness, free, false, Modifiers::SHIFT);
    assert_eq!(state(&harness).points, 0);
}
