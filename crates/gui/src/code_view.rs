use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use eframe::{
    egui::{self, Response, TextEdit, TextFormat},
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
        // closure that defines the layout job
        let mut layouter = |ui: &egui::Ui, s: &dyn egui::TextBuffer, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            let indent = 16.0;

            if let Some((syn, theme)) = &highlighter {
                let mut highlight_state = syntect::highlighting::HighlightState::new(
                    &highlighting::Highlighter::new(theme),
                    syntect::parsing::ScopeStack::new(),
                );
                let mut parse_state = syntect::parsing::ParseState::new(syn);

                for (i, line) in s.as_str().lines().enumerate() {
                    // Breakpoint marker
                    if breakpoint_positions.contains(&(i + 1)) {
                        layout_job.append(
                            "•",
                            0.0,
                            TextFormat {
                                color: Color32::from_rgb(255, 0, 0),
                                ..Default::default()
                            },
                        );
                    }

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
                            format.background = Color32::from_gray(128);
                        }
                        layout_job.append(text, leading, format);
                        char_offset += text.len();
                    }

                    layout_job.append("\n", 0.0, TextFormat::default());
                }
            } else {
                // Fallback: no syntax highlighting
                for (i, line) in s.as_str().lines().enumerate() {
                    if breakpoint_positions.contains(&(i + 1)) {
                        layout_job.append(
                            "•",
                            0.0,
                            TextFormat {
                                color: Color32::from_rgb(255, 0, 0),
                                ..Default::default()
                            },
                        );
                    }
                    if highlight_line && i == current_line.wrapping_sub(1) {
                        layout_job.append(
                            line,
                            indent,
                            TextFormat {
                                background: Color32::from_gray(128),
                                ..Default::default()
                            },
                        );
                    } else {
                        layout_job.append(line, indent, TextFormat::default());
                    }
                    layout_job.append("\n", indent, TextFormat::default());
                }
            }

            ui.fonts_mut(|f| f.layout_job(layout_job))
        };

        let response = egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(TextEdit::multiline(&mut self.content).layouter(&mut layouter))
        });

        // handle jumping to the breakpoint
        if *self.jump {
            let mut state = response.state;
            let num_lines = self.content.lines().count();
            let position_fractional = self.current_line as f32 / num_lines as f32;

            let window_centre_pos = (position_fractional * response.content_size.y) as i32;
            let window_pos = (window_centre_pos - ((response.inner_rect.max.y / 2.0) as i32))
                .max(0)
                .min((response.content_size.y - response.inner_rect.max.y) as i32);

            state.offset.y = window_pos as f32;
            state.store(ui.ctx(), response.id);
        }

        self.update_breakpoints(&response);

        response.inner
    }
}

impl CodeView<'_> {
    fn update_breakpoints(
        &mut self,
        scroll_response: &egui::scroll_area::ScrollAreaOutput<Response>,
    ) {
        let text_response = &scroll_response.inner;
        if !text_response.clicked_by(egui::PointerButton::Primary) {
            return;
        }

        let Some(screen_pos) = text_response.interact_pointer_pos() else {
            return;
        };

        // Check if the click is in the gutter region (left 16px of the visible area)
        let gutter_right = scroll_response.inner_rect.left() + 16.0;
        if screen_pos.x > gutter_right {
            return;
        }

        // Calculate the row height from the font
        let row_height =
            scroll_response.content_size.y / self.content.lines().count().max(1) as f32;

        // Convert screen position to content position
        let content_y =
            (screen_pos.y - scroll_response.inner_rect.top()) + scroll_response.state.offset.y;

        // Calculate 1-indexed line number
        let line = (content_y / row_height).floor() as usize + 1;
        let num_lines = self.content.lines().count();
        if line < 1 || line > num_lines {
            return;
        }

        let breakpoint = debugger::Breakpoint {
            path: self.file_path.clone(),
            line,
            name: None,
        };

        if self.breakpoints.contains(&breakpoint) {
            self.breakpoints.remove(&breakpoint);
        } else {
            self.breakpoints.insert(breakpoint);
        }
    }
}
