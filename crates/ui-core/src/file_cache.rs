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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn get_or_load_reads_file_and_caches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.py");
        std::fs::write(&path, "print('hello')").unwrap();

        let mut cache = FileCache::default();
        let content = cache.get_or_load(&path);
        assert_eq!(content, Some("print('hello')"));

        // Should now be cached
        assert!(cache.contains(&path));
    }

    #[test]
    fn get_or_load_returns_none_for_missing_file() {
        let mut cache = FileCache::default();
        let result = cache.get_or_load(Path::new("/nonexistent/file.py"));
        assert_eq!(result, None);
    }

    #[test]
    fn get_returns_none_when_not_cached() {
        let cache = FileCache::default();
        assert_eq!(cache.get(Path::new("/foo.py")), None);
    }

    #[test]
    fn get_returns_content_after_get_or_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.py");
        std::fs::write(&path, "x = 1").unwrap();

        let mut cache = FileCache::default();
        cache.get_or_load(&path);
        assert_eq!(cache.get(&path), Some("x = 1"));
    }

    #[test]
    fn insert_makes_content_available() {
        let mut cache = FileCache::default();
        let path = PathBuf::from("/virtual/file.py");
        cache.insert(path.clone(), "content".to_string());
        assert_eq!(cache.get(&path), Some("content"));
        assert!(cache.contains(&path));
    }

    #[test]
    fn contains_returns_false_for_unknown_path() {
        let cache = FileCache::default();
        assert!(!cache.contains(Path::new("/unknown.py")));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cache = FileCache::default();
        cache.insert(PathBuf::from("/a.py"), "a".to_string());
        cache.insert(PathBuf::from("/b.py"), "b".to_string());
        assert!(cache.contains(Path::new("/a.py")));

        cache.clear();
        assert!(!cache.contains(Path::new("/a.py")));
        assert!(!cache.contains(Path::new("/b.py")));
    }

    #[test]
    fn get_or_load_caches_on_first_access_returns_same_on_second() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.py");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "line1").unwrap();
        drop(f);

        let mut cache = FileCache::default();
        let first = cache.get_or_load(&path).unwrap().to_string();

        // Modify the file on disk -- cache should return the old content
        std::fs::write(&path, "modified").unwrap();
        let second = cache.get_or_load(&path).unwrap().to_string();
        assert_eq!(first, second);
    }
}
