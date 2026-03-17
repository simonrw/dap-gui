## Why

The code view currently uses egui's default monospace font, which lacks character and readability for a developer-focused debugger UI. Lilex is a modern, open-source monospace font designed specifically for code with ligature support and excellent readability. Additionally, there's no way to adjust font size during a debugging session, forcing users to restart with different settings.

## What Changes

- Bundle the Lilex font (OFL-licensed) and load it as the monospace font for the code view across all GUI variants
- Add keyboard shortcuts to increase (`Ctrl/Cmd + =`) and decrease (`Ctrl/Cmd + -`) the code view font size at runtime
- Persist the user's font size preference across sessions using the existing `state` crate

## Capabilities

### New Capabilities
- `custom-code-font`: Bundling and loading the Lilex font as the code view's monospace typeface
- `font-size-shortcuts`: Keyboard shortcuts to increase/decrease code view font size with persistence

### Modified Capabilities
<!-- No existing specs to modify -->

## Impact

- **Dependencies**: Lilex font files (`.ttf`/`.otf`) added to the repository (OFL license)
- **Code**: `code_view.rs` (font ID changes), `main.rs` in all GUI crates (font loading, shortcut handling), `state` crate (font size persistence)
- **Binary size**: Increases slightly due to bundled font files
- **All theme variants**: gui, gui-catppuccin, gui-dracula, gui-material, gui-nord, gui-native all need font loading
