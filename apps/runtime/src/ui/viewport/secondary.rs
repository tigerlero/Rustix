use super::manager::viewport_texture_id;
use super::Viewport;

/// Show a secondary viewport as a floating egui window.
pub fn show_secondary_viewport(
    ctx: &egui::Context,
    vp: &mut Viewport,
    index: usize,
) {
    let tex_id = viewport_texture_id(index);
    let rect_key = egui::Id::new(format!("viewport_rect_{}", index));
    let valid_key = egui::Id::new(format!("viewport_offscreen_valid_{}", index));
    let pos_key = egui::Id::new(format!("viewport_pos_{}", index));
    let size_key = egui::Id::new(format!("viewport_size_{}", index));

    let saved_pos = ctx.data(|d| d.get_temp::<egui::Pos2>(pos_key));
    let saved_size = ctx.data(|d| d.get_temp::<egui::Vec2>(size_key));
    let gen = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0));

    let mut window = egui::Window::new(&vp.name)
        .id(egui::Id::new(("viewport_win", index, gen)))
        .open(&mut vp.open)
        .default_size([400.0, 300.0]);
    if let Some(pos) = saved_pos {
        window = window.default_pos(pos);
    }
    if let Some(size) = saved_size {
        window = window.default_size([size.x, size.y]);
    }

    if let Some(inner) = window.show(ctx, |ui| {
        let rect = ui.max_rect();
        ctx.data_mut(|d| d.insert_temp(rect_key, rect));

        let has_offscreen = ctx.data(|d| d.get_temp::<bool>(valid_key).unwrap_or(false));
        if has_offscreen {
            let size = rect.size();
            if size.x > 0.0 && size.y > 0.0 {
                let image_rect = egui::Rect::from_min_size(rect.min, size);
                ui.painter().image(tex_id, image_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
        }
    }) {
        let rect = inner.response.rect;
        ctx.data_mut(|d| d.insert_temp(pos_key, rect.min));
        ctx.data_mut(|d| d.insert_temp(size_key, rect.size()));
    }
}
