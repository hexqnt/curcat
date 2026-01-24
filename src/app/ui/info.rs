use super::super::{
    AxisCalUi, CurcatApp, describe_aspect_ratio, format_system_time, human_readable_bytes,
    total_pixel_count,
};
use crate::types::{AxisUnit, AxisValue, CoordSystem, PolarMapping};
use egui::{Color32, RichText};

impl CurcatApp {
    pub(crate) fn ui_status_bar(&self, ui: &mut egui::Ui) {
        let points_count = self.points.points.len();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Points: {points_count}"))
                    .small()
                    .color(Color32::from_gray(180)),
            );
            if let Some(msg) = &self.ui.last_status {
                ui.separator();
                ui.label(
                    RichText::new(msg.as_str())
                        .small()
                        .color(Color32::from_gray(200)),
                );
            }
        });
    }

    pub(crate) fn ui_image_info_window(&mut self, ctx: &egui::Context) {
        if !self.ui.info_window_open {
            return;
        }

        egui::Window::new("Image info")
            .open(&mut self.ui.info_window_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(image) = &self.image.image {
                    ui.heading("File");
                    if let Some(meta) = self.image.meta.as_ref() {
                        ui.label(format!("Source: {}", meta.source_label()));
                        ui.label(format!("Name: {}", meta.display_name()));
                        if let Some(path) = meta.path() {
                            ui.label(format!("Path: {}", path.display()));
                        }
                        if let Some(bytes) = meta.byte_len() {
                            ui.label(format!(
                                "Size: {} ({bytes} bytes)",
                                human_readable_bytes(bytes),
                            ));
                        } else {
                            ui.label("Size: Unknown");
                        }
                        if let Some(modified) = meta.last_modified() {
                            ui.label(format!("Modified: {}", format_system_time(modified),));
                        } else {
                            ui.label("Modified: Unknown");
                        }
                    } else {
                        ui.label("No captured file metadata for this image.");
                    }

                    ui.add_space(6.0);
                    ui.heading("Image");
                    let [w, h] = image.size;
                    ui.label(format!("Dimensions: {w} × {h} px"));
                    if let Some(aspect_text) = describe_aspect_ratio(image.size) {
                        ui.label(format!("Aspect ratio: {aspect_text}"));
                    } else {
                        ui.label("Aspect ratio: n/a");
                    }
                    let total_pixels = total_pixel_count(image.size);
                    ui.label(format!(
                        "Pixels: {total_pixels} ({:.2} MP)",
                        total_pixels as f64 / 1_000_000.0
                    ));
                    let rgba_bytes = total_pixels.saturating_mul(4);
                    ui.label(format!(
                        "RGBA memory estimate: {} ({rgba_bytes} bytes)",
                        human_readable_bytes(rgba_bytes),
                    ));
                    ui.label(format!(
                        "Current zoom: {}",
                        Self::format_zoom(self.image.zoom)
                    ));
                } else {
                    ui.label("Load an image to inspect its metadata.");
                }
            });
    }

    pub(crate) fn ui_points_info_window(&mut self, ctx: &egui::Context) {
        if !self.ui.points_info_window_open {
            return;
        }

        let (x_mapping, y_mapping) = self.cartesian_mappings();
        let polar_mapping = self.polar_mapping();
        self.ensure_point_numeric_cache(
            self.calibration.coord_system,
            x_mapping.as_ref(),
            y_mapping.as_ref(),
            polar_mapping.as_ref(),
        );

        let mut open = self.ui.points_info_window_open;
        egui::Window::new("Points info")
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                let total = self.points.points.len();
                ui.heading("Points");
                ui.label(format!("Placed: {total}"));
                if total == 0 {
                    ui.label("Add points to see stats.");
                    return;
                }

                let calibrated = self
                    .points
                    .points
                    .iter()
                    .filter(|p| p.x_numeric.is_some() && p.y_numeric.is_some())
                    .count();
                if calibrated != total {
                    ui.label(
                        RichText::new(format!("Calibrated pairs: {calibrated} (need both axes)"))
                            .weak(),
                    );
                }

                ui.add_space(6.0);
                ui.heading("Ranges");
                match self.calibration.coord_system {
                    crate::types::CoordSystem::Cartesian => {
                        self.render_axis_stats(ui, "X axis", AxisKind::X, x_mapping.as_ref());
                        self.render_axis_stats(ui, "Y axis", AxisKind::Y, y_mapping.as_ref());
                    }
                    crate::types::CoordSystem::Polar => {
                        self.render_polar_axis_stats(
                            ui,
                            "Angle",
                            AxisKind::X,
                            polar_mapping.as_ref(),
                        );
                        self.render_polar_axis_stats(
                            ui,
                            "Radius",
                            AxisKind::Y,
                            polar_mapping.as_ref(),
                        );
                    }
                }

                ui.add_space(6.0);
                ui.heading("Calibration");
                self.render_calibration_stats(ui, polar_mapping.as_ref());

                ui.add_space(6.0);
                ui.heading("Geometry");
                self.render_geometry_stats(ui);
            });
        self.ui.points_info_window_open = open;
    }
}

#[derive(Clone, Copy)]
enum AxisKind {
    X,
    Y,
}

#[derive(Clone, Copy)]
struct RangeF64 {
    min: f64,
    max: f64,
}

impl RangeF64 {
    const fn span(self) -> f64 {
        self.max - self.min
    }
}

#[derive(Clone, Copy)]
struct RangeF32 {
    min: f32,
    max: f32,
}

impl RangeF32 {
    const fn span(self) -> f32 {
        self.max - self.min
    }
}

impl CurcatApp {
    fn render_axis_stats(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        axis: AxisKind,
        mapping: Option<&crate::types::AxisMapping>,
    ) {
        let numeric_range = mapping.and_then(|_| self.axis_numeric_range(axis));
        let pixel_range = self.axis_pixel_range(axis);

        if let (Some(range), Some(map)) = (numeric_range, mapping) {
            let min = AxisValue::from_scalar_seconds(map.unit, range.min)
                .map_or_else(|| "out of range".to_string(), |v| v.format());
            let max = AxisValue::from_scalar_seconds(map.unit, range.max)
                .map_or_else(|| "out of range".to_string(), |v| v.format());
            let span = format_span(map.unit, range.span());
            ui.label(format!("{label}: {min} … {max} (Δ {span})"));
            if let Some(pix) = pixel_range {
                ui.label(
                    RichText::new(format!(
                        "{label} pixels: {:.1} … {:.1} (Δ {:.1} px)",
                        pix.min,
                        pix.max,
                        pix.span()
                    ))
                    .weak(),
                );
            }
        } else if let Some(pix) = pixel_range {
            ui.label(format!(
                "{label} (px): {:.1} … {:.1} (Δ {:.1} px)",
                pix.min,
                pix.max,
                pix.span()
            ));
            if mapping.is_none() {
                ui.label(RichText::new("Calibrate this axis to see numeric values.").weak());
            }
        } else {
            ui.label(format!("{label}: no data"));
        }
    }

    fn render_calibration_stats(&self, ui: &mut egui::Ui, polar_mapping: Option<&PolarMapping>) {
        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                let x_len = axis_length(&self.calibration.cal_x);
                let y_len = axis_length(&self.calibration.cal_y);
                if let Some(len) = x_len {
                    ui.label(format!("X axis length: {len:.1} px"));
                } else {
                    ui.label(RichText::new("X axis not set").weak());
                }
                if let Some(len) = y_len {
                    ui.label(format!("Y axis length: {len:.1} px"));
                } else {
                    ui.label(RichText::new("Y axis not set").weak());
                }

                if let Some(ortho) = self.axis_orthogonality() {
                    ui.label(format!(
                        "Angle between axes: {:.2}° (offset {:.2}° from 90°)",
                        ortho.actual_deg, ortho.delta_from_right_deg
                    ));
                } else {
                    ui.label(
                        RichText::new("Add both calibration axes to measure orthogonality.").weak(),
                    );
                }
            }
            CoordSystem::Polar => {
                if let Some(origin) = self.calibration.polar_cal.origin {
                    ui.label(format!("Origin: @ ({:.1}, {:.1})", origin.x, origin.y));
                } else {
                    ui.label(RichText::new("Origin not set").weak());
                }

                let (rp1, rp2) = (
                    self.calibration.polar_cal.radius.p1,
                    self.calibration.polar_cal.radius.p2,
                );
                if let (Some(p1), Some(p2)) = (rp1, rp2) {
                    if let Some(origin) = self.calibration.polar_cal.origin {
                        let d1 = (p1 - origin).length();
                        let d2 = (p2 - origin).length();
                        ui.label(format!("Radius points: R1 {:.1} px, R2 {:.1} px", d1, d2));
                    } else {
                        ui.label(
                            RichText::new("Radius points set (origin needed for lengths).").weak(),
                        );
                    }
                } else {
                    ui.label(RichText::new("Radius points not set").weak());
                }

                let (ap1, ap2) = (
                    self.calibration.polar_cal.angle.p1,
                    self.calibration.polar_cal.angle.p2,
                );
                if ap1.is_some() && ap2.is_some() {
                    let unit = polar_mapping.map_or(
                        self.calibration.polar_cal.angle_unit,
                        crate::types::PolarMapping::angle_unit,
                    );
                    if let (Some(v1), Some(v2)) = (
                        crate::types::parse_axis_value(
                            &self.calibration.polar_cal.angle.v1_text,
                            AxisUnit::Float,
                        ),
                        crate::types::parse_axis_value(
                            &self.calibration.polar_cal.angle.v2_text,
                            AxisUnit::Float,
                        ),
                    ) {
                        let v1 = match v1 {
                            AxisValue::Float(v) => v,
                            _ => 0.0,
                        };
                        let v2 = match v2 {
                            AxisValue::Float(v) => v,
                            _ => 0.0,
                        };
                        ui.label(format!(
                            "Angle values: {} … {} {}",
                            AxisValue::Float(v1).format(),
                            AxisValue::Float(v2).format(),
                            unit.label()
                        ));
                    } else {
                        ui.label(RichText::new("Angle points set (values invalid).").weak());
                    }
                } else {
                    ui.label(RichText::new("Angle points not set").weak());
                }
            }
        }
    }

    fn render_polar_axis_stats(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        axis: AxisKind,
        polar_mapping: Option<&PolarMapping>,
    ) {
        let numeric_range = self.axis_numeric_range(axis);
        if let Some(range) = numeric_range {
            let min = AxisValue::Float(range.min).format();
            let max = AxisValue::Float(range.max).format();
            let span = AxisValue::Float(range.span()).format();
            let unit = match axis {
                AxisKind::X => polar_mapping
                    .map_or(
                        self.calibration.polar_cal.angle_unit,
                        crate::types::PolarMapping::angle_unit,
                    )
                    .label(),
                AxisKind::Y => "",
            };
            if unit.is_empty() {
                ui.label(format!("{label}: {min} … {max} (Δ {span})"));
            } else {
                ui.label(format!("{label}: {min} … {max} (Δ {span} {unit})"));
            }
        } else {
            ui.label(format!("{label}: no data"));
        }
    }

    fn render_geometry_stats(&self, ui: &mut egui::Ui) {
        if let Some((xr, yr)) = self.pixel_bounds() {
            ui.label(format!(
                "Pixel bounds: x {:.1}…{:.1}, y {:.1}…{:.1}",
                xr.min, xr.max, yr.min, yr.max
            ));
            ui.label(format!("Span: {:.1} × {:.1} px", xr.span(), yr.span()));
        } else {
            ui.label(RichText::new("No points for geometry stats.").weak());
        }

        if let Some((avg, total)) = self.pixel_step_stats() {
            ui.label(format!("Average step: {avg:.1} px"));
            ui.label(RichText::new(format!("Total polyline length: {total:.1} px")).weak());
        }
    }

    fn axis_numeric_range(&self, axis: AxisKind) -> Option<RangeF64> {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for p in &self.points.points {
            let val = match axis {
                AxisKind::X => p.x_numeric,
                AxisKind::Y => p.y_numeric,
            };
            if let Some(v) = val {
                min = min.min(v);
                max = max.max(v);
            }
        }
        if min.is_finite() && max.is_finite() {
            Some(RangeF64 { min, max })
        } else {
            None
        }
    }

    fn axis_pixel_range(&self, axis: AxisKind) -> Option<RangeF32> {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for p in &self.points.points {
            let v = match axis {
                AxisKind::X => p.pixel.x,
                AxisKind::Y => p.pixel.y,
            };
            min = min.min(v);
            max = max.max(v);
        }
        if min.is_finite() && max.is_finite() {
            Some(RangeF32 { min, max })
        } else {
            None
        }
    }

    fn pixel_bounds(&self) -> Option<(RangeF32, RangeF32)> {
        match (
            self.axis_pixel_range(AxisKind::X),
            self.axis_pixel_range(AxisKind::Y),
        ) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        }
    }

    fn pixel_step_stats(&self) -> Option<(f32, f32)> {
        if self.points.points.len() < 2 {
            return None;
        }
        let mut total = 0.0_f32;
        for pair in self.points.points.windows(2) {
            if let [a, b] = pair {
                total += (b.pixel - a.pixel).length();
            }
        }
        let avg = total / ((self.points.points.len() - 1) as f32);
        Some((avg, total))
    }

    fn axis_orthogonality(&self) -> Option<AxisOrthogonality> {
        let (xp1, xp2) = (self.calibration.cal_x.p1?, self.calibration.cal_x.p2?);
        let (yp1, yp2) = (self.calibration.cal_y.p1?, self.calibration.cal_y.p2?);
        let vx = xp2 - xp1;
        let vy = yp2 - yp1;
        let lx = vx.length();
        let ly = vy.length();
        if lx <= f32::EPSILON || ly <= f32::EPSILON {
            return None;
        }
        let dot = vx.dot(vy);
        let cos_theta = (dot / (lx * ly)).clamp(-1.0, 1.0);
        let angle_rad = cos_theta.acos();
        let delta = (std::f32::consts::FRAC_PI_2 - angle_rad).abs();
        Some(AxisOrthogonality {
            actual_deg: angle_rad.to_degrees(),
            delta_from_right_deg: delta.to_degrees(),
        })
    }
}

struct AxisOrthogonality {
    actual_deg: f32,
    delta_from_right_deg: f32,
}

fn axis_length(cal: &AxisCalUi) -> Option<f32> {
    let (p1, p2) = (cal.p1?, cal.p2?);
    let len = (p2 - p1).length();
    if len > f32::EPSILON { Some(len) } else { None }
}

fn format_span(unit: AxisUnit, span: f64) -> String {
    match unit {
        AxisUnit::Float => AxisValue::from_scalar_seconds(AxisUnit::Float, span)
            .map_or_else(|| format!("{span:.6}"), |v| v.format()),
        AxisUnit::DateTime => format_duration(span),
    }
}

fn format_duration(seconds: f64) -> String {
    const MINUTE: f64 = 60.0;
    const HOUR: f64 = 3600.0;
    const DAY: f64 = 86_400.0;
    if seconds >= DAY {
        format!("{:.3} d", seconds / DAY)
    } else if seconds >= HOUR {
        format!("{:.3} h", seconds / HOUR)
    } else if seconds >= MINUTE {
        format!("{:.3} min", seconds / MINUTE)
    } else {
        format!("{seconds:.3} s")
    }
}
