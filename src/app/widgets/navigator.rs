use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Ui, Vec2, emath::RectTransform, pos2};

use crate::i18n::UiLanguage;

/// An image navigator reports a requested image center without changing app state.
#[must_use = "Show the navigator overlay with show()."]
pub(in crate::app) struct Navigator {
    texture_id: egui::TextureId,
    image_rect: egui::Rect,
    image_size: Vec2,
    viewport_rect: egui::Rect,
    language: UiLanguage,
}

impl Navigator {
    pub(in crate::app) const fn new(
        texture_id: egui::TextureId,
        image_rect: egui::Rect,
        image_size: Vec2,
        viewport_rect: egui::Rect,
        language: UiLanguage,
    ) -> Self {
        Self {
            texture_id,
            image_rect,
            image_size,
            viewport_rect,
            language,
        }
    }

    pub(in crate::app) fn show(self, ui: &Ui) -> Option<Pos2> {
        let Self {
            texture_id,
            image_rect,
            image_size,
            viewport_rect,
            ..
        } = self;
        if image_size.x <= f32::EPSILON || image_size.y <= f32::EPSILON {
            return None;
        }

        let display_size = image_rect.size();
        let is_large = display_size.x > viewport_rect.width() * 1.10
            || display_size.y > viewport_rect.height() * 1.10;
        if !is_large {
            return None;
        }

        let minimap_max = Vec2::new(190.0, 145.0);
        let scale = (minimap_max.x / image_size.x).min(minimap_max.y / image_size.y);
        if !scale.is_finite() || scale <= f32::EPSILON {
            return None;
        }
        let thumb_size = image_size * scale;
        let frame_padding = Vec2::new(8.0, 8.0);
        let panel_size = thumb_size + frame_padding * 2.0;
        let outer_rect = ui.max_rect();
        let panel_rect = egui::Rect::from_min_size(
            pos2(
                outer_rect.right() - panel_size.x - 12.0,
                outer_rect.top() + 12.0,
            ),
            panel_size,
        );
        let thumb_rect = egui::Rect::from_min_size(panel_rect.min + frame_padding, thumb_size);

        let mut requested_center = None;
        let id = ui.make_persistent_id("navigator_minimap");
        let response = ui.interact(panel_rect, id, Sense::click_and_drag());
        let hover_text = match self.language {
            crate::i18n::UiLanguage::En => "Navigator: click or drag to pan the viewport",
            crate::i18n::UiLanguage::Ru => {
                "Навигатор: кликните или тяните, чтобы панорамировать вид"
            }
        };
        let response = response.on_hover_text(hover_text);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, response.enabled(), hover_text)
        });
        if (response.clicked() || response.dragged())
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_target = image_target(pointer, thumb_rect, image_size, scale);
            requested_center = Some(image_target);
        }

        let [r, g, b, _] = Color32::from_rgb(120, 185, 255).to_array();
        let painter = ui.painter();
        painter.rect_filled(
            panel_rect,
            CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(20, 24, 28, 190),
        );
        painter.rect_stroke(
            panel_rect,
            CornerRadius::same(6),
            egui::Stroke::new(1.0_f32, Color32::from_gray(90)),
            egui::StrokeKind::Outside,
        );
        painter.image(
            texture_id,
            thumb_rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        painter.rect_stroke(
            thumb_rect,
            CornerRadius::same(3),
            egui::Stroke::new(1.0_f32, Color32::from_gray(130)),
            egui::StrokeKind::Outside,
        );

        if let Some(marker) = viewport_marker(image_rect, viewport_rect, thumb_rect) {
            painter.rect_filled(
                marker,
                CornerRadius::same(2),
                Color32::from_rgba_unmultiplied(r, g, b, 30),
            );
            painter.rect_stroke(
                marker,
                CornerRadius::same(2),
                egui::Stroke::new(1.4, Color32::from_rgba_unmultiplied(r, g, b, 235)),
                egui::StrokeKind::Outside,
            );
        }
        requested_center
    }
}

fn image_target(pointer: Pos2, thumb_rect: egui::Rect, image_size: Vec2, scale: f32) -> Pos2 {
    let local = thumb_rect.clamp(pointer) - thumb_rect.min;
    pos2(
        (local.x / scale).clamp(0.0, image_size.x),
        (local.y / scale).clamp(0.0, image_size.y),
    )
}

/// Map the visible image into the thumbnail and keep thin markers legible at its edges.
fn viewport_marker(image: Rect, viewport: Rect, thumbnail: Rect) -> Option<Rect> {
    let visible = image.intersect(viewport);
    if !visible.is_positive() {
        return None;
    }
    let marker = RectTransform::from_to(image, thumbnail).transform_rect(visible);
    let size = marker.size().max(Vec2::splat(4.0)).min(thumbnail.size());
    let min = (marker.center() - size * 0.5).clamp(thumbnail.min, thumbnail.max - size);
    Some(Rect::from_min_size(min, size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigator_maps_thumbnail_positions_and_clamps_drag_outside_image() {
        let size = Vec2::new(640.0, 480.0);
        let scale = 0.25;
        let rect = egui::Rect::from_min_size(pos2(10.0, 20.0), size * scale);
        for (pointer, expected) in [
            (rect.min, Pos2::ZERO),
            (rect.center(), pos2(320.0, 240.0)),
            (rect.max, pos2(640.0, 480.0)),
            (rect.min - Vec2::splat(100.0), Pos2::ZERO),
            (rect.max + Vec2::splat(100.0), pos2(640.0, 480.0)),
        ] {
            assert_eq!(image_target(pointer, rect, size, scale), expected);
        }
    }

    #[test]
    fn viewport_marker_maps_visible_fraction_and_ignores_nonoverlapping_viewports() {
        let image = Rect::from_min_size(pos2(100.0, 200.0), Vec2::new(1280.0, 960.0));
        let thumbnail = Rect::from_min_size(pos2(20.0, 30.0), Vec2::new(160.0, 120.0));
        let viewport = Rect::from_min_size(image.center(), image.size() * 0.5);
        let marker = viewport_marker(image, viewport, thumbnail).unwrap();
        assert_eq!(marker.min, thumbnail.center());
        assert_eq!(marker.max, thumbnail.max);
        assert_eq!(
            viewport_marker(image, image.expand(100.0), thumbnail),
            Some(thumbnail)
        );
        assert!(
            viewport_marker(image, image.translate(Vec2::new(2000.0, 0.0)), thumbnail).is_none()
        );
    }

    #[test]
    fn thin_viewport_markers_keep_minimum_size_inside_thumbnail() {
        let image = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1000.0));
        let thumbnail = Rect::from_min_size(pos2(20.0, 30.0), Vec2::splat(100.0));
        for corner in [image.min, image.max - Vec2::splat(1.0)] {
            let viewport = Rect::from_min_size(corner, Vec2::splat(1.0));
            let marker = viewport_marker(image, viewport, thumbnail).unwrap();
            assert_eq!(marker.size(), Vec2::splat(4.0));
            assert!(thumbnail.contains_rect(marker));
        }
        let narrow_thumbnail = Rect::from_min_size(thumbnail.min, Vec2::new(100.0, 1.0));
        let marker = viewport_marker(image, image, narrow_thumbnail).unwrap();
        assert!(narrow_thumbnail.contains_rect(marker));
    }
}
