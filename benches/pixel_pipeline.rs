#![feature(portable_simd)]
// Бенчмарк подключает внутренние модули напрямую, поэтому их crate-видимость
// выглядит для Clippy избыточной только в этой отдельной crate.
#![allow(dead_code, clippy::redundant_pub_crate)]

#[path = "../src/snap/behavior.rs"]
mod behavior;
#[path = "../src/snap/color.rs"]
mod color;
#[path = "../src/image/filters.rs"]
mod filters;
#[path = "../src/snap/maps.rs"]
mod maps;
#[path = "../src/pixel_simd.rs"]
mod pixel_simd;
#[path = "../src/snap/search.rs"]
mod search;
#[path = "../src/util.rs"]
mod util;

use egui::{Color32, ColorImage};
use filters::{ImageFilters, apply_image_filters};
use maps::SnapMapCache;
use std::hint::black_box;
use std::time::{Duration, Instant};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const PIXEL_COUNT: usize = WIDTH * HEIGHT;

fn test_image() -> ColorImage {
    let pixels = (0..PIXEL_COUNT)
        .map(|index| {
            let column = index % WIDTH;
            let row = index / WIDTH;
            let red = u8::try_from((column * 17 + row * 11) & 0xff).unwrap_or_default();
            let green = u8::try_from((column * 7 + row * 23) & 0xff).unwrap_or_default();
            let blue = u8::try_from((column * 29 + row * 3) & 0xff).unwrap_or_default();
            Color32::from_rgb(red, green, blue)
        })
        .collect();
    ColorImage::new([WIDTH, HEIGHT], pixels)
}

fn measure(mut operation: impl FnMut(), samples: usize) -> Duration {
    operation();
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        operation();
        times.push(started.elapsed());
    }
    times.sort_unstable();
    times[times.len() / 2]
}

fn report(name: &str, elapsed: Duration) {
    let pixel_count = f64::from(
        u32::try_from(PIXEL_COUNT).expect("benchmark image pixel count must fit into u32"),
    );
    let nanos_per_pixel = elapsed.as_secs_f64() * 1.0e9 / pixel_count;
    println!("{name:24} {elapsed:>10.3?}  {nanos_per_pixel:>7.3} ns/pixel");
}

fn main() {
    let image = test_image();
    let gamma_one = ImageFilters {
        brightness: 0.12,
        contrast: 0.25,
        invert: true,
        ..ImageFilters::default()
    };
    let gamma = ImageFilters {
        brightness: -0.08,
        contrast: 0.2,
        gamma: 2.2,
        ..ImageFilters::default()
    };
    let blur = ImageFilters {
        blur_radius: 8,
        ..ImageFilters::default()
    };
    let target = Color32::from_rgb(40, 120, 220);

    report(
        "filter/gamma-1",
        measure(
            || {
                black_box(apply_image_filters(black_box(&image), black_box(gamma_one)));
            },
            15,
        ),
    );
    report(
        "filter/gamma-2.2",
        measure(
            || {
                black_box(apply_image_filters(black_box(&image), black_box(gamma)));
            },
            9,
        ),
    );
    report(
        "filter/blur-8",
        measure(
            || {
                black_box(apply_image_filters(black_box(&image), black_box(blur)));
            },
            9,
        ),
    );
    report(
        "snap/cache-build",
        measure(
            || {
                black_box(SnapMapCache::build(
                    black_box(&image),
                    black_box(target),
                    black_box(96.0),
                ));
            },
            9,
        ),
    );
}
