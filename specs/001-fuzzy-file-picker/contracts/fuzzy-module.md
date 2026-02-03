# Contract: Fuzzy Matching Module

**Module**: `crates/tui-poc/src/fuzzy.rs`

## Public Interface

### Types

```rust
/// A file tracked by Git
pub struct TrackedFile {
    /// Path relative to repository root
    pub relative_path: PathBuf,
    /// Full filesystem path
    pub absolute_path: PathBuf,
}

/// A file matching the fuzzy search query
pub struct FuzzyMatch {
    /// The matched file
    pub file: TrackedFile,
    /// Match quality score (higher = better)
    pub score: i64,
    /// Character indices that matched (for highlighting)
    pub matched_indices: Vec<usize>,
}
```

### Functions

```rust
/// Get the Git repository root directory.
/// Returns None if not in a Git repository.
pub fn find_repo_root() -> Option<PathBuf>;

/// Enumerate all files tracked by Git in the repository.
/// Returns an error if git command fails or not in a repo.
pub fn list_git_files(repo_root: &Path) -> Result<Vec<TrackedFile>, std::io::Error>;

/// Perform fuzzy matching on a list of files.
/// Returns matches sorted by score (descending), then alphabetically.
/// Empty query returns all files (no filtering).
pub fn fuzzy_filter(
    files: &[TrackedFile],
    query: &str,
) -> Vec<FuzzyMatch>;
```

## Behavior Specification

### find_repo_root

- Runs `git rev-parse --show-toplevel`
- Returns `Some(PathBuf)` on success
- Returns `None` if:
  - Not in a Git repository
  - Git command not found
  - Command fails for any reason

### list_git_files

- Runs `git ls-files --cached --others --exclude-standard`
- Converts each line to a `TrackedFile`
- `relative_path` = line from git output
- `absolute_path` = repo_root.join(relative_path)
- Returns `Err` if:
  - Git command fails
  - Output is not valid UTF-8

### fuzzy_filter

- Uses `nucleo_matcher::Matcher` with default config for matching
- Matches against `relative_path` display string
- Score of 0 means no match (filtered out)
- Empty query returns all files with score=0
- Results sorted by:
  1. Score descending (higher scores = better matches)
  2. Path alphabetically ascending

## Error Handling

All errors use `std::io::Error` for simplicity:
- Git not found: `ErrorKind::NotFound`
- Not a repo: `ErrorKind::Other` with descriptive message
- Command failed: `ErrorKind::Other` with stderr content

## Performance Requirements

| Operation | Target | Max |
|-----------|--------|-----|
| list_git_files (10k files) | 50ms | 200ms |
| list_git_files (50k files) | 200ms | 500ms |
| fuzzy_filter (10k files) | 20ms | 50ms |
| fuzzy_filter (50k files) | 50ms | 100ms |

## Dependencies

```toml
[dependencies]
nucleo-matcher = "0.3"
```
