use crate::project::{ProjectEntry, ProjectType};

#[allow(deprecated)]
pub fn startup_screen(
    ctx: &egui::Context,
    recent: &[ProjectEntry],
    _screen: &mut crate::project::AppScreen,
    open_project: &std::cell::RefCell<Option<String>>,
    new_project: &std::cell::RefCell<Option<String>>,
    show_new_project_type: &std::cell::Cell<bool>,
    new_project_type: &std::cell::Cell<ProjectType>,
    show_settings: &std::cell::Cell<bool>,
) {
    // Black / orange theme matching the website
    let bg = egui::Color32::from_rgb(11, 12, 16);
    let panel_bg = egui::Color32::from_rgb(26, 29, 35);
    let surface = egui::Color32::from_rgb(38, 42, 50);
    let border = egui::Color32::from_rgb(42, 47, 56);
    let accent = egui::Color32::from_rgb(230, 126, 34);
    let _accent_hover = egui::Color32::from_rgb(211, 84, 0);
    let text_primary = egui::Color32::from_rgb(240, 246, 252);
    let text_secondary = egui::Color32::from_rgb(139, 148, 158);

    // Load logo texture once and cache it in ctx memory
    let logo_texture: Option<egui::TextureHandle> = ctx.data(|d| {
        d.get_temp(egui::Id::new("startup_logo"))
    });
    let logo_texture = logo_texture.or_else(|| {
        let logo_bytes = include_bytes!("../../../../docs/rustix-logo.png");
        image::load_from_memory(logo_bytes).ok().and_then(|img| {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let handle = ctx.load_texture(
                "startup_logo",
                egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
                egui::TextureOptions::LINEAR,
            );
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("startup_logo"), handle.clone()));
            Some(handle)
        })
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_size();
        let pw = (avail.x * 0.50).min(640.0).max(500.0);
        let ph = (avail.y * 0.60).min(480.0).max(380.0);

        let (_, rect) = ui.allocate_space(avail);
        ui.painter().rect_filled(rect, 0.0, bg);

        let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(pw, ph));
        let shadow_rect = panel_rect.translate(egui::vec2(0.0, 4.0));
        ui.painter().rect_filled(shadow_rect, 12.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 80));
        ui.painter().rect_filled(panel_rect, 12.0, panel_bg);
        ui.painter().rect_stroke(panel_rect, 12.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);

        let inner = panel_rect.shrink(28.0);
        #[allow(deprecated)]
        ui.allocate_ui_at_rect(inner, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);

                // Logo image
                if let Some(ref tex) = logo_texture {
                    ui.image((tex.id(), egui::vec2(64.0, 64.0)));
                    ui.add_space(4.0);
                }

                ui.label(egui::RichText::new("Rustix").size(26.0).color(text_primary).strong());
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Engine").size(13.0).color(accent).strong());
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Project Hub").size(11.0).color(text_secondary));
                ui.add_space(20.0);

                let sep_rect = ui.allocate_space(egui::vec2(60.0, 2.0)).1;
                ui.painter().rect_filled(sep_rect, 1.0, accent);
                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    let lw = inner.width() * 0.48;
                    let rw = inner.width() - lw - 20.0;

                    ui.vertical(|ui| {
                        ui.set_min_width(lw);
                        ui.label(egui::RichText::new("RECENT").size(10.0).color(text_secondary));
                        ui.add_space(6.0);

if recent.is_empty() {
                             egui::Frame::NONE
                                 .fill(surface)
                                 .stroke(egui::Stroke::new(1.0, border))
                                 .corner_radius(egui::CornerRadius::same(6))
                                 .show(ui, |ui| {
                                     ui.set_min_width(lw);
                                     ui.add_space(16.0);
                                     ui.label(egui::RichText::new("No recent projects")
                                         .size(12.0).color(text_secondary));
                                     ui.label(egui::RichText::new("Open a project to get started")
                                         .size(11.0).color(text_secondary));
                                     ui.add_space(16.0);
                                 });
                         } else {
                             egui::Frame::NONE
                                 .fill(surface)
                                 .stroke(egui::Stroke::new(1.0, border))
                                 .corner_radius(egui::CornerRadius::same(6))
                                 .show(ui, |ui| {
                                    ui.set_min_width(lw);
                                    let name_font = egui::FontId::proportional(13.0);
                                    let path_font = egui::FontId::proportional(10.0);
                                    for proj in recent.iter() {
                                        let item_h = 44.0;
                                        let id = ui.next_auto_id();
                                        let (_, rect) = ui.allocate_space(egui::vec2(lw, item_h));
                                        let resp = ui.interact(rect, id, egui::Sense::click());
                                        if resp.hovered() {
                                            ui.painter().rect_filled(rect.shrink(2.0), 4.0, egui::Color32::from_rgb(50, 55, 65));
                                        }
                                        ui.painter().text(
                                            egui::pos2(rect.min.x + 10.0, rect.min.y + 8.0),
                                            egui::Align2::LEFT_TOP,
                                            &proj.name,
                                            name_font.clone(),
                                            text_primary,
                                        );
                                        ui.painter().text(
                                            egui::pos2(rect.min.x + 10.0, rect.min.y + 26.0),
                                            egui::Align2::LEFT_TOP,
                                            format!("{}  ·  {}", proj.path, proj.last_opened),
                                            path_font.clone(),
                                            text_secondary,
                                        );
                                        if resp.clicked() {
                                            *open_project.borrow_mut() = Some(proj.path.clone());
                                        }
                                    }
                                });
                        }
                    });

                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.set_min_width(rw);
                        ui.label(egui::RichText::new("GET STARTED").size(10.0).color(text_secondary));
                        ui.add_space(12.0);

                        let btn_size = egui::vec2(rw, 44.0);

let new_btn = egui::Button::new(
                             egui::RichText::new("New Project").size(14.0).color(egui::Color32::WHITE)
                         )
                         .min_size(btn_size)
                         .fill(accent)
                         .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(new_btn).clicked() {
                            show_new_project_type.set(true);
                        }

                        ui.add_space(10.0);

let open_btn = egui::Button::new(
                             egui::RichText::new("Open Project\u{2026}").size(14.0).color(text_primary)
                         )
                         .min_size(btn_size)
                         .fill(surface)
                         .stroke(egui::Stroke::new(1.0, border))
                         .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(open_btn).clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Open Project")
                                .pick_folder()
                            {
                                *open_project.borrow_mut() = Some(path.to_string_lossy().to_string());
                            }
                        }

                        ui.add_space(16.0);

                        let settings_btn = egui::Button::new(
                            egui::RichText::new("Settings").size(14.0).color(text_primary)
                        )
                        .min_size(btn_size)
                        .fill(surface)
                        .stroke(egui::Stroke::new(1.0, border))
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(settings_btn).clicked() {
                            show_settings.set(!show_settings.get());
                        }

                        ui.add_space(8.0);

                        let website_btn = egui::Button::new(
                            egui::RichText::new("Website").size(14.0).color(text_primary)
                        )
                        .min_size(btn_size)
                        .fill(surface)
                        .stroke(egui::Stroke::new(1.0, border))
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(website_btn).clicked() {
                            let _ = open::that("https://tigerlero.github.io/Rustix/");
                        }

                        ui.add_space(24.0);
                        ui.label(egui::RichText::new("Create a new project or open an")
                            .size(11.0).color(text_secondary));
                        ui.label(egui::RichText::new("existing one to begin editing.")
                            .size(11.0).color(text_secondary));
                    });
                });
            });
        });
    });

    // Project type selection dialog
    if show_new_project_type.get() {
        egui::Window::new("New Project")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Choose project type:");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let btn3d = egui::Button::new(
                        egui::RichText::new("3D Project").size(16.0).color(egui::Color32::WHITE)
                    )
                    .min_size(egui::vec2(120.0, 60.0))
                    .fill(accent)
                    .corner_radius(egui::CornerRadius::same(6));
                    if ui.add(btn3d).clicked() {
                        new_project_type.set(ProjectType::Dim3);
                        show_new_project_type.set(false);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Create New Project")
                            .pick_folder()
                        {
                            new_project.borrow_mut().replace(path.to_string_lossy().to_string());
                        }
                    }
                    let btn2d = egui::Button::new(
                        egui::RichText::new("2D Project").size(16.0).color(egui::Color32::WHITE)
                    )
                    .min_size(egui::vec2(120.0, 60.0))
                    .fill(accent)
                    .corner_radius(egui::CornerRadius::same(6));
                    if ui.add(btn2d).clicked() {
                        new_project_type.set(ProjectType::Dim2);
                        show_new_project_type.set(false);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Create New Project")
                            .pick_folder()
                        {
                            new_project.borrow_mut().replace(path.to_string_lossy().to_string());
                        }
                    }
                });
                ui.add_space(8.0);
                let cancel_btn = egui::Button::new(
                    egui::RichText::new("Cancel").size(14.0).color(text_primary)
                )
                .fill(surface)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(egui::CornerRadius::same(6));
                if ui.add(cancel_btn).clicked() {
                    show_new_project_type.set(false);
                }
            });
    }
}
