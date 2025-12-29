//! Terminal image backend detection and selection

#[cfg(feature = "images")]
use mdx_core::config::ImageBackend;

#[cfg(feature = "images")]
/// Detect the best available image backend for the current terminal
pub fn detect_backend() -> ImageBackend {
    // Check for Kitty terminal
    if is_kitty() {
        return ImageBackend::Kitty;
    }

    // Check for iTerm2
    if is_iterm2() {
        return ImageBackend::ITerm2;
    }

    // Check for Sixel support
    if supports_sixel() {
        return ImageBackend::Sixel;
    }

    // No supported backend
    ImageBackend::None
}

#[cfg(feature = "images")]
/// Check if running in Kitty terminal
fn is_kitty() -> bool {
    // Kitty sets TERM=xterm-kitty
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("kitty") {
            return true;
        }
    }

    // Also check KITTY_WINDOW_ID environment variable
    std::env::var("KITTY_WINDOW_ID").is_ok()
}

#[cfg(feature = "images")]
/// Check if running in iTerm2
fn is_iterm2() -> bool {
    // iTerm2 sets TERM_PROGRAM=iTerm.app
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        if term_program == "iTerm.app" {
            return true;
        }
    }

    // Also check LC_TERMINAL for iTerm2
    if let Ok(lc_terminal) = std::env::var("LC_TERMINAL") {
        if lc_terminal == "iTerm2" {
            return true;
        }
    }

    false
}

#[cfg(feature = "images")]
/// Check if terminal supports Sixel graphics
fn supports_sixel() -> bool {
    // Check TERM variable for known Sixel-capable terminals
    if let Ok(term) = std::env::var("TERM") {
        // xterm with Sixel support
        if term.contains("sixel") {
            return true;
        }

        // mlterm supports Sixel
        if term.contains("mlterm") {
            return true;
        }
    }

    // Could also query terminal capabilities via terminfo,
    // but that's complex and not always reliable
    // For now, rely on TERM environment variable

    false
}

#[cfg(feature = "images")]
/// Select backend based on configuration and terminal capabilities
pub fn select_backend(config_backend: ImageBackend) -> ImageBackend {
    match config_backend {
        ImageBackend::Auto => detect_backend(),
        ImageBackend::Kitty => {
            // Validate that Kitty is actually available
            if is_kitty() {
                ImageBackend::Kitty
            } else {
                ImageBackend::None
            }
        }
        ImageBackend::ITerm2 => {
            // Validate that iTerm2 is actually available
            if is_iterm2() {
                ImageBackend::ITerm2
            } else {
                ImageBackend::None
            }
        }
        ImageBackend::Sixel => {
            // Validate that Sixel is actually available
            if supports_sixel() {
                ImageBackend::Sixel
            } else {
                ImageBackend::None
            }
        }
        ImageBackend::None => ImageBackend::None,
    }
}

#[cfg(all(test, feature = "images"))]
mod tests {
    use super::*;

    #[test]
    fn test_detect_backend_returns_valid() {
        // This test just ensures the function runs without panicking
        // The actual result depends on the environment
        let backend = detect_backend();
        assert!(matches!(
            backend,
            ImageBackend::Kitty
                | ImageBackend::ITerm2
                | ImageBackend::Sixel
                | ImageBackend::None
        ));
    }

    #[test]
    fn test_select_backend_none() {
        let selected = select_backend(ImageBackend::None);
        assert_eq!(selected, ImageBackend::None);
    }

    #[test]
    fn test_select_backend_auto() {
        // Should return one of the valid backends
        let selected = select_backend(ImageBackend::Auto);
        assert!(matches!(
            selected,
            ImageBackend::Kitty
                | ImageBackend::ITerm2
                | ImageBackend::Sixel
                | ImageBackend::None
        ));
    }

    #[test]
    fn test_is_kitty_checks_env() {
        // Save original env
        let original_term = std::env::var("TERM").ok();
        let original_window_id = std::env::var("KITTY_WINDOW_ID").ok();

        // Test with TERM=xterm-kitty
        std::env::set_var("TERM", "xterm-kitty");
        assert!(is_kitty());

        // Test with KITTY_WINDOW_ID
        std::env::set_var("TERM", "xterm-256color");
        std::env::set_var("KITTY_WINDOW_ID", "1");
        assert!(is_kitty());

        // Clean up
        match original_term {
            Some(v) => std::env::set_var("TERM", v),
            None => std::env::remove_var("TERM"),
        }
        match original_window_id {
            Some(v) => std::env::set_var("KITTY_WINDOW_ID", v),
            None => std::env::remove_var("KITTY_WINDOW_ID"),
        }
    }

    #[test]
    fn test_is_iterm2_checks_env() {
        // Save original env
        let original_term_program = std::env::var("TERM_PROGRAM").ok();
        let original_lc_terminal = std::env::var("LC_TERMINAL").ok();

        // Test with TERM_PROGRAM=iTerm.app
        std::env::set_var("TERM_PROGRAM", "iTerm.app");
        assert!(is_iterm2());

        // Test with LC_TERMINAL=iTerm2
        std::env::remove_var("TERM_PROGRAM");
        std::env::set_var("LC_TERMINAL", "iTerm2");
        assert!(is_iterm2());

        // Clean up
        match original_term_program {
            Some(v) => std::env::set_var("TERM_PROGRAM", v),
            None => std::env::remove_var("TERM_PROGRAM"),
        }
        match original_lc_terminal {
            Some(v) => std::env::set_var("LC_TERMINAL", v),
            None => std::env::remove_var("LC_TERMINAL"),
        }
    }

    #[test]
    fn test_supports_sixel_checks_term() {
        // Save original env
        let original_term = std::env::var("TERM").ok();

        // Test with TERM containing sixel
        std::env::set_var("TERM", "xterm-sixel");
        assert!(supports_sixel());

        // Test with mlterm
        std::env::set_var("TERM", "mlterm");
        assert!(supports_sixel());

        // Test with non-sixel terminal
        std::env::set_var("TERM", "xterm-256color");
        assert!(!supports_sixel());

        // Clean up
        match original_term {
            Some(v) => std::env::set_var("TERM", v),
            None => std::env::remove_var("TERM"),
        }
    }
}
