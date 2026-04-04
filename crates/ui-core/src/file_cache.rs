use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A simple in-memory cache of file contents keyed by path.
///
/// Both the TUI and GUI crates maintain this to avoid re-reading files from
/// disk on every frame.
#[derive(Default)]
pub struct FileCache {
    entries: HashMap<PathBuf, String>,
}

impl FileCache {
    /// Get the content of a file, reading from disk on first access.
    /// Returns `None` if the file cannot be read.
    pub fn get_or_load(&mut self, path: &Path) -> Option<&str> {
        if !self.entries.contains_key(path) {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    self.entries.insert(path.to_path_buf(), content);
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "failed to read file");
                    return None;
                }
            }
        }
        self.entries.get(path).map(|s| s.as_str())
    }

    /// Get cached content without loading from disk.
    pub fn get(&self, path: &Path) -> Option<&str> {
        self.entries.get(path).map(|s| s.as_str())
    }

    /// Insert or replace a file's content.
    pub fn insert(&mut self, path: PathBuf, content: String) {
        self.entries.insert(path, content);
    }

    /// Check if a file is cached.
    pub fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
