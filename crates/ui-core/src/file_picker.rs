use std::path::PathBuf;

/// State for the fuzzy file picker popup.
///
/// This is the shared, frontend-agnostic file picker state. Both the TUI
/// and GUI use this for query management, cursor navigation, and lazy loading
/// of git-tracked files.
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

impl Default for FilePickerState {
    fn default() -> Self {
        Self {
            open: false,
            query: String::new(),
            cursor: 0,
            git_files: Vec::new(),
            git_files_loaded: false,
            results: Vec::new(),
        }
    }
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
                if let Some(root) = fuzzy::find_repo_root() {
                    if let Ok(files) = fuzzy::list_git_files(&root) {
                        self.git_files = files;
                    }
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
