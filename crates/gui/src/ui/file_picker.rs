use std::path::PathBuf;

use eframe::{
    egui::{self, Align2, Context, Key, TextEdit},
    epaint::{Color32, text::LayoutJob},
};

use crate::DebuggerAppState;

/// Result of rendering the file picker overlay.
pub(crate) enum FilePickerResult {
    /// No action taken (picker still open or closed without selection).
    None,
    /// User selected a file.
    Selected(PathBuf),
}

/// Render the file picker overlay. Returns a `FilePickerResult` indicating
/// whether a file was selected.
pub(crate) fn show(ctx: &Context, state: &mut DebuggerAppState) -> FilePickerResult {
    // Lazy-load git files on first open
    state
        .file_picker
        .ensure_loaded(&state.debug_root_dir.clone());
    let fp = &mut state.file_picker;

    let mut result = FilePickerResult::None;

    // Handle keyboard input before rendering the window
    let mut close = false;
    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            close = true;
        }
        if i.key_pressed(Key::ArrowDown) {
            fp.cursor_down();
        }
        if i.key_pressed(Key::ArrowUp) {
            fp.cursor_up();
        }
        if i.key_pressed(Key::Enter) && !fp.results.is_empty() {
            let selected = &fp.results[fp.cursor];
            result = FilePickerResult::Selected(selected.file.absolute_path.clone());
            close = true;
        }
    });

    if close {
        fp.close();
        return result;
    }

    let screen_rect = ctx.input(|i| i.viewport_rect());
    let picker_width = (screen_rect.width() * 0.5).max(300.0).min(600.0);

    egui::Window::new("file_picker")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_TOP, [0.0, 40.0])
        .fixed_size([picker_width, 300.0])
        .show(ctx, |ui| {
            // Search input
            let input_response = ui.add(
                TextEdit::singleline(&mut fp.query)
                    .hint_text("Search files...")
                    .desired_width(f32::INFINITY),
            );

            // Auto-focus the text input
            input_response.request_focus();

            // Re-filter on every frame (input may have changed)
            fp.refilter();

            ui.separator();

            // Results list
            let max_visible = 15;
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (i, m) in fp.results.iter().take(max_visible).enumerate() {
                        let is_selected = i == fp.cursor;
                        let path_str = m.file.relative_path.to_string_lossy();

                        // Build a LayoutJob with highlighted match characters
                        let mut job = LayoutJob::default();
                        let base_color = if is_selected {
                            Color32::WHITE
                        } else {
                            ui.visuals().text_color()
                        };
                        let match_color = Color32::from_rgb(255, 200, 50);

                        for (ci, ch) in path_str.char_indices() {
                            let color = if m.matched_indices.contains(&ci) {
                                match_color
                            } else {
                                base_color
                            };
                            let mut buf = [0u8; 4];
                            job.append(
                                ch.encode_utf8(&mut buf),
                                0.0,
                                egui::TextFormat {
                                    color,
                                    ..Default::default()
                                },
                            );
                        }

                        let bg = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            Color32::TRANSPARENT
                        };

                        let frame = egui::Frame::new().fill(bg).inner_margin(4.0);
                        frame.show(ui, |ui| {
                            ui.label(job);
                        });
                    }
                });
        });

    result
}
