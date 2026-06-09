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
    let bg = egui::Color32::from_rgb(22, 22, 28);
    let panel_bg = egui::Color32::from_rgb(30, 30, 38);
    let surface = egui::Color32::from_rgb(38, 38, 46);
    let border = egui::Color32::from_rgb(50, 50, 60);
    let accent = egui::Color32::from_rgb(72, 120, 240);
    let text_primary = egui::Color32::from_rgb(220, 220, 228);
    let text_secondary = egui::Color32::from_rgb(140, 140, 155);

    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_size();
        let pw = (avail.x * 0.50).min(640.0).max(500.0);
        let ph = (avail.y * 0.55).min(440.0).max(340.0);

        let (_, rect) = ui.allocate_space(avail);
        ui.painter().rect_filled(rect, 0.0, bg);

        let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(pw, ph));
        let shadow_rect = panel_rect.translate(egui::vec2(0.0, 2.0));
        ui.painter().rect_filled(shadow_rect, 10.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 60));
        ui.painter().rect_filled(panel_rect, 10.0, panel_bg);
        ui.painter().rect_stroke(panel_rect, 10.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);

        let inner = panel_rect.shrink(28.0);
        #[allow(deprecated)]
        ui.allocate_ui_at_rect(inner, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);

                ui.label(egui::RichText::new("Rustix").size(28.0).color(text_primary).strong());
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Engine").size(14.0).color(accent));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Project Hub").size(12.0).color(text_secondary));
                ui.add_space(24.0);

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
                                            ui.painter().rect_filled(rect.shrink(2.0), 4.0, egui::Color32::from_rgb(44, 44, 54));
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
                    if ui.add_sized(egui::vec2(120.0, 60.0), egui::Button::new(
                        egui::RichText::new("3D Project").size(16.0).color(egui::Color32::WHITE)
                    )).clicked() {
                        new_project_type.set(ProjectType::Dim3);
                        show_new_project_type.set(false);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Create New Project")
                            .pick_folder()
                        {
                            new_project.borrow_mut().replace(path.to_string_lossy().to_string());
                        }
                    }
                    if ui.add_sized(egui::vec2(120.0, 60.0), egui::Button::new(
                        egui::RichText::new("2D Project").size(16.0).color(egui::Color32::WHITE)
                    )).clicked() {
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
                if ui.button("Cancel").clicked() {
                    show_new_project_type.set(false);
                }
            });
    }
}
