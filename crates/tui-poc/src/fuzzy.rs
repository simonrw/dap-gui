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

    for file in files {
        let path_str = file.relative_path.to_string_lossy();
        if let Some(score) = matcher.fuzzy_match(&path_str, query) {
            let indices = matcher.fuzzy_indices(&path_str, query).unwrap_or_default();
            if score > 0 {
                matches.push(FuzzyMatch {
                    file: file.clone(),
                    score: score as i64,
                    matched_indices: indices,
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
