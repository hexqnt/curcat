//! Shared workload for the profiling example and the ignored timing test.

use std::{env, error::Error, num::NonZeroU32, path::PathBuf, time::Instant};

use curcat::testing::UiLanguage;
use curcat_test_support::{
    IMAGE_SIZE, STEP_DT, VIEWPORT_SIZE, ZOOM_CYCLE_FRAMES, harness, state, zoom,
};

const INITIAL_FRAMES: usize = 8;

struct Settings {
    frames: NonZeroU32,
    points: NonZeroU32,
    output: Option<PathBuf>,
}

impl Settings {
    fn from_env() -> Result<Self, Box<dyn Error>> {
        let frames = setting("CURCAT_PROFILE_FRAMES", 2400)?;
        if frames.get() < ZOOM_CYCLE_FRAMES {
            return Err(format!("CURCAT_PROFILE_FRAMES must be at least {ZOOM_CYCLE_FRAMES} to measure both zoom directions").into());
        }
        Ok(Self {
            frames,
            points: setting("CURCAT_PROFILE_POINTS", 1000)?,
            output: env::var_os("CURCAT_PROFILE_OUTPUT").map(PathBuf::from),
        })
    }
}

fn setting(name: &str, default: u32) -> Result<NonZeroU32, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| format!("{name} must be a positive integer").into()),
        Err(env::VarError::NotPresent) => {
            Ok(NonZeroU32::new(default).expect("Default must be positive"))
        }
        Err(error) => Err(format!("Invalid {name}: {error}").into()),
    }
}

/// Measure the shared zoom scenario after warming up the harness.
///
/// # Errors
/// Returns an error for invalid settings, output failures, or a broken zoom scenario.
///
/// # Panics
/// Panics if the in-memory app fixture cannot be initialized.
pub fn profile() -> Result<(), Box<dyn Error>> {
    let settings = Settings::from_env()?;
    if let Some(parent) = settings.output.as_deref().and_then(std::path::Path::parent)
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create output directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let frames = settings.frames.get();
    let points = settings.points.get();
    let expected_points = usize::try_from(points)?;
    let mut harness = harness(points, UiLanguage::En);
    harness.run_steps(INITIAL_FRAMES);
    for frame in 0..ZOOM_CYCLE_FRAMES {
        zoom(&mut harness, frame);
    }
    let mut samples = Vec::with_capacity(usize::try_from(frames)?);
    let mut min_zoom = f32::MAX;
    let mut max_zoom = f32::MIN;
    for frame in 0..frames {
        let start = Instant::now();
        zoom(&mut harness, frame);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        let zoom = state(&harness).zoom;
        min_zoom = min_zoom.min(zoom);
        max_zoom = max_zoom.max(zoom);
    }
    if max_zoom - min_zoom <= 0.1 {
        return Err("Zoom scenario did not move".into());
    }
    if state(&harness).points != expected_points {
        return Err("Zoom scenario changed picked points".into());
    }
    let total: f64 = samples.iter().sum();
    samples.sort_unstable_by(f64::total_cmp);
    let percentile = |percent: usize| samples[(samples.len() - 1) * percent / 100];
    let result = serde_json::json!({
        "schema": 1,
        "scenario": "image-zoom-v1",
        "points": points,
        "frames": frames,
        "viewport": VIEWPORT_SIZE,
        "image": IMAGE_SIZE,
        "step_dt": STEP_DT,
        "warmup_frames": INITIAL_FRAMES + usize::try_from(ZOOM_CYCLE_FRAMES)?,
        "zoom_span": max_zoom - min_zoom,
        "mean_ms": total / f64::from(frames),
        "p50_ms": percentile(50),
        "p95_ms": percentile(95),
        "p99_ms": percentile(99),
        "max_ms": samples.last().expect("Frame count is positive"),
        "total_ms": total,
    });
    let json = serde_json::to_string_pretty(&result)?;
    if let Some(path) = settings.output {
        std::fs::write(&path, &json).map_err(|error| {
            format!(
                "Failed to write profiling result to {}: {error}",
                path.display()
            )
        })?;
    }
    println!("{json}");
    Ok(())
}
