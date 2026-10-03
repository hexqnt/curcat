use curcat::testing::UiLanguage;
use curcat_test_support::{ZOOM_CYCLE_FRAMES, ZOOM_LEG_FRAMES, harness, state, zoom};

#[path = "../../workloads/zoom.rs"]
mod workload;

#[test]
fn zoom_scenario_moves_in_both_directions_and_preserves_points() {
    let mut harness = harness(100, UiLanguage::En);
    harness.run_steps(8);
    let start = state(&harness).zoom;
    for frame in 0..ZOOM_LEG_FRAMES {
        zoom(&mut harness, frame);
    }
    let peak = state(&harness).zoom;
    assert!(peak > start + 0.1);
    for frame in ZOOM_LEG_FRAMES..ZOOM_CYCLE_FRAMES {
        zoom(&mut harness, frame);
    }
    let state = state(&harness);
    assert!(state.zoom < peak - 0.1);
    assert_eq!(state.points, 100);
}

#[test]
#[ignore = "Optimized CPU workload: run ./profiling.sh ui"]
fn profile_zoom() {
    workload::profile().expect("Profiling workload failed");
}
