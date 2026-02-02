use egui::{Color32, ColorImage};

/// Simple image filter settings applied to the displayed pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageFilters {
    pub brightness: f32,
    pub contrast: f32,
    pub gamma: f32,
    pub invert: bool,
    pub threshold: f32,
    pub threshold_enabled: bool,
    pub blur_radius: u32,
}

impl Default for ImageFilters {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            invert: false,
            threshold: 0.5,
            threshold_enabled: false,
            blur_radius: 0,
        }
    }
}

impl ImageFilters {
    pub fn sanitized(self) -> Self {
        let brightness = self.brightness.clamp(-1.0, 1.0);
        let contrast = self.contrast.clamp(-1.0, 1.0);
        let gamma = self.gamma.clamp(0.2, 5.0);
        let threshold = self.threshold.clamp(0.0, 1.0);
        let blur_radius = self.blur_radius.min(24);
        Self {
            brightness,
            contrast,
            gamma,
            invert: self.invert,
            threshold,
            threshold_enabled: self.threshold_enabled,
            blur_radius,
        }
    }

    pub fn is_identity(self) -> bool {
        let sanitized = self.sanitized();
        sanitized.brightness.abs() <= f32::EPSILON
            && sanitized.contrast.abs() <= f32::EPSILON
            && (sanitized.gamma - 1.0).abs() <= f32::EPSILON
            && !sanitized.invert
            && !sanitized.threshold_enabled
            && sanitized.blur_radius == 0
    }
}

/// Apply the provided filter settings to a base image.
pub fn apply_image_filters(base: &ColorImage, filters: ImageFilters) -> ColorImage {
    if base.pixels.is_empty() {
        return base.clone();
    }
    let filters = filters.sanitized();
    if filters.is_identity() {
        return base.clone();
    }

    let mut pixels = if filters.blur_radius > 0 {
        box_blur(base, filters.blur_radius)
    } else {
        base.pixels.clone()
    };

    let contrast_factor = 1.0 + filters.contrast;
    let inv_gamma = if (filters.gamma - 1.0).abs() <= f32::EPSILON {
        1.0
    } else {
        1.0 / filters.gamma
    };

    for pixel in &mut pixels {
        let [r, g, b, a] = pixel.to_array();
        let mut rf = f32::from(r) / 255.0;
        let mut gf = f32::from(g) / 255.0;
        let mut bf = f32::from(b) / 255.0;

        rf = (rf + filters.brightness).clamp(0.0, 1.0);
        gf = (gf + filters.brightness).clamp(0.0, 1.0);
        bf = (bf + filters.brightness).clamp(0.0, 1.0);

        rf = (rf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);
        gf = (gf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);
        bf = (bf - 0.5).mul_add(contrast_factor, 0.5).clamp(0.0, 1.0);

        if (inv_gamma - 1.0).abs() > f32::EPSILON {
            rf = rf.powf(inv_gamma).clamp(0.0, 1.0);
            gf = gf.powf(inv_gamma).clamp(0.0, 1.0);
            bf = bf.powf(inv_gamma).clamp(0.0, 1.0);
        }

        if filters.invert {
            rf = 1.0 - rf;
            gf = 1.0 - gf;
            bf = 1.0 - bf;
        }

        if filters.threshold_enabled {
            let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
            let v = if luma >= filters.threshold { 1.0 } else { 0.0 };
            rf = v;
            gf = v;
            bf = v;
        }

        *pixel =
            Color32::from_rgba_unmultiplied(float_to_u8(rf), float_to_u8(gf), float_to_u8(bf), a);
    }

    ColorImage::new(base.size, pixels)
}

fn float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn box_blur(image: &ColorImage, radius: u32) -> Vec<Color32> {
    let [width, height] = image.size;
    if radius == 0 || width == 0 || height == 0 {
        return image.pixels.clone();
    }
    let radius = radius as usize;
    let row_len = width;
    let mut horiz = vec![[0u8; 4]; width * height];

    for y in 0..height {
        let row_start = y * row_len;
        let mut prefix = vec![[0u32; 4]; width + 1];
        for x in 0..width {
            let [r, g, b, a] = image.pixels[row_start + x].to_array();
            let prev = prefix[x];
            prefix[x + 1] = [
                prev[0] + r as u32,
                prev[1] + g as u32,
                prev[2] + b as u32,
                prev[3] + a as u32,
            ];
        }
        for x in 0..width {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius).min(width - 1);
            let count = (x1 - x0 + 1) as u32;
            let sum = prefix[x1 + 1];
            let base = prefix[x0];
            horiz[row_start + x] = [
                ((sum[0] - base[0] + count / 2) / count) as u8,
                ((sum[1] - base[1] + count / 2) / count) as u8,
                ((sum[2] - base[2] + count / 2) / count) as u8,
                ((sum[3] - base[3] + count / 2) / count) as u8,
            ];
        }
    }

    let mut out = vec![Color32::TRANSPARENT; width * height];
    for x in 0..width {
        let mut prefix = vec![[0u32; 4]; height + 1];
        for y in 0..height {
            let idx = y * row_len + x;
            let [r, g, b, a] = horiz[idx];
            let prev = prefix[y];
            prefix[y + 1] = [
                prev[0] + r as u32,
                prev[1] + g as u32,
                prev[2] + b as u32,
                prev[3] + a as u32,
            ];
        }
        for y in 0..height {
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(height - 1);
            let count = (y1 - y0 + 1) as u32;
            let sum = prefix[y1 + 1];
            let base = prefix[y0];
            let idx = y * row_len + x;
            out[idx] = Color32::from_rgba_unmultiplied(
                ((sum[0] - base[0] + count / 2) / count) as u8,
                ((sum[1] - base[1] + count / 2) / count) as u8,
                ((sum[2] - base[2] + count / 2) / count) as u8,
                ((sum[3] - base[3] + count / 2) / count) as u8,
            );
        }
    }

    out
}
