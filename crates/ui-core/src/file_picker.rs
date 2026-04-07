use std::path::PathBuf;

/// State for the fuzzy file picker popup.
///
/// This is the shared, frontend-agnostic file picker state. Both the TUI
/// and GUI use this for query management, cursor navigation, and lazy loading
/// of git-tracked files.
#[derive(Default)]
pub struct FilePickerState {
    /// Whether the file picker is open.
    pub open: bool,
    /// Current filter query.
    pub query: String,
    /// Selected cursor index in filtered results.
    pub cursor: usize,
    /// Cached git file list.
    pub git_files: Vec<fuzzy::TrackedFile>,
    /// Whether git files have been loaded.
    pub git_files_loaded: bool,
    /// Current filtered results.
    pub results: Vec<fuzzy::FuzzyMatch>,
}

impl FilePickerState {
    /// Ensure git files are loaded (lazy).
    ///
    /// The `debug_root_dir` is used as the primary root to list git files from.
    /// If listing fails, falls back to `fuzzy::find_repo_root()`.
    pub fn ensure_loaded(&mut self, debug_root_dir: &std::path::Path) {
        if self.git_files_loaded {
            return;
        }
        self.git_files_loaded = true;
        match fuzzy::list_git_files(debug_root_dir) {
            Ok(files) => self.git_files = files,
            Err(e) => {
                tracing::warn!(error = %e, "failed to list git files");
                if let Some(root) = fuzzy::find_repo_root()
                    && let Ok(files) = fuzzy::list_git_files(&root)
                {
                    self.git_files = files;
                }
            }
        }
        self.refilter();
    }

    /// Recompute filtered results from current query.
    pub fn refilter(&mut self) {
        self.results = fuzzy::fuzzy_filter(&self.git_files, &self.query);
        // Clamp cursor
        if self.results.is_empty() {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(self.results.len() - 1);
        }
    }

    /// Move cursor down.
    pub fn cursor_down(&mut self) {
        if !self.results.is_empty() {
            self.cursor = (self.cursor + 1).min(self.results.len() - 1);
        }
    }

    /// Move cursor up.
    pub fn cursor_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Select the current item and close the picker. Returns the selected path if any.
    pub fn select(&mut self) -> Option<PathBuf> {
        if self.results.is_empty() {
            return None;
        }
        let path = self.results[self.cursor].file.absolute_path.clone();
        self.close();
        Some(path)
    }

    /// Close picker and reset query.
    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.cursor = 0;
    }

    /// Open the picker.
    pub fn open(&mut self, debug_root_dir: &std::path::Path) {
        self.open = true;
        self.query.clear();
        self.cursor = 0;
        self.ensure_loaded(debug_root_dir);
        self.refilter();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked(name: &str) -> fuzzy::TrackedFile {
        fuzzy::TrackedFile {
            relative_path: PathBuf::from(name),
            absolute_path: PathBuf::from(format!("/project/{name}")),
        }
    }

    fn picker_with_files(files: &[&str]) -> FilePickerState {
        let git_files: Vec<fuzzy::TrackedFile> = files.iter().map(|n| tracked(n)).collect();
        let results: Vec<fuzzy::FuzzyMatch> = git_files
            .iter()
            .map(|f| fuzzy::FuzzyMatch {
                file: f.clone(),
                score: 0,
                matched_indices: vec![],
            })
            .collect();
        FilePickerState {
            open: true,
            query: String::new(),
            cursor: 0,
            git_files,
            git_files_loaded: true,
            results,
        }
    }

    // ── close ────────────────────────────────────────────────────────

    #[test]
    fn close_resets_state() {
        let mut fp = picker_with_files(&["a.py", "b.py"]);
        fp.query = "test".to_string();
        fp.cursor = 1;

        fp.close();

        assert!(!fp.open);
        assert!(fp.query.is_empty());
        assert_eq!(fp.cursor, 0);
    }

    // ── cursor navigation ────────────────────────────────────────────

    #[test]
    fn cursor_down_increments() {
        let mut fp = picker_with_files(&["a.py", "b.py", "c.py"]);
        assert_eq!(fp.cursor, 0);
        fp.cursor_down();
        assert_eq!(fp.cursor, 1);
        fp.cursor_down();
        assert_eq!(fp.cursor, 2);
    }

    #[test]
    fn cursor_down_clamps_at_last() {
        let mut fp = picker_with_files(&["a.py", "b.py"]);
        fp.cursor_down();
        fp.cursor_down();
        fp.cursor_down(); // past end
        assert_eq!(fp.cursor, 1);
    }

    #[test]
    fn cursor_down_on_empty_is_noop() {
        let mut fp = picker_with_files(&[]);
        fp.cursor_down();
        assert_eq!(fp.cursor, 0);
    }

    #[test]
    fn cursor_up_decrements() {
        let mut fp = picker_with_files(&["a.py", "b.py", "c.py"]);
        fp.cursor = 2;
        fp.cursor_up();
        assert_eq!(fp.cursor, 1);
        fp.cursor_up();
        assert_eq!(fp.cursor, 0);
    }

    #[test]
    fn cursor_up_clamps_at_zero() {
        let mut fp = picker_with_files(&["a.py"]);
        fp.cursor_up();
        assert_eq!(fp.cursor, 0);
    }

    // ── select ───────────────────────────────────────────────────────

    #[test]
    fn select_returns_path_and_closes() {
        let mut fp = picker_with_files(&["src/main.py", "src/lib.py"]);
        fp.cursor = 1;
        let path = fp.select();
        assert_eq!(path, Some(PathBuf::from("/project/src/lib.py")));
        assert!(!fp.open);
    }

    #[test]
    fn select_returns_none_when_empty() {
        let mut fp = picker_with_files(&[]);
        assert_eq!(fp.select(), None);
    }

    // ── refilter ─────────────────────────────────────────────────────

    #[test]
    fn refilter_clamps_cursor_when_results_shrink() {
        let mut fp = picker_with_files(&["aaa.py", "bbb.py", "ccc.py"]);
        fp.cursor = 2;

        // Simulate filtering to fewer results
        fp.query = "aaa".to_string();
        fp.refilter();

        // Cursor should be clamped
        assert!(fp.cursor < fp.results.len() || fp.results.is_empty());
    }

    #[test]
    fn refilter_with_empty_query_shows_all() {
        let mut fp = picker_with_files(&["a.py", "b.py"]);
        fp.query.clear();
        fp.refilter();
        // fuzzy_filter with empty query returns all files
        assert_eq!(fp.results.len(), 2);
    }

    // ── default ──────────────────────────────────────────────────────

    #[test]
    fn default_state_is_closed_and_empty() {
        let fp = FilePickerState::default();
        assert!(!fp.open);
        assert!(fp.query.is_empty());
        assert_eq!(fp.cursor, 0);
        assert!(fp.git_files.is_empty());
        assert!(!fp.git_files_loaded);
        assert!(fp.results.is_empty());
    }
}
