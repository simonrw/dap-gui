use std::path::Path;
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{self, HighlightState, Highlighter, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Convert a syntect RGBA colour to a ratatui RGB colour.
fn syntect_to_ratatui(c: highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// Detect syntax from a file path extension.
fn detect_syntax(path: &Path) -> Option<&'static SyntaxReference> {
    let ext = path.extension()?.to_str()?;
    SYNTAX_SET.find_syntax_by_extension(ext)
}

/// A checkpoint of the parser state at a specific line, enabling
/// fast highlighting when scrolling backwards.
struct Checkpoint {
    line: usize,
    parse_state: ParseState,
    highlight_state: HighlightState,
}

/// A styled text segment: ratatui style + owned string content.
pub type StyledSegment = (Style, String);

/// Syntax highlighter that caches parse state checkpoints for efficient
/// viewport-only rendering.
pub struct SyntaxHighlighter {
    syntax: Option<&'static SyntaxReference>,
    checkpoints: Vec<Checkpoint>,
    /// How often to save a checkpoint (every N lines).
    checkpoint_interval: usize,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            syntax: None,
            checkpoints: Vec::new(),
            checkpoint_interval: 100,
        }
    }

    /// Prepare for a new file. Clears cached checkpoints.
    pub fn set_file(&mut self, path: &Path) {
        self.syntax = detect_syntax(path);
        self.checkpoints.clear();
    }

    /// Highlight a range of lines from `content`, returning one `Vec<StyledSegment>` per line.
    ///
    /// `start_line` and `end_line` are 0-indexed. Only the lines in this range are
    /// syntax-highlighted; state for lines before `start_line` is fast-forwarded using
    /// cached checkpoints.
    pub fn highlight_lines(
        &mut self,
        content: &str,
        start_line: usize,
        end_line: usize,
    ) -> Vec<Vec<StyledSegment>> {
        let syntax = match self.syntax {
            Some(s) => s,
            None => {
                // No syntax: return unstyled lines
                return content
                    .lines()
                    .skip(start_line)
                    .take(end_line.saturating_sub(start_line))
                    .map(|line| vec![(Style::default(), line.to_string())])
                    .collect();
            }
        };

        let theme = &THEME_SET.themes["base16-ocean.dark"];
        let highlighter = Highlighter::new(theme);

        // Find the best checkpoint at or before start_line
        let (mut current_line, mut parse_state, mut highlight_state) =
            self.find_checkpoint(syntax, &highlighter, start_line);

        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::with_capacity(end_line.saturating_sub(start_line));

        // Fast-forward from checkpoint to start_line, saving new checkpoints along the way
        while current_line < start_line && current_line < lines.len() {
            let line_with_nl = format!("{}\n", lines[current_line]);
            let ops = parse_state
                .parse_line(&line_with_nl, &SYNTAX_SET)
                .unwrap_or_default();
            // Advance highlight state (we discard the output)
            let _ = highlighting::HighlightIterator::new(
                &mut highlight_state,
                &ops,
                &line_with_nl,
                &highlighter,
            )
            .collect::<Vec<_>>();

            current_line += 1;

            // Save checkpoint
            if current_line % self.checkpoint_interval == 0 {
                self.save_checkpoint(current_line, &parse_state, &highlight_state);
            }
        }

        // Now highlight the visible lines
        for line_idx in start_line..end_line.min(lines.len()) {
            let line = lines[line_idx];
            let line_with_nl = format!("{line}\n");

            let ops = parse_state
                .parse_line(&line_with_nl, &SYNTAX_SET)
                .unwrap_or_default();

            let regions: Vec<(highlighting::Style, &str)> = highlighting::HighlightIterator::new(
                &mut highlight_state,
                &ops,
                &line_with_nl,
                &highlighter,
            )
            .collect();

            // Convert syntect styles to ratatui styles, clipping to original line length
            let mut styled_spans = Vec::new();
            let mut char_offset = 0;
            for (style, text) in &regions {
                if char_offset >= line.len() {
                    break;
                }
                let remaining = line.len() - char_offset;
                let text = if text.len() > remaining {
                    &text[..remaining]
                } else {
                    text
                };
                if text.is_empty() {
                    continue;
                }

                let mut ratatui_style = Style::default().fg(syntect_to_ratatui(style.foreground));
                if style.font_style.contains(highlighting::FontStyle::BOLD) {
                    ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                }
                if style.font_style.contains(highlighting::FontStyle::ITALIC) {
                    ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                }

                styled_spans.push((ratatui_style, text.to_string()));
                char_offset += text.len();
            }

            result.push(styled_spans);

            current_line = line_idx + 1;
            if current_line % self.checkpoint_interval == 0 {
                self.save_checkpoint(current_line, &parse_state, &highlight_state);
            }
        }

        result
    }

    /// Convert highlighted spans into ratatui `Line`s with line numbers and optional decorations.
    pub fn build_lines(
        highlighted: &[Vec<StyledSegment>],
        start_line: usize,
        gutter_width: usize,
        cursor_line: usize,
        search_matches: &[(usize, usize, usize)],
        current_match_idx: usize,
        exec_line: Option<usize>,
        breakpoint_lines: &std::collections::HashSet<usize>,
        selection_range: Option<(usize, usize)>,
        inline_evals: &std::collections::HashMap<usize, String>,
    ) -> Vec<Line<'static>> {
        let match_bg = Color::Rgb(100, 100, 0);
        let current_match_bg = Color::Rgb(180, 120, 0);
        let cursor_bg = Color::Rgb(40, 44, 52);
        let exec_bg = Color::Rgb(50, 60, 30); // greenish background for execution line
        let selection_bg = Color::Rgb(40, 50, 70); // bluish background for visual selection

        highlighted
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let line_idx = start_line + i;
                let line_num = line_idx + 1;
                let is_cursor = line_idx == cursor_line;
                let is_exec = exec_line == Some(line_idx);
                let is_selected = selection_range
                    .map_or(false, |(start, end)| line_idx >= start && line_idx <= end);

                // Pick the background color: exec > selection > cursor > none
                let bg = if is_exec {
                    exec_bg
                } else if is_selected {
                    selection_bg
                } else if is_cursor {
                    cursor_bg
                } else {
                    Color::Reset
                };

                // Line number gutter with execution/breakpoint markers
                let has_bp = breakpoint_lines.contains(&line_num);
                let gutter_marker = if is_exec && has_bp {
                    "\u{25b6}" // ▶ (exec takes precedence, but on a bp line)
                } else if is_exec {
                    "\u{25b6}" // ▶
                } else if has_bp {
                    "\u{25cf}" // ●
                } else {
                    " "
                };
                let gutter_style = if is_exec {
                    Style::default().fg(Color::Yellow).bg(exec_bg)
                } else if has_bp {
                    Style::default().fg(Color::Red).bg(if is_cursor {
                        cursor_bg
                    } else {
                        Color::Reset
                    })
                } else if is_cursor {
                    Style::default().fg(Color::White).bg(cursor_bg)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let num_str = format!("{gutter_marker}{:>width$} ", line_num, width = gutter_width);

                let mut line_spans: Vec<Span<'static>> = vec![Span::styled(num_str, gutter_style)];

                // Collect search matches for this line
                let line_matches: Vec<(usize, usize, bool)> = search_matches
                    .iter()
                    .enumerate()
                    .filter(|(_, (l, _, _))| *l == line_idx)
                    .map(|(idx, (_, start, len))| (*start, *len, idx == current_match_idx))
                    .collect();

                // Build the code spans, overlaying search highlights
                if line_matches.is_empty() {
                    // No search matches: just apply line highlight if needed
                    for (style, text) in spans {
                        let style = if bg != Color::Reset {
                            style.bg(bg)
                        } else {
                            *style
                        };
                        line_spans.push(Span::styled(text.clone(), style));
                    }
                } else {
                    // Need to split spans at match boundaries
                    let mut byte_pos = 0;
                    for (style, text) in spans {
                        let span_start = byte_pos;
                        let span_end = byte_pos + text.len();
                        let base_style = if bg != Color::Reset {
                            style.bg(bg)
                        } else {
                            *style
                        };

                        let mut pos = 0;
                        for &(m_start, m_len, is_current) in &line_matches {
                            let m_end = m_start + m_len;
                            // Skip matches outside this span
                            if m_end <= span_start + pos || m_start >= span_end {
                                continue;
                            }
                            // Emit non-matching prefix
                            let overlap_start = m_start.saturating_sub(span_start).max(pos);
                            if overlap_start > pos {
                                line_spans.push(Span::styled(
                                    text[pos..overlap_start].to_string(),
                                    base_style,
                                ));
                                pos = overlap_start;
                            }
                            // Emit matching portion
                            let overlap_end = m_end.saturating_sub(span_start).min(text.len());
                            if overlap_end > pos {
                                let bg = if is_current {
                                    current_match_bg
                                } else {
                                    match_bg
                                };
                                line_spans.push(Span::styled(
                                    text[pos..overlap_end].to_string(),
                                    base_style.bg(bg),
                                ));
                                pos = overlap_end;
                            }
                        }
                        // Remaining non-matching suffix
                        if pos < text.len() {
                            line_spans.push(Span::styled(text[pos..].to_string(), base_style));
                        }
                        byte_pos = span_end;
                    }
                }

                // Append inline evaluation annotation if present
                if let Some(eval_text) = inline_evals.get(&line_idx) {
                    let eval_style = if eval_text.starts_with("!!") {
                        Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
                    } else {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::DIM)
                    };
                    line_spans.push(Span::styled(format!("  {eval_text}"), eval_style));
                }

                Line::from(line_spans)
            })
            .collect()
    }

    // ── Internal ──────────────────────────────────────────────────────────

    fn find_checkpoint(
        &self,
        syntax: &SyntaxReference,
        highlighter: &Highlighter,
        target_line: usize,
    ) -> (usize, ParseState, HighlightState) {
        // Find the latest checkpoint at or before target_line
        let best = self
            .checkpoints
            .iter()
            .filter(|cp| cp.line <= target_line)
            .max_by_key(|cp| cp.line);

        match best {
            Some(cp) => (cp.line, cp.parse_state.clone(), cp.highlight_state.clone()),
            None => (
                0,
                ParseState::new(syntax),
                HighlightState::new(highlighter, ScopeStack::new()),
            ),
        }
    }

    fn save_checkpoint(
        &mut self,
        line: usize,
        parse_state: &ParseState,
        highlight_state: &HighlightState,
    ) {
        // Don't duplicate
        if self.checkpoints.iter().any(|cp| cp.line == line) {
            return;
        }
        self.checkpoints.push(Checkpoint {
            line,
            parse_state: parse_state.clone(),
            highlight_state: highlight_state.clone(),
        });
    }
}
