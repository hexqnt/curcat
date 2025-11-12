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
pub struct AppConfig {
    pub curve_line: StrokeStyle,
    pub curve_points: PointStyle,
    pub pan_speed: f32,
    pub crosshair: CrosshairStyle,
    pub image_limits: ImageLimits,
    pub attention_highlight: StrokeStyle,
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
