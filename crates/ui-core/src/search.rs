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
        let query_len_bytes = query_lower.len();

        // Track absolute byte position in the original content, including
        // the actual newline bytes (\n or \r\n) that `lines()` strips.
        let mut line_start_byte = 0usize;
        for (line_idx, line) in content.lines().enumerate() {
            let line_lower = line.to_lowercase();

            // Build a mapping from byte position in `line_lower` back to
            // byte position in the original `line`. This is necessary because
            // `to_lowercase()` can change the byte length of characters
            // (e.g. İ U+0130 is 2 bytes, its lowercase i\u{307} is 3 bytes).
            let lower_to_orig = build_lower_to_orig_map(line, &line_lower);

            for (lower_pos, _) in line_lower.match_indices(&query_lower) {
                let orig_start = lower_to_orig[lower_pos];
                let orig_end = if lower_pos + query_len_bytes < lower_to_orig.len() {
                    lower_to_orig[lower_pos + query_len_bytes]
                } else {
                    line.len()
                };
                self.matches.push(SearchMatch {
                    line: line_idx,
                    byte_start_in_line: orig_start,
                    byte_offset: line_start_byte + orig_start,
                    length: orig_end - orig_start,
                });
            }

            // Advance past the line content and the actual newline bytes.
            // `content.lines()` strips both \n and \r\n, so we must account
            // for the real newline length to keep byte_offset correct.
            line_start_byte += line.len();
            let rest = &content[line_start_byte..];
            if rest.starts_with("\r\n") {
                line_start_byte += 2;
            } else if rest.starts_with('\n') {
                line_start_byte += 1;
            }
            // else: last line with no trailing newline -- nothing to add
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

/// Build a mapping from byte index in `lower` to byte index in `orig`.
///
/// `lower` must be `orig.to_lowercase()`. For each byte position `i` in
/// `lower`, `result[i]` gives the byte position in `orig` of the original
/// character that lowercased to include byte `i`.
///
/// This is necessary because `to_lowercase()` can change byte lengths:
/// e.g. `İ` (U+0130, 2 bytes) lowercases to `i\u{307}` (3 bytes).
fn build_lower_to_orig_map(orig: &str, lower: &str) -> Vec<usize> {
    let mut map = Vec::with_capacity(lower.len() + 1);
    let mut orig_chars = orig.char_indices().peekable();
    let mut lower_chars = lower.chars().peekable();
    while lower_chars.peek().is_some() {
        // Advance one original character
        let (orig_byte, orig_ch) = match orig_chars.next() {
            Some(pair) => pair,
            None => break,
        };

        // The lowercase expansion of `orig_ch`
        let mut expanded_byte_count = 0usize;
        for lower_ch in orig_ch.to_lowercase() {
            let consumed = match lower_chars.next() {
                Some(c) => {
                    debug_assert_eq!(c, lower_ch, "mismatch between to_lowercase() expansions");
                    c.len_utf8()
                }
                None => break,
            };
            // Each byte in this expansion maps back to `orig_byte`
            for _ in 0..consumed {
                map.push(orig_byte);
            }
            expanded_byte_count += consumed;
            let _ = expanded_byte_count; // suppress unused warning
        }
    }

    // Sentinel entry so `map[lower.len()]` == `orig.len()` for end-of-match lookups
    map.push(orig.len());
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_state(query: &str) -> SearchState {
        let mut s = SearchState::default();
        s.query = query.to_string();
        s
    }

    #[test]
    fn basic_ascii_search() {
        let mut s = make_state("hello");
        let content = "say hello world\nhello again";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 2);

        // First match: line 0, byte 4
        assert_eq!(s.matches[0].line, 0);
        assert_eq!(s.matches[0].byte_start_in_line, 4);
        assert_eq!(s.matches[0].byte_offset, 4);
        assert_eq!(s.matches[0].length, 5);
        assert_eq!(&content[4..9], "hello");

        // Second match: line 1, byte 0
        assert_eq!(s.matches[1].line, 1);
        assert_eq!(s.matches[1].byte_start_in_line, 0);
        assert_eq!(s.matches[1].byte_offset, 16); // 15 chars + \n
        assert_eq!(s.matches[1].length, 5);
    }

    #[test]
    fn case_insensitive_ascii() {
        let mut s = make_state("Hello");
        let content = "HELLO world";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].byte_start_in_line, 0);
        assert_eq!(s.matches[0].length, 5);
        // byte offsets refer to original content
        assert_eq!(&content[0..5], "HELLO");
    }

    #[test]
    fn crlf_byte_offsets() {
        let mut s = make_state("b");
        let content = "a\r\nb\r\nc";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].line, 1);
        assert_eq!(s.matches[0].byte_start_in_line, 0);
        // "a" is 1 byte + \r\n is 2 bytes = byte offset 3
        assert_eq!(s.matches[0].byte_offset, 3);
        assert_eq!(&content[3..4], "b");
    }

    #[test]
    fn multibyte_utf8_offsets() {
        let mut s = make_state("world");
        // "café" is 5 bytes (é = 2 bytes), then " world"
        let content = "café world";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);
        // "café " = c(1) + a(1) + f(1) + é(2) + space(1) = 6 bytes
        assert_eq!(s.matches[0].byte_start_in_line, 6);
        assert_eq!(s.matches[0].byte_offset, 6);
        assert_eq!(s.matches[0].length, 5);
        assert_eq!(&content[6..11], "world");
    }

    #[test]
    fn case_fold_with_byte_length_change() {
        // İ (U+0130, Latin Capital Letter I With Dot Above) is 2 bytes.
        // Its lowercase is "i\u{307}" which is 3 bytes.
        // We search for text AFTER this character to verify offsets are correct.
        let mut s = make_state("end");
        let content = "İ end";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);
        // İ is 2 bytes + space is 1 byte = offset 3
        assert_eq!(s.matches[0].byte_start_in_line, 3);
        assert_eq!(s.matches[0].byte_offset, 3);
        assert_eq!(s.matches[0].length, 3);
        assert_eq!(&content[3..6], "end");
    }

    #[test]
    fn navigation_wraps() {
        let mut s = make_state("a");
        let content = "a b a";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.current_match, 0);

        s.next_match();
        assert_eq!(s.current_match, 1);

        s.next_match();
        assert_eq!(s.current_match, 0); // wraps

        s.prev_match();
        assert_eq!(s.current_match, 1); // wraps back
    }

    #[test]
    fn empty_query_clears_matches() {
        let mut s = make_state("a");
        let content = "abc";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);

        s.query = String::new();
        s.update(content, Path::new("test2.py")); // different file to bust cache
        assert_eq!(s.matches.len(), 0);
    }

    #[test]
    fn no_trailing_newline() {
        let mut s = make_state("end");
        let content = "start\nend";
        s.update(content, Path::new("test.py"));
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].line, 1);
        assert_eq!(s.matches[0].byte_offset, 6); // "start" + \n
        assert_eq!(&content[6..9], "end");
    }

    #[test]
    fn build_lower_to_orig_map_ascii() {
        let orig = "Hello";
        let lower = orig.to_lowercase();
        let map = build_lower_to_orig_map(orig, &lower);
        // ASCII: 1:1 mapping
        assert_eq!(map, vec![0, 1, 2, 3, 4, 5]); // 5 entries + sentinel
    }

    #[test]
    fn build_lower_to_orig_map_expansion() {
        // İ (2 bytes) lowercases to "i\u{307}" (3 bytes)
        let orig = "İx";
        let lower = orig.to_lowercase();
        assert_eq!(lower, "i\u{307}x");
        let map = build_lower_to_orig_map(orig, &lower);
        // İ at orig byte 0, expands to 3 bytes in lower: [0, 0, 0]
        // x at orig byte 2, 1 byte in lower: [2]
        // sentinel: [3]
        assert_eq!(map, vec![0, 0, 0, 2, 3]);
    }
}
