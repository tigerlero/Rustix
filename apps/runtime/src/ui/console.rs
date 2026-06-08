use std::path::Path;

use rustix_audio::{AudioEngine, SoundInstance};
use crate::waveform;
use crate::project::DockPosition;

pub fn show_console(
    ctx: &egui::Context,
    project_dir: &Option<String>,
    audio_engine: &mut Option<AudioEngine>,
    audio_instance: &mut Option<SoundInstance>,
    waveform_viewer: &mut waveform::WaveformViewer,
    dock: DockPosition,
) {
    let gen = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0));
    let panel_id = egui::Id::new(("console_tabs", gen));
    let height_key = egui::Id::new("console_height");
    let desired_height = ctx.data(|d| d.get_temp::<f32>(height_key)).unwrap_or(160.0);
    let result = super::dock::show_docked(ctx, "Console", panel_id, dock, desired_height, |ui| {
        let tab_id = egui::Id::new("bottom_tab");
        let mut active_tab = ctx.data(|d| d.get_temp::<usize>(tab_id).unwrap_or(0));

        ui.horizontal(|ui| {
            if ui.selectable_label(active_tab == 0, "Console").clicked() { active_tab = 0; }
            if ui.selectable_label(active_tab == 1, "Asset Browser").clicked() { active_tab = 1; }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if active_tab == 0 && ui.button("Clear").clicked() {
                    rustix_core::log_capture::clear_logs();
                }
                if active_tab == 1 && ui.button("Refresh").clicked() {
                    // Refresh the asset list on next frame
                }
            });
        });
        ctx.data_mut(|d| d.insert_temp(tab_id, active_tab));
        ui.separator();

        if active_tab == 0 {
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                let logs = rustix_core::log_capture::get_logs();
                for entry in logs {
                    let color = match entry.level {
                        tracing::Level::ERROR => egui::Color32::from_rgb(240, 80, 80),
                        tracing::Level::WARN => egui::Color32::from_rgb(240, 200, 50),
                        tracing::Level::INFO => egui::Color32::from_rgb(180, 200, 220),
                        tracing::Level::DEBUG => egui::Color32::from_rgb(140, 140, 160),
                        tracing::Level::TRACE => egui::Color32::from_rgb(100, 100, 120),
                    };
                    ui.label(egui::RichText::new(format!("{}", entry)).color(color));
                }
            });
        } else {
            if let Some(ref dir) = project_dir {
                let path = Path::new(dir);
                let mut entries: Vec<_> = std::fs::read_dir(path)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .collect();
                entries.sort_by_key(|e| !e.file_type().map(|t| t.is_dir()).unwrap_or(false));

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let sel_id = egui::Id::new("asset_browser_selected_audio");
                    let mut selected_audio: Option<String> = ctx.data(|d| d.get_temp::<String>(sel_id));
                    for entry in &entries {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let full_path = path.join(&name);
                        let ft = entry.file_type().ok();
                        let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
                        let ext = Path::new(&name).extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

                        let is_audio = matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "flac");
                        let (icon, color) = if is_dir {
                            ("[DIR]", egui::Color32::from_rgb(240, 200, 80))
                        } else {
                            match ext.as_str() {
                                "glb" | "gltf" | "obj" | "fbx" => ("[MODEL]", egui::Color32::from_rgb(130, 200, 250)),
                                "png" | "jpg" | "jpeg" | "hdr" | "exr" => ("[TEX]", egui::Color32::from_rgb(100, 220, 140)),
                                "wav" | "mp3" | "ogg" | "flac" => ("[AUDIO]", egui::Color32::from_rgb(250, 150, 200)),
                                "wgsl" | "glsl" | "vert" | "frag" | "spv" => ("[SHADER]", egui::Color32::from_rgb(200, 180, 100)),
                                "rs" | "lua" | "py" => ("[CODE]", egui::Color32::from_rgb(180, 200, 220)),
                                "rustixproj" => ("[PROJ]", egui::Color32::from_rgb(120, 240, 200)),
                                _ => ("[FILE]", egui::Color32::from_rgb(160, 160, 170)),
                            }
                        };
                        let resp = ui.selectable_label(false, egui::RichText::new(format!("{icon}  {name}")).color(color).size(12.0));
                        if resp.clicked() {
                            if is_audio {
                                selected_audio = Some(full_path.to_string_lossy().to_string());
                            } else {
                                selected_audio = None;
                            }
                        }
                    }
                    ctx.data_mut(|d| {
                        if let Some(ref path) = selected_audio {
                            d.insert_temp(sel_id, path.clone());
                        } else {
                            d.remove_temp::<String>(sel_id);
                        }
                    });

                    let mut close_preview = false;
                    if let Some(ref audio_path) = selected_audio {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Audio Preview").strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("\u{2715} Close").clicked() {
                                    close_preview = true;
                                }
                            });
                        });
                        let path_buf = std::path::PathBuf::from(audio_path);
                        let file_name = path_buf.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        ui.label(format!("File: {file_name}"));

                        let probe_id = egui::Id::new(("audio_probe", audio_path.as_str()));
                        let mut probe_data: Option<(Vec<f32>, u32, u16)> = ctx.data(|d| d.get_temp::<(Vec<f32>, u32, u16)>(probe_id));
                        if probe_data.is_none() {
                            if let Ok((samples, sr, ch)) = rustix_audio::decoder::decode_audio(&path_buf) {
                                let dur = samples.len() as f32 / sr as f32 / ch.max(1) as f32;
                                ui.label(egui::RichText::new(format!("{sr}Hz  {ch}ch  {dur:.2}s")).weak());
                                let len = samples.len().min(4096);
                                probe_data = Some((samples[..len].to_vec(), sr, ch));
                                ctx.data_mut(|d| d.insert_temp(probe_id, probe_data.clone()));
                            }
                        } else if let Some((ref samples, sr, ch)) = probe_data {
                            let dur = samples.len() as f32 / sr as f32 / ch.max(1) as f32;
                            ui.label(egui::RichText::new(format!("{sr}Hz  {ch}ch  {dur:.2}s")).weak());
                        }

                        if let Some((ref samples, sr, ch)) = probe_data {
                            let playhead = audio_instance.as_ref().and_then(|inst| {
                                let dur = inst.decoded_samples().len() as f32 / inst.sample_rate() as f32 / inst.channels().max(1) as f32;
                                if inst.is_playing() { Some(dur * 0.5) } else { None }
                            });
                            waveform_viewer.show(ui, samples, ch, sr, playhead);
                        }

                        if let Some(ref mut engine) = audio_engine {
                            ui.horizontal(|ui| {
                                if ui.button("\u{25b6} Play").clicked() {
                                    if let Ok(inst) = engine.play_sound_file(&path_buf) {
                                        *audio_instance = Some(inst);
                                    }
                                }
                                if audio_instance.is_some() {
                                    if ui.button("\u{23f9} Stop").clicked() {
                                        *audio_instance = None;
                                    }
                                }
                            });
                        } else {
                            ui.label(egui::RichText::new("Audio engine not available").weak());
                        }
                    }
                    if close_preview {
                        ctx.data_mut(|d| d.remove_temp::<String>(sel_id));
                    }
                });
            } else {
                ui.label("Open a project to browse its assets.");
            }
        }
    });
    if let Some(inner) = result {
        let actual_height = inner.response.rect.height();
        ctx.data_mut(|d| d.insert_temp(height_key, actual_height));
    }
}
