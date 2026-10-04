use curcat::testing::UiLanguage;
use curcat_test_support::AppHarness;
use egui_kittest::kittest::Queryable as _;

mod accessibility;
mod controls;
mod dialogs;
mod image;
mod presentation;

fn harness() -> AppHarness {
    curcat_test_support::harness(0, UiLanguage::En)
}

fn advance(harness: &mut AppHarness) {
    // Status messages use wall-clock timers, so do not wait for all repaint requests to stop.
    harness.run_steps(3);
}

fn click(harness: &mut AppHarness, label: &str) {
    harness.get_by_label(label).click();
    advance(harness);
}
