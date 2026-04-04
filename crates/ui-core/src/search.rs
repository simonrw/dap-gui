use std::path::PathBuf;

/// A single search match with both per-line and absolute byte offsets.
///
/// Both coordinate systems are stored so that either rendering backend
/// can use whichever is most natural (ratatui uses per-line, egui uses absolute).
#[derive(Clone, Debug)]
pub struct SearchMatch {
    /// 0-based line index.
    pub line: usize,
    /// Byte offset of the match start *within the line*.
    pub byte_start_in_line: usize,
    /// Byte offset of the match start in the full content string.
    pub byte_offset: usize,
    /// Length of the match in bytes.
    pub length: usize,
}

/// State for in-file text search.
///
/// Shared between TUI and GUI. Each frontend can add extra rendering-specific
/// fields on its own (e.g. `request_focus`, `scroll_to_match` for the GUI).
pub struct SearchState {
    /// Whether the search bar is visible.
    pub active: bool,
    /// Current query text.
    pub query: String,
    /// Cached matches.
    pub matches: Vec<SearchMatch>,
    /// Index of the currently highlighted match.
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
    ///
    /// This performs a case-insensitive search and stores both per-line
    /// and absolute byte offsets for each match.
    pub fn update(&mut self, content: &str, file_path: &std::path::Path) -> bool {
        if self.query == self.last_query && file_path == self.last_file {
            return false;
        }
        self.last_query = self.query.clone();
        self.last_file = file_path.to_path_buf();
        self.matches.clear();
        self.current_match = 0;

        if self.query.is_empty() {
            return true;
        }

        let query_lower = self.query.to_lowercase();
        let mut line_start_byte = 0usize;
        for (line_idx, line) in content.lines().enumerate() {
            let line_lower = line.to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let byte_start = start + pos;
                self.matches.push(SearchMatch {
                    line: line_idx,
                    byte_start_in_line: byte_start,
                    byte_offset: line_start_byte + byte_start,
                    length: self.query.len(),
                });
                start = byte_start + 1;
            }
            // +1 for the newline character
            line_start_byte += line.len() + 1;
        }
        true
    }

    /// Navigate to the next match (wrapping).
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match (wrapping).
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
        self.matches.get(self.current_match).map(|m| m.line)
    }

    /// Reset the cache (e.g. when the file changes). The next `update()` call
    /// will recompute matches.
    pub fn reset(&mut self) {
        self.last_query.clear();
        self.last_file = PathBuf::new();
        self.matches.clear();
        self.current_match = 0;
    }
}
