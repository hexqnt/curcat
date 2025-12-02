use std::fs;
use std::path::PathBuf;

use directories::{BaseDirs, ProjectDirs};
use egui::{Color32, Stroke};
use serde::Deserialize;

const CONFIG_FILE_NAME: &str = "curcat.toml";

fn alpha_to_u8(alpha: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StrokeStyle {
    pub color: [u8; 3],
    pub alpha: f32,
    pub thickness: f32,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: [80, 200, 120],
            alpha: 1.0,
            thickness: 2.0,
        }
    }
}

impl StrokeStyle {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.color[0],
            self.color[1],
            self.color[2],
            alpha_to_u8(self.alpha),
        )
    }

    pub fn stroke(&self) -> Stroke {
        Stroke {
            width: self.thickness.max(0.1),
            color: self.color32(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PointStyle {
    pub color: [u8; 3],
    pub alpha: f32,
    pub radius: f32,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self {
            color: [200, 80, 80],
            alpha: 1.0,
            radius: 3.0,
        }
    }
}

impl PointStyle {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.color[0],
            self.color[1],
            self.color[2],
            alpha_to_u8(self.alpha),
        )
    }

    pub const fn radius(&self) -> f32 {
        self.radius.max(0.1)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CrosshairStyle {
    pub color: [u8; 3],
    pub alpha: f32,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        Self {
            color: [200, 200, 200],
            alpha: 0.8,
        }
    }
}

impl CrosshairStyle {
    pub fn color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.color[0],
            self.color[1],
            self.color[2],
            alpha_to_u8(self.alpha),
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ExportConfig {
    pub samples_max: u32,
    pub auto_rel_tolerance: f32,
    pub auto_ref_samples: u32,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            samples_max: 10_000,
            auto_rel_tolerance: 0.005,
            auto_ref_samples: 2048,
        }
    }
}

impl ExportConfig {
    pub fn samples_max_sanitized(&self) -> usize {
        const MIN_ALLOWED: u32 = 10;
        const MAX_ALLOWED: u32 = 1_000_000;
        let clamped = self.samples_max.clamp(MIN_ALLOWED, MAX_ALLOWED);
        clamped as usize
    }

    pub fn auto_rel_tolerance_sanitized(&self) -> f64 {
        let t = self.auto_rel_tolerance;
        let clamped = t.clamp(1.0e-6, 1.0);
        f64::from(clamped)
    }

    pub fn auto_ref_samples_sanitized(&self) -> usize {
        const MIN_REF: u32 = 16;
        const MAX_REF: u32 = 65_536;
        let clamped = self.auto_ref_samples.clamp(MIN_REF, MAX_REF);
        clamped as usize
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub curve_line: StrokeStyle,
    pub curve_points: PointStyle,
    pub pan_speed: f32,
    pub crosshair: CrosshairStyle,
    pub image_limits: ImageLimits,
    pub attention_highlight: StrokeStyle,
    pub export: ExportConfig,
    pub auto_place: AutoPlaceConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            curve_line: StrokeStyle::default(),
            curve_points: PointStyle::default(),
            pan_speed: 1.0,
            crosshair: CrosshairStyle::default(),
            image_limits: ImageLimits::default(),
            attention_highlight: StrokeStyle {
                color: [220, 70, 70],
                alpha: 1.0,
                thickness: 1.2,
            },
            export: ExportConfig::default(),
            auto_place: AutoPlaceConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        for path in Self::candidate_paths() {
            if let Ok(contents) = fs::read_to_string(&path) {
                match toml::from_str::<Self>(&contents) {
                    Ok(cfg) => return cfg,
                    Err(err) => {
                        eprintln!("Failed to parse config {}: {err}", path.display());
                    }
                }
            }
        }
        Self::default()
    }

    pub const fn pan_speed_factor(&self) -> f32 {
        self.pan_speed.clamp(0.01, 50.0)
    }

    pub fn effective_image_limits(&self) -> ImageLimits {
        self.image_limits.sanitized()
    }

    pub fn auto_place(&self) -> AutoPlaceConfig {
        self.auto_place.sanitized()
    }

    fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Ok(exe_path) = std::env::current_exe()
            && let Some(dir) = exe_path.parent()
        {
            paths.push(dir.join(CONFIG_FILE_NAME));
        }

        if let Some(proj_dirs) = ProjectDirs::from("dev", "Curcat", "Curcat") {
            paths.push(proj_dirs.config_dir().join(CONFIG_FILE_NAME));
        }

        if let Some(base_dirs) = BaseDirs::new() {
            paths.push(base_dirs.config_dir().join("curcat").join(CONFIG_FILE_NAME));
        }

        paths
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ImageLimits {
    pub image_dim: u32,
    pub total_pixels: u64,
    pub alloc_bytes: u64,
}

impl Default for ImageLimits {
    fn default() -> Self {
        Self {
            image_dim: 12_000,
            total_pixels: 80_000_000,       // ~80 MP
            alloc_bytes: 512 * 1024 * 1024, // 512 MiB
        }
    }
}

impl ImageLimits {
    pub fn sanitized(&self) -> Self {
        // Clamp to reasonable operating bounds to avoid pathological configs.
        let dim = self.image_dim.clamp(64, 100_000);
        let pixels = self.total_pixels.clamp(1_000_000, 5_000_000_000); // 1 MP .. 5 GP
        let alloc = self
            .alloc_bytes
            .clamp(8 * 1024 * 1024, 8 * 1024 * 1024 * 1024); // 8 MiB .. 8 GiB
        Self {
            image_dim: dim,
            total_pixels: pixels,
            alloc_bytes: alloc,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct AutoPlaceConfig {
    pub hold_activation_secs: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    pub distance_per_speed: f32,
    pub time_min_secs: f32,
    pub time_max_secs: f32,
    pub time_per_speed: f32,
    pub pause_speed_threshold: f32,
    pub pause_timeout_ms: u32,
    pub dedup_radius: f32,
    pub speed_smoothing: f32,
}

impl Default for AutoPlaceConfig {
    fn default() -> Self {
        Self {
            hold_activation_secs: 1.25,
            distance_min: 2.5,
            distance_max: 24.0,
            distance_per_speed: 0.01,
            time_min_secs: 0.05,
            time_max_secs: 0.28,
            time_per_speed: 28.0,
            pause_speed_threshold: 6.0,
            pause_timeout_ms: 160,
            dedup_radius: 1.5,
            speed_smoothing: 0.25,
        }
    }
}

impl AutoPlaceConfig {
    pub fn sanitized(&self) -> Self {
        let hold_activation_secs = self.hold_activation_secs.clamp(0.1, 10.0);
        let distance_min = self.distance_min.clamp(0.1, 200.0);
        let distance_max = self.distance_max.clamp(distance_min, 1_000.0);
        let distance_per_speed = self.distance_per_speed.clamp(0.0, 1.0);
        let time_min_secs = self.time_min_secs.clamp(0.01, 2.0);
        let time_max_secs = self.time_max_secs.clamp(time_min_secs, 3.0);
        let time_per_speed = self.time_per_speed.clamp(0.1, 1_000.0);
        let pause_speed_threshold = self.pause_speed_threshold.clamp(0.0, 1_000.0);
        let pause_timeout_ms = self.pause_timeout_ms.clamp(0, 10_000);
        let dedup_radius = self.dedup_radius.clamp(0.0, 200.0);
        let speed_smoothing = self.speed_smoothing.clamp(0.0, 1.0);
        Self {
            hold_activation_secs,
            distance_min,
            distance_max,
            distance_per_speed,
            time_min_secs,
            time_max_secs,
            time_per_speed,
            pause_speed_threshold,
            pause_timeout_ms,
            dedup_radius,
            speed_smoothing,
        }
    }
}
