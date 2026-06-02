/// Configure egui fonts using bundled Noto fonts embedded via `include_bytes!`.
///
/// Fallback chain:
///   Proportional: noto_sans → [Ubuntu-Light] → noto_emoji
///   Monospace:    noto_mono → [Hack] → noto_emoji
///
/// noto_emoji catches emoji and symbols (▶ ⏹ 🔊); box-drawing (└ ─) and
/// arrows (→) are covered by egui's built-in Ubuntu-Light / Hack.
///
/// Fonts are compiled into the binary, so rendering is deterministic across
/// platforms (no dependency on OS-installed fonts).
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let noto_sans = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
    let noto_mono = include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf");
    let noto_emoji = include_bytes!("../../../assets/fonts/NotoEmoji-Regular.ttf");

    fonts.font_data.insert(
        "noto_sans".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_sans.to_vec())),
    );
    fonts.font_data.insert(
        "noto_mono".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_mono.to_vec())),
    );
    fonts.font_data.insert(
        "noto_emoji".into(),
        std::sync::Arc::new(egui::FontData::from_owned(noto_emoji.to_vec())),
    );

    // Proportional: prefer Noto Sans, fall back to Noto Emoji for symbols/emoji
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        fam.insert(0, "noto_sans".into());
        fam.push("noto_emoji".into());
    }

    // Monospace: prefer Noto Mono, fall back to Noto Emoji
    if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        fam.insert(0, "noto_mono".into());
        fam.push("noto_emoji".into());
    }

    ctx.set_fonts(fonts);
}
