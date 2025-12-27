//! Theme system for dark/light modes

use mdx_core::config::ThemeVariant;
use ratatui::style::{Color, Modifier, Style};

/// Theme definition
#[derive(Clone, Debug)]
pub struct Theme {
    pub base: Style,
    pub heading: [Style; 6],
    pub code: Style,
    pub link: Style,
    pub quote: Style,
    pub list_marker: Style,
    pub toc_bg: Color,
    pub toc_border: Color,
    pub toc_active: Style,
    pub cursor_line_bg: Color,
    pub status_bar_fg: Color,
    pub status_bar_bg: Color,
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

    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            base: Style::default().fg(Color::White),
            heading: [
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
            ],
            code: Style::default().fg(Color::Yellow),
            link: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::LightRed),
            toc_bg: Color::Black,
            toc_border: Color::DarkGray,
            toc_active: Style::default()
                .fg(Color::Black)
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(40, 40, 40), // Subtle dark gray
            status_bar_fg: Color::Black,
            status_bar_bg: Color::LightBlue,
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Green),
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Red),
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Yellow),
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            base: Style::default().fg(Color::Black),
            heading: [
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::Rgb(150, 100, 0))
                    .add_modifier(Modifier::BOLD), // Dark yellow
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ],
            code: Style::default().fg(Color::Rgb(150, 75, 0)), // Orange-brown
            link: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::UNDERLINED),
            quote: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            list_marker: Style::default().fg(Color::Red),
            toc_bg: Color::White,
            toc_border: Color::Gray,
            toc_active: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            cursor_line_bg: Color::Rgb(230, 230, 250), // Light lavender
            status_bar_fg: Color::White,
            status_bar_bg: Color::Blue,
            #[cfg(feature = "git")]
            diff_add: Style::default().fg(Color::Green),
            #[cfg(feature = "git")]
            diff_del: Style::default().fg(Color::Red),
            #[cfg(feature = "git")]
            diff_mod: Style::default().fg(Color::Rgb(150, 100, 0)), // Dark yellow
        }
    }
}
