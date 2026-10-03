#![feature(portable_simd)]

mod app;
mod config;
mod export;
mod i18n;
mod image;
mod interp;
mod pixel_simd;
mod project;
mod snap;
mod types;
mod util;

pub use app::CurcatApp;

#[cfg(feature = "testing")]
pub use app::testing;
