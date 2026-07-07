pub fn safe_usize_to_f32(value: usize) -> f32 {
    let clamped = value.min(u32::MAX as usize);
    let as_u32 = u32::try_from(clamped).unwrap_or(u32::MAX);
    #[allow(clippy::cast_precision_loss)]
    {
        as_u32 as f32
    }
}

pub fn rounded_u8(value: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        value.round().clamp(0.0, f32::from(u8::MAX)) as u8
    }
}

pub const fn u32_to_f32(value: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

pub const fn i32_to_f32(value: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        value as f32
    }
}

pub const fn saturating_f32_to_i32(value: f32) -> i32 {
    #[allow(clippy::cast_precision_loss)]
    const MAX: f32 = i32::MAX as f32;
    #[allow(clippy::cast_precision_loss)]
    const MIN: f32 = i32::MIN as f32;
    #[allow(clippy::cast_possible_truncation)]
    {
        if value.is_nan() {
            0
        } else {
            value.clamp(MIN, MAX).round() as i32
        }
    }
}
