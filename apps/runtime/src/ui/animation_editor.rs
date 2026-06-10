//! Animation Editor UI for skeletal animation.
//!
//! Allows loading entities with Skeleton components, editing bone transforms,
//! creating animation clips with keyframes per bone, and previewing playback.

use std::collections::HashMap;
use rustix_core::math::{Vec3, Quat};
use rustix_animation::{Skeleton, Bone, BoneAnimationTracks, SkeletalAnimationClip, Keyframe, QuatKeyframe};

/// State of the animation editor.
#[derive(Debug, Clone)]
pub struct AnimationEditor {
    pub show: bool,
    pub active_entity: Option<hecs::Entity>,
    pub selected_bone: Option<usize>,
    pub clips: HashMap<String, SkeletalAnimationClip>,
    pub current_clip: String,
    pub current_time: f32,
    pub playing: bool,
    pub looping: bool,
    pub playback_speed: f32,
    pub new_clip_name: String,
    pub bone_pos_backup: Option<Vec3>,
    pub bone_rot_backup: Option<Vec3>,
    pub bone_scl_backup: Option<Vec3>,
}

impl Default for AnimationEditor {
    fn default() -> Self {
        Self {
            show: false,
            active_entity: None,
            selected_bone: None,
            clips: HashMap::new(),
            current_clip: String::new(),
            current_time: 0.0,
            playing: false,
            looping: true,
            playback_speed: 1.0,
            new_clip_name: String::new(),
            bone_pos_backup: None,
            bone_rot_backup: None,
            bone_scl_backup: None,
        }
    }
}

impl AnimationEditor {
    pub fn current_clip_mut(&mut self) -> Option<&mut SkeletalAnimationClip> {
        self.clips.get_mut(&self.current_clip)
    }

    pub fn current_clip_ref(&self) -> Option<&SkeletalAnimationClip> {
        self.clips.get(&self.current_clip)
    }

    pub fn add_clip(&mut self, name: impl Into<String>, duration: f32) {
        let name = name.into();
        self.clips.insert(name.clone(), SkeletalAnimationClip::new(&name, duration));
        self.current_clip = name;
    }

    pub fn delete_clip(&mut self, name: &str) {
        self.clips.remove(name);
        if self.current_clip == name {
            self.current_clip = self.clips.keys().next().cloned().unwrap_or_default();
        }
    }

    pub fn set_keyframe(&mut self, bone_name: &str, time: f32, pos: Vec3, rot_euler: Vec3, scl: Vec3) {
        if let Some(clip) = self.clips.get_mut(&self.current_clip) {
            let tracks = clip.bone_tracks_mut(bone_name);
            // Position keyframe
            let pos_idx = tracks.position_track.keyframes.binary_search_by(|k| k.time.partial_cmp(&time).unwrap());
            match pos_idx {
                Ok(i) => tracks.position_track.keyframes[i].value = pos,
                Err(i) => tracks.position_track.keyframes.insert(i, Keyframe { time, value: pos }),
            }
            // Rotation keyframe
            let quat = Quat::from_euler(rustix_core::math::EulerRot::XYZ, rot_euler.x, rot_euler.y, rot_euler.z);
            let rot_idx = tracks.rotation_track.keyframes.binary_search_by(|k| k.time.partial_cmp(&time).unwrap());
            match rot_idx {
                Ok(i) => tracks.rotation_track.keyframes[i].value = quat,
                Err(i) => tracks.rotation_track.keyframes.insert(i, QuatKeyframe { time, value: quat }),
            }
            // Scale keyframe
            let scl_idx = tracks.scale_track.keyframes.binary_search_by(|k| k.time.partial_cmp(&time).unwrap());
            match scl_idx {
                Ok(i) => tracks.scale_track.keyframes[i].value = scl,
                Err(i) => tracks.scale_track.keyframes.insert(i, Keyframe { time, value: scl }),
            }
        }
    }

    pub fn delete_keyframe_at_time(&mut self, bone_name: &str, time: f32) {
        if let Some(clip) = self.clips.get_mut(&self.current_clip) {
            if let Some(tracks) = clip.bone_tracks.get_mut(bone_name) {
                tracks.position_track.keyframes.retain(|k| (k.time - time).abs() > 0.001);
                tracks.rotation_track.keyframes.retain(|k| (k.time - time).abs() > 0.001);
                tracks.scale_track.keyframes.retain(|k| (k.time - time).abs() > 0.001);
            }
        }
    }

    pub fn sample_current_clip(&self, time: f32) -> HashMap<String, (Option<Vec3>, Option<Quat>, Option<Vec3>)> {
        self.current_clip_ref()
            .map(|c| c.sample_pose(time))
            .unwrap_or_default()
    }

    pub fn update_playback(&mut self, dt: f32) {
        if !self.playing { return; }
        let duration = self.current_clip_ref().map(|c| c.duration).unwrap_or(1.0);
        self.current_time += dt * self.playback_speed;
        if self.current_time > duration {
            if self.looping {
                self.current_time %= duration;
            } else {
                self.current_time = duration;
                self.playing = false;
            }
        }
    }
}

pub fn show_animation_editor(
    ctx: &egui::Context,
    editor: &mut AnimationEditor,
    world: &mut hecs::World,
    dirty: &std::cell::Cell<bool>,
) {
    if !editor.show { return; }

    let mut open = editor.show;
    egui::Window::new("Animation Editor")
        .open(&mut open)
        .default_size([800.0, 600.0])
        .show(ctx, |ui| {
            // Try to get skeleton from active entity, or from first selected entity
            let skeleton_entity = editor.active_entity.or_else(|| {
                world.query::<(hecs::Entity, &crate::scene::Name)>()
                    .iter()
                    .next()
                    .map(|(e, _)| e)
            });

            let mut skeleton_opt: Option<Skeleton> = None;
            if let Some(entity) = skeleton_entity {
                if let Ok(skel) = world.get::<&Skeleton>(entity) {
                    skeleton_opt = Some((*skel).clone());
                    editor.active_entity = Some(entity);
                }
            }

            let Some(mut skeleton) = skeleton_opt else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select an entity with a Skeleton component to edit animations.");
                });
                return;
            };

            ui.horizontal(|ui| {
                ui.label("Entity:");
                if let Some(e) = editor.active_entity {
                    if let Ok(name) = world.get::<&crate::scene::Name>(e) {
                        ui.label(egui::RichText::new(&name.0).strong());
                    }
                }
            });
            ui.separator();

            // -- Top: Clip controls --
            ui.horizontal(|ui| {
                ui.label("Clip:");
                let clip_names: Vec<String> = editor.clips.keys().cloned().collect();
                let current = editor.current_clip.clone();
                egui::ComboBox::from_id_source("anim_clip_select")
                    .selected_text(if current.is_empty() { "(none)" } else { &current })
                    .show_ui(ui, |ui| {
                        for name in &clip_names {
                            if ui.selectable_label(current == *name, name).clicked() {
                                editor.current_clip = name.clone();
                            }
                        }
                    });

                ui.add(egui::TextEdit::singleline(&mut editor.new_clip_name).hint_text("New clip name"));
                if ui.button("+ Add").clicked() && !editor.new_clip_name.is_empty() {
                    let name = editor.new_clip_name.clone();
                    editor.add_clip(&name, 2.0);
                    editor.new_clip_name.clear();
                }
                if ui.button("- Del").clicked() && !editor.current_clip.is_empty() {
                    editor.delete_clip(&editor.current_clip.clone());
                }
                if let Some(clip) = editor.current_clip_mut() {
                    ui.add(egui::DragValue::new(&mut clip.duration).prefix("Duration: ").speed(0.1).range(0.1..=60.0));
                }
            });

            ui.separator();

            // -- Playback controls --
            ui.horizontal(|ui| {
                let play_text = if editor.playing { "⏸ Pause" } else { "▶ Play" };
                if ui.button(play_text).clicked() {
                    editor.playing = !editor.playing;
                }
                if ui.button("⏹ Stop").clicked() {
                    editor.playing = false;
                    editor.current_time = 0.0;
                }
                ui.checkbox(&mut editor.looping, "Loop");
                ui.add(egui::DragValue::new(&mut editor.playback_speed).prefix("Speed: ").speed(0.1).range(0.1..=5.0));
                if let Some(clip) = editor.current_clip_ref() {
                    let mut t = editor.current_time;
                    ui.add(egui::Slider::new(&mut t, 0.0..=clip.duration).text("Time"));
                    editor.current_time = t;
                }
            });
            ui.separator();

            // If playing, sample clip and write back to skeleton
            if editor.playing {
                let pose = editor.sample_current_clip(editor.current_time);
                for (bone_name, (pos, rot, scl)) in pose {
                    if let Some(idx) = skeleton.find_bone_index(&bone_name) {
                        if let Some(p) = pos {
                            skeleton.bones[idx].local_pos = p;
                        }
                        if let Some(r) = rot {
                            let (rx, ry, rz) = r.to_euler(rustix_core::math::EulerRot::XYZ);
                            skeleton.bones[idx].local_rot = Vec3::new(rx, ry, rz);
                        }
                        if let Some(s) = scl {
                            skeleton.bones[idx].local_scl = s;
                        }
                    }
                }
            }

            // -- Three-column layout: Bone tree | Transform editor | Timeline preview --
            egui::SidePanel::left("anim_bone_tree")
                .resizable(true)
                .default_width(200.0)
                .show_inside(ui, |ui| {
                    ui.heading("Bones");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, bone) in skeleton.bones.iter().enumerate() {
                            let name = bone.name_str();
                            let indent = if bone.parent == u16::MAX { 0.0 } else { 20.0 };
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                let is_selected = editor.selected_bone == Some(i);
                                let text = egui::RichText::new(name).color(if is_selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.visuals().text_color()
                                });
                                if ui.selectable_label(is_selected, text).clicked() {
                                    editor.selected_bone = Some(i);
                                    editor.bone_pos_backup = Some(bone.local_pos);
                                    editor.bone_rot_backup = Some(bone.local_rot);
                                    editor.bone_scl_backup = Some(bone.local_scl);
                                }
                            });
                        }
                    });
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                if let Some(bone_idx) = editor.selected_bone {
                    let bone = &mut skeleton.bones[bone_idx];
                    ui.heading(format!("Edit: {}", bone.name_str()));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Position");
                        ui.add(egui::DragValue::new(&mut bone.local_pos.x).prefix("X: ").speed(0.01));
                        ui.add(egui::DragValue::new(&mut bone.local_pos.y).prefix("Y: ").speed(0.01));
                        ui.add(egui::DragValue::new(&mut bone.local_pos.z).prefix("Z: ").speed(0.01));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut bone.local_rot.x).prefix("X: ").speed(0.01).suffix(" rad"));
                        ui.add(egui::DragValue::new(&mut bone.local_rot.y).prefix("Y: ").speed(0.01).suffix(" rad"));
                        ui.add(egui::DragValue::new(&mut bone.local_rot.z).prefix("Z: ").speed(0.01).suffix(" rad"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        ui.add(egui::DragValue::new(&mut bone.local_scl.x).prefix("X: ").speed(0.01));
                        ui.add(egui::DragValue::new(&mut bone.local_scl.y).prefix("Y: ").speed(0.01));
                        ui.add(egui::DragValue::new(&mut bone.local_scl.z).prefix("Z: ").speed(0.01));
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Set Keyframe").clicked() {
                            editor.set_keyframe(
                                bone.name_str(),
                                editor.current_time,
                                bone.local_pos,
                                bone.local_rot,
                                bone.local_scl,
                            );
                        }
                        if ui.button("Del Keyframe").clicked() {
                            editor.delete_keyframe_at_time(bone.name_str(), editor.current_time);
                        }
                        if ui.button("Reset Bone").clicked() {
                            if let Some(p) = editor.bone_pos_backup {
                                bone.local_pos = p;
                            }
                            if let Some(r) = editor.bone_rot_backup {
                                bone.local_rot = r;
                            }
                            if let Some(s) = editor.bone_scl_backup {
                                bone.local_scl = s;
                            }
                        }
                    });

                    // Show keyframe list for this bone
                    if let Some(clip) = editor.current_clip_ref() {
                        if let Some(tracks) = clip.bone_tracks.get(bone.name_str()) {
                            ui.separator();
                            ui.label("Keyframes:");
                            for kf in &tracks.position_track.keyframes {
                                ui.label(format!("  t={:.2}s pos=({:.2},{:.2},{:.2})", kf.time, kf.value.x, kf.value.y, kf.value.z));
                            }
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Select a bone from the left panel to edit its transform.");
                    });
                }
            });

            // -- Bottom: Timeline --
            egui::TopBottomPanel::bottom("anim_timeline")
                .resizable(false)
                .default_height(80.0)
                .show_inside(ui, |ui| {
                    ui.heading("Timeline");
                    let duration = editor.current_clip_ref().map(|c| c.duration).unwrap_or(1.0);
                    let mut t = editor.current_time;
                    let resp = ui.add(egui::Slider::new(&mut t, 0.0..=duration).show_value(true));
                    if resp.changed() {
                        editor.playing = false;
                        editor.current_time = t;
                    }

                    // Draw keyframe markers
                    if let Some(bone_idx) = editor.selected_bone {
                        let bone_name = skeleton.bones[bone_idx].name_str();
                        let track_keys = editor.current_clip_ref()
                            .and_then(|clip| clip.bone_tracks.get(bone_name))
                            .map(|tracks| tracks.position_track.keyframes.clone())
                            .unwrap_or_default();
                        let width = resp.rect.width();
                        let left = resp.rect.left();
                        if width > 0.0 && duration > 0.0 {
                            for kf in &track_keys {
                                let x = left + (kf.time / duration) * width;
                                ui.painter().circle_filled(
                                    egui::pos2(x, resp.rect.center().y),
                                    4.0,
                                    egui::Color32::from_rgb(230, 126, 34),
                                );
                            }
                        }
                    }
                });

            // Write modified skeleton back to ECS
            if let Some(entity) = editor.active_entity {
                if let Ok(mut skel_mut) = world.get::<&mut Skeleton>(entity) {
                    *skel_mut = skeleton.clone();
                    dirty.set(true);
                }
            }
        });

    editor.show = open;
}
