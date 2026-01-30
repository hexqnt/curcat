use std::fmt;
use std::fs;
use std::path::PathBuf;

use directories::{BaseDirs, ProjectDirs};
use egui::{Color32, Stroke};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, Visitor},
};

const CONFIG_FILE_NAME: &str = "curcat.toml";

/// Hex-encoded RGBA color stored as raw bytes (`#RRGGBBAA` on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor([u8; 4]);

impl HexColor {
    /// Create an opaque color from RGB bytes.
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    /// Create a color from RGBA bytes.
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// Convert into `Color32` with the stored opacity.
    pub const fn to_color32(self) -> Color32 {
        let [r, g, b, a] = self.0;
        Color32::from_rgba_unmultiplied_const(r, g, b, a)
    }

    fn fmt_hex(self) -> String {
        let [r, g, b, a] = self.0;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }

    fn parse_hex(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        let hex = trimmed
            .strip_prefix('#')
            .or_else(|| trimmed.strip_prefix("0x"))
            .unwrap_or(trimmed);
        if hex.len() != 6 && hex.len() != 8 {
            return Err(format!(
                "expected a hex color like #RRGGBB or #RRGGBBAA, got \"{value}\""
            ));
        }
        if !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(format!("invalid hex color \"{value}\""));
        }
        let parse_component = |range: std::ops::Range<usize>| -> Result<u8, String> {
            u8::from_str_radix(&hex[range], 16)
                .map_err(|_| format!("invalid hex color \"{value}\""))
        };
        let r = parse_component(0..2)?;
        let g = parse_component(2..4)?;
        let b = parse_component(4..6)?;
        let a = if hex.len() == 8 {
            parse_component(6..8)?
        } else {
            255
        };
        Ok(Self::from_rgba(r, g, b, a))
    }
}

impl Serialize for HexColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.fmt_hex())
    }
}

impl<'de> Deserialize<'de> for HexColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct HexColorVisitor;

        impl Visitor<'_> for HexColorVisitor {
            type Value = HexColor;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex color string like \"#FF0000\" or \"#FF0000AA\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                HexColor::parse_hex(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_str(HexColorVisitor)
    }
}

/// Stroke appearance settings for curves and outlines.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StrokeStyle {
    pub color: HexColor,
    pub thickness: f32,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: HexColor::from_rgb(80, 200, 120),
            thickness: 2.0,
        }
    }
}

impl StrokeStyle {
    /// Return the stroke color with embedded alpha.
    pub const fn color32(&self) -> Color32 {
        self.color.to_color32()
    }

    /// Build an `egui::Stroke` with sanitized thickness.
    pub const fn stroke(&self) -> Stroke {
        Stroke {
            width: self.thickness.max(0.1),
            color: self.color32(),
        }
    }
}

/// Appearance settings for picked points.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PointStyle {
    pub color: HexColor,
    pub radius: f32,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self {
            color: HexColor::from_rgb(200, 80, 80),
            radius: 3.0,
        }
    }
}

impl PointStyle {
    /// Return the point color with embedded alpha.
    pub const fn color32(&self) -> Color32 {
        self.color.to_color32()
    }

    /// Radius constrained to a sensible minimum.
    pub const fn radius(&self) -> f32 {
        self.radius.max(0.1)
    }
}

/// Appearance settings for the hover crosshair.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CrosshairStyle {
    pub color: HexColor,
}

impl Default for CrosshairStyle {
    fn default() -> Self {
        Self {
            color: HexColor::from_rgba(200, 200, 200, 204),
        }
    }
}

impl CrosshairStyle {
    /// Return the crosshair color with embedded alpha.
    pub const fn color32(&self) -> Color32 {
        self.color.to_color32()
    }
}

/// Parameters controlling export and auto-sampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// `samples_max` clamped to operational bounds.
    pub fn samples_max_sanitized(&self) -> usize {
        const MIN_ALLOWED: u32 = 10;
        const MAX_ALLOWED: u32 = 1_000_000;
        let clamped = self.samples_max.clamp(MIN_ALLOWED, MAX_ALLOWED);
        clamped as usize
    }

    /// `auto_rel_tolerance` clamped to a safe range.
    pub fn auto_rel_tolerance_sanitized(&self) -> f64 {
        let t = self.auto_rel_tolerance;
        let clamped = t.clamp(1.0e-6, 1.0);
        f64::from(clamped)
    }

    /// `auto_ref_samples` clamped to operational bounds.
    pub fn auto_ref_samples_sanitized(&self) -> usize {
        const MIN_REF: u32 = 16;
        const MAX_REF: u32 = 65_536;
        let clamped = self.auto_ref_samples.clamp(MIN_REF, MAX_REF);
        clamped as usize
    }
}

/// Root application configuration loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub curve_line: StrokeStyle,
    pub curve_points: PointStyle,
    pub pan_speed: f32,
    pub smooth_zoom: bool,
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
            smooth_zoom: true,
            crosshair: CrosshairStyle::default(),
            image_limits: ImageLimits::default(),
            attention_highlight: StrokeStyle {
                color: HexColor::from_rgb(220, 70, 70),
                thickness: 1.2,
            },
            export: ExportConfig::default(),
            auto_place: AutoPlaceConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from known locations or fall back to defaults.
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

    /// Sanitized multiplier for panning speed.
    pub const fn pan_speed_factor(&self) -> f32 {
        self.pan_speed.clamp(0.01, 50.0)
    }

    /// Apply safety bounds to image limits.
    pub fn effective_image_limits(&self) -> ImageLimits {
        self.image_limits.sanitized()
    }

    /// Apply safety bounds to auto-place parameters.
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

/// Limits for image decoding to guard against resource abuse.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Clamp values to conservative, safe bounds.
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

/// Parameters that govern auto-placement of points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    /// Clamp values to keep auto-placement stable and predictable.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize)]
    struct ColorWrapper {
        color: HexColor,
    }

    #[test]
    fn parses_hex_without_alpha_defaults_to_opaque() {
        let wrapper: ColorWrapper = toml::from_str(r##"color = "#50C878""##).unwrap();
        assert_eq!(
            wrapper.color.to_color32().to_srgba_unmultiplied(),
            [0x50, 0xC8, 0x78, 0xFF]
        );
    }

    #[test]
    fn parses_hex_with_alpha() {
        let wrapper: ColorWrapper = toml::from_str(r##"color = "#C85050CC""##).unwrap();
        assert_eq!(
            wrapper.color.to_color32().to_srgba_unmultiplied(),
            [0xC8, 0x50, 0x50, 0xCC]
        );
    }

    #[test]
    fn serializes_back_to_hex() {
        let wrapper = ColorWrapper {
            color: HexColor::from_rgba(0x01, 0x02, 0x03, 0x04),
        };
        let serialized = toml::to_string(&wrapper).unwrap();
        assert!(
            serialized
                .trim()
                .eq_ignore_ascii_case(r##"color = "#01020304""##)
        );
    }

    #[test]
    fn keeps_alpha_in_color32() {
        let color = HexColor::from_rgba(255, 0, 0, 128).to_color32();
        assert_eq!(color.to_srgba_unmultiplied(), [255, 0, 0, 128]);
    }

    #[test]
    fn rejects_legacy_rgb_arrays() {
        let parsed: Result<ColorWrapper, _> = toml::from_str("color = [1, 2, 3]");
        assert!(parsed.is_err());
    }

    #[test]
    fn accepts_lowercase_hex() {
        let wrapper: ColorWrapper = toml::from_str(r##"color = "#c85050ff""##).unwrap();
        assert_eq!(
            wrapper.color.to_color32().to_srgba_unmultiplied(),
            [0xC8, 0x50, 0x50, 0xFF]
        );
    }
}
