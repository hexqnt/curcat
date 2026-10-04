//! Shared deterministic fixtures and input events for UI tests and CPU profiling.

#![expect(
    clippy::missing_panics_doc,
    reason = "Invalid fixtures should fail the scenario immediately."
)]

use curcat::{
    CurcatApp,
    testing::{self, AppState, UiLanguage},
};
use egui::{Color32, ColorImage, Event, Modifiers, PointerButton, Pos2, Vec2};
use egui_kittest::Harness;

pub const STEP_DT: f32 = 1.0 / 60.0;
pub const VIEWPORT_SIZE: [u16; 2] = [1200, 800];
pub const IMAGE_SIZE: [usize; 2] = [640, 480];
pub const ZOOM_LEG_FRAMES: u32 = 60;
pub const ZOOM_CYCLE_FRAMES: u32 = 2 * ZOOM_LEG_FRAMES;
pub type AppHarness = Harness<'static, Option<CurcatApp>>;

#[must_use]
pub fn harness(points: u32, language: UiLanguage) -> AppHarness {
    harness_with(move |ctx| {
        let pixels = (0..points).map(|index| {
            let fraction = f64::from(index) / f64::from(points);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Fixture coordinates fit in f32."
            )]
            egui::pos2(
                fraction.mul_add(480.0, 80.0) as f32,
                (fraction * 20.0).sin().mul_add(100.0, 240.0) as f32,
            )
        });
        testing::from_image(
            ctx,
            ColorImage::filled(IMAGE_SIZE, Color32::WHITE),
            pixels,
            language,
        )
    })
}

/// Reuse the deterministic harness for fixtures with different initial app state.
#[must_use]
pub fn harness_with(
    mut create_app: impl FnMut(&egui::Context) -> CurcatApp + 'static,
) -> AppHarness {
    Harness::builder()
        .with_size(VIEWPORT_SIZE.map(f32::from))
        .with_step_dt(STEP_DT)
        .with_max_steps(60)
        .build_ui_state(
            move |ui, app: &mut Option<CurcatApp>| {
                let app = app.get_or_insert_with(|| create_app(ui.ctx()));
                testing::render(app, ui);
            },
            None,
        )
}

#[must_use]
pub fn app(harness: &AppHarness) -> &CurcatApp {
    harness
        .state()
        .as_ref()
        .expect("App fixture must be initialized")
}

#[must_use]
pub fn state(harness: &AppHarness) -> AppState<'_> {
    app(harness).inspect()
}

#[must_use]
pub fn image_rect(harness: &AppHarness) -> egui::Rect {
    state(harness)
        .image_rect
        .expect("Fixture image must be visible")
}

#[must_use]
pub fn screen_pixel(harness: &AppHarness, pixel: Pos2) -> Pos2 {
    let state = state(harness);
    state.image_rect.expect("Fixture image must be visible").min + pixel.to_vec2() * state.zoom
}

pub fn move_pointer(harness: &mut AppHarness, pos: Pos2) {
    harness.input_mut().events.push(Event::PointerMoved(pos));
    harness.step();
}

pub fn button(harness: &mut AppHarness, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    harness.input_mut().events.extend([
        Event::ModifiersChanged(modifiers),
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers,
        },
    ]);
    harness.step();
}

pub fn click_image(harness: &mut AppHarness, pixel: Pos2) {
    let pos = screen_pixel(harness, pixel);
    button(harness, pos, true, Modifiers::NONE);
    button(harness, pos, false, Modifiers::NONE);
}

/// Alternate zoom direction while keeping the pointer over the image.
pub fn zoom(harness: &mut AppHarness, frame: u32) {
    let direction = if frame % ZOOM_CYCLE_FRAMES < ZOOM_LEG_FRAMES {
        1.0
    } else {
        -1.0
    };
    let pos = image_rect(harness).center();
    harness.input_mut().events.extend([
        Event::ModifiersChanged(Modifiers::NONE),
        Event::PointerMoved(pos),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: Vec2::new(0.0, direction * 4.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    harness.step();
}
