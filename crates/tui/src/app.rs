use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crossterm::event::KeyEvent;
use launch_configuration::LaunchConfiguration;
use state::StateManager;

use crate::async_bridge::UiCommand;
use crate::event::AppEvent;
use crate::session::Session;

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
    #[allow(dead_code)] // Used in Phase 5 for session restart
    pub config_path: PathBuf,
    pub debug_root_dir: PathBuf,

    // Session
    pub session: Option<Session>,
    /// Wakeup sender passed to new sessions so they can nudge the event loop.
    pub wakeup_tx: crossbeam_channel::Sender<()>,

    // UI breakpoints (survive across sessions)
    pub ui_breakpoints: HashSet<debugger::Breakpoint>,

    // Output buffer: (category, text)
    pub output_lines: Vec<(String, String)>,

    // Thread info: (thread_id, reason)
    pub threads: Vec<(i64, String)>,

    // Variables child cache: variables_reference -> children
    pub variables_cache: HashMap<i64, Vec<dap_types::Variable>>,

    // Persistence
    pub state_manager: StateManager,

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

    // REPL
    pub repl_input: String,
    pub repl_history: Vec<(String, String, bool)>, // (input, output, is_error)

    // Status line messages
    pub status_message: Option<String>,
    pub status_error: Option<String>,
}

impl App {
    pub fn new(
        configs: Vec<LaunchConfiguration>,
        config_names: Vec<String>,
        selected_config_index: usize,
        config_path: PathBuf,
        debug_root_dir: PathBuf,
        state_manager: StateManager,
        wakeup_tx: crossbeam_channel::Sender<()>,
        initial_breakpoints: Vec<debugger::Breakpoint>,
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
            session: None,
            wakeup_tx,
            ui_breakpoints: initial_breakpoints.into_iter().collect(),
            output_lines: Vec::new(),
            threads: Vec::new(),
            variables_cache: HashMap::new(),
            state_manager,
            file_cache: HashMap::new(),
            code_view: CodeViewState::default(),
            search: SearchState::default(),
            file_picker: FilePickerState::default(),
            show_help: false,
            repl_input: String::new(),
            repl_history: Vec::new(),
            status_message: None,
            status_error: None,
        }
    }

    /// Process a single event.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Resize(_, _) => {} // ratatui handles resize automatically
            AppEvent::Tick => self.drain_debugger_events(),
            AppEvent::Mouse(_) => {}
            AppEvent::Debugger(_) => {} // events arrive via session channel, drained on tick
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        crate::input::handle_key(self, key);
    }

    /// Drain all pending debugger events from the session's channel.
    pub fn drain_debugger_events(&mut self) {
        // Collect events and errors while briefly borrowing the session
        let (events, errors) = {
            let Some(session) = &mut self.session else {
                return;
            };
            let events: Vec<debugger::Event> = session.debugger_event_rx.try_iter().collect();
            let errors = session.bridge.drain_errors();
            (events, errors)
        };

        for err in errors {
            tracing::error!(operation = err.operation, error = %err.error, "bridge error");
            self.status_error = Some(format!("{}: {}", err.operation, err.error));
        }

        for event in &events {
            // Update session state (borrows self.session briefly)
            if let Some(session) = &mut self.session {
                session.handle_event(event);
            }

            // Handle event side effects (borrows self freely)
            match event {
                debugger::Event::Paused(state) => {
                    self.mode = AppMode::Paused;
                    self.ui_breakpoints = state.breakpoints.iter().cloned().collect();
                    self.variables_cache.clear();
                    self.jump_to_execution_line(state);
                }
                debugger::Event::ScopeChange(state) => {
                    self.mode = AppMode::Paused;
                    self.ui_breakpoints = state.breakpoints.iter().cloned().collect();
                    self.variables_cache.clear();
                    self.jump_to_execution_line(state);
                }
                debugger::Event::Running => {
                    self.mode = AppMode::Running;
                    self.status_message = Some("Running...".to_string());
                }
                debugger::Event::Initialised => {
                    self.mode = AppMode::Running;
                    self.status_message = Some("Debugger initialised".to_string());
                }
                debugger::Event::Ended => {
                    self.mode = AppMode::Terminated;
                    self.status_message = Some("Debugee terminated".to_string());
                }
                debugger::Event::Output { category, output } => {
                    self.output_lines.push((category.clone(), output.clone()));
                }
                debugger::Event::Thread { reason, thread_id } => match reason.as_str() {
                    "started" => {
                        self.threads.push((*thread_id, reason.clone()));
                    }
                    "exited" => {
                        self.threads.retain(|(id, _)| *id != *thread_id);
                    }
                    _ => {}
                },
                debugger::Event::Error(msg) => {
                    tracing::error!(msg, "debugger error event");
                    self.status_error = Some(msg.clone());
                }
                debugger::Event::Uninitialised => {}
            }
        }
    }

    /// Jump code view to the current execution line.
    fn jump_to_execution_line(&mut self, state: &debugger::ProgramState) {
        let frame = &state.paused_frame.frame;
        if let Some(source) = &frame.source {
            if let Some(path) = &source.path {
                // Open the file if it's different from the current one
                if self.code_view.file_path.as_ref() != Some(path) {
                    self.open_file(path.clone());
                }
                // Jump cursor to execution line (DAP lines are 1-indexed)
                let line = (frame.line as usize).saturating_sub(1);
                self.code_view.cursor_line = line;
            }
        }
    }

    /// Start a debug session with the currently selected configuration.
    pub fn start_session(&mut self) {
        if self.session.is_some() {
            self.status_error = Some("Session already active".to_string());
            return;
        }

        if self.configs.is_empty() {
            self.status_error = Some("No launch configurations available".to_string());
            return;
        }

        let config = self.configs[self.selected_config_index].clone();
        let breakpoints: Vec<debugger::Breakpoint> = self.collect_all_breakpoints();

        self.mode = AppMode::Initialising;
        self.status_message = Some("Starting debug session...".to_string());
        self.status_error = None;
        self.output_lines.clear();
        self.threads.clear();
        self.variables_cache.clear();

        match Session::start(
            &config,
            &breakpoints,
            &mut self.debug_root_dir,
            self.wakeup_tx.clone(),
        ) {
            Ok(session) => {
                self.session = Some(session);
                self.mode = AppMode::Running;
                self.status_message = Some("Session started".to_string());
            }
            Err(e) => {
                self.mode = AppMode::NoSession;
                self.status_error = Some(format!("Failed to start session: {e}"));
                tracing::error!(error = %e, "failed to start debug session");
            }
        }
    }

    /// Stop the current debug session.
    pub fn stop_session(&mut self) {
        if let Some(session) = &self.session {
            session.bridge.send(UiCommand::Terminate);
        }
    }

    /// Shut down the current debug session and clean up.
    pub fn shutdown_session(&mut self) {
        if let Some(session) = &self.session {
            session.bridge.send(UiCommand::Shutdown);
        }
        self.session = None;
        self.mode = AppMode::NoSession;
        self.status_message = Some("Session ended".to_string());
    }

    /// Collect all breakpoints (UI + persisted) for session start.
    fn collect_all_breakpoints(&self) -> Vec<debugger::Breakpoint> {
        let mut bps: Vec<debugger::Breakpoint> = self.ui_breakpoints.iter().cloned().collect();

        // Add persisted breakpoints
        if let Some(project_state) = self
            .state_manager
            .current()
            .projects
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == self.debug_root_dir)
        {
            for bp in &project_state.breakpoints {
                let normalised = debugger::utils::normalise_path(&bp.path).into_owned();
                let mut bp = bp.clone();
                bp.path = std::fs::canonicalize(&normalised).unwrap_or(normalised);
                if !bps.contains(&bp) {
                    bps.push(bp);
                }
            }
        }

        bps
    }

    /// Persist current breakpoints to state file.
    #[allow(dead_code)] // Used in Phase 4 for breakpoint toggle
    pub fn persist_breakpoints(&mut self) {
        let breakpoints: Vec<_> = self.ui_breakpoints.iter().cloned().collect();
        if let Err(e) = self
            .state_manager
            .set_project_breakpoints(self.debug_root_dir.clone(), breakpoints)
        {
            tracing::warn!(error = %e, "failed to persist breakpoints");
        }
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
