//! Общие SIMD-примитивы для обработки пикселей.

use egui::Color32;
use std::simd::Simd;
use std::simd::num::SimdUint;

pub const LANES: usize = 8;
pub type F32x8 = Simd<f32, LANES>;
pub type U32x8 = Simd<u32, LANES>;

/// Восемь пикселей, распакованных в SIMD-векторы цветовых каналов.
#[derive(Debug, Clone, Copy)]
pub struct RgbaBlock8 {
    pub red: F32x8,
    pub green: F32x8,
    pub blue: F32x8,
    pub packed: U32x8,
}

/// Распаковывает RGBA без промежуточного массива для каждого канала.
/// Явный little-endian порядок делает позиции каналов независимыми от платформы.
#[inline]
pub fn unpack_rgba_block(colors: &[Color32; LANES]) -> RgbaBlock8 {
    let packed = U32x8::from_array(std::array::from_fn(|lane| {
        u32::from_le_bytes(colors[lane].to_array())
    }));
    let channel_mask = U32x8::splat(u32::from(u8::MAX));
    RgbaBlock8 {
        red: (packed & channel_mask).cast(),
        green: ((packed >> 8) & channel_mask).cast(),
        blue: ((packed >> 16) & channel_mask).cast(),
        packed,
    }
}
