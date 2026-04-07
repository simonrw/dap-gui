use std::path::PathBuf;

/// State for the code view panel.
///
/// Tracks the currently displayed file, cursor position, scroll offset,
/// and visual selection. This is primarily used by the TUI (keyboard-driven
/// navigation) but is kept in ui-core for reuse.
#[derive(Default)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn cv_with_file(total_lines: usize) -> CodeViewState {
        let mut cv = CodeViewState::default();
        cv.open_file(PathBuf::from("/tmp/test.py"), total_lines);
        cv
    }

    // ── open_file ────────────────────────────────────────────────────

    #[test]
    fn open_file_sets_path_and_total_lines() {
        let mut cv = CodeViewState::default();
        cv.open_file(PathBuf::from("/project/main.py"), 42);
        assert_eq!(cv.file_path.as_deref(), Some(Path::new("/project/main.py")));
        assert_eq!(cv.total_lines, 42);
    }

    #[test]
    fn open_file_resets_cursor_and_scroll() {
        let mut cv = cv_with_file(100);
        cv.cursor_line = 50;
        cv.scroll_offset = 40;
        cv.selection_anchor = Some(45);

        cv.open_file(PathBuf::from("/other.py"), 20);
        assert_eq!(cv.cursor_line, 0);
        assert_eq!(cv.scroll_offset, 0);
        assert_eq!(cv.selection_anchor, None);
        assert_eq!(cv.total_lines, 20);
    }

    // ── move_cursor_down ─────────────────────────────────────────────

    #[test]
    fn move_cursor_down_basic() {
        let mut cv = cv_with_file(10);
        cv.move_cursor_down(3);
        assert_eq!(cv.cursor_line, 3);
    }

    #[test]
    fn move_cursor_down_clamps_to_last_line() {
        let mut cv = cv_with_file(10);
        cv.move_cursor_down(100);
        assert_eq!(cv.cursor_line, 9);
    }

    #[test]
    fn move_cursor_down_on_empty_file_is_noop() {
        let mut cv = cv_with_file(0);
        cv.move_cursor_down(5);
        assert_eq!(cv.cursor_line, 0);
    }

    #[test]
    fn move_cursor_down_from_last_line_stays() {
        let mut cv = cv_with_file(5);
        cv.cursor_line = 4;
        cv.move_cursor_down(1);
        assert_eq!(cv.cursor_line, 4);
    }

    // ── move_cursor_up ───────────────────────────────────────────────

    #[test]
    fn move_cursor_up_basic() {
        let mut cv = cv_with_file(10);
        cv.cursor_line = 5;
        cv.move_cursor_up(2);
        assert_eq!(cv.cursor_line, 3);
    }

    #[test]
    fn move_cursor_up_clamps_to_zero() {
        let mut cv = cv_with_file(10);
        cv.cursor_line = 3;
        cv.move_cursor_up(100);
        assert_eq!(cv.cursor_line, 0);
    }

    #[test]
    fn move_cursor_up_at_zero_stays() {
        let mut cv = cv_with_file(10);
        cv.move_cursor_up(1);
        assert_eq!(cv.cursor_line, 0);
    }

    // ── go_to_top / go_to_bottom ─────────────────────────────────────

    #[test]
    fn go_to_top() {
        let mut cv = cv_with_file(10);
        cv.cursor_line = 7;
        cv.go_to_top();
        assert_eq!(cv.cursor_line, 0);
    }

    #[test]
    fn go_to_bottom() {
        let mut cv = cv_with_file(10);
        cv.go_to_bottom();
        assert_eq!(cv.cursor_line, 9);
    }

    #[test]
    fn go_to_bottom_on_empty_file_stays_at_zero() {
        let mut cv = cv_with_file(0);
        cv.go_to_bottom();
        assert_eq!(cv.cursor_line, 0);
    }

    // ── ensure_cursor_visible ────────────────────────────────────────

    #[test]
    fn ensure_cursor_visible_zero_viewport_is_noop() {
        let mut cv = cv_with_file(100);
        cv.cursor_line = 50;
        cv.scroll_offset = 10;
        cv.ensure_cursor_visible(0);
        assert_eq!(cv.scroll_offset, 10); // unchanged
    }

    #[test]
    fn ensure_cursor_visible_scrolls_down_when_cursor_below_viewport() {
        let mut cv = cv_with_file(100);
        cv.cursor_line = 30;
        cv.scroll_offset = 0;
        cv.ensure_cursor_visible(20);
        // cursor should be within viewport; margin = 3
        // scroll_offset = cursor_line - (viewport_height - margin - 1) = 30 - 16 = 14
        assert!(cv.scroll_offset > 0);
        assert!(cv.cursor_line < cv.scroll_offset + 20);
    }

    #[test]
    fn ensure_cursor_visible_scrolls_up_when_cursor_above_viewport() {
        let mut cv = cv_with_file(100);
        cv.cursor_line = 5;
        cv.scroll_offset = 20;
        cv.ensure_cursor_visible(20);
        // scroll should have come down to make cursor visible
        assert!(cv.scroll_offset <= cv.cursor_line);
    }

    #[test]
    fn ensure_cursor_visible_no_change_when_cursor_in_viewport() {
        let mut cv = cv_with_file(100);
        cv.cursor_line = 15;
        cv.scroll_offset = 10;
        cv.ensure_cursor_visible(20);
        // cursor at 15, viewport 10..30, margin 3 -> cursor is at offset 5 from start
        // 15 >= 10 + 3 = 13 and 15 < 10 + 20 - 3 = 27, so no scroll change
        assert_eq!(cv.scroll_offset, 10);
    }

    #[test]
    fn ensure_cursor_visible_small_viewport_margin_clamped() {
        // viewport=4 => margin = min(3, 4/2) = 2
        let mut cv = cv_with_file(100);
        cv.cursor_line = 50;
        cv.scroll_offset = 0;
        cv.ensure_cursor_visible(4);
        // Should scroll so cursor is visible with margin=2
        assert!(cv.cursor_line >= cv.scroll_offset);
        assert!(cv.cursor_line < cv.scroll_offset + 4);
    }

    // ── selection_range ──────────────────────────────────────────────

    #[test]
    fn selection_range_none_when_no_anchor() {
        let cv = cv_with_file(10);
        assert_eq!(cv.selection_range(), None);
    }

    #[test]
    fn selection_range_anchor_before_cursor() {
        let mut cv = cv_with_file(10);
        cv.selection_anchor = Some(2);
        cv.cursor_line = 7;
        assert_eq!(cv.selection_range(), Some((2, 7)));
    }

    #[test]
    fn selection_range_anchor_after_cursor() {
        let mut cv = cv_with_file(10);
        cv.selection_anchor = Some(7);
        cv.cursor_line = 2;
        assert_eq!(cv.selection_range(), Some((2, 7)));
    }

    #[test]
    fn selection_range_anchor_equals_cursor() {
        let mut cv = cv_with_file(10);
        cv.selection_anchor = Some(5);
        cv.cursor_line = 5;
        assert_eq!(cv.selection_range(), Some((5, 5)));
    }
}
