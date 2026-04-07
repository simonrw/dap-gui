use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use eframe::{
    egui::{self, TextEdit, TextFormat},
    epaint::{Color32, text::LayoutJob},
};
use syntect::highlighting::{self, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Re-export shared SearchMatch for use in code_view rendering.
pub use ui_core::search::SearchMatch;

/// GUI-specific search state wrapping the shared `SearchState`.
///
/// Adds `request_focus` and `scroll_to_match` rendering hints that are
/// specific to the egui frontend.
#[derive(Default)]
pub struct GuiSearchState {
    pub inner: ui_core::search::SearchState,
    /// Whether we should request focus on the search input this frame.
    pub request_focus: bool,
    /// Whether we need to scroll to the current match.
    pub scroll_to_match: bool,
}

impl std::ops::Deref for GuiSearchState {
    type Target = ui_core::search::SearchState;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for GuiSearchState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl GuiSearchState {
    /// Recompute matches, setting scroll_to_match if matches were found.
    pub fn update(&mut self, content: &str, file_path: &Path) {
        if self.inner.update(content, file_path) {
            self.scroll_to_match = !self.inner.matches.is_empty();
        }
    }

    /// Navigate to the next match and request scroll.
    pub fn next_match(&mut self) {
        self.inner.next_match();
        self.scroll_to_match = true;
    }

    /// Navigate to the previous match and request scroll.
    pub fn prev_match(&mut self) {
        self.inner.prev_match();
        self.scroll_to_match = true;
    }

    /// Reset the cache for a new file.
    pub fn recompute_for_new_file(&mut self) {
        self.inner.reset();
    }
}

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
    /// Font size for code text
    font_size: f32,
    /// Search matches to highlight (byte offsets into content)
    search_matches: &'a [SearchMatch],
    /// Index of the current active match (for distinct highlighting)
    current_search_match: usize,
    /// Optional line to scroll to (1-indexed), used for search navigation
    scroll_to_line: Option<usize>,
}

impl<'a> CodeView<'a> {
    /// Create a new code view
    ///
    /// If `jump` is supplied, then jump to that position in the code viewer. If this occurs, then
    /// `jump` will be reset to `false`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content: &'a str,
        current_line: usize,
        highlight_line: bool,
        breakpoints: &'a mut HashSet<debugger::Breakpoint>,
        jump: &'a bool,
        file_path: PathBuf,
        is_dark: bool,
        font_size: f32,
        search_matches: &'a [SearchMatch],
        current_search_match: usize,
        scroll_to_line: Option<usize>,
    ) -> Self {
        Self {
            content,
            current_line,
            highlight_line,
            breakpoints,
            jump,
            file_path,
            is_dark,
            font_size,
            search_matches,
            current_search_match,
            scroll_to_line,
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
        let search_matches = self.search_matches;
        let current_search_match = self.current_search_match;
        let match_bg = if self.is_dark {
            Color32::from_rgba_unmultiplied(255, 255, 0, 60)
        } else {
            Color32::from_rgba_unmultiplied(255, 200, 0, 100)
        };
        let current_match_bg = if self.is_dark {
            Color32::from_rgba_unmultiplied(255, 150, 0, 140)
        } else {
            Color32::from_rgba_unmultiplied(255, 120, 0, 160)
        };

        let bullet = "•";

        // Helper: append a token to the layout job, splitting at search match boundaries
        // to apply match background colours. `abs_offset` is the byte offset of this token
        // in the original source content.
        let append_with_search_highlights =
            |layout_job: &mut LayoutJob,
             text: &str,
             leading: f32,
             base_format: TextFormat,
             abs_offset: usize| {
                if search_matches.is_empty() || text.is_empty() {
                    layout_job.append(text, leading, base_format);
                    return;
                }

                let token_start = abs_offset;
                let token_end = abs_offset + text.len();
                let mut pos = 0usize; // position within `text`
                let mut first = true;

                for (match_idx, m) in search_matches.iter().enumerate() {
                    let m_start = m.byte_offset;
                    let m_end = m.byte_offset + m.length;

                    // Skip matches that don't overlap this token
                    if m_end <= token_start + pos || m_start >= token_end {
                        continue;
                    }

                    // Emit non-matching prefix within this token
                    let overlap_start = m_start.saturating_sub(token_start).max(pos);
                    if overlap_start > pos {
                        let lead = if first { leading } else { 0.0 };
                        first = false;
                        layout_job.append(&text[pos..overlap_start], lead, base_format.clone());
                        pos = overlap_start;
                    }

                    // Emit the matching portion
                    let overlap_end = m_end.saturating_sub(token_start).min(text.len());
                    if overlap_end > pos {
                        let lead = if first { leading } else { 0.0 };
                        first = false;
                        let bg = if match_idx == current_search_match {
                            current_match_bg
                        } else {
                            match_bg
                        };
                        let mut fmt = base_format.clone();
                        fmt.background = bg;
                        layout_job.append(&text[pos..overlap_end], lead, fmt);
                        pos = overlap_end;
                    }
                }

                // Emit remaining non-matching suffix
                if pos < text.len() {
                    let lead = if first { leading } else { 0.0 };
                    layout_job.append(&text[pos..], lead, base_format);
                }
            };

        // closure that defines the layout job
        let font_size = self.font_size;
        let mut layouter = |ui: &egui::Ui, s: &dyn egui::TextBuffer, _wrap_width: f32| {
            let mut layout_job = LayoutJob::default();
            let indent = 4.0;
            let bullet_format = |color| TextFormat {
                font_id: egui::FontId::monospace(font_size),
                color,
                ..Default::default()
            };

            // Track absolute byte offset in the source content
            let mut line_byte_offset: usize = 0;

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
                        Color32::from_rgb(255, 0, 0)
                    } else if hovered_gutter_line == Some(line_num) {
                        Color32::from_rgba_unmultiplied(255, 0, 0, 80)
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
                            format.background = Color32::from_gray(128);
                        }
                        append_with_search_highlights(
                            &mut layout_job,
                            text,
                            leading,
                            format,
                            line_byte_offset + char_offset,
                        );
                        char_offset += text.len();
                    }

                    layout_job.append("\n", 0.0, TextFormat::default());
                    // +1 for the newline character
                    line_byte_offset += line.len() + 1;
                }
            } else {
                // Fallback: no syntax highlighting
                for (i, line) in s.as_str().lines().enumerate() {
                    let line_num = i + 1;
                    let bullet_color = if breakpoint_positions.contains(&line_num) {
                        Color32::from_rgb(255, 0, 0)
                    } else if hovered_gutter_line == Some(line_num) {
                        Color32::from_rgba_unmultiplied(255, 0, 0, 80)
                    } else {
                        Color32::TRANSPARENT
                    };
                    layout_job.append(bullet, 0.0, bullet_format(bullet_color));

                    let is_current = highlight_line && i == current_line.wrapping_sub(1);
                    let base_format = if is_current {
                        TextFormat {
                            background: Color32::from_gray(128),
                            ..Default::default()
                        }
                    } else {
                        TextFormat::default()
                    };
                    append_with_search_highlights(
                        &mut layout_job,
                        line,
                        indent,
                        base_format,
                        line_byte_offset,
                    );
                    layout_job.append("\n", 0.0, TextFormat::default());
                    line_byte_offset += line.len() + 1;
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

        // Scroll to a specific line (used by both debugger jump and search navigation)
        let scroll_target = if *self.jump {
            Some(self.current_line)
        } else {
            self.scroll_to_line
        };

        if let Some(target_line) = scroll_target {
            let mut state = response.state;
            let num_lines = self.content.lines().count().max(1) as f32;
            let viewport_height = response.inner_rect.height();
            let row_height = response.content_size.y / num_lines;
            let target_y = target_line.saturating_sub(1) as f32 * row_height;

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
            if text_response.clicked_by(egui::PointerButton::Primary)
                && let Some(screen_pos) = text_response.interact_pointer_pos()
            {
                let gutter_right = response.inner_rect.left() + 16.0;
                if screen_pos.x <= gutter_right
                    && let Some(line) = line_at_screen_y(screen_pos.y)
                {
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

        response.inner.response
    }
}
