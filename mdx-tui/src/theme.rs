//! Theme system for dark/light modes

use mdx_core::config::ThemeVariant;
use ratatui::style::Style;

/// Theme definition
#[derive(Clone, Debug)]
pub struct Theme {
    pub base: Style,
    pub heading: [Style; 6],
    pub code: Style,
    pub link: Style,
    pub quote: Style,
    pub list_marker: Style,
    pub toc_active: Style,
    #[cfg(feature = "git")]
    pub diff_add: Style,
    #[cfg(feature = "git")]
    pub diff_del: Style,
    #[cfg(feature = "git")]
    pub diff_mod: Style,
}

impl Theme {
    /// Create a theme for the given variant
    pub fn for_variant(_variant: ThemeVariant) -> Self {
        // TODO: Implementation in Stage 6
        Self {
            base: Style::default(),
            heading: [Style::default(); 6],
            code: Style::default(),
            link: Style::default(),
            quote: Style::default(),
            list_marker: Style::default(),
            toc_active: Style::default(),
            #[cfg(feature = "git")]
            diff_add: Style::default(),
            #[cfg(feature = "git")]
            diff_del: Style::default(),
            #[cfg(feature = "git")]
            diff_mod: Style::default(),
        }
    }
}
