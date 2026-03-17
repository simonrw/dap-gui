## Context

The DAP GUI debugger uses egui/eframe and displays source code in `code_view.rs` using `egui::FontId::monospace()` with the Body text style size. Fonts are not explicitly loaded — egui's built-in default monospace font is used. There are 6 GUI theme variants (base, catppuccin, dracula, material, nord, native) plus a separate gui-iced crate. Each variant has its own `main.rs` that configures `egui::Style`. The `state` crate provides cross-session persistence.

## Goals / Non-Goals

**Goals:**
- Bundle and load Lilex as the monospace font for the code view
- Provide `Ctrl/Cmd + =` and `Ctrl/Cmd + -` shortcuts to adjust font size at runtime
- Persist the chosen font size across sessions
- Work consistently across all egui-based theme variants

**Non-Goals:**
- Making the font itself user-configurable (only Lilex for now)
- Adjusting font size for non-code UI elements
- Supporting the gui-iced crate (different framework)
- Font ligature configuration

## Decisions

### 1. Font loading via egui's `FontDefinitions`

Load Lilex `.ttf` at app startup using `cc.egui_ctx.set_fonts()` in each theme variant's `main.rs`. Map it as the `Monospace` font family. This is egui's standard approach for custom fonts.

**Alternative considered**: Loading at first `CodeView` render — rejected because egui font definitions are context-global and should be set once at startup.

### 2. Font file bundling with `include_bytes!`

Embed the font file at compile time using `include_bytes!()` in a shared module. This avoids runtime file path issues and keeps deployment as a single binary.

**Alternative considered**: Loading from filesystem at runtime — rejected for simplicity and portability.

### 3. Shared font module in the `gui` crate

Create a `fonts` module in the `gui` crate that exports a function to install Lilex into an egui context. All theme variants already depend on `gui`, so they can call this shared function. This avoids duplicating font loading code across 6 crates.

### 4. Font size state in `DebuggerAppState`

Add a `code_font_size: f32` field to `DebuggerAppState`. The `CodeView` widget will accept this size instead of reading from `ui.style().text_styles`. Shortcuts modify this field, and it's persisted via the `state` crate.

**Alternative considered**: Modifying the global egui `TextStyle::Monospace` size — rejected because it would affect all monospace text in the UI, not just the code view.

### 5. Shortcut handling in `renderer.rs`

Handle `Ctrl/Cmd + =`/`-` in `render_ui()` alongside the existing `Ctrl+P` shortcut. Use `egui::Modifiers::command` for cross-platform Ctrl/Cmd support. Clamp font size to a reasonable range (8.0–32.0).

### 6. Font file selection

Use Lilex Regular weight only (single `.ttf` file). This keeps binary size increase minimal (~200KB) while providing good readability for code.

## Risks / Trade-offs

- **Binary size increase (~200KB)**: Acceptable for a desktop application. → No mitigation needed.
- **Font rendering differences across platforms**: egui uses its own text renderer, so rendering is consistent. → Low risk.
- **Lilex font updates**: Font is bundled at build time, requires rebuild to update. → Acceptable; font updates are rare.
