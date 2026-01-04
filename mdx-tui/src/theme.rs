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

    /// Dark theme (default) - Inspired by One Dark Pro / Nord
    pub fn dark() -> Self {
        Self {
            base: Style::default().fg(Color::Rgb(220, 220, 220)), // Soft white
            heading: [
                // H1: Bright blue - highest priority
                Style::default()
                    .fg(Color::Rgb(97, 175, 239))
                    .add_modifier(Modifier::BOLD),
                // H2: Cyan - secondary
                Style::default()
                    .fg(Color::Rgb(86, 182, 194))
                    .add_modifier(Modifier::BOLD),
                // H3: Green - tertiary
                Style::default()
                    .fg(Color::Rgb(152, 195, 121))
                    .add_modifier(Modifier::BOLD),
                // H4: Purple - quaternary
                Style::default()
                    .fg(Color::Rgb(198, 120, 221))
                    .add_modifier(Modifier::BOLD),
                // H5: Orange
                Style::default()
                    .fg(Color::Rgb(229, 192, 123))
                    .add_modifier(Modifier::BOLD),
                // H6: Muted gray-blue
                Style::default()
                    .fg(Color::Rgb(150, 160, 180))
                    .add_modifier(Modifier::BOLD),
            ],
            code: Style::default().fg(Color::Rgb(229, 192, 123)), // Warm amber
            code_block_bg: Color::Rgb(40, 44, 52),                // Dark background for code blocks
            link: Style::default()
                .fg(Color::Rgb(97, 175, 239)) // Bright blue
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(Color::Rgb(130, 140, 150)) // Readable gray
                .add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::Rgb(224, 108, 117)), // Soft red
            toc_bg: Color::Rgb(30, 30, 30),                              // Subtle dark background
            toc_border: Color::Rgb(60, 60, 60),                          // Visible border
            toc_active: Style::default()
                .fg(Color::Rgb(30, 30, 30))
                .bg(Color::Rgb(86, 182, 194)) // Cyan highlight
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(45, 48, 55), // Visible but subtle
            status_bar_fg: Color::Rgb(220, 220, 220),
            status_bar_bg: Color::Rgb(52, 61, 70), // Muted blue-gray
            collapsed_block_bg: Color::Rgb(35, 38, 45), // Slightly darker than normal bg
            collapsed_indicator_fg: Color::Rgb(86, 182, 194), // Cyan, matches TOC active
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Rgb(152, 195, 121)), // Softer green
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Rgb(224, 108, 117)), // Softer red
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Rgb(229, 192, 123)), // Amber
        }
    }

    /// Light theme - Inspired by GitHub Light / Solarized Light
    pub fn light() -> Self {
        Self {
            base: Style::default().fg(Color::Rgb(36, 41, 46)), // Near-black, easier on eyes
            heading: [
                // H1: Deep blue - highest priority
                Style::default()
                    .fg(Color::Rgb(3, 102, 214))
                    .add_modifier(Modifier::BOLD),
                // H2: Teal - secondary
                Style::default()
                    .fg(Color::Rgb(0, 128, 128))
                    .add_modifier(Modifier::BOLD),
                // H3: Forest green - tertiary
                Style::default()
                    .fg(Color::Rgb(34, 134, 58))
                    .add_modifier(Modifier::BOLD),
                // H4: Purple - quaternary
                Style::default()
                    .fg(Color::Rgb(111, 66, 193))
                    .add_modifier(Modifier::BOLD),
                // H5: Dark orange
                Style::default()
                    .fg(Color::Rgb(227, 98, 9))
                    .add_modifier(Modifier::BOLD),
                // H6: Dark gray
                Style::default()
                    .fg(Color::Rgb(88, 96, 105))
                    .add_modifier(Modifier::BOLD),
            ],
            code: Style::default().fg(Color::Rgb(212, 73, 80)), // Warm red-brown
            code_block_bg: Color::Rgb(246, 248, 250),           // Very light gray background
            link: Style::default()
                .fg(Color::Rgb(3, 102, 214)) // Deep blue
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(Color::Rgb(106, 115, 125)) // Readable medium gray
                .add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::Rgb(212, 73, 80)), // Warm red
            toc_bg: Color::Rgb(250, 251, 252), // Very light gray, not harsh white
            toc_border: Color::Rgb(209, 213, 218), // Soft gray border
            toc_active: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(3, 102, 214)) // Deep blue highlight
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(240, 246, 252), // Light blue tint
            status_bar_fg: Color::Rgb(255, 255, 255),
            status_bar_bg: Color::Rgb(36, 41, 46), // Dark background for contrast
            collapsed_block_bg: Color::Rgb(235, 240, 245), // Slightly darker than normal bg
            collapsed_indicator_fg: Color::Rgb(0, 128, 128), // Teal, matches H2
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Rgb(34, 134, 58)), // Forest green
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Rgb(212, 73, 80)), // Warm red
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Rgb(227, 98, 9)), // Dark orange
        }
    }
}
