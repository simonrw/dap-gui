use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use eframe::{
    egui::{self, TextEdit, TextFormat},
    epaint::{Color32, text::LayoutJob},
};
use syntect::highlighting::{self, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Convert a syntect color to an egui Color32
fn syntect_color_to_egui(c: highlighting::Color) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}

/// Detect language from file extension and return the syntax reference
fn detect_syntax(path: &Path) -> Option<&'static syntect::parsing::SyntaxReference> {
    let ext = path.extension()?.to_str()?;
    SYNTAX_SET.find_syntax_by_extension(ext)
}

/// Code view that shows debugger related things
///
/// Note: we assume that breakpoints have been filtered for the file that `content` is read from
pub struct CodeView<'a> {
    /// Read-only view into the text content
    content: &'a str,
    /// Optionally highlight the line the debugger has stopped on (1-indexed)
    current_line: usize,
    highlight_line: bool,
    /// Line numbers to add breakpoint markers to (1-indexed)
    breakpoints: &'a mut HashSet<debugger::Breakpoint>,
    /// Should we jump to the current position or not?
    jump: &'a bool,
    /// Path of the file being displayed (used for breakpoints and syntax detection)
    file_path: PathBuf,
    /// Whether the UI is using dark mode
    is_dark: bool,
}

impl<'a> CodeView<'a> {
    /// Create a new code view
    ///
    /// If `jump` is supplied, then jump to that position in the code viewer. If this occurs, then
    /// `jump` will be reset to `false`.
    pub fn new(
        content: &'a str,
        current_line: usize,
        highlight_line: bool,
        breakpoints: &'a mut HashSet<debugger::Breakpoint>,
        jump: &'a bool,
        file_path: PathBuf,
        is_dark: bool,
    ) -> Self {
        Self {
            content,
            current_line,
            highlight_line,
            breakpoints,
            jump,
            file_path,
            is_dark,
        }
    }

    fn breakpoint_positions(&self) -> HashSet<usize> {
        HashSet::from_iter(self.breakpoints.iter().map(|b| b.line))
    }
}

impl egui::Widget for CodeView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        let breakpoint_positions = self.breakpoint_positions();

        // Read hovered gutter line from previous frame (stored after scroll area renders)
        let hover_id = egui::Id::new("code_view_hovered_gutter_line");
        let hovered_gutter_line: Option<usize> = ui.data(|d| d.get_temp::<usize>(hover_id));

        // Set up syntax highlighting
        let syntax = detect_syntax(&self.file_path);
        let theme_name = if self.is_dark {
            "base16-ocean.dark"
        } else {
            "base16-ocean.light"
        };
        let theme = &THEME_SET.themes[theme_name];
        let highlighter = syntax.map(|syn| {
            syntect::highlighting::Highlighter::new(theme);
            (syn, theme)
        });

        let highlight_line = self.highlight_line;
        let current_line = self.current_line;

        let bullet = "•";

        // closure that defines the layout job
        let mut layouter = |ui: &egui::Ui, s: &dyn egui::TextBuffer, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            let indent = 4.0;
            let bullet_format = |color| TextFormat {
                font_id: egui::FontId::monospace(
                    ui.style().text_styles[&egui::TextStyle::Body].size,
                ),
                color,
                ..Default::default()
            };

            if let Some((syn, theme)) = &highlighter {
                let mut highlight_state = syntect::highlighting::HighlightState::new(
                    &highlighting::Highlighter::new(theme),
                    syntect::parsing::ScopeStack::new(),
                );
                let mut parse_state = syntect::parsing::ParseState::new(syn);

                for (i, line) in s.as_str().lines().enumerate() {
                    let line_num = i + 1;
                    // Always render bullet to reserve space
                    let bullet_color = if breakpoint_positions.contains(&line_num) {
                        Color32::from_rgb(255, 85, 85)
                    } else if hovered_gutter_line == Some(line_num) {
                        Color32::from_rgba_unmultiplied(255, 85, 85, 80)
                    } else {
                        Color32::TRANSPARENT
                    };
                    layout_job.append(bullet, 0.0, bullet_format(bullet_color));

                    let line_with_newline = format!("{line}\n");
                    let ops = parse_state
                        .parse_line(&line_with_newline, &SYNTAX_SET)
                        .unwrap_or_default();
                    let regions = syntect::highlighting::HighlightIterator::new(
                        &mut highlight_state,
                        &ops,
                        &line_with_newline,
                        &highlighting::Highlighter::new(theme),
                    )
                    .collect::<Vec<_>>();

                    let is_current = highlight_line && i == current_line.wrapping_sub(1);

                    // Append highlighted tokens for the line (not the trailing newline)
                    let mut char_offset = 0;
                    for (style, text) in &regions {
                        if char_offset >= line.len() {
                            break;
                        }
                        // Clip text to not exceed the original line (exclude trailing \n)
                        let remaining = line.len() - char_offset;
                        let text = if text.len() > remaining {
                            &text[..remaining]
                        } else {
                            text
                        };
                        if text.is_empty() {
                            continue;
                        }

                        let first_token = char_offset == 0;
                        let leading = if first_token { indent } else { 0.0 };
                        let mut format = TextFormat {
                            color: syntect_color_to_egui(style.foreground),
                            ..Default::default()
                        };
                        if is_current {
                            format.background = Color32::from_rgb(68, 71, 90);
                        }
                        layout_job.append(text, leading, format);
                        char_offset += text.len();
                    }

                    layout_job.append("\n", 0.0, TextFormat::default());
                }
            } else {
                // Fallback: no syntax highlighting
                for (i, line) in s.as_str().lines().enumerate() {
                    let line_num = i + 1;
                    let bullet_color = if breakpoint_positions.contains(&line_num) {
                        Color32::from_rgb(255, 85, 85)
                    } else if hovered_gutter_line == Some(line_num) {
                        Color32::from_rgba_unmultiplied(255, 85, 85, 80)
                    } else {
                        Color32::TRANSPARENT
                    };
                    layout_job.append(bullet, 0.0, bullet_format(bullet_color));

                    if highlight_line && i == current_line.wrapping_sub(1) {
                        layout_job.append(
                            line,
                            indent,
                            TextFormat {
                                background: Color32::from_rgb(68, 71, 90),
                                ..Default::default()
                            },
                        );
                    } else {
                        layout_job.append(line, indent, TextFormat::default());
                    }
                    layout_job.append("\n", 0.0, TextFormat::default());
                }
            }

            ui.fonts_mut(|f| f.layout_job(layout_job))
        };

        let response = egui::ScrollArea::vertical().show(ui, |ui| {
            TextEdit::multiline(&mut self.content)
                .desired_width(f32::INFINITY)
                .layouter(&mut layouter)
                .show(ui)
        });

        let galley_pos = response.inner.galley_pos;
        let galley = &response.inner.galley;
        let text = self.content;

        // Helper: map a screen-space y coordinate to a 1-indexed line number
        // galley_pos is in screen space (accounts for scroll), so no offset needed
        let line_at_screen_y = |screen_y: f32| -> Option<usize> {
            let galley_y = screen_y - galley_pos.y;
            let cursor = galley.cursor_from_pos(egui::vec2(0.0, galley_y));
            let line = text[..cursor.index.min(text.len())]
                .chars()
                .filter(|&c| c == '\n')
                .count()
                + 1;
            let num_lines = text.lines().count().max(1);
            if line >= 1 && line <= num_lines {
                Some(line)
            } else {
                None
            }
        };

        // handle jumping to the breakpoint
        if *self.jump {
            let mut state = response.state;
            let num_lines = self.content.lines().count().max(1) as f32;
            let viewport_height = response.inner_rect.height();
            let row_height = response.content_size.y / num_lines;
            let target_y = self.current_line.saturating_sub(1) as f32 * row_height;

            // Only scroll if the target line is outside the visible area (with margin)
            let current_top = state.offset.y;
            let current_bottom = current_top + viewport_height;
            let margin = row_height * 3.0;

            if target_y < current_top + margin || target_y + row_height > current_bottom - margin {
                // Center the target line in the viewport
                let scroll_y = (target_y - viewport_height / 2.0 + row_height / 2.0)
                    .max(0.0)
                    .min((response.content_size.y - viewport_height).max(0.0));
                state.offset.y = scroll_y;
            }
            state.store(ui.ctx(), response.id);
        }

        // Detect gutter hover and store for next frame's layouter
        {
            let pointer_pos = ui.input(|i| i.pointer.hover_pos());
            let gutter_right = response.inner_rect.left() + 16.0;

            let new_hovered: Option<usize> = pointer_pos.and_then(|pos| {
                if pos.x > gutter_right || !response.inner_rect.contains(pos) {
                    return None;
                }
                line_at_screen_y(pos.y)
            });

            if new_hovered.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            ui.data_mut(|d| {
                if let Some(line) = new_hovered {
                    d.insert_temp(hover_id, line);
                } else {
                    d.remove_temp::<usize>(hover_id);
                }
            });
        }

        // Handle breakpoint click
        {
            let text_response = &response.inner.response;
            if text_response.clicked_by(egui::PointerButton::Primary) {
                if let Some(screen_pos) = text_response.interact_pointer_pos() {
                    let gutter_right = response.inner_rect.left() + 16.0;
                    if screen_pos.x <= gutter_right {
                        if let Some(line) = line_at_screen_y(screen_pos.y) {
                            let existing = self
                                .breakpoints
                                .iter()
                                .find(|b| b.path == self.file_path && b.line == line)
                                .cloned();

                            if let Some(bp) = existing {
                                self.breakpoints.remove(&bp);
                            } else {
                                self.breakpoints.insert(debugger::Breakpoint {
                                    path: self.file_path.clone(),
                                    line,
                                    name: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        response.inner.response
    }
}
