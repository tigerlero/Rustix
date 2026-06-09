//! Waveform visualization widget for audio preview.

use egui::{Color32, Pos2, Sense, Stroke, Vec2};

/// Renders an interactive waveform of audio samples.
///
/// Features: zoom, horizontal scroll, playhead cursor, channel coloring.
pub struct WaveformViewer {
    /// Zoom level: samples per pixel (lower = more zoomed in)
    zoom: f32,
    /// Horizontal scroll offset in samples
    scroll: f32,
    /// Whether auto-scroll follows the playhead
    follow_playhead: bool,
}

impl Default for WaveformViewer {
    fn default() -> Self {
        Self { zoom: 128.0, scroll: 0.0, follow_playhead: true }
    }
}

impl WaveformViewer {
    pub fn new() -> Self { Self::default() }

    /// Draw the waveform with a playhead cursor.
    ///
    /// - `samples`: interleaved f32 audio samples
    /// - `channels`: number of channels (1 = mono, 2 = stereo)
    /// - `sample_rate`: samples per second
    /// - `playhead_seconds`: current playback position (None = not playing)
    ///
    /// Returns `true` if the user interacted with the widget.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        samples: &[f32],
        channels: u16,
        sample_rate: u32,
        playhead_seconds: Option<f32>,
    ) -> egui::Response {
        let desired = Vec2::new(ui.available_width(), 80.0);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());

        if samples.is_empty() {
            return response;
        }

        let painter = ui.painter();
        let bg = Color32::from_rgb(22, 22, 28);
        let wave_color = Color32::from_rgb(80, 180, 240);
        let wave_color_r = Color32::from_rgb(240, 100, 120);
        let center_line = Color32::from_rgb(50, 50, 60);
        let playhead_color = Color32::from_rgb(255, 200, 50);
        let time_mark = Color32::from_rgba_premultiplied(100, 100, 120, 60);

        painter.rect_filled(rect, 2.0, bg);

        let total_frames = (samples.len() / channels.max(1) as usize) as f32;
        let mid_y = rect.center().y;
        let amp = rect.height() * 0.4;

        // Scroll handling
        if response.dragged() {
            self.scroll -= response.drag_delta().x * self.zoom;
            self.follow_playhead = false;
        }
        // Zoom with scroll wheel
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_delta != 0.0 {
                self.zoom = (self.zoom * (1.0 - scroll_delta * 0.005)).clamp(1.0, total_frames / rect.width().max(1.0));
                self.follow_playhead = false;
            }
        }

        // Auto-follow playhead
        if self.follow_playhead {
            if let Some(pos_sec) = playhead_seconds {
                let playhead_sample = pos_sec * sample_rate as f32 * channels as f32;
                self.scroll = playhead_sample - rect.width() * self.zoom * 0.5;
            }
        }

        let scroll = self.scroll.max(0.0);
        let samples_per_pixel = self.zoom;
        let start_sample = scroll as usize;
        let visible_samples = (rect.width() * samples_per_pixel) as usize;

        // Time markers
        let seconds_per_pixel = samples_per_pixel / (sample_rate as f32 * channels as f32).max(1.0);
        let marker_interval = if seconds_per_pixel * 100.0 < 1.0 { 1.0 } else if seconds_per_pixel * 100.0 < 5.0 { 5.0 } else { 10.0 };
        let mut marker_time = ((scroll as f32 / (sample_rate as f32 * channels.max(1) as f32)) / marker_interval).ceil() * marker_interval;
        while (marker_time * sample_rate as f32 * channels as f32) < (scroll + visible_samples as f32) {
            let x = rect.min.x + (marker_time * sample_rate as f32 * channels as f32 - scroll) / samples_per_pixel;
            if x >= rect.min.x && x <= rect.max.x {
                painter.line_segment(
                    [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
                    Stroke::new(1.0, time_mark),
                );
                painter.text(
                    Pos2::new(x + 2.0, rect.min.y + 12.0),
                    egui::Align2::LEFT_TOP,
                    format!("{marker_time:.0}s"),
                    egui::FontId::proportional(10.0),
                    time_mark,
                );
            }
            marker_time += marker_interval;
        }

        // Center line
        painter.line_segment(
            [Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)],
            Stroke::new(1.0, center_line),
        );

        // Draw waveform
        let step = (samples_per_pixel * 0.5).max(1.0) as usize;
        let ch_count = channels.max(1) as usize;

        for x_offset in (0..rect.width() as usize).step_by(2) {
            let sample_idx = start_sample + (x_offset as f32 * samples_per_pixel) as usize;
            if sample_idx + step >= samples.len() { break; }

            let x = rect.min.x + x_offset as f32;

            // Find min/max in this pixel column
            let mut min_val = 1.0f32;
            let mut max_val = -1.0f32;
            let mut min_val_r = 1.0f32;
            let mut max_val_r = -1.0f32;
            let has_stereo = ch_count >= 2;

            for si in (0..step).step_by(ch_count) {
                let idx = (sample_idx + si).min(samples.len().saturating_sub(1));
                let v = samples[idx];
                min_val = min_val.min(v);
                max_val = max_val.max(v);
                if has_stereo {
                    let idx_r = (idx + 1).min(samples.len().saturating_sub(1));
                    let v_r = samples[idx_r];
                    min_val_r = min_val_r.min(v_r);
                    max_val_r = max_val_r.max(v_r);
                }
            }

            // Draw channel bars
            let top = mid_y + max_val * amp;
            let bot = mid_y + min_val * amp;
            painter.line_segment(
                [Pos2::new(x, top), Pos2::new(x, bot)],
                Stroke::new(1.5, wave_color),
            );

            if has_stereo {
                let top_r = mid_y + max_val_r * amp;
                let bot_r = mid_y + min_val_r * amp;
                painter.line_segment(
                    [Pos2::new(x + 1.0, top_r), Pos2::new(x + 1.0, bot_r)],
                    Stroke::new(1.0, wave_color_r),
                );
            }
        }

        // Playhead cursor
        if let Some(pos_sec) = playhead_seconds {
            let playhead_sample = pos_sec * sample_rate as f32 * channels as f32;
            let px = rect.min.x + (playhead_sample - scroll) / samples_per_pixel;
            if px >= rect.min.x && px <= rect.max.x {
                painter.line_segment(
                    [Pos2::new(px, rect.min.y), Pos2::new(px, rect.max.y)],
                    Stroke::new(2.0, playhead_color),
                );
                // Triangle marker at top
                let tri = [
                    Pos2::new(px - 5.0, rect.min.y),
                    Pos2::new(px + 5.0, rect.min.y),
                    Pos2::new(px, rect.min.y + 8.0),
                ];
                painter.add(egui::Shape::convex_polygon(
                    tri.to_vec(), playhead_color, Stroke::NONE,
                ));
            }
        }

        // Border
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 50, 60)), egui::StrokeKind::Inside);

        response
    }
}
