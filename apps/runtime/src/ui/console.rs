use crate::project::DockPosition;

pub fn show_console(
    ctx: &egui::Context,
    _project_dir: &Option<String>,
    _audio_engine: &mut Option<rustix_audio::AudioEngine>,
    _audio_instance: &mut Option<rustix_audio::SoundInstance>,
    _waveform_viewer: &mut crate::waveform::WaveformViewer,
    dock: DockPosition,
) {
    let gen = ctx.data(|d| d.get_temp::<u64>(egui::Id::new("layout_generation")).unwrap_or(0));
    let panel_id = egui::Id::new(("console_panel", gen));
    let height_key = egui::Id::new("console_height");
    let desired_height = ctx.data(|d| d.get_temp::<f32>(height_key)).unwrap_or(160.0);
    let result = super::dock::show_docked(ctx, "Console", panel_id, dock, desired_height, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Console");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Clear").clicked() {
                    rustix_core::log_capture::clear_logs();
                }
            });
        });
        ui.separator();

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
    });
    if let Some(inner) = result {
        let actual_height = inner.response.rect.height();
        ctx.data_mut(|d| d.insert_temp(height_key, actual_height));
    }
}
