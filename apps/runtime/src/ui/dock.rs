use crate::project::DockPosition;

/// Render content in a dockable container based on `dock` position.
/// Returns `Some(InnerResponse)` for visible positions, `None` for `Hidden`.
pub fn show_docked<R>(
    ctx: &egui::Context,
    title: &str,
    id: egui::Id,
    dock: DockPosition,
    default_size: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<egui::InnerResponse<R>> {
    match dock {
        DockPosition::Left => Some(
            egui::Panel::left(id)
                .resizable(true)
                .default_size(default_size)
                .show(ctx, content),
        ),
        DockPosition::Right => Some(
            egui::Panel::right(id)
                .resizable(true)
                .default_size(default_size)
                .show(ctx, content),
        ),
        DockPosition::Bottom => Some(
            egui::Panel::bottom(id)
                .resizable(true)
                .default_size(default_size)
                .show(ctx, content),
        ),
        DockPosition::Floating => egui::Window::new(title)
            .id(id)
            .default_size([default_size.max(200.0), 300.0])
            .show(ctx, content)
            .and_then(|ir| ir.inner.map(|inner| egui::InnerResponse { inner, response: ir.response })),
        DockPosition::Hidden => None,
    }
}
