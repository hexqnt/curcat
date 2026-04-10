use crate::config::{AppConfig, ImageLimits};
use crate::util::u32_to_f32;
use anyhow::Context as _;
use egui::ColorImage;
use image::{ImageReader, imageops::FilterType};
use resvg::{tiny_skia, usvg};
use std::io::{BufRead, Cursor, Read, Seek};
use std::path::Path;
use std::sync::{Arc, OnceLock};

const HARD_LIMIT_SIDE: u32 = 32_768;
const HARD_LIMIT_TOTAL_PIXELS: u64 = 500_000_000;
const HARD_LIMIT_ALLOC_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const HARD_LIMITS: ImageLimits = ImageLimits {
    image_dim: HARD_LIMIT_SIDE,
    total_pixels: HARD_LIMIT_TOTAL_PIXELS,
    alloc_bytes: HARD_LIMIT_ALLOC_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageLoadPolicy {
    AskUser,
    AutoscaleToConfig,
    IgnoreConfigWithHardCap,
}

#[derive(Debug, Clone)]
pub struct ImageLimitInfo {
    pub source_width: u32,
    pub source_height: u32,
    pub source_total_pixels: u64,
    pub source_rgba_bytes: u64,
    pub config_limits: ImageLimits,
    pub hard_limits: ImageLimits,
    pub autoscale_width: Option<u32>,
    pub autoscale_height: Option<u32>,
    pub can_autoscale: bool,
    pub can_ignore_limits: bool,
}

impl ImageLimitInfo {
    pub const fn autoscale_size(&self) -> Option<[u32; 2]> {
        match (self.autoscale_width, self.autoscale_height) {
            (Some(w), Some(h)) => Some([w, h]),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum ImageLoadOutcome {
    Ready(ColorImage),
    NeedsLimitDecision(ImageLimitInfo),
}

#[derive(Debug, Clone, Copy)]
struct SourceMetrics {
    width: u32,
    height: u32,
    total_pixels: u64,
    rgba_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
enum DecodePlan {
    OriginalConfig,
    OriginalHard,
    ResizeConfig([u32; 2]),
}

#[derive(Debug)]
enum PlanDecision {
    Proceed(DecodePlan),
    Prompt(ImageLimitInfo),
}

/// Декодирует изображение из файла с политикой лимитов.
pub fn decode_image_from_path(
    cfg: &AppConfig,
    path: &Path,
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    let bytes =
        std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let resources_dir = path.parent();
    decode_image_data(cfg, &bytes, Some(path), resources_dir, policy)
}

/// Декодирует изображение из массива байт с политикой лимитов.
pub fn decode_image_from_bytes(
    cfg: &AppConfig,
    bytes: &[u8],
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    decode_image_data(cfg, bytes, None, None, policy)
}

/// Декодирует изображение из RGBA-буфера буфера обмена с политикой лимитов.
pub fn decode_image_from_clipboard_rgba(
    cfg: &AppConfig,
    width: usize,
    height: usize,
    rgba: &[u8],
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    let width_u32 = u32::try_from(width).context("Clipboard image width is too large")?;
    let height_u32 = u32::try_from(height).context("Clipboard image height is too large")?;
    let metrics = source_metrics(width_u32, height_u32)?;
    let expected_len = usize::try_from(metrics.rgba_bytes)
        .context("Clipboard image is too large to fit in memory")?;
    if rgba.len() < expected_len {
        anyhow::bail!("Paste failed: clipboard image data is truncated.");
    }

    let cfg_limits = cfg.effective_image_limits();
    let decision = decide_plan(metrics, &cfg_limits, policy)?;
    match decision {
        PlanDecision::Prompt(info) => Ok(ImageLoadOutcome::NeedsLimitDecision(info)),
        PlanDecision::Proceed(DecodePlan::ResizeConfig(target)) => {
            let resized = resize_rgba_buffer(
                width_u32,
                height_u32,
                &rgba[..expected_len],
                target[0],
                target[1],
            )?;
            Ok(ImageLoadOutcome::Ready(resized))
        }
        PlanDecision::Proceed(DecodePlan::OriginalConfig | DecodePlan::OriginalHard) => {
            let image = ColorImage::from_rgba_unmultiplied([width, height], &rgba[..expected_len]);
            Ok(ImageLoadOutcome::Ready(image))
        }
    }
}

fn decode_image_data(
    cfg: &AppConfig,
    bytes: &[u8],
    path_hint: Option<&Path>,
    resources_dir: Option<&Path>,
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    let cfg_limits = cfg.effective_image_limits();
    let force_svg = path_hint
        .and_then(Path::extension)
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz"));

    if force_svg || looks_like_svg(bytes) {
        match parse_svg_tree(bytes, resources_dir) {
            Ok(tree) => {
                return decode_svg_tree(&tree, &cfg_limits, policy);
            }
            Err(err) if force_svg => {
                return Err(err.context("Failed to parse SVG data"));
            }
            Err(_) => {}
        }
    }

    decode_raster_bytes(bytes, cfg_limits, policy)
}

fn decode_raster_bytes(
    bytes: &[u8],
    cfg_limits: ImageLimits,
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    let (width, height) = raster_dimensions(bytes)?;
    let metrics = source_metrics(width, height)?;
    let decision = decide_plan(metrics, &cfg_limits, policy)?;

    match decision {
        PlanDecision::Prompt(info) => Ok(ImageLoadOutcome::NeedsLimitDecision(info)),
        PlanDecision::Proceed(plan) => {
            let decode_limits = match plan {
                DecodePlan::OriginalConfig => cfg_limits,
                DecodePlan::OriginalHard | DecodePlan::ResizeConfig(_) => HARD_LIMITS.clone(),
            };
            let decoded = decode_raster_with_limits(bytes, &decode_limits)?;
            let output = match plan {
                DecodePlan::ResizeConfig([target_w, target_h]) => {
                    dynamic_image_to_color_with_resize(&decoded, target_w, target_h)
                }
                DecodePlan::OriginalConfig | DecodePlan::OriginalHard => {
                    dynamic_image_to_color_with_resize(&decoded, width, height)
                }
            };
            Ok(ImageLoadOutcome::Ready(output))
        }
    }
}

fn decode_svg_tree(
    tree: &usvg::Tree,
    cfg_limits: &ImageLimits,
    policy: ImageLoadPolicy,
) -> anyhow::Result<ImageLoadOutcome> {
    let source_size = tree.size().to_int_size();
    let source_w = source_size.width();
    let source_h = source_size.height();
    let metrics = source_metrics(source_w, source_h)?;
    let decision = decide_plan(metrics, cfg_limits, policy)?;

    match decision {
        PlanDecision::Prompt(info) => Ok(ImageLoadOutcome::NeedsLimitDecision(info)),
        PlanDecision::Proceed(plan) => {
            let target = match plan {
                DecodePlan::OriginalConfig | DecodePlan::OriginalHard => [source_w, source_h],
                DecodePlan::ResizeConfig(size) => size,
            };
            let color = render_svg_to_color_image(tree, target[0], target[1])?;
            Ok(ImageLoadOutcome::Ready(color))
        }
    }
}

fn parse_svg_tree(bytes: &[u8], resources_dir: Option<&Path>) -> anyhow::Result<usvg::Tree> {
    let mut options = usvg::Options {
        resources_dir: resources_dir.map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    options.fontdb = shared_fontdb();
    usvg::Tree::from_data(bytes, &options).context("Invalid SVG content")
}

fn shared_fontdb() -> Arc<usvg::fontdb::Database> {
    static FONT_DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONT_DB
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

fn render_svg_to_color_image(
    tree: &usvg::Tree,
    target_w: u32,
    target_h: u32,
) -> anyhow::Result<ColorImage> {
    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or_else(|| anyhow::anyhow!("Failed to allocate SVG render target"))?;

    let source_size = tree.size().to_int_size();
    let source_w = source_size.width();
    let source_h = source_size.height();
    let transform = if source_w == target_w && source_h == target_h {
        tiny_skia::Transform::identity()
    } else {
        tiny_skia::Transform::from_scale(
            u32_to_f32(target_w) / u32_to_f32(source_w),
            u32_to_f32(target_h) / u32_to_f32(source_h),
        )
    };

    resvg::render(tree, transform, &mut pixmap.as_mut());
    let width = usize::try_from(target_w).context("SVG width does not fit usize")?;
    let height = usize::try_from(target_h).context("SVG height does not fit usize")?;
    Ok(ColorImage::from_rgba_premultiplied(
        [width, height],
        pixmap.data(),
    ))
}

fn raster_dimensions(bytes: &[u8]) -> anyhow::Result<(u32, u32)> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("Failed to detect image format")?;
    reader.no_limits();
    reader
        .into_dimensions()
        .context("Failed to read image dimensions")
}

fn decode_raster_with_limits(
    bytes: &[u8],
    limits: &ImageLimits,
) -> anyhow::Result<image::DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("Failed to detect image format")?;
    apply_raster_limits(&mut reader, limits);
    reader.decode().context("Failed to decode image data")
}

fn apply_raster_limits<R>(reader: &mut ImageReader<R>, limits: &ImageLimits)
where
    R: Read + Seek + BufRead,
{
    let mut reader_limits = image::Limits::default();
    reader_limits.max_image_width = Some(limits.image_dim);
    reader_limits.max_image_height = Some(limits.image_dim);
    reader_limits.max_alloc = Some(limits.alloc_bytes);
    reader.limits(reader_limits);
}

fn dynamic_image_to_color_with_resize(
    img: &image::DynamicImage,
    target_w: u32,
    target_h: u32,
) -> ColorImage {
    let rgba = img.to_rgba8();
    let resized = if rgba.width() == target_w && rgba.height() == target_h {
        rgba
    } else {
        image::imageops::resize(&rgba, target_w, target_h, FilterType::CatmullRom)
    };
    ColorImage::from_rgba_unmultiplied(
        [resized.width() as usize, resized.height() as usize],
        resized.as_raw(),
    )
}

fn resize_rgba_buffer(
    source_w: u32,
    source_h: u32,
    rgba: &[u8],
    target_w: u32,
    target_h: u32,
) -> anyhow::Result<ColorImage> {
    let source = image::RgbaImage::from_raw(source_w, source_h, rgba.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Invalid clipboard RGBA data"))?;
    let resized = if source_w == target_w && source_h == target_h {
        source
    } else {
        image::imageops::resize(&source, target_w, target_h, FilterType::CatmullRom)
    };
    Ok(ColorImage::from_rgba_unmultiplied(
        [resized.width() as usize, resized.height() as usize],
        resized.as_raw(),
    ))
}

fn source_metrics(width: u32, height: u32) -> anyhow::Result<SourceMetrics> {
    if width == 0 || height == 0 {
        anyhow::bail!("Image has invalid zero dimensions");
    }
    let total_pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .context("Image dimensions are too large")?;
    let rgba_bytes = total_pixels
        .checked_mul(4)
        .context("Image RGBA footprint is too large")?;
    Ok(SourceMetrics {
        width,
        height,
        total_pixels,
        rgba_bytes,
    })
}

fn decide_plan(
    metrics: SourceMetrics,
    config_limits: &ImageLimits,
    policy: ImageLoadPolicy,
) -> anyhow::Result<PlanDecision> {
    let config_exceeded = exceeds_limits(metrics, config_limits);
    let hard_exceeded = exceeds_limits(metrics, &HARD_LIMITS);

    let autoscale_size = if hard_exceeded {
        None
    } else {
        compute_autoscale_size(metrics, config_limits)
    };

    let info = ImageLimitInfo {
        source_width: metrics.width,
        source_height: metrics.height,
        source_total_pixels: metrics.total_pixels,
        source_rgba_bytes: metrics.rgba_bytes,
        config_limits: config_limits.clone(),
        hard_limits: HARD_LIMITS.clone(),
        autoscale_width: autoscale_size.map(|s| s[0]),
        autoscale_height: autoscale_size.map(|s| s[1]),
        can_autoscale: autoscale_size.is_some() && !hard_exceeded,
        can_ignore_limits: !hard_exceeded,
    };

    match policy {
        ImageLoadPolicy::AskUser => {
            if config_exceeded {
                Ok(PlanDecision::Prompt(info))
            } else {
                Ok(PlanDecision::Proceed(DecodePlan::OriginalConfig))
            }
        }
        ImageLoadPolicy::AutoscaleToConfig => {
            if hard_exceeded {
                anyhow::bail!(
                    "Image exceeds hard safety limit: {}x{} exceeds {} px side or exceeds {} MP / {} bytes",
                    metrics.width,
                    metrics.height,
                    HARD_LIMITS.image_dim,
                    HARD_LIMITS.total_pixels / 1_000_000,
                    HARD_LIMITS.alloc_bytes
                );
            }
            if config_exceeded {
                let Some(size) = autoscale_size else {
                    anyhow::bail!(
                        "Image cannot be autoscaled to fit configured limits: {}x{}",
                        metrics.width,
                        metrics.height
                    );
                };
                Ok(PlanDecision::Proceed(DecodePlan::ResizeConfig(size)))
            } else {
                Ok(PlanDecision::Proceed(DecodePlan::OriginalConfig))
            }
        }
        ImageLoadPolicy::IgnoreConfigWithHardCap => {
            if hard_exceeded {
                anyhow::bail!(
                    "Image exceeds hard safety limit: {}x{} exceeds {} px side or exceeds {} MP / {} bytes",
                    metrics.width,
                    metrics.height,
                    HARD_LIMITS.image_dim,
                    HARD_LIMITS.total_pixels / 1_000_000,
                    HARD_LIMITS.alloc_bytes
                );
            }
            Ok(PlanDecision::Proceed(DecodePlan::OriginalHard))
        }
    }
}

const fn exceeds_limits(metrics: SourceMetrics, limits: &ImageLimits) -> bool {
    metrics.width > limits.image_dim
        || metrics.height > limits.image_dim
        || metrics.total_pixels > limits.total_pixels
        || metrics.rgba_bytes > limits.alloc_bytes
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn compute_autoscale_size(metrics: SourceMetrics, limits: &ImageLimits) -> Option<[u32; 2]> {
    let alloc_pixels = limits.alloc_bytes / 4;
    let pixel_limit = limits.total_pixels.min(alloc_pixels);
    if pixel_limit == 0 {
        return None;
    }

    let mut scale = 1.0_f64;
    scale = scale.min(f64::from(limits.image_dim) / f64::from(metrics.width));
    scale = scale.min(f64::from(limits.image_dim) / f64::from(metrics.height));
    scale = scale.min((pixel_limit as f64 / metrics.total_pixels as f64).sqrt());

    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }

    let mut w = (f64::from(metrics.width) * scale).floor().max(1.0) as u32;
    let mut h = (f64::from(metrics.height) * scale).floor().max(1.0) as u32;

    while (w > limits.image_dim
        || h > limits.image_dim
        || u64::from(w) * u64::from(h) > limits.total_pixels
        || u64::from(w) * u64::from(h) * 4 > limits.alloc_bytes)
        && (w > 1 || h > 1)
    {
        if w >= h && w > 1 {
            w -= 1;
            h = ((u64::from(metrics.height) * u64::from(w)) / u64::from(metrics.width))
                .max(1)
                .try_into()
                .ok()?;
        } else if h > 1 {
            h -= 1;
            w = ((u64::from(metrics.width) * u64::from(h)) / u64::from(metrics.height))
                .max(1)
                .try_into()
                .ok()?;
        } else {
            break;
        }
    }

    if w == 0
        || h == 0
        || w > limits.image_dim
        || h > limits.image_dim
        || u64::from(w) * u64::from(h) > limits.total_pixels
        || u64::from(w) * u64::from(h) * 4 > limits.alloc_bytes
    {
        None
    } else {
        Some([w, h])
    }
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return true;
    }

    let mut start = bytes;
    if start.starts_with(&[0xEF, 0xBB, 0xBF]) {
        start = &start[3..];
    }
    let trimmed = start.iter().copied().skip_while(u8::is_ascii_whitespace);
    let mut probe = [0_u8; 256];
    let mut len = 0_usize;
    for byte in trimmed.take(256) {
        probe[len] = byte.to_ascii_lowercase();
        len += 1;
    }
    let probe = &probe[..len];

    probe.starts_with(b"<svg")
        || probe.starts_with(b"<?xml")
        || probe.windows(4).any(|w| w == b"<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with_limits(image_dim: u32, total_pixels: u64, alloc_bytes: u64) -> AppConfig {
        AppConfig {
            image_limits: ImageLimits {
                image_dim,
                total_pixels,
                alloc_bytes,
            },
            ..AppConfig::default()
        }
    }

    fn svg_bytes(width: u32, height: u32) -> Vec<u8> {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><rect width="100%" height="100%" fill="#ff0000"/></svg>"##
        )
        .into_bytes()
    }

    const SVGZ_SAMPLE: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x6d, 0xcc, 0x49, 0x0a, 0x80,
        0x30, 0x0c, 0x40, 0xd1, 0xab, 0x84, 0xb8, 0x6f, 0x8a, 0x22, 0x82, 0xb4, 0xbd, 0x8c, 0x76,
        0x82, 0x3a, 0x50, 0x83, 0xf1, 0xf8, 0xd6, 0xbd, 0xeb, 0xff, 0x79, 0xe6, 0xba, 0x23, 0x3c,
        0x5b, 0xd9, 0x2f, 0x8b, 0x89, 0xf9, 0x9c, 0x89, 0x44, 0x44, 0xc9, 0xa0, 0x8e, 0x1a, 0xa9,
        0xd7, 0x5a, 0x53, 0x3b, 0x10, 0x24, 0xaf, 0x9c, 0x2c, 0x4e, 0x08, 0xc9, 0xe7, 0x98, 0xd8,
        0xe2, 0x88, 0xce, 0x54, 0xbf, 0xf0, 0x6f, 0x82, 0x90, 0x4b, 0xb1, 0xd8, 0x85, 0xd0, 0x04,
        0x8d, 0xe4, 0xcc, 0xc7, 0xb8, 0x17, 0x4d, 0x69, 0x3f, 0x8a, 0x6e, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn decodes_svg() {
        let cfg = cfg_with_limits(10_000, 200_000_000, 1_000_000_000);
        let svg = svg_bytes(9, 7);
        let outcome = decode_image_from_bytes(&cfg, &svg, ImageLoadPolicy::AskUser).unwrap();
        let ImageLoadOutcome::Ready(color) = outcome else {
            panic!("Expected ready image");
        };
        assert_eq!(color.size, [9, 7]);
    }

    #[test]
    fn decodes_svgz() {
        let cfg = cfg_with_limits(10_000, 200_000_000, 1_000_000_000);
        let outcome = decode_image_from_bytes(&cfg, SVGZ_SAMPLE, ImageLoadPolicy::AskUser).unwrap();
        let ImageLoadOutcome::Ready(color) = outcome else {
            panic!("Expected ready image");
        };
        assert_eq!(color.size, [7, 5]);
    }

    #[test]
    fn returns_prompt_for_config_limit_exceed() {
        let cfg = cfg_with_limits(1_000, 1_000_000, 50_000_000);
        let svg = svg_bytes(4_000, 2_000);
        let outcome = decode_image_from_bytes(&cfg, &svg, ImageLoadPolicy::AskUser).unwrap();
        let ImageLoadOutcome::NeedsLimitDecision(info) = outcome else {
            panic!("Expected limit prompt");
        };
        assert_eq!(info.source_width, 4_000);
        assert_eq!(info.source_height, 2_000);
        assert!(info.can_autoscale);
        assert!(info.can_ignore_limits);
    }

    #[test]
    fn autoscale_fits_limits_and_preserves_aspect() {
        let cfg = cfg_with_limits(1_000, 1_000_000, 200_000_000);
        let svg = svg_bytes(4_000, 2_000);
        let outcome =
            decode_image_from_bytes(&cfg, &svg, ImageLoadPolicy::AutoscaleToConfig).unwrap();
        let ImageLoadOutcome::Ready(color) = outcome else {
            panic!("Expected ready image");
        };
        assert_eq!(color.size, [1_000, 500]);
        let width = u64::try_from(color.size[0]).unwrap();
        let height = u64::try_from(color.size[1]).unwrap();
        assert!(width * height <= cfg.image_limits.total_pixels);
    }

    #[test]
    fn ignore_policy_respects_hard_cap() {
        let cfg = cfg_with_limits(100_000, 5_000_000_000, 8 * 1024 * 1024 * 1024);
        let svg = svg_bytes(40_000, 100);
        let err = decode_image_from_bytes(&cfg, &svg, ImageLoadPolicy::IgnoreConfigWithHardCap)
            .unwrap_err();
        assert!(err.to_string().contains("hard safety limit"));
    }

    #[test]
    fn hard_cap_breach_disables_prompt_actions() {
        let cfg = cfg_with_limits(1_000, 1_000_000, 50_000_000);
        let svg = svg_bytes(40_000, 100);
        let outcome = decode_image_from_bytes(&cfg, &svg, ImageLoadPolicy::AskUser).unwrap();
        let ImageLoadOutcome::NeedsLimitDecision(info) = outcome else {
            panic!("Expected limit prompt");
        };
        assert!(!info.can_autoscale);
        assert!(!info.can_ignore_limits);
    }

    #[test]
    fn clipboard_prompt_autoscale_and_ignore() {
        let cfg = cfg_with_limits(1_000, 1_000_000, 200_000_000);
        let width = 4_000_usize;
        let height = 2_000_usize;
        let rgba = vec![255_u8; width * height * 4];

        let ask_outcome =
            decode_image_from_clipboard_rgba(&cfg, width, height, &rgba, ImageLoadPolicy::AskUser)
                .unwrap();
        let ImageLoadOutcome::NeedsLimitDecision(_) = ask_outcome else {
            panic!("Expected limit prompt");
        };

        let autoscaled = decode_image_from_clipboard_rgba(
            &cfg,
            width,
            height,
            &rgba,
            ImageLoadPolicy::AutoscaleToConfig,
        )
        .unwrap();
        let ImageLoadOutcome::Ready(autoscaled) = autoscaled else {
            panic!("Expected ready autoscaled image");
        };
        assert_eq!(autoscaled.size, [1_000, 500]);

        let ignored = decode_image_from_clipboard_rgba(
            &cfg,
            width,
            height,
            &rgba,
            ImageLoadPolicy::IgnoreConfigWithHardCap,
        )
        .unwrap();
        let ImageLoadOutcome::Ready(ignored) = ignored else {
            panic!("Expected ready ignored image");
        };
        assert_eq!(ignored.size, [4_000, 2_000]);
    }
}
