use super::super::{
    CurcatApp, describe_aspect_ratio, format_system_time, human_readable_bytes, total_pixel_count,
};
use super::stats::{AxisKind, axis_length, format_span};
use crate::i18n::TextKey;
use crate::types::{AxisUnit, AxisValue, CoordSystem, PolarMapping};
use egui::{Color32, RichText};

impl CurcatApp {
    pub(crate) fn ui_status_bar(&mut self, ui: &mut egui::Ui) {
        let points_count = self.points.points.len();
        let i18n = self.i18n();
        ui.horizontal(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(i18n.format_points_count(points_count))
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.ui_language_selector(ui);
                ui.add_space(6.0);
                egui::widgets::global_theme_preference_switch(ui);
            });
        });
    }

    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn ui_image_info_window(&mut self, ctx: &egui::Context) {
        if !self.ui.info_window_open {
            return;
        }

        let i18n = self.i18n();
        egui::Window::new(i18n.text(TextKey::ImageInfoWindow))
            .open(&mut self.ui.info_window_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(image) = &self.image.image {
                    ui.heading(i18n.text(TextKey::FileSection));
                    if let Some(meta) = self.image.meta.as_ref() {
                        let source_label = match (self.ui.language, meta.source_label()) {
                            (crate::i18n::UiLanguage::Ru, "File on disk") => "Файл на диске",
                            (crate::i18n::UiLanguage::Ru, "Dropped bytes") => "Перетащенные байты",
                            (crate::i18n::UiLanguage::Ru, "Clipboard") => "Буфер обмена",
                            _ => meta.source_label(),
                        };
                        let display_name = match (self.ui.language, meta.display_name().as_str()) {
                            (crate::i18n::UiLanguage::Ru, "Clipboard image") => {
                                "Изображение из буфера обмена".to_string()
                            }
                            (crate::i18n::UiLanguage::Ru, "Unnamed drop") => {
                                "Перетащенный файл без имени".to_string()
                            }
                            _ => meta.display_name(),
                        };
                        ui.label(i18n.format_source(source_label));
                        ui.label(i18n.format_name(&display_name));
                        if let Some(path) = meta.path() {
                            ui.label(i18n.format_path(&path.display().to_string()));
                        }
                        if let Some(bytes) = meta.byte_len() {
                            let size_hint = match self.ui.language {
                                crate::i18n::UiLanguage::En => {
                                    format!("{} ({bytes} bytes)", human_readable_bytes(bytes))
                                }
                                crate::i18n::UiLanguage::Ru => {
                                    format!("{} ({bytes} байт)", human_readable_bytes(bytes))
                                }
                            };
                            ui.label(i18n.format_size(&size_hint));
                        } else {
                            ui.label(i18n.format_size(i18n.text(TextKey::SizeUnknown)));
                        }
                        if let Some(modified) = meta.last_modified() {
                            ui.label(i18n.format_modified(&format_system_time(modified)));
                        } else {
                            ui.label(i18n.format_modified(i18n.text(TextKey::ModifiedUnknown)));
                        }
                    } else {
                        ui.label(i18n.text(TextKey::NoFileMetadataForImage));
                    }

                    ui.add_space(6.0);
                    ui.heading(i18n.text(TextKey::ImageSection));
                    let [w, h] = image.size;
                    ui.label(i18n.format_dimensions(w, h));
                    if let Some(aspect_text) = describe_aspect_ratio(image.size) {
                        ui.label(i18n.format_aspect_ratio(&aspect_text));
                    } else {
                        ui.label(i18n.format_aspect_ratio(i18n.text(TextKey::AspectRatioNa)));
                    }
                    let total_pixels = total_pixel_count(image.size);
                    ui.label(i18n.format_pixels(total_pixels, total_pixels as f64 / 1_000_000.0));
                    let rgba_bytes = total_pixels.saturating_mul(4);
                    ui.label(i18n.format_rgba_memory_estimate(
                        &human_readable_bytes(rgba_bytes),
                        rgba_bytes,
                    ));
                    ui.label(i18n.format_current_zoom(&Self::format_zoom(self.image.zoom)));
                } else {
                    ui.label(i18n.text(TextKey::LoadImageToInspectMetadata));
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
        let i18n = self.i18n();
        egui::Window::new(i18n.text(TextKey::PointsInfoWindow))
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                let total = self.points.points.len();
                ui.heading(i18n.text(TextKey::Points));
                ui.label(i18n.format_placed_points(total));
                if total == 0 {
                    ui.label(i18n.text(TextKey::AddPointsToSeeStats));
                    return;
                }

                let calibrated = self
                    .points
                    .points
                    .iter()
                    .filter(|p| p.x_numeric.is_some() && p.y_numeric.is_some())
                    .count();
                if calibrated != total {
                    ui.label(RichText::new(i18n.format_calibrated_pairs(calibrated)).weak());
                }

                ui.add_space(6.0);
                ui.heading(i18n.text(TextKey::Ranges));
                match self.calibration.coord_system {
                    crate::types::CoordSystem::Cartesian => {
                        self.render_axis_stats(
                            ui,
                            i18n.text(TextKey::XAxis),
                            AxisKind::X,
                            x_mapping.as_ref(),
                        );
                        self.render_axis_stats(
                            ui,
                            i18n.text(TextKey::YAxis),
                            AxisKind::Y,
                            y_mapping.as_ref(),
                        );
                    }
                    crate::types::CoordSystem::Polar => {
                        self.render_polar_axis_stats(
                            ui,
                            i18n.text(TextKey::Angle),
                            AxisKind::X,
                            polar_mapping.as_ref(),
                        );
                        self.render_polar_axis_stats(
                            ui,
                            i18n.text(TextKey::Radius),
                            AxisKind::Y,
                            polar_mapping.as_ref(),
                        );
                    }
                }

                ui.add_space(6.0);
                ui.heading(i18n.text(TextKey::CalibrationSection));
                self.render_calibration_stats(ui, polar_mapping.as_ref());

                ui.add_space(6.0);
                ui.heading(i18n.text(TextKey::Geometry));
                self.render_geometry_stats(ui);
            });
        self.ui.points_info_window_open = open;
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
        let out_of_range = match self.ui.language {
            crate::i18n::UiLanguage::En => "out of range",
            crate::i18n::UiLanguage::Ru => "вне диапазона",
        };

        if let (Some(range), Some(map)) = (numeric_range, mapping) {
            let min = AxisValue::from_scalar_seconds(map.unit, range.min)
                .map_or_else(|| out_of_range.to_string(), |v| v.format());
            let max = AxisValue::from_scalar_seconds(map.unit, range.max)
                .map_or_else(|| out_of_range.to_string(), |v| v.format());
            let span = format_span(map.unit, range.span());
            ui.label(self.i18n().format_axis_range(label, &min, &max, &span));
            if let Some(pix) = pixel_range {
                ui.label(
                    RichText::new(self.i18n().format_axis_pixels(
                        label,
                        pix.min,
                        pix.max,
                        pix.span(),
                    ))
                    .weak(),
                );
            }
        } else if let Some(pix) = pixel_range {
            ui.label(
                self.i18n()
                    .format_axis_pixels_only(label, pix.min, pix.max, pix.span()),
            );
            if mapping.is_none() {
                ui.label(RichText::new(self.t(TextKey::CalibrateAxisToSeeNumericValues)).weak());
            }
        } else {
            ui.label(format!("{label}: {}", self.t(TextKey::NoData)));
        }
    }

    fn render_calibration_stats(&self, ui: &mut egui::Ui, polar_mapping: Option<&PolarMapping>) {
        match self.calibration.coord_system {
            CoordSystem::Cartesian => {
                let x_len = axis_length(&self.calibration.cal_x);
                let y_len = axis_length(&self.calibration.cal_y);
                if let Some(len) = x_len {
                    ui.label(self.i18n().format_x_axis_length(len));
                } else {
                    ui.label(RichText::new(self.t(TextKey::XAxisNotSet)).weak());
                }
                if let Some(len) = y_len {
                    ui.label(self.i18n().format_y_axis_length(len));
                } else {
                    ui.label(RichText::new(self.t(TextKey::YAxisNotSet)).weak());
                }

                if let Some(ortho) = self.axis_orthogonality() {
                    ui.label(
                        self.i18n()
                            .format_axes_angle(ortho.actual_deg, ortho.delta_from_right_deg),
                    );
                } else {
                    ui.label(
                        RichText::new(self.t(TextKey::AddBothAxesToMeasureOrthogonality)).weak(),
                    );
                }
            }
            CoordSystem::Polar => {
                if let Some(origin) = self.calibration.polar_cal.origin {
                    ui.label(self.i18n().format_origin_coords(origin.x, origin.y));
                } else {
                    ui.label(RichText::new(self.t(TextKey::OriginNotSet)).weak());
                }

                let (rp1, rp2) = (
                    self.calibration.polar_cal.radius.p1,
                    self.calibration.polar_cal.radius.p2,
                );
                if let (Some(p1), Some(p2)) = (rp1, rp2) {
                    if let Some(origin) = self.calibration.polar_cal.origin {
                        let d1 = (p1 - origin).length();
                        let d2 = (p2 - origin).length();
                        ui.label(self.i18n().format_radius_points(d1, d2));
                    } else {
                        ui.label(RichText::new(self.t(TextKey::RadiusPointsSetNeedOrigin)).weak());
                    }
                } else {
                    ui.label(RichText::new(self.t(TextKey::RadiusPointsNotSet)).weak());
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
                            AxisValue::DateTime(_) => 0.0,
                        };
                        let v2 = match v2 {
                            AxisValue::Float(v) => v,
                            AxisValue::DateTime(_) => 0.0,
                        };
                        ui.label(format!(
                            "{}",
                            self.i18n().format_angle_values(
                                &AxisValue::Float(v1).format(),
                                &AxisValue::Float(v2).format(),
                                unit.label(),
                            )
                        ));
                    } else {
                        ui.label(
                            RichText::new(self.t(TextKey::AnglePointsSetValuesInvalid)).weak(),
                        );
                    }
                } else {
                    ui.label(RichText::new(self.t(TextKey::AnglePointsNotSet)).weak());
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
                ui.label(self.i18n().format_axis_range(label, &min, &max, &span));
            } else {
                ui.label(format!("{label}: {min} … {max} (Δ {span} {unit})"));
            }
        } else {
            ui.label(format!("{label}: {}", self.t(TextKey::NoData)));
        }
    }

    fn render_geometry_stats(&self, ui: &mut egui::Ui) {
        if let Some((xr, yr)) = self.pixel_bounds() {
            ui.label(
                self.i18n()
                    .format_pixel_bounds(xr.min, xr.max, yr.min, yr.max),
            );
            ui.label(self.i18n().format_span(xr.span(), yr.span()));
        } else {
            ui.label(RichText::new(self.t(TextKey::NoPointsForGeometryStats)).weak());
        }

        if let Some((avg, total)) = self.pixel_step_stats() {
            ui.label(self.i18n().format_average_step(avg));
            ui.label(RichText::new(self.i18n().format_total_polyline_length(total)).weak());
        }
    }
}
