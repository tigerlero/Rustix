//! Terrain Editor UI panel.
//!
//! Provides brush sculpting controls, splat painting channel selection,
//! and foliage scatter settings.

use egui::Color32;
use rustix_core::math::Vec3;
use rustix_core::ecs::EcsWorld;

use crate::scene::{Name, Transform, MeshComponent, Material};
use crate::terrain::{Terrain, TerrainEditor, BrushMode};
use crate::project::DockPosition;

/// Show the terrain editor as a dockable egui panel.
pub fn show_terrain_editor(
    ctx: &egui::Context,
    terrain_editor: &mut TerrainEditor,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
    dock: DockPosition,
) {
    if !terrain_editor.show {
        return;
    }

    let mut window = egui::Window::new("Terrain Editor");
    window = match dock {
        DockPosition::Left => window.anchor(egui::Align2::LEFT_TOP, [220.0, 40.0]),
        DockPosition::Right => window.anchor(egui::Align2::RIGHT_TOP, [-260.0, 40.0]),
        DockPosition::Bottom => window.anchor(egui::Align2::CENTER_BOTTOM, [0.0, -160.0]),
        DockPosition::Floating => window,
        DockPosition::Hidden => return,
    };

    window.show(ctx, |ui| {
        ui.heading("Brush");
        ui.add_space(4.0);

        // Brush mode buttons
        ui.horizontal(|ui| {
            let modes = [
                BrushMode::Raise,
                BrushMode::Lower,
                BrushMode::Smooth,
                BrushMode::Flatten,
            ];
            for mode in modes {
                let selected = terrain_editor.brush_mode == mode;
                let color = if selected {
                    Color32::from_rgb(80, 160, 220)
                } else {
                    Color32::from_rgb(60, 60, 60)
                };
                if ui.add(egui::Button::new(mode.label()).fill(color)).clicked() {
                    terrain_editor.brush_mode = mode;
                }
            }
        });

        ui.add_space(4.0);

        // Splat paint buttons
        ui.label("Splat Paint:");
        ui.horizontal(|ui| {
            let channels = [
                ("R", Color32::from_rgb(220, 80, 80), 0u8),
                ("G", Color32::from_rgb(80, 220, 80), 1u8),
                ("B", Color32::from_rgb(80, 80, 220), 2u8),
                ("A", Color32::from_rgb(200, 200, 200), 3u8),
            ];
            for (label, color, channel) in channels {
                let selected = matches!(terrain_editor.brush_mode, BrushMode::Splat(c) if c == channel);
                let btn_color = if selected { color } else { Color32::from_rgb(60, 60, 60) };
                if ui.add(egui::Button::new(label).fill(btn_color)).clicked() {
                    terrain_editor.brush_mode = BrushMode::Splat(channel);
                    terrain_editor.splat_channel = channel;
                }
            }
        });

        ui.separator();

        egui::Grid::new("brush_params").num_columns(2).show(ui, |ui| {
            ui.label("Radius:");
            ui.add(egui::Slider::new(&mut terrain_editor.brush_radius, 0.5..=50.0));
            ui.end_row();

            ui.label("Strength:");
            ui.add(egui::Slider::new(&mut terrain_editor.brush_strength, 0.01..=5.0));
            ui.end_row();
        });

        ui.separator();
        ui.heading("Foliage Scatter");
        ui.add_space(4.0);

        egui::Grid::new("foliage_params").num_columns(2).show(ui, |ui| {
            ui.label("Count:");
            ui.add(egui::Slider::new(&mut terrain_editor.foliage_density, 0..=500));
            ui.end_row();

            ui.label("Scale:");
            ui.add(egui::Slider::new(&mut terrain_editor.foliage_scale, 0.1..=3.0));
            ui.end_row();
        });

        if ui.button("Scatter Foliage").clicked() {
            scatter_foliage_entities(terrain_editor, world, selected_entities, dirty);
        }

        ui.separator();

        if ui.button("Clear Foliage").clicked() {
            let to_remove: Vec<hecs::Entity> = world
                .query_mut::<(hecs::Entity, &Name)>()
                .into_iter()
                .filter(|(_, n)| n.0.starts_with("Foliage"))
                .map(|(e, _)| e)
                .collect();
            for e in to_remove {
                let _ = world.despawn(e);
            }
            dirty.set(true);
        }
    });
}

fn scatter_foliage_entities(
    terrain_editor: &mut TerrainEditor,
    world: &mut EcsWorld,
    selected_entities: &std::cell::RefCell<Vec<hecs::Entity>>,
    dirty: &std::cell::Cell<bool>,
) {
    // Find terrain component (drop query borrow before any mutable ops)
    let terrain_opt: Option<Terrain> = {
        let mut q = world.query::<(hecs::Entity, &Name, &Terrain)>();
        q.iter()
            .find(|(_, n, _)| n.0 == "Terrain")
            .map(|(_, _, t)| t.clone())
    };

    let terrain = match terrain_opt {
        Some(t) => t,
        None => {
            // Create default terrain if none exists
            let t = Terrain::new(200.0, 65);
            let entity = world.spawn((
                Name("Terrain".to_string()),
                Transform {
                    position: Vec3::ZERO,
                    rotation: Vec3::ZERO,
                    scale: Vec3::ONE,
                },
                MeshComponent("Terrain".into()),
                Material {
                    base_color: Vec3::new(0.3, 0.5, 0.2),
                    alpha: 1.0,
                    roughness: 0.9,
                    metallic: 0.0,
                    ao: 1.0,
                    emissive: 0.0,
                },
                t.clone(),
            ));
            selected_entities.borrow_mut().push(entity);
            terrain_editor.regen_needed = true;
            t
        }
    };

    use crate::terrain::scatter_foliage;
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(42);

    let instances = scatter_foliage(&terrain, terrain_editor.foliage_density, terrain_editor.foliage_scale, seed);
    let mut spawned = Vec::new();
    for (i, (pos, scl)) in instances.iter().enumerate() {
        let name = format!("Foliage_{}", i);
        let e = world.spawn((
            Name(name),
            Transform {
                position: *pos,
                rotation: Vec3::new(0.0, 0.0, 0.0),
                scale: Vec3::splat(*scl),
            },
            MeshComponent("Cube".into()),
            Material {
                base_color: Vec3::new(0.1, 0.6, 0.1),
                alpha: 1.0,
                roughness: 0.8,
                metallic: 0.0,
                ao: 1.0,
                emissive: 0.0,
            },
        ));
        spawned.push(e);
    }

    if !spawned.is_empty() {
        tracing::info!("scattered {} foliage instances", spawned.len());
        dirty.set(true);
    }
}
