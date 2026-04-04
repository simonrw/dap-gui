use std::path::PathBuf;

/// State for the code view panel.
///
/// Tracks the currently displayed file, cursor position, scroll offset,
/// and visual selection. This is primarily used by the TUI (keyboard-driven
/// navigation) but is kept in ui-core for reuse.
pub struct CodeViewState {
    /// Currently displayed file path.
    pub file_path: Option<PathBuf>,
    /// Cursor line (0-indexed) -- the line the user is "on" for navigation.
    pub cursor_line: usize,
    /// Scroll offset (first visible line, 0-indexed).
    pub scroll_offset: usize,
    /// Total lines in the current file.
    pub total_lines: usize,
    /// Visual selection anchor line (0-indexed). When set, selection spans
    /// from anchor to cursor_line (inclusive, in either direction).
    pub selection_anchor: Option<usize>,
}

impl Default for CodeViewState {
    fn default() -> Self {
        Self {
            file_path: None,
            cursor_line: 0,
            scroll_offset: 0,
            total_lines: 0,
            selection_anchor: None,
        }
    }
}

impl CodeViewState {
    /// Open a file, resetting cursor and scroll.
    pub fn open_file(&mut self, path: PathBuf, total_lines: usize) {
        self.file_path = Some(path);
        self.cursor_line = 0;
        self.scroll_offset = 0;
        self.total_lines = total_lines;
        self.selection_anchor = None;
    }

    /// Move cursor down by `n` lines, clamping to file bounds.
    pub fn move_cursor_down(&mut self, n: usize) {
        if self.total_lines == 0 {
            return;
        }
        self.cursor_line = (self.cursor_line + n).min(self.total_lines - 1);
    }

    /// Move cursor up by `n` lines, clamping to 0.
    pub fn move_cursor_up(&mut self, n: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(n);
    }

    /// Jump to top of file.
    pub fn go_to_top(&mut self) {
        self.cursor_line = 0;
    }

    /// Jump to bottom of file.
    pub fn go_to_bottom(&mut self) {
        if self.total_lines > 0 {
            self.cursor_line = self.total_lines - 1;
        }
    }

    /// Ensure the cursor is visible by adjusting scroll_offset.
    /// `viewport_height` is the number of visible lines in the widget.
    pub fn ensure_cursor_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        let margin = 3.min(viewport_height / 2);
        if self.cursor_line < self.scroll_offset + margin {
            self.scroll_offset = self.cursor_line.saturating_sub(margin);
        } else if self.cursor_line >= self.scroll_offset + viewport_height - margin {
            self.scroll_offset = self
                .cursor_line
                .saturating_sub(viewport_height - margin - 1);
        }
    }

    /// Get the selection range (start, end) inclusive, 0-indexed. None if no selection.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_line);
            let end = anchor.max(self.cursor_line);
            (start, end)
        })
    }
}
