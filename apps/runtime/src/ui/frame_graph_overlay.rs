use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use rustix_render::graph::{FrameGraphSnapshot, PassQueue};

/// Show a debug overlay window visualizing the last frame graph snapshot.
pub fn show_frame_graph_overlay(ctx: &egui::Context, open: &mut bool, snapshot: &FrameGraphSnapshot) {
    let window = egui::Window::new("Frame Graph")
        .open(open)
        .default_size([640.0, 480.0])
        .resizable(true);

    window.show(ctx, |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            ui.vertical(|ui| {
                // Summary stats
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Summary").strong());
                });
                ui.label(format!("Passes: {}", snapshot.passes.len()));
                ui.label(format!("Resources: {}", snapshot.textures.len()));
                ui.label(format!("Merged groups: {}", snapshot.merged_groups.len()));
                ui.label(format!("Transient memory: {} bytes", snapshot.transient_memory_size));
                ui.label(format!("Transient images: {}", snapshot.transient_image_count));
                ui.separator();

                // Merged groups legend
                if !snapshot.merged_groups.is_empty() {
                    ui.label(RichText::new("Merged Groups").strong());
                    for (idx, &(start, end)) in snapshot.merged_groups.iter().enumerate() {
                        let color = group_color(idx);
                        ui.horizontal(|ui| {
                            ui.colored_label(color, "■");
                            ui.label(format!("Group {}: passes {} → {}", idx + 1, start, end));
                        });
                    }
                    ui.separator();
                }

                // Pass timeline
                ui.label(RichText::new("Pass Timeline").strong());
                let pass_node_size = Vec2::new(140.0, 60.0);
                let gap = Vec2::new(16.0, 24.0);
                let cols = 4usize;
                let total_width = cols as f32 * (pass_node_size.x + gap.x);

                let cursor = ui.cursor();
                let origin = cursor.min;

                // Draw merged group backgrounds first
                for (gidx, &(start, end)) in snapshot.merged_groups.iter().enumerate() {
                    if start == end { continue; }
                    let start_col = start % cols;
                    let start_row = start / cols;
                    let end_col = end % cols;
                    let end_row = end / cols;

                    let top_left = Pos2::new(
                        origin.x + start_col as f32 * (pass_node_size.x + gap.x) - gap.x * 0.3,
                        origin.y + start_row as f32 * (pass_node_size.y + gap.y) - gap.y * 0.3,
                    );
                    let bottom_right = Pos2::new(
                        origin.x + end_col as f32 * (pass_node_size.x + gap.x) + pass_node_size.x + gap.x * 0.3,
                        origin.y + end_row as f32 * (pass_node_size.y + gap.y) + pass_node_size.y + gap.y * 0.3,
                    );
                    let group_rect = Rect::from_min_max(top_left, bottom_right);
                    ui.painter().rect_filled(group_rect, 4.0, group_color(gidx).gamma_multiply(0.15));
                    ui.painter().rect_stroke(group_rect, 4.0, Stroke::new(2.0, group_color(gidx)), egui::StrokeKind::Inside);
                }

                // Draw pass nodes
                for (i, pass) in snapshot.passes.iter().enumerate() {
                    let col = i % cols;
                    let row = i / cols;
                    let pos = Pos2::new(
                        origin.x + col as f32 * (pass_node_size.x + gap.x),
                        origin.y + row as f32 * (pass_node_size.y + gap.y),
                    );
                    let pass_rect = Rect::from_min_size(pos, pass_node_size);

                    let queue_color = match pass.queue {
                        PassQueue::Graphics => Color32::from_rgb(70, 130, 180),
                        PassQueue::Compute => Color32::from_rgb(180, 100, 70),
                    };

                    ui.painter().rect_filled(pass_rect, 4.0, Color32::from_gray(28));
                    ui.painter().rect_stroke(pass_rect, 4.0, Stroke::new(1.5, queue_color), egui::StrokeKind::Inside);

                    let name_pos = Pos2::new(pos.x + 6.0, pos.y + 4.0);
                    ui.painter().text(
                        name_pos,
                        egui::Align2::LEFT_TOP,
                        pass.name,
                        egui::FontId::proportional(13.0),
                        Color32::WHITE,
                    );

                    let queue_label = match pass.queue {
                        PassQueue::Graphics => "GFX",
                        PassQueue::Compute => "COMP",
                    };
                    let queue_pos = Pos2::new(pos.x + pass_node_size.x - 4.0, pos.y + 4.0);
                    ui.painter().text(
                        queue_pos,
                        egui::Align2::RIGHT_TOP,
                        queue_label,
                        egui::FontId::proportional(10.0),
                        queue_color,
                    );

                    // Color attachments
                    let mut attachment_text = String::new();
                    if !pass.color_attachments.is_empty() {
                        let names: Vec<String> = pass.color_attachments.iter()
                            .map(|rid| format!("R{}", rid.0))
                            .collect();
                        attachment_text.push_str(&format!("color: {}", names.join(", ")));
                    }
                    if let Some(d) = pass.depth_attachment {
                        attachment_text.push_str(&format!("  depth: R{}", d.0));
                    }
                    if !attachment_text.is_empty() {
                        let att_pos = Pos2::new(pos.x + 6.0, pos.y + pass_node_size.y - 6.0);
                        ui.painter().text(
                            att_pos,
                            egui::Align2::LEFT_BOTTOM,
                            &attachment_text,
                            egui::FontId::proportional(10.0),
                            Color32::from_gray(180),
                        );
                    }

                    // Barriers text
                    if let Some(barrier) = snapshot.barriers.get(i) {
                        let before_count = barrier.before.len();
                        let after_count = barrier.after.len();
                        if before_count > 0 || after_count > 0 {
                            let barrier_text = format!("B:{} A:{}", before_count, after_count);
                            let b_pos = Pos2::new(pos.x + pass_node_size.x - 4.0, pos.y + pass_node_size.y - 6.0);
                            ui.painter().text(
                                b_pos,
                                egui::Align2::RIGHT_BOTTOM,
                                barrier_text,
                                egui::FontId::proportional(9.0),
                                Color32::from_rgb(200, 180, 100),
                            );
                        }
                    }

                    // Sampled textures tooltip on hover
                    let response = ui.interact(pass_rect, egui::Id::new(format!("fg_pass_{}", i)), egui::Sense::hover());
                    if !pass.sampled_textures.is_empty() {
                        let sampled: Vec<String> = pass.sampled_textures.iter()
                            .map(|rid| format!("R{}", rid.0))
                            .collect();
                        response.on_hover_ui(|ui| {
                            ui.label(format!("Sampled: {}", sampled.join(", ")));
                            if pass.clear_color {
                                ui.label(format!("Clear: {:?}", pass.clear_value));
                            }
                            if pass.clear_depth {
                                ui.label("Clear depth");
                            }
                        });
                    }
                }

                // Advance cursor past the drawn area
                let rows = (snapshot.passes.len() + cols - 1) / cols;
                let used_height = rows.max(1) as f32 * (pass_node_size.y + gap.y);
                ui.allocate_space(Vec2::new(total_width, used_height));

                ui.separator();

                // Resource lifetime table
                ui.label(RichText::new("Resource Lifetimes").strong());
                egui::Grid::new("resource_lifetimes_grid")
                    .num_columns(5)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("ID");
                        ui.label("Format");
                        ui.label("Extent");
                        ui.label("First Pass");
                        ui.label("Last Pass");
                        ui.end_row();

                        for (i, tex) in snapshot.textures.iter().enumerate() {
                            ui.label(format!("R{}", i));
                            ui.label(format!("{:?}", tex.format));
                            ui.label(format!("{}x{}", tex.extent.width, tex.extent.height));
                            if let Some(Some((first, last))) = snapshot.lifetimes.get(i) {
                                ui.label(format!("{}", first));
                                ui.label(format!("{}", last));
                            } else {
                                ui.label("-");
                                ui.label("-");
                            }
                            ui.end_row();
                        }
                    });

                ui.separator();

                // Barriers detail
                ui.label(RichText::new("Barriers").strong());
                for (i, barrier) in snapshot.barriers.iter().enumerate() {
                    ui.collapsing(format!("Pass {} — {} before / {} after", i, barrier.before.len(), barrier.after.len()), |ui| {
                        for b in &barrier.before {
                            ui.label(format!("  before: {:?} → {:?}", b.old_layout, b.new_layout));
                        }
                        for b in &barrier.after {
                            ui.label(format!("  after: {:?} → {:?}", b.old_layout, b.new_layout));
                        }
                    });
                }
            });
        });
    });
}

fn group_color(index: usize) -> Color32 {
    let palette = [
        Color32::from_rgb(100, 180, 100),
        Color32::from_rgb(180, 100, 180),
        Color32::from_rgb(100, 140, 180),
        Color32::from_rgb(180, 160, 100),
        Color32::from_rgb(100, 180, 180),
        Color32::from_rgb(180, 120, 120),
    ];
    palette[index % palette.len()]
}
