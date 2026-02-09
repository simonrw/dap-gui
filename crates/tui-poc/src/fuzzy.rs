use std::path::{Path, PathBuf};
use std::process::Command;

use nucleo_matcher::{Config, Matcher};

/// A file tracked by Git.
#[derive(Clone, Debug)]
pub struct TrackedFile {
    /// Path relative to repository root.
    pub relative_path: PathBuf,
    /// Full filesystem path.
    pub absolute_path: PathBuf,
}

/// A file matching the fuzzy search query.
#[derive(Clone, Debug)]
pub struct FuzzyMatch {
    /// The matched file.
    pub file: TrackedFile,
    /// Match quality score (higher = better).
    pub score: i64,
    /// Character indices that matched (for highlighting).
    pub matched_indices: Vec<usize>,
}

/// Get the Git repository root directory.
/// Returns None if not in a Git repository.
pub fn find_repo_root() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }

    Some(PathBuf::from(root))
}

/// Enumerate all files tracked by Git in the repository.
/// Returns an error if git command fails or not in a repo.
pub fn list_git_files(repo_root: &Path) -> Result<Vec<TrackedFile>, std::io::Error> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("git ls-files failed: {stderr}"),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let relative_path = PathBuf::from(line);
        let absolute_path = repo_root.join(&relative_path);
        files.push(TrackedFile {
            relative_path,
            absolute_path,
        });
    }

    Ok(files)
}

/// Perform fuzzy matching on a list of files.
/// Returns matches sorted by score (descending), then alphabetically.
/// Empty query returns all files (no filtering).
pub fn fuzzy_filter(files: &[TrackedFile], query: &str) -> Vec<FuzzyMatch> {
    if query.is_empty() {
        return files
            .iter()
            .cloned()
            .map(|file| FuzzyMatch {
                file,
                score: 0,
                matched_indices: Vec::new(),
            })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut matches = Vec::new();

    // Allocate buffers for conversion to utf32
    let mut haystack_buf = Vec::new();
    let mut needle_buf = Vec::new();

    for file in files {
        let path_str = file.relative_path.to_string_lossy();

        haystack_buf.clear();
        let haystack = nucleo_matcher::Utf32Str::new(&path_str, &mut haystack_buf);

        needle_buf.clear();
        let needle = nucleo_matcher::Utf32Str::new(query, &mut needle_buf);

        if let Some(score) = matcher.fuzzy_match(haystack, needle) {
            if score > 0 {
                let mut indices = Vec::new();
                matcher.fuzzy_indices(haystack, needle, &mut indices);

                // Convert u32 indices to usize for matched_indices
                let matched_indices = indices.iter().map(|&i| i as usize).collect();

                matches.push(FuzzyMatch {
                    file: file.clone(),
                    score: score as i64,
                    matched_indices,
                });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.file.relative_path.cmp(&b.file.relative_path))
    });

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_files(paths: &[&str]) -> Vec<TrackedFile> {
        paths
            .iter()
            .map(|p| TrackedFile {
                relative_path: PathBuf::from(p),
                absolute_path: PathBuf::from("/repo").join(p),
            })
            .collect()
    }

    #[test]
    fn empty_query_returns_all_files() {
        let files = make_files(&["src/main.rs", "Cargo.toml", "README.md"]);
        let results = fuzzy_filter(&files, "");
        assert_eq!(results.len(), 3);
        for m in &results {
            assert_eq!(m.score, 0);
            assert!(m.matched_indices.is_empty());
        }
    }

    #[test]
    fn empty_files_returns_empty() {
        let results = fuzzy_filter(&[], "test");
        assert!(results.is_empty());
    }

    #[test]
    fn no_match_returns_empty() {
        let files = make_files(&["src/main.rs", "Cargo.toml"]);
        let results = fuzzy_filter(&files, "zzzzzzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn exact_filename_match() {
        let files = make_files(&["src/main.rs", "src/lib.rs", "Cargo.toml"]);
        let results = fuzzy_filter(&files, "main.rs");
        assert!(!results.is_empty());
        assert_eq!(results[0].file.relative_path, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn partial_match() {
        let files = make_files(&["src/main.rs", "src/lib.rs", "tests/test_main.rs"]);
        let results = fuzzy_filter(&files, "main");
        assert!(!results.is_empty());
        // All results should contain "main" in their path
        for m in &results {
            let path = m.file.relative_path.to_string_lossy();
            assert!(
                path.contains("main"),
                "expected path to contain 'main', got: {}",
                path
            );
        }
    }

    #[test]
    fn score_ordering_better_matches_first() {
        let files = make_files(&[
            "src/something/deeply/nested/main.rs",
            "main.rs",
            "src/main_helper.rs",
        ]);
        let results = fuzzy_filter(&files, "main.rs");
        assert!(results.len() >= 2);
        // Scores should be in descending order
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "scores not in descending order: {} < {}",
                window[0].score,
                window[1].score
            );
        }
    }

    #[test]
    fn matched_indices_are_populated() {
        let files = make_files(&["src/main.rs"]);
        let results = fuzzy_filter(&files, "main");
        assert_eq!(results.len(), 1);
        assert!(
            !results[0].matched_indices.is_empty(),
            "matched_indices should be non-empty for a match"
        );
    }

    #[test]
    fn fuzzy_matching_non_contiguous() {
        let files = make_files(&["src/my_awesome_file.rs", "other.txt"]);
        let results = fuzzy_filter(&files, "maf");
        // "maf" should fuzzy-match "my_awesome_file" (m, a, f)
        assert!(
            !results.is_empty(),
            "expected fuzzy match for 'maf' in 'my_awesome_file.rs'"
        );
    }
}
