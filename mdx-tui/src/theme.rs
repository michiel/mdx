//! Theme system for dark/light modes

use mdx_core::config::ThemeVariant;
use ratatui::style::{Color, Modifier, Style};

/// Theme definition
#[derive(Clone, Debug)]
pub struct Theme {
    pub base: Style,
    pub heading: [Style; 6],
    pub code: Style,
    pub code_block_bg: Color,
    pub link: Style,
    pub quote: Style,
    pub list_marker: Style,
    pub toc_bg: Color,
    pub toc_border: Color,
    pub toc_active: Style,
    pub cursor_line_bg: Color,
    pub status_bar_fg: Color,
    pub status_bar_bg: Color,
    pub collapsed_block_bg: Color,
    pub collapsed_indicator_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub scrollbar_track: Color,
    pub scrollbar_track_unfocused: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_thumb_unfocused: Color,
    #[cfg(feature = "git")]
    pub diff_add: Style,
    #[cfg(feature = "git")]
    pub diff_del: Style,
    #[cfg(feature = "git")]
    pub diff_mod: Style,
}

impl Theme {
    /// Create a theme for the given variant
    pub fn for_variant(variant: ThemeVariant) -> Self {
        match variant {
            ThemeVariant::Dark => Self::dark(),
            ThemeVariant::Light => Self::light(),
        }
    }

    /// Dark theme — Cyberpunk neon palette
    ///
    /// Near-black background with high-saturation neon accents. The
    /// contrast ratios are high enough to be readable; the palette is
    /// intentionally loud — think terminal circa 2077.
    pub fn dark() -> Self {
        Self {
            // Slightly blue-tinted white on near-black. The explicit bg
            // ensures pane backgrounds are correct even on non-dark terminals.
            base: Style::default()
                .fg(Color::Rgb(220, 220, 255))
                .bg(Color::Rgb(10, 10, 18)),
            heading: [
                // H1: neon rose / hot pink
                Style::default()
                    .fg(Color::Rgb(255, 45, 120))
                    .add_modifier(Modifier::BOLD),
                // H2: electric cyan
                Style::default()
                    .fg(Color::Rgb(0, 229, 255))
                    .add_modifier(Modifier::BOLD),
                // H3: acid green
                Style::default()
                    .fg(Color::Rgb(57, 255, 20))
                    .add_modifier(Modifier::BOLD),
                // H4: electric violet
                Style::default()
                    .fg(Color::Rgb(191, 95, 255))
                    .add_modifier(Modifier::BOLD),
                // H5: neon orange
                Style::default()
                    .fg(Color::Rgb(255, 110, 0))
                    .add_modifier(Modifier::BOLD),
                // H6: neon yellow
                Style::default()
                    .fg(Color::Rgb(255, 230, 0))
                    .add_modifier(Modifier::BOLD),
            ],
            code: Style::default().fg(Color::Rgb(255, 210, 0)), // neon amber / gold
            code_block_bg: Color::Rgb(5, 5, 20),                // deeper blue-black for blocks
            link: Style::default()
                .fg(Color::Rgb(0, 180, 255)) // electric sky-blue
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(Color::Rgb(120, 120, 180)) // muted purple-gray, still readable
                .add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::Rgb(255, 45, 120)), // hot pink bullets
            toc_bg: Color::Rgb(8, 8, 22),
            toc_border: Color::Rgb(50, 50, 100),
            toc_active: Style::default()
                .fg(Color::Rgb(10, 10, 18))
                .bg(Color::Rgb(0, 229, 255)) // electric cyan highlight
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(20, 20, 45), // dark purple-blue glow
            status_bar_fg: Color::Rgb(10, 10, 18),
            status_bar_bg: Color::Rgb(0, 229, 255), // electric cyan bar
            collapsed_block_bg: Color::Rgb(15, 15, 35),
            collapsed_indicator_fg: Color::Rgb(0, 229, 255),
            border: Color::Rgb(40, 40, 80),
            border_focused: Color::Rgb(255, 45, 120), // hot pink focused border
            scrollbar_track: Color::Rgb(25, 25, 50),
            scrollbar_track_unfocused: Color::Rgb(18, 18, 36),
            scrollbar_thumb: Color::Rgb(255, 45, 120), // hot pink thumb
            scrollbar_thumb_unfocused: Color::Rgb(80, 80, 140),
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Rgb(57, 255, 20)), // acid green
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Rgb(255, 45, 120)), // hot pink
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Rgb(255, 210, 0)), // neon gold
        }
    }

    /// Light theme — GitHub Light / Solarized Light palette
    ///
    /// The explicit `bg` on `base` is the critical fix: without it ratatui
    /// inherits the terminal's default background, which on a dark terminal
    /// makes near-black text invisible. Setting it to near-white ensures the
    /// pane content area is always readable regardless of terminal theme.
    pub fn light() -> Self {
        Self {
            base: Style::default()
                .fg(Color::Rgb(36, 41, 46))   // near-black text
                .bg(Color::Rgb(255, 255, 255)), // explicit white background — fixes "text disappears"
            heading: [
                // H1: Deep blue
                Style::default()
                    .fg(Color::Rgb(3, 102, 214))
                    .add_modifier(Modifier::BOLD),
                // H2: Teal
                Style::default()
                    .fg(Color::Rgb(0, 112, 120))
                    .add_modifier(Modifier::BOLD),
                // H3: Forest green
                Style::default()
                    .fg(Color::Rgb(34, 134, 58))
                    .add_modifier(Modifier::BOLD),
                // H4: Purple
                Style::default()
                    .fg(Color::Rgb(111, 66, 193))
                    .add_modifier(Modifier::BOLD),
                // H5: Dark orange
                Style::default()
                    .fg(Color::Rgb(210, 90, 0))
                    .add_modifier(Modifier::BOLD),
                // H6: Dark gray
                Style::default()
                    .fg(Color::Rgb(88, 96, 105))
                    .add_modifier(Modifier::BOLD),
            ],
            code: Style::default()
                .fg(Color::Rgb(175, 30, 60)) // deep crimson — visible on white
                .bg(Color::Rgb(255, 245, 248)), // faint pink tint for inline code
            code_block_bg: Color::Rgb(245, 247, 250), // very light blue-gray for blocks
            link: Style::default()
                .fg(Color::Rgb(3, 102, 214))
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(Color::Rgb(87, 96, 106)) // medium gray, clearly readable
                .add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::Rgb(3, 102, 214)), // deep blue bullets
            toc_bg: Color::Rgb(248, 250, 252),
            toc_border: Color::Rgb(200, 208, 216),
            toc_active: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(3, 102, 214))
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(230, 240, 255), // soft blue highlight
            status_bar_fg: Color::Rgb(255, 255, 255),
            status_bar_bg: Color::Rgb(36, 41, 46), // dark status bar keeps the contrast frame
            collapsed_block_bg: Color::Rgb(232, 238, 245),
            collapsed_indicator_fg: Color::Rgb(0, 112, 120),
            border: Color::Rgb(208, 215, 222),
            border_focused: Color::Rgb(3, 102, 214),
            scrollbar_track: Color::Rgb(220, 228, 236),
            scrollbar_track_unfocused: Color::Rgb(232, 237, 242),
            scrollbar_thumb: Color::Rgb(3, 102, 214),
            scrollbar_thumb_unfocused: Color::Rgb(140, 152, 165),
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Rgb(34, 134, 58)),
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Rgb(200, 30, 50)),
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Rgb(210, 90, 0)),
        }
    }
}
