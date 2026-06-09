use crate::app_state::AppState;

/// Render post-process settings in a compact multi-column layout.
/// Call from inside any egui window or panel.
pub fn post_process_panel(ui: &mut egui::Ui, app: &mut AppState) {
    ui.label(egui::RichText::new("Post-Process").size(14.0).strong());
    ui.add_space(8.0);

    ui.columns(3, |cols| {
        // ── Column 1: Bloom + SSAO ──
        cols[0].vertical(|ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Bloom").size(12.0).strong());
                ui.add(egui::Slider::new(&mut app.bloom_threshold, 0.0..=5.0).text("Threshold"));
                ui.add(egui::Slider::new(&mut app.bloom_intensity, 0.0..=2.0).text("Intensity"));
            });
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("SSAO").size(12.0).strong());
                ui.checkbox(&mut app.ssao_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut app.ssao_radius, 0.1..=2.0).text("Radius"));
                ui.add(egui::Slider::new(&mut app.ssao_bias, 0.0..=0.1).text("Bias"));
                ui.add(egui::Slider::new(&mut app.ssao_power, 0.5..=3.0).text("Power"));
                ui.add(egui::Slider::new(&mut app.ssao_intensity, 0.0..=3.0).text("Intensity"));
            });
        });

        // ── Column 2: TAA + SSR + Fog ──
        cols[1].vertical(|ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("TAA").size(12.0).strong());
                ui.checkbox(&mut app.taa_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut app.taa_blend_factor, 0.0..=0.5).text("Blend"));
            });
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("SSR").size(12.0).strong());
                ui.checkbox(&mut app.ssr_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut app.ssr_max_steps, 8.0..=128.0).text("Steps"));
                ui.add(egui::Slider::new(&mut app.ssr_stride, 1.0..=8.0).text("Stride"));
                ui.add(egui::Slider::new(&mut app.ssr_max_dist, 10.0..=100.0).text("Dist"));
            });
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("Volumetric Fog").size(12.0).strong());
                ui.checkbox(&mut app.fog_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut app.fog_density, 0.0..=0.1).text("Density"));
                ui.add(egui::Slider::new(&mut app.fog_scattering, 0.0..=2.0).text("Scattering"));
                ui.add(egui::Slider::new(&mut app.fog_height_falloff, 0.0..=0.5).text("Height Falloff"));
                ui.add(egui::Slider::new(&mut app.fog_max_dist, 10.0..=200.0).text("Max Dist"));
                ui.add(egui::Slider::new(&mut app.fog_max_steps, 8.0..=128.0).text("Steps"));
                ui.add(egui::Slider::new(&mut app.fog_sun_intensity, 0.0..=2.0).text("Sun Intensity"));
            });
        });

        // ── Column 3: Skybox + Rendering Tech ──
        cols[2].vertical(|ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Skybox / Atmosphere").size(12.0).strong());
                ui.checkbox(&mut app.skybox_enabled, "Enabled");
                ui.add(egui::Slider::new(&mut app.skybox_rayleigh, 0.0..=5.0).text("Rayleigh"));
                ui.add(egui::Slider::new(&mut app.skybox_mie, 0.0..=2.0).text("Mie"));
                ui.add(egui::Slider::new(&mut app.skybox_zenith_shift, -0.5..=0.5).text("Zenith Shift"));
                ui.add(egui::Slider::new(&mut app.skybox_exposure, 0.1..=3.0).text("Exposure"));
            });
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("Rendering Tech").size(12.0).strong());
                ui.checkbox(&mut app.instanced_enabled, "Instanced Rendering");
                ui.checkbox(&mut app.gpu_culling_enabled, "GPU Culling");
                ui.checkbox(&mut app.mesh_shader_enabled, "Mesh Shaders (NV)");
                ui.checkbox(&mut app.oit_enabled, "Ordered Independent Transparency");
            });
        });
    });
}
