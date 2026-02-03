# Contract: Command Palette Modifications

**Location**: `crates/tui-poc/src/main.rs`

## Interface Changes

### App Struct Modifications

```rust
impl App {
    // EXISTING (unchanged)
    command_palette_open: bool,
    command_palette_input: String,
    command_palette_cursor: usize,

    // MODIFIED: Now stores FuzzyMatch instead of String
    command_palette_filtered: Vec<FuzzyMatch>,

    // NEW: Cached Git file list
    git_files: Vec<TrackedFile>,
    git_files_loaded: bool,
    repo_root: Option<PathBuf>,
}
```

### Modified Methods

```rust
impl App {
    /// Open the command palette.
    /// Loads Git files if not already cached.
    fn open_command_palette(&mut self);

    /// Update filtered files based on current query.
    /// Uses fuzzy matching instead of substring.
    fn update_filtered_files(&mut self);

    /// Get display string for a match (relative path).
    fn get_match_display(&self, m: &FuzzyMatch) -> String;

    /// Handle selection of a file from the palette.
    fn select_command_palette_item(&mut self);
}
```

## Behavior Specification

### open_command_palette

**Before** (existing):
```rust
fn open_command_palette(&mut self) {
    self.command_palette_open = true;
    self.command_palette_input.clear();
    self.command_palette_cursor = 0;
    self.update_filtered_files();
}
```

**After** (modified):
```rust
fn open_command_palette(&mut self) {
    self.command_palette_open = true;
    self.command_palette_input.clear();
    self.command_palette_cursor = 0;

    // Load Git files on first open or refresh
    if !self.git_files_loaded {
        self.load_git_files();
    }

    self.update_filtered_files();
}

fn load_git_files(&mut self) {
    match fuzzy::find_repo_root() {
        Some(root) => {
            self.repo_root = Some(root.clone());
            match fuzzy::list_git_files(&root) {
                Ok(files) => {
                    self.git_files = files;
                    self.git_files_loaded = true;
                }
                Err(e) => {
                    tracing::warn!("Failed to list git files: {}", e);
                    self.git_files = vec![];
                    self.git_files_loaded = true;
                }
            }
        }
        None => {
            tracing::info!("Not in a git repository");
            self.git_files = vec![];
            self.git_files_loaded = true;
        }
    }
}
```

### update_filtered_files

**Before** (existing):
```rust
fn update_filtered_files(&mut self) {
    let query = self.command_palette_input.to_lowercase();
    self.command_palette_filtered = self
        .get_file_list()
        .into_iter()
        .filter(|f| f.to_lowercase().contains(&query))
        .collect();
    if self.command_palette_cursor >= self.command_palette_filtered.len() {
        self.command_palette_cursor = 0;
    }
}
```

**After** (modified):
```rust
fn update_filtered_files(&mut self) {
    self.command_palette_filtered = fuzzy::fuzzy_filter(
        &self.git_files,
        &self.command_palette_input,
    );

    if self.command_palette_cursor >= self.command_palette_filtered.len() {
        self.command_palette_cursor = 0;
    }
}
```

### select_command_palette_item

**Before** (existing):
```rust
fn select_command_palette_item(&mut self) {
    if let Some(filename) = self.command_palette_filtered.get(self.command_palette_cursor) {
        // Find path in cache and switch to it
        if let Some(path) = self.file_cache.keys().find(|p| {
            p.file_name().map(|n| n.to_string_lossy().to_string())
                == Some(filename.clone())
        }) {
            let path = path.clone();
            self.switch_to_file(&path);
        }
    }
    self.close_command_palette();
}
```

**After** (modified):
```rust
fn select_command_palette_item(&mut self) {
    if let Some(matched) = self.command_palette_filtered.get(self.command_palette_cursor) {
        let path = matched.file.absolute_path.clone();

        // Load file if not in cache
        if self.load_file(&path) {
            self.switch_to_file(&path);
        } else {
            // Show error in status line
            self.show_error(format!("Could not open: {}", path.display()));
        }
    }
    self.close_command_palette();
}
```

## Display Changes

### Palette Item Rendering

**Before**: Filename only (e.g., "main.rs")
**After**: Relative path (e.g., "src/main.rs") with match highlighting

```rust
fn render_palette_item(&self, m: &FuzzyMatch, selected: bool) -> Span {
    let path_str = m.file.relative_path.display().to_string();

    // Highlight matched characters
    let styled = highlight_matches(&path_str, &m.matched_indices);

    if selected {
        styled.bg(Color::DarkGray).bold()
    } else {
        styled
    }
}

fn highlight_matches(text: &str, indices: &[usize]) -> Spans {
    // Build spans with highlighted characters
    let mut spans = vec![];
    let mut last_idx = 0;

    for &idx in indices {
        if idx > last_idx {
            spans.push(Span::raw(&text[last_idx..idx]));
        }
        spans.push(Span::styled(
            &text[idx..idx+1],
            Style::default().fg(Color::Yellow).bold()
        ));
        last_idx = idx + 1;
    }
    if last_idx < text.len() {
        spans.push(Span::raw(&text[last_idx..]));
    }

    Spans::from(spans)
}
```

### Empty State Messages

| Condition | Message |
|-----------|---------|
| Not in Git repo | "Not in a git repository" |
| No files tracked | "No files tracked by git" |
| No matches | "No matches for: {query}" |

## Keyboard Handling

Unchanged:
- `Ctrl+P`: Open palette
- `Escape`: Close palette
- `Enter`: Select file
- `Up/Down` or `j/k`: Navigate
- Typing: Filter

## Performance Requirements

| Operation | Target | Max |
|-----------|--------|-----|
| open_command_palette (first time, 10k files) | 100ms | 300ms |
| open_command_palette (cached) | 1ms | 5ms |
| update_filtered_files (10k files) | 20ms | 50ms |
| render palette (100 visible items) | 5ms | 16ms |
