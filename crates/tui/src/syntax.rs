use std::path::Path;
use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::highlighting::{self, HighlightState, Highlighter, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::theme::Theme;

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
        syntect_theme: &str,
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

        let theme = &THEME_SET.themes[syntect_theme];
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
        search_matches: &[crate::app::SearchMatch],
        current_match_idx: usize,
        exec_line: Option<usize>,
        breakpoint_lines: &std::collections::HashSet<usize>,
        selection_range: Option<(usize, usize)>,
        inline_evals: &std::collections::HashMap<usize, String>,
        theme: &Theme,
    ) -> Vec<Line<'static>> {
        let match_bg = theme.search_match_bg;
        let current_match_bg = theme.search_current_bg;
        let cursor_bg = theme.cursor_line_bg;
        let exec_bg = theme.exec_line_bg;
        let selection_bg = theme.code_selection_bg;

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
                    Style::default().fg(theme.accent).bg(exec_bg)
                } else if has_bp {
                    Style::default().fg(theme.error).bg(if is_cursor {
                        cursor_bg
                    } else {
                        Color::Reset
                    })
                } else if is_cursor {
                    Style::default().fg(theme.text).bg(cursor_bg)
                } else {
                    Style::default().fg(theme.text_muted)
                };
                let num_str = format!("{gutter_marker}{:>width$} ", line_num, width = gutter_width);

                let mut line_spans: Vec<Span<'static>> = vec![Span::styled(num_str, gutter_style)];

                // Collect search matches for this line
                let line_matches: Vec<(usize, usize, bool)> = search_matches
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.line == line_idx)
                    .map(|(idx, m)| (m.byte_start_in_line, m.length, idx == current_match_idx))
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
                        Style::default().fg(theme.error).add_modifier(Modifier::DIM)
                    } else {
                        Style::default()
                            .fg(theme.success)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn test_theme() -> Theme {
        Theme::dark()
    }

    // ── SyntaxHighlighter::highlight_lines ────────────────────────────

    #[test]
    fn highlight_lines_no_syntax_returns_unstyled() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/file.unknownext"));

        let content = "line one\nline two\nline three\n";
        let result = h.highlight_lines(content, 0, 3, "base16-ocean.dark");

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].len(), 1);
        assert_eq!(result[0][0].1, "line one");
        assert_eq!(result[0][0].0, Style::default());
    }

    #[test]
    fn highlight_lines_python_returns_styled_spans() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/test.py"));

        let content = "def hello():\n    pass\n";
        let result = h.highlight_lines(content, 0, 2, "base16-ocean.dark");

        assert_eq!(result.len(), 2);
        // Python keywords should produce multiple styled spans
        assert!(!result[0].is_empty());
        // At least one span should have a non-default foreground color
        let has_color = result[0]
            .iter()
            .any(|(style, _)| style.fg != Some(Color::Reset) && style.fg.is_some());
        assert!(has_color, "expected syntax-colored spans for Python");
    }

    #[test]
    fn highlight_lines_respects_range() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/test.py"));

        let content = "line0\nline1\nline2\nline3\nline4\n";
        let result = h.highlight_lines(content, 2, 4, "base16-ocean.dark");

        assert_eq!(result.len(), 2); // lines 2 and 3
    }

    #[test]
    fn highlight_lines_empty_content() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/test.py"));

        let result = h.highlight_lines("", 0, 0, "base16-ocean.dark");
        assert!(result.is_empty());
    }

    #[test]
    fn highlight_lines_range_beyond_content_is_clamped() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/test.py"));

        let content = "line0\nline1\n";
        let result = h.highlight_lines(content, 0, 100, "base16-ocean.dark");

        assert_eq!(result.len(), 2); // only 2 lines exist
    }

    // ── Checkpoint caching ────────────────────────────────────────────

    #[test]
    fn set_file_clears_checkpoints() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/a.py"));

        // Generate enough lines to trigger checkpoints
        let content: String = (0..150).map(|i| format!("x = {i}\n")).collect();
        h.highlight_lines(&content, 0, 150, "base16-ocean.dark");
        assert!(!h.checkpoints.is_empty());

        // Switch file: checkpoints cleared
        h.set_file(Path::new("/tmp/b.py"));
        assert!(h.checkpoints.is_empty());
    }

    #[test]
    fn highlighting_same_range_twice_uses_cache() {
        let mut h = SyntaxHighlighter::new();
        h.set_file(Path::new("/tmp/test.py"));

        let content: String = (0..200).map(|i| format!("x = {i}\n")).collect();

        let r1 = h.highlight_lines(&content, 100, 110, "base16-ocean.dark");
        let checkpoint_count = h.checkpoints.len();

        // Highlighting again should reuse existing checkpoints
        let r2 = h.highlight_lines(&content, 100, 110, "base16-ocean.dark");
        assert_eq!(h.checkpoints.len(), checkpoint_count);
        assert_eq!(r1.len(), r2.len());
    }

    // ── build_lines ──────────────────────────────────────────────────

    fn make_plain_spans(lines: &[&str]) -> Vec<Vec<StyledSegment>> {
        lines
            .iter()
            .map(|l| vec![(Style::default(), l.to_string())])
            .collect()
    }

    #[test]
    fn build_lines_basic_gutter_numbers() {
        let spans = make_plain_spans(&["hello", "world"]);
        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,    // start_line
            2,    // gutter_width
            0,    // cursor_line
            &[],  // search_matches
            0,    // current_match_idx
            None, // exec_line
            &HashSet::new(),
            None, // selection_range
            &HashMap::new(),
            &test_theme(),
        );

        assert_eq!(lines.len(), 2);
        // First span of each line should be the gutter
        let gutter0 = &lines[0].spans[0];
        assert!(
            gutter0.content.contains(" 1 "),
            "gutter should show line 1, got: {:?}",
            gutter0.content
        );

        let gutter1 = &lines[1].spans[0];
        assert!(
            gutter1.content.contains(" 2 "),
            "gutter should show line 2, got: {:?}",
            gutter1.content
        );
    }

    #[test]
    fn build_lines_cursor_line_has_background() {
        let spans = make_plain_spans(&["line0", "line1", "line2"]);
        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            1, // cursor on line 1
            &[],
            0,
            None,
            &HashSet::new(),
            None,
            &HashMap::new(),
            &test_theme(),
        );

        // The code spans on cursor line (index 1) should have a background
        let cursor_code_span = &lines[1].spans[1]; // span after gutter
        assert!(
            cursor_code_span.style.bg.is_some(),
            "cursor line should have a background color"
        );

        // Non-cursor lines should have no background (Reset)
        let other_code_span = &lines[0].spans[1];
        assert!(
            other_code_span.style.bg.is_none() || other_code_span.style.bg == Some(Color::Reset),
            "non-cursor line should have no background"
        );
    }

    #[test]
    fn build_lines_breakpoint_marker() {
        let spans = make_plain_spans(&["line0", "line1"]);
        let mut bp_lines = HashSet::new();
        bp_lines.insert(2); // line 2 (1-indexed) = index 1

        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            0,
            &[],
            0,
            None,
            &bp_lines,
            None,
            &HashMap::new(),
            &test_theme(),
        );

        // The gutter of line 1 (0-indexed) should have the breakpoint marker ●
        let gutter = &lines[1].spans[0];
        assert!(
            gutter.content.contains('\u{25cf}'),
            "breakpoint line gutter should contain ●, got: {:?}",
            gutter.content
        );
    }

    #[test]
    fn build_lines_execution_line_marker() {
        let spans = make_plain_spans(&["line0", "line1"]);
        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            0,
            &[],
            0,
            Some(1), // exec line at index 1
            &HashSet::new(),
            None,
            &HashMap::new(),
            &test_theme(),
        );

        // Gutter should contain the execution marker ▶
        let gutter = &lines[1].spans[0];
        assert!(
            gutter.content.contains('\u{25b6}'),
            "exec line gutter should contain ▶, got: {:?}",
            gutter.content
        );

        // Should have exec background
        let code_span = &lines[1].spans[1];
        assert!(
            code_span.style.bg.is_some(),
            "exec line should have a background"
        );
    }

    #[test]
    fn build_lines_selection_range_has_background() {
        let spans = make_plain_spans(&["a", "b", "c", "d"]);
        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            0,
            &[],
            0,
            None,
            &HashSet::new(),
            Some((1, 2)), // select lines 1-2
            &HashMap::new(),
            &test_theme(),
        );

        // Lines 1 and 2 should have selection background
        let sel1 = &lines[1].spans[1];
        assert!(
            sel1.style.bg.is_some(),
            "selected line should have background"
        );

        let sel2 = &lines[2].spans[1];
        assert!(
            sel2.style.bg.is_some(),
            "selected line should have background"
        );

        // Line 3 should not
        let non_sel = &lines[3].spans[1];
        assert!(
            non_sel.style.bg.is_none() || non_sel.style.bg == Some(Color::Reset),
            "non-selected line should have no background"
        );
    }

    #[test]
    fn build_lines_inline_evaluation_appended() {
        let spans = make_plain_spans(&["x = 1", "y = 2"]);
        let mut evals = HashMap::new();
        evals.insert(0, "= 42".to_string());

        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            0,
            &[],
            0,
            None,
            &HashSet::new(),
            None,
            &evals,
            &test_theme(),
        );

        // Last span of line 0 should contain the eval annotation
        let last_span = lines[0].spans.last().unwrap();
        assert!(
            last_span.content.contains("= 42"),
            "should have inline eval, got: {:?}",
            last_span.content
        );
        assert_eq!(last_span.style.fg, Some(Color::Green));
    }

    #[test]
    fn build_lines_inline_error_evaluation_is_red() {
        let spans = make_plain_spans(&["bad"]);
        let mut evals = HashMap::new();
        evals.insert(0, "!! NameError".to_string());

        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            0,
            &[],
            0,
            None,
            &HashSet::new(),
            None,
            &evals,
            &test_theme(),
        );

        let last_span = lines[0].spans.last().unwrap();
        assert!(last_span.content.contains("!! NameError"));
        assert_eq!(last_span.style.fg, Some(Color::Red));
    }

    #[test]
    fn build_lines_search_match_highlighted() {
        let spans = make_plain_spans(&["hello world"]);
        let matches = vec![crate::app::SearchMatch {
            line: 0,
            byte_start_in_line: 0,
            byte_offset: 0,
            length: 5,
        }];

        let lines = SyntaxHighlighter::build_lines(
            &spans,
            0,
            1,
            99, // cursor elsewhere
            &matches,
            0, // current match index = 0
            None,
            &HashSet::new(),
            None,
            &HashMap::new(),
            &test_theme(),
        );

        // Should have at least 3 spans: gutter, matched "hello", remaining " world"
        assert!(
            lines[0].spans.len() >= 3,
            "expected split spans for search match, got {} spans",
            lines[0].spans.len()
        );

        // The match span should have a special background
        let match_span = &lines[0].spans[1]; // first code span = the match
        assert!(
            match_span.style.bg.is_some() && match_span.style.bg != Some(Color::Reset),
            "search match should have highlight background"
        );
    }

    #[test]
    fn build_lines_with_start_line_offset() {
        let spans = make_plain_spans(&["mid1", "mid2"]);
        let lines = SyntaxHighlighter::build_lines(
            &spans,
            10, // start_line = 10
            3,
            10,
            &[],
            0,
            None,
            &HashSet::new(),
            None,
            &HashMap::new(),
            &test_theme(),
        );

        assert_eq!(lines.len(), 2);
        let gutter0 = &lines[0].spans[0];
        assert!(
            gutter0.content.contains(" 11 "),
            "first visible line should be 11, got: {:?}",
            gutter0.content
        );
    }
}
