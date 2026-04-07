use ratatui::style::Color;

/// Resolved theme mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Complete color palette for the TUI.
///
/// Every `Color` reference in the UI should come from here so that switching
/// between dark and light mode is a single palette swap.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,

    /// Syntect theme name for code syntax highlighting.
    pub syntect_theme: &'static str,

    // ── Borders ──────────────────────────────────────────────────────
    pub border_focused: Color,
    pub border_unfocused: Color,

    // ── Text ─────────────────────────────────────────────────────────
    pub text: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    // ── Accent ───────────────────────────────────────────────────────
    pub accent: Color,
    pub accent_alt: Color,

    // ── Semantic ─────────────────────────────────────────────────────
    pub error: Color,
    pub success: Color,
    pub warning: Color,

    // ── Status bar ───────────────────────────────────────────────────
    pub status_badge_fg: Color,

    // ── Backgrounds ──────────────────────────────────────────────────
    pub selection_bg: Color,
    pub cursor_line_bg: Color,
    pub exec_line_bg: Color,
    pub search_match_bg: Color,
    pub search_current_bg: Color,
    pub code_selection_bg: Color,

    // ── Controls bar key badges ──────────────────────────────────────
    pub key_badge_fg: Color,
    pub key_badge_bg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            syntect_theme: "base16-ocean.dark",

            border_focused: Color::Cyan,
            border_unfocused: Color::DarkGray,

            text: Color::White,
            text_secondary: Color::Gray,
            text_muted: Color::DarkGray,

            accent: Color::Yellow,
            accent_alt: Color::Cyan,

            error: Color::Red,
            success: Color::Green,
            warning: Color::Yellow,

            status_badge_fg: Color::Black,

            selection_bg: Color::Rgb(50, 50, 80),
            cursor_line_bg: Color::Rgb(40, 44, 52),
            exec_line_bg: Color::Rgb(50, 60, 30),
            search_match_bg: Color::Rgb(100, 100, 0),
            search_current_bg: Color::Rgb(180, 120, 0),
            code_selection_bg: Color::Rgb(40, 50, 70),

            key_badge_fg: Color::Black,
            key_badge_bg: Color::Cyan,
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            syntect_theme: "base16-ocean.light",

            border_focused: Color::Blue,
            border_unfocused: Color::Gray,

            text: Color::Black,
            text_secondary: Color::DarkGray,
            text_muted: Color::Gray,

            accent: Color::Rgb(180, 130, 0),
            accent_alt: Color::Blue,

            error: Color::Red,
            success: Color::Rgb(0, 140, 0),
            warning: Color::Yellow,

            status_badge_fg: Color::White,

            selection_bg: Color::Rgb(200, 210, 235),
            cursor_line_bg: Color::Rgb(232, 232, 238),
            exec_line_bg: Color::Rgb(215, 240, 195),
            search_match_bg: Color::Rgb(255, 255, 120),
            search_current_bg: Color::Rgb(255, 190, 70),
            code_selection_bg: Color::Rgb(180, 200, 230),

            key_badge_fg: Color::White,
            key_badge_bg: Color::Blue,
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }
}

/// Detect the current system theme preference.
pub fn detect_theme_mode() -> ThemeMode {
    match dark_light::detect() {
        Ok(dark_light::Mode::Light) => ThemeMode::Light,
        Ok(dark_light::Mode::Dark) | Ok(dark_light::Mode::Unspecified) => ThemeMode::Dark,
        Err(e) => {
            tracing::warn!(error = %e, "could not detect system theme, defaulting to dark");
            ThemeMode::Dark
        }
    }
}
