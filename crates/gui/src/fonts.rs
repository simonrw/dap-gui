use eframe::egui;

const LILEX_REGULAR: &[u8] = include_bytes!("../fonts/Lilex-Regular.ttf");

/// Install the Lilex font as the monospace font family.
pub fn install_lilex(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Lilex-Regular".to_owned(),
        egui::FontData::from_static(LILEX_REGULAR).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Lilex-Regular".to_owned());
    ctx.set_fonts(fonts);
}
