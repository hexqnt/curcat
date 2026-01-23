use egui::Color32;

pub(super) fn color_luminance(color: Color32) -> f32 {
    let [r, g, b, _] = color.to_array();
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

pub(super) fn color_similarity_value(color: Color32, target: Color32, tolerance: f32) -> f32 {
    let [tr, tg, tb, _] = target.to_array();
    let [r, g, b, _] = color.to_array();
    let dr = f32::from(r) - f32::from(tr);
    let dg = f32::from(g) - f32::from(tg);
    let db = f32::from(b) - f32::from(tb);
    let diff = (dr * dr + dg * dg + db * db).sqrt();
    let tol = tolerance.max(1.0);
    ((tol - diff).max(0.0) / tol).clamp(0.0, 1.0)
}
