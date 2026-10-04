use curcat::testing::UiLanguage;
use curcat_test_support::{AppHarness, image_rect, state};
use egui::{
    Color32, ColorImage, Event, Key, Modifiers, MouseWheelUnit, TouchPhase, accesskit::Role,
    style::ScrollStyle, vec2,
};
use egui_kittest::kittest::Queryable as _;

// Allow zoom and scrollbar visibility animations to finish at the harness frame rate.
const SETTLE_FRAMES: usize = 30;

fn image_scrollbar_counts(harness: &AppHarness) -> [usize; 2] {
    let viewport = state(harness).image_viewport.unwrap();
    let mut bars = [0, 0];
    for bar in harness.get_all_by_role(Role::ScrollBar) {
        let rect = bar.rect();
        if rect.min.x >= viewport.min.x && rect.min.y >= viewport.min.y {
            bars[usize::from(rect.height() > rect.width())] += 1;
        }
    }
    bars
}

#[test]
fn image_scrollbars_remain_stable_when_zooming_out_and_fitting() {
    for (style_name, style) in [
        ("floating", ScrollStyle::floating()),
        ("solid", ScrollStyle::solid()),
        ("thin", ScrollStyle::thin()),
    ] {
        for pixels_per_point in [1.0, 1.25, 1.5, 2.0] {
            let mut harness = curcat_test_support::harness_with(move |ctx| {
                ctx.all_styles_mut(|settings| settings.spacing.scroll = style);
                let mut app = curcat::testing::from_image(
                    ctx,
                    ColorImage::filled([700, 1000], Color32::WHITE),
                    [],
                    UiLanguage::En,
                );
                app.set_smooth_zoom(true);
                app
            });
            harness.set_pixels_per_point(pixels_per_point);
            harness.run_steps(SETTLE_FRAMES);
            let mut previous_bars = image_scrollbar_counts(&harness);
            for frame in 0..120 {
                let pointer = state(&harness).image_viewport.unwrap().center();
                harness.input_mut().events.extend([
                    Event::PointerMoved(pointer),
                    Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        phase: TouchPhase::Move,
                        delta: vec2(0.0, -12.0),
                        modifiers: Modifiers::NONE,
                    },
                ]);
                harness.step();
                let bars = image_scrollbar_counts(&harness);
                for (axis, (current, previous)) in bars.into_iter().zip(previous_bars).enumerate() {
                    assert!(
                        current <= previous,
                        "Scrollbar reappeared: scale {pixels_per_point}, style {style_name}, frame {frame}, axis {axis}",
                    );
                }
                previous_bars = bars;
            }
            harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
            harness.run_steps(SETTLE_FRAMES);
            let settled_image = image_rect(&harness);
            let settled_viewport = state(&harness).image_viewport;
            for frame in 0..60 {
                harness.step();
                assert_eq!(
                    image_scrollbar_counts(&harness),
                    [0, 0],
                    "Scale: {pixels_per_point}, style: {style_name}, frame: {frame}"
                );
                assert_eq!(image_rect(&harness), settled_image);
                assert_eq!(state(&harness).image_viewport, settled_viewport);
            }
        }
    }
}

#[test]
fn image_scrollbars_only_appear_on_overflowing_axes() {
    for (size, expected_bars) in [
        ([200, 150], [0, 0]),
        ([1600, 150], [1, 0]),
        ([200, 1200], [0, 1]),
        ([1600, 1200], [1, 1]),
    ] {
        let mut harness = curcat_test_support::harness_with(move |ctx| {
            curcat::testing::from_image(
                ctx,
                ColorImage::filled(size, Color32::WHITE),
                [],
                UiLanguage::En,
            )
        });
        harness.run_steps(SETTLE_FRAMES);
        assert_eq!(
            image_scrollbar_counts(&harness),
            expected_bars,
            "Image size: {size:?}"
        );
    }
}
