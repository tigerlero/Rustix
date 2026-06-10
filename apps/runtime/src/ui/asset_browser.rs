use std::path::{Path, PathBuf};
use crate::project::DockPosition;
use rustix_core::ecs::EcsWorld;

/// State for the asset browser panel, persisted across frames via egui temp data.
#[derive(Clone)]
struct AssetBrowserState {
    current_path: PathBuf,
    search_query: String,
    selected_file: Option<PathBuf>,
    show_folders: bool,
    view_mode: ViewMode,
}

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    List,
    Grid,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        Self {
            current_path: PathBuf::new(),
            search_query: String::new(),
            selected_file: None,
            show_folders: true,
            view_mode: ViewMode::List,
        }
    }
}

/// Show the asset browser panel.
pub fn show_asset_browser(
    ctx: &egui::Context,
    project_dir: &Option<String>,
    dock: DockPosition,
    world: &mut EcsWorld,
) {
    let gen = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0));
    let panel_id = egui::Id::new(("asset_browser", gen));
    let width_key = egui::Id::new("asset_browser_width");
    let desired_width = ctx.data(|d| d.get_temp::<f32>(width_key)).unwrap_or(260.0);

    let result = super::dock::show_docked(ctx, "Asset Browser", panel_id, dock, desired_width, |ui| {
        let state_id = egui::Id::new("asset_browser_state");
        let mut state: AssetBrowserState = ctx.data(|d| {
            d.get_temp::<AssetBrowserState>(state_id).unwrap_or_default()
        });

        // Header toolbar
        ui.horizontal(|ui| {
            ui.heading("Asset Browser");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let view_text = match state.view_mode {
                    ViewMode::List => "List",
                    ViewMode::Grid => "Grid",
                };
                if ui.selectable_label(false, view_text).clicked() {
                    state.view_mode = match state.view_mode {
                        ViewMode::List => ViewMode::Grid,
                        ViewMode::Grid => ViewMode::List,
                    };
                }
                if ui.selectable_label(state.show_folders, "Folders").clicked() {
                    state.show_folders = !state.show_folders;
                }
                if ui.button("Refresh").clicked() {
                    // Force refresh on next frame by clearing cached entries
                }
                if ui.button("Up").clicked() {
                    let _ = state.current_path.pop();
                }
            });
        });
        ui.separator();

        // Search bar
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut state.search_query);
            if ui.button("Clear").clicked() {
                state.search_query.clear();
            }
        });
        ui.separator();

        // Breadcrumb
        if let Some(ref dir) = project_dir {
            let root = Path::new(dir);
            let current = state.current_path.clone();
            let rel = current.strip_prefix(root).unwrap_or(&current);
            let breadcrumb_parts: Vec<_> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(current.as_path() == Path::new(""), "Project").clicked() {
                    state.current_path = PathBuf::new();
                }
                let mut accum = PathBuf::new();
                for part in &breadcrumb_parts {
                    accum.push(part);
                    ui.label(" / ");
                    if ui.selectable_label(false, part.as_str()).clicked() {
                        state.current_path = root.join(&accum);
                    }
                }
            });
            ui.separator();

            let display_path = if state.current_path.as_os_str().is_empty() {
                root.to_path_buf()
            } else {
                state.current_path.clone()
            };

            let entries = read_sorted_entries(&display_path);

            // Main content: optional folder tree + file list
            let available_height = ui.available_height();
            let tree_width = if state.show_folders { 160.0 } else { 0.0 };

            egui::SidePanel::left("asset_browser_tree")
                .resizable(true)
                .default_width(tree_width)
                .width_range(100.0..=300.0)
                .show_inside(ui, |ui| {
                    ui.label(egui::RichText::new("Folders").strong());
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(available_height).show(ui, |ui| {
                        show_folder_tree(ui, root, root, &mut state.current_path);
                    });
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                if state.view_mode == ViewMode::Grid {
                    show_file_grid(ui, &entries, &state.search_query, &mut state.selected_file, world);
                } else {
                    show_file_list(ui, &entries, &state.search_query, &mut state.selected_file, world);
                }
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Open a project to browse assets.");
            });
        }

        ctx.data_mut(|d| d.insert_temp(state_id, state));
    });

    if let Some(inner) = result {
        let actual_width = inner.response.rect.width();
        ctx.data_mut(|d| d.insert_temp(width_key, actual_width));
    }
}

fn read_sorted_entries(path: &Path) -> Vec<std::fs::DirEntry> {
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir) // dirs first
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });
    entries
}

fn show_folder_tree(
    ui: &mut egui::Ui,
    root: &Path,
    current: &Path,
    selected_path: &mut PathBuf,
) {
    if let Ok(entries) = std::fs::read_dir(current) {
        let mut dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();
        dirs.sort_by_key(|e| e.file_name());

        for entry in dirs {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden folders
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let is_selected = *selected_path == path;

            ui.horizontal(|ui| {
                let folder_icon = egui::RichText::new("📁").size(14.0);
                let label = egui::RichText::new(&name).size(12.0);
                let resp = ui.selectable_label(is_selected, egui::RichText::new(format!("📁 {}", name)).size(12.0));
                if resp.clicked() {
                    *selected_path = path.clone();
                }
            });

            // Recursively show subfolders if this folder is along the selected path
            if selected_path.starts_with(&path) && path != *selected_path {
                ui.indent("subfolder", |ui| {
                    show_folder_tree(ui, root, &path, selected_path);
                });
            }
        }
    }
}

fn show_file_list(
    ui: &mut egui::Ui,
    entries: &[std::fs::DirEntry],
    search: &str,
    selected_file: &mut Option<PathBuf>,
    _world: &mut EcsWorld,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("asset_browser_grid")
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Type").strong().size(11.0));
                ui.label(egui::RichText::new("Name").strong().size(11.0));
                ui.label(egui::RichText::new("Size").strong().size(11.0));
                ui.end_row();

                for entry in entries {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !search.is_empty() && !name.to_lowercase().contains(&search.to_lowercase()) {
                        continue;
                    }

                    let ft = entry.file_type().ok();
                    let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
                    let ext = Path::new(&name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    let (icon, color) = file_icon_and_color(is_dir, &ext);
                    let size_str = if is_dir {
                        "--".to_string()
                    } else {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        format_file_size(size)
                    };

                    let full_path = entry.path();
                    let is_selected = selected_file.as_ref() == Some(&full_path);

                    ui.label(egui::RichText::new(icon).color(color).size(12.0));

                    let resp = ui.selectable_label(is_selected, egui::RichText::new(&name).size(12.0));
                    if resp.clicked() {
                        if is_dir {
                            // Navigation handled by parent if needed; here we just select
                        } else {
                            *selected_file = Some(full_path.clone());
                        }
                    }

                    ui.label(egui::RichText::new(size_str).weak().size(11.0));
                    ui.end_row();
                }
            });
    });
}

fn show_file_grid(
    ui: &mut egui::Ui,
    entries: &[std::fs::DirEntry],
    search: &str,
    selected_file: &mut Option<PathBuf>,
    _world: &mut EcsWorld,
) {
    let item_width = 80.0;
    let item_height = 90.0;
    let spacing = 8.0;
    let available_width = ui.available_width();
    let cols = ((available_width + spacing) / (item_width + spacing)).max(1.0) as usize;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for row_entries in entries.chunks(cols) {
            ui.horizontal(|ui| {
                for entry in row_entries {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !search.is_empty() && !name.to_lowercase().contains(&search.to_lowercase()) {
                        continue;
                    }

                    let ft = entry.file_type().ok();
                    let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
                    let ext = Path::new(&name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    let (icon, color) = file_icon_and_color(is_dir, &ext);
                    let full_path = entry.path();
                    let is_selected = selected_file.as_ref() == Some(&full_path);

                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(item_width, item_height),
                        egui::Sense::click_and_drag(),
                    );

                    let visuals = ui.style().interact(&resp);
                    let bg = if is_selected {
                        ui.visuals().selection.bg_fill
                    } else if resp.hovered() {
                        visuals.bg_fill
                    } else {
                        egui::Color32::TRANSPARENT
                    };

                    ui.painter().rect_filled(rect, egui::CornerRadius::same(4), bg);
                    if is_selected || resp.hovered() {
                        ui.painter().rect_stroke(rect, egui::CornerRadius::same(4), egui::Stroke::new(1.0, visuals.fg_stroke.color), egui::StrokeKind::Inside);
                    }

                    // Draw icon centered
                    let icon_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2((item_width - 32.0) * 0.5, 12.0),
                        egui::vec2(32.0, 32.0),
                    );
                    ui.painter().text(
                        icon_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon,
                        egui::FontId::proportional(28.0),
                        color,
                    );

                    // Draw filename (truncated)
                    let text_rect = egui::Rect::from_min_size(
                        rect.min + egui::vec2(4.0, 52.0),
                        egui::vec2(item_width - 8.0, 34.0),
                    );
                    let display_name = if name.len() > 12 {
                        format!("{}...", &name[..9])
                    } else {
                        name.clone()
                    };
                    ui.painter().text(
                        text_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &display_name,
                        egui::FontId::proportional(10.0),
                        ui.visuals().text_color(),
                    );

                    if resp.clicked() {
                        if !is_dir {
                            *selected_file = Some(full_path.clone());
                        }
                    }
                }
            });
        }
    });
}

fn file_icon_and_color(is_dir: bool, ext: &str) -> (&'static str, egui::Color32) {
    if is_dir {
        return ("📁", egui::Color32::from_rgb(240, 200, 80));
    }
    match ext {
        "glb" | "gltf" | "obj" | "fbx" => ("🧊", egui::Color32::from_rgb(130, 200, 250)),
        "png" | "jpg" | "jpeg" | "hdr" | "exr" | "tga" | "bmp" | "webp" => {
            ("🖼", egui::Color32::from_rgb(100, 220, 140))
        }
        "wav" | "mp3" | "ogg" | "flac" | "aac" => {
            ("🔊", egui::Color32::from_rgb(250, 150, 200))
        }
        "wgsl" | "glsl" | "vert" | "frag" | "spv" | "hlsl" => {
            ("🎨", egui::Color32::from_rgb(200, 180, 100))
        }
        "rs" | "lua" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" => {
            ("📜", egui::Color32::from_rgb(180, 200, 220))
        }
        "rustixproj" => ("⚙", egui::Color32::from_rgb(120, 240, 200)),
        "scene" | "json" | "yaml" | "yml" | "toml" | "xml" => {
            ("📋", egui::Color32::from_rgb(200, 200, 180))
        }
        "md" | "txt" | "log" => ("📝", egui::Color32::from_rgb(180, 180, 180)),
        "ttf" | "otf" | "fon" => ("🔤", egui::Color32::from_rgb(220, 180, 140)),
        "anim" | "skeleton" => ("🦴", egui::Color32::from_rgb(220, 160, 120)),
        _ => ("📄", egui::Color32::from_rgb(160, 160, 170)),
    }
}

fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1} {}", size, UNITS[unit_idx])
}
