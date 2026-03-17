## 1. Font Asset Setup

- [x] 1.1 Download Lilex Regular TTF file and add it to the repository (e.g., `crates/gui/fonts/Lilex-Regular.ttf`)
- [x] 1.2 Create `crates/gui/src/fonts.rs` module that uses `include_bytes!` to embed the font and exports a function to install it into an egui context as the `Monospace` family

## 2. Font Loading

- [x] 2.1 Call the font installation function from `crates/gui/src/main.rs` in the eframe `setup` callback

## 3. Code View Font Size

- [x] 3.1 Add `code_font_size: f32` field to `DebuggerAppState` with a default of 14.0
- [x] 3.2 Update `CodeView` to accept a `font_size` parameter and use it instead of reading from `ui.style().text_styles`
- [x] 3.3 Pass `code_font_size` from state when constructing `CodeView` in the renderer

## 4. Keyboard Shortcuts

- [x] 4.1 Add `Ctrl/Cmd + =` shortcut in `render_ui()` to increase `code_font_size` by 1.0 (clamped to max 32.0)
- [x] 4.2 Add `Ctrl/Cmd + -` shortcut in `render_ui()` to decrease `code_font_size` by 1.0 (clamped to min 8.0)

## 5. Font Size Persistence

- [x] 5.1 Add font size to the `state` crate's persisted data so it saves/restores across sessions

## 6. Verification

- [x] 6.1 Run `cargo check --all-targets --all-features` to verify compilation
- [x] 6.2 Run `cargo xtask test && cargo xtask doctest` to verify tests pass
- [ ] 6.3 Run `bin/capture_screenshot` to verify the font renders correctly
