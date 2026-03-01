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
    if !state.git_files_loaded {
        state.git_files_loaded = true;
        if let Some(root) = fuzzy::find_repo_root() {
            match fuzzy::list_git_files(&root) {
                Ok(files) => state.git_files = files,
                Err(e) => tracing::warn!(error = %e, "failed to list git files"),
            }
        }
        state.file_picker_results = fuzzy::fuzzy_filter(&state.git_files, &state.file_picker_input);
    }

    let mut result = FilePickerResult::None;

    // Handle keyboard input before rendering the window
    let mut close = false;
    ctx.input(|i| {
        if i.key_pressed(Key::Escape) {
            close = true;
        }
        if i.key_pressed(Key::ArrowDown) {
            if !state.file_picker_results.is_empty() {
                state.file_picker_cursor =
                    (state.file_picker_cursor + 1).min(state.file_picker_results.len() - 1);
            }
        }
        if i.key_pressed(Key::ArrowUp) {
            state.file_picker_cursor = state.file_picker_cursor.saturating_sub(1);
        }
        if i.key_pressed(Key::Enter) && !state.file_picker_results.is_empty() {
            let selected = &state.file_picker_results[state.file_picker_cursor];
            result = FilePickerResult::Selected(selected.file.absolute_path.clone());
            close = true;
        }
    });

    if close {
        state.file_picker_open = false;
        state.file_picker_input.clear();
        state.file_picker_cursor = 0;
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
                TextEdit::singleline(&mut state.file_picker_input)
                    .hint_text("Search files...")
                    .desired_width(f32::INFINITY),
            );

            // Auto-focus the text input
            input_response.request_focus();

            // Re-filter on every frame (input may have changed)
            let prev_len = state.file_picker_results.len();
            state.file_picker_results =
                fuzzy::fuzzy_filter(&state.git_files, &state.file_picker_input);

            // Clamp cursor
            if state.file_picker_results.is_empty() {
                state.file_picker_cursor = 0;
            } else {
                // Reset cursor when results change significantly
                if state.file_picker_results.len() != prev_len {
                    state.file_picker_cursor = 0;
                }
                state.file_picker_cursor = state
                    .file_picker_cursor
                    .min(state.file_picker_results.len() - 1);
            }

            ui.separator();

            // Results list
            let max_visible = 15;
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (i, m) in state
                        .file_picker_results
                        .iter()
                        .take(max_visible)
                        .enumerate()
                    {
                        let is_selected = i == state.file_picker_cursor;
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
