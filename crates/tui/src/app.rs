use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::KeyEvent;
use launch_configuration::LaunchConfiguration;

use crate::event::AppEvent;

/// The current mode of the application.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Variants used as phases are implemented
pub enum AppMode {
    /// No debug session is active.
    NoSession,
    /// A debug session is starting up.
    Initialising,
    /// The debugee is running.
    Running,
    /// The debugee is paused at a breakpoint or step.
    Paused,
    /// The debugee has terminated.
    Terminated,
}

/// Which pane currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    CodeView,
    CallStack,
    Breakpoints,
    Variables,
    Output,
    Repl,
}

impl Focus {
    const ORDER: &[Focus] = &[
        Focus::CodeView,
        Focus::CallStack,
        Focus::Breakpoints,
        Focus::Variables,
        Focus::Output,
        Focus::Repl,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        Self::ORDER[(idx + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }
}

/// Which tab is visible in the bottom panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Variables,
    Output,
    Repl,
}

/// Whether the app is currently capturing text input (suppresses normal keybindings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal keybindings active.
    Normal,
    /// Typing into file picker search.
    FilePicker,
    /// Typing into code view search.
    Search,
}

// ── Code view state ───────────────────────────────────────────────────────

/// State for the source code viewer.
pub struct CodeViewState {
    /// Currently displayed file path.
    pub file_path: Option<PathBuf>,
    /// Cursor line (0-indexed) -- the line the user is "on" for navigation.
    pub cursor_line: usize,
    /// Scroll offset (first visible line, 0-indexed).
    pub scroll_offset: usize,
    /// Total lines in the current file.
    pub total_lines: usize,
}

impl Default for CodeViewState {
    fn default() -> Self {
        Self {
            file_path: None,
            cursor_line: 0,
            scroll_offset: 0,
            total_lines: 0,
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
}

// ── Search state ──────────────────────────────────────────────────────────

/// State for in-file text search.
pub struct SearchState {
    /// Whether the search bar is visible.
    pub active: bool,
    /// Current query text.
    pub query: String,
    /// Cached matches: (line_index_0based, byte_start_in_line, byte_len).
    pub matches: Vec<(usize, usize, usize)>,
    /// Index of the current highlighted match.
    pub current_match: usize,
    /// The query used to compute the current matches (cache key).
    last_query: String,
    /// The file path used to compute the current matches (cache key).
    last_file: PathBuf,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            active: false,
            query: String::new(),
            matches: Vec::new(),
            current_match: 0,
            last_query: String::new(),
            last_file: PathBuf::new(),
        }
    }
}

impl SearchState {
    /// Recompute matches if the query or file changed.
    pub fn update(&mut self, content: &str, file_path: &PathBuf) {
        if self.query == self.last_query && file_path == &self.last_file {
            return;
        }
        self.last_query = self.query.clone();
        self.last_file = file_path.clone();
        self.matches.clear();
        self.current_match = 0;

        if self.query.is_empty() {
            return;
        }

        let query_lower = self.query.to_lowercase();
        for (line_idx, line) in content.lines().enumerate() {
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let byte_start = start + pos;
                self.matches.push((line_idx, byte_start, self.query.len()));
                start = byte_start + 1;
            }
        }
    }

    /// Navigate to next match.
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to previous match.
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.matches.len() - 1
            } else {
                self.current_match - 1
            };
        }
    }

    /// Get the line (0-indexed) of the current match, if any.
    pub fn current_match_line(&self) -> Option<usize> {
        self.matches
            .get(self.current_match)
            .map(|&(line, _, _)| line)
    }

    /// Reset when file changes.
    pub fn reset(&mut self) {
        self.last_query.clear();
        self.last_file = PathBuf::new();
        self.matches.clear();
        self.current_match = 0;
    }
}

// ── File picker state ─────────────────────────────────────────────────────

/// State for the fuzzy file picker popup.
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
    pub fn ensure_loaded(&mut self) {
        if self.git_files_loaded {
            return;
        }
        self.git_files_loaded = true;
        if let Some(root) = fuzzy::find_repo_root() {
            match fuzzy::list_git_files(&root) {
                Ok(files) => self.git_files = files,
                Err(e) => tracing::warn!(error = %e, "failed to list git files"),
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
    pub fn open(&mut self) {
        self.open = true;
        self.query.clear();
        self.cursor = 0;
        self.ensure_loaded();
        self.refilter();
    }
}

// ── Main app struct ───────────────────────────────────────────────────────

#[allow(dead_code)] // Fields used as phases are implemented
pub struct App {
    pub mode: AppMode,
    pub focus: Focus,
    pub bottom_tab: BottomTab,
    pub input_mode: InputMode,
    pub should_quit: bool,

    // Configuration
    pub configs: Vec<LaunchConfiguration>,
    pub config_names: Vec<String>,
    pub selected_config_index: usize,
    pub config_path: PathBuf,
    pub debug_root_dir: PathBuf,

    // File content cache: path -> content
    pub file_cache: HashMap<PathBuf, String>,

    // Code view
    pub code_view: CodeViewState,

    // Search
    pub search: SearchState,

    // File picker
    pub file_picker: FilePickerState,

    // Help overlay
    pub show_help: bool,
}

impl App {
    pub fn new(
        configs: Vec<LaunchConfiguration>,
        config_names: Vec<String>,
        selected_config_index: usize,
        config_path: PathBuf,
        debug_root_dir: PathBuf,
    ) -> Self {
        Self {
            mode: AppMode::NoSession,
            focus: Focus::CodeView,
            bottom_tab: BottomTab::Variables,
            input_mode: InputMode::Normal,
            should_quit: false,
            configs,
            config_names,
            selected_config_index,
            config_path,
            debug_root_dir,
            file_cache: HashMap::new(),
            code_view: CodeViewState::default(),
            search: SearchState::default(),
            file_picker: FilePickerState::default(),
            show_help: false,
        }
    }

    /// Process a single event.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Resize(_, _) => {} // ratatui handles resize automatically
            AppEvent::Tick => {}         // triggers a redraw
            AppEvent::Mouse(_) => {}     // mouse support later
            AppEvent::Debugger(_) => {}  // wired up in Phase 3
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        crate::input::handle_key(self, key);
    }

    /// Open a file in the code view. Loads from cache or reads from disk.
    pub fn open_file(&mut self, path: PathBuf) {
        // Load content if not cached
        if !self.file_cache.contains_key(&path) {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.file_cache.insert(path.clone(), content);
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "failed to read file");
                    return;
                }
            }
        }

        let content = &self.file_cache[&path];
        let total_lines = content.lines().count();
        self.code_view.open_file(path, total_lines);
        self.search.reset();
    }

    /// Get the current file content, if a file is open.
    pub fn current_file_content(&self) -> Option<&str> {
        let path = self.code_view.file_path.as_ref()?;
        self.file_cache.get(path).map(|s| s.as_str())
    }
}
