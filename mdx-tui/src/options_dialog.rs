//! Options dialog for configuration management

use mdx_core::{config::ThemeVariant, Config};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Cancel,
    Ok,
    Save,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionField {
    Theme,
    TocEnabled,
    TocSide,
    TocWidth,
    SafeMode,
    NoExec,
    Utf8Graphics,
    ShowScrollbar,
    SkipFrontMatter,
    #[cfg(feature = "watch")]
    WatchEnabled,
    #[cfg(feature = "watch")]
    AutoReload,
    #[cfg(feature = "git")]
    GitDiff,
    #[cfg(feature = "images")]
    ImagesEnabled,
}

impl OptionField {
    pub fn all() -> Vec<OptionField> {
        vec![
            OptionField::Theme,
            OptionField::TocEnabled,
            OptionField::TocSide,
            OptionField::TocWidth,
            OptionField::SafeMode,
            OptionField::NoExec,
            OptionField::Utf8Graphics,
            OptionField::ShowScrollbar,
            OptionField::SkipFrontMatter,
            #[cfg(feature = "watch")]
            OptionField::WatchEnabled,
            #[cfg(feature = "watch")]
            OptionField::AutoReload,
            #[cfg(feature = "git")]
            OptionField::GitDiff,
            #[cfg(feature = "images")]
            OptionField::ImagesEnabled,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            OptionField::Theme => "Theme",
            OptionField::TocEnabled => "Table of Contents",
            OptionField::TocSide => "TOC Side",
            OptionField::TocWidth => "TOC Width",
            OptionField::SafeMode => "Safe Mode",
            OptionField::NoExec => "No Exec",
            OptionField::Utf8Graphics => "UTF-8 Graphics",
            OptionField::ShowScrollbar => "Show Scrollbar",
            OptionField::SkipFrontMatter => "Skip Front Matter",
            #[cfg(feature = "watch")]
            OptionField::WatchEnabled => "File Watching",
            #[cfg(feature = "watch")]
            OptionField::AutoReload => "Auto Reload",
            #[cfg(feature = "git")]
            OptionField::GitDiff => "Git Diff",
            #[cfg(feature = "images")]
            OptionField::ImagesEnabled => "Images",
        }
    }
}

pub struct OptionsDialog {
    pub editing_config: Config,
    pub original_config: Config,
    pub selected_index: usize,
    pub focused_button: DialogButton,
    pub fields: Vec<OptionField>,
}

impl OptionsDialog {
    pub fn new(config: &Config) -> Self {
        Self {
            editing_config: config.clone(),
            original_config: config.clone(),
            selected_index: 0,
            focused_button: DialogButton::Cancel,
            fields: OptionField::all(),
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.fields.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    pub fn toggle_current(&mut self) {
        if self.selected_index >= self.fields.len() {
            return;
        }

        match self.fields[self.selected_index] {
            OptionField::Theme => {
                self.editing_config.theme = match self.editing_config.theme {
                    ThemeVariant::Dark => ThemeVariant::Light,
                    ThemeVariant::Light => ThemeVariant::Dark,
                };
            }
            OptionField::TocEnabled => {
                self.editing_config.toc.enabled = !self.editing_config.toc.enabled;
            }
            OptionField::TocSide => {
                use mdx_core::config::TocSide;
                self.editing_config.toc.side = match self.editing_config.toc.side {
                    TocSide::Left => TocSide::Right,
                    TocSide::Right => TocSide::Left,
                };
            }
            OptionField::TocWidth => {
                // Cycle through common widths
                self.editing_config.toc.width = match self.editing_config.toc.width {
                    w if w < 32 => 32,
                    32 => 40,
                    40 => 50,
                    _ => 25,
                };
            }
            OptionField::SafeMode => {
                self.editing_config.security.safe_mode = !self.editing_config.security.safe_mode;
            }
            OptionField::NoExec => {
                self.editing_config.security.no_exec = !self.editing_config.security.no_exec;
            }
            OptionField::Utf8Graphics => {
                self.editing_config.render.use_utf8_graphics =
                    !self.editing_config.render.use_utf8_graphics;
            }
            OptionField::ShowScrollbar => {
                self.editing_config.render.show_scrollbar =
                    !self.editing_config.render.show_scrollbar;
            }
            OptionField::SkipFrontMatter => {
                self.editing_config.render.skip_front_matter =
                    !self.editing_config.render.skip_front_matter;
            }
            #[cfg(feature = "watch")]
            OptionField::WatchEnabled => {
                self.editing_config.watch.enabled = !self.editing_config.watch.enabled;
            }
            #[cfg(feature = "watch")]
            OptionField::AutoReload => {
                self.editing_config.watch.auto_reload = !self.editing_config.watch.auto_reload;
            }
            #[cfg(feature = "git")]
            OptionField::GitDiff => {
                self.editing_config.git.diff = !self.editing_config.git.diff;
            }
            #[cfg(feature = "images")]
            OptionField::ImagesEnabled => {
                self.editing_config.images.enabled = !self.editing_config.images.enabled;
            }
        }
    }

    pub fn get_value_string(&self, field: &OptionField) -> String {
        match field {
            OptionField::Theme => format!("{:?}", self.editing_config.theme),
            OptionField::TocEnabled => format!("{}", self.editing_config.toc.enabled),
            OptionField::TocSide => format!("{:?}", self.editing_config.toc.side),
            OptionField::TocWidth => format!("{}", self.editing_config.toc.width),
            OptionField::SafeMode => format!("{}", self.editing_config.security.safe_mode),
            OptionField::NoExec => format!("{}", self.editing_config.security.no_exec),
            OptionField::Utf8Graphics => {
                format!("{}", self.editing_config.render.use_utf8_graphics)
            }
            OptionField::ShowScrollbar => {
                format!("{}", self.editing_config.render.show_scrollbar)
            }
            OptionField::SkipFrontMatter => {
                format!("{}", self.editing_config.render.skip_front_matter)
            }
            #[cfg(feature = "watch")]
            OptionField::WatchEnabled => format!("{}", self.editing_config.watch.enabled),
            #[cfg(feature = "watch")]
            OptionField::AutoReload => format!("{}", self.editing_config.watch.auto_reload),
            #[cfg(feature = "git")]
            OptionField::GitDiff => format!("{}", self.editing_config.git.diff),
            #[cfg(feature = "images")]
            OptionField::ImagesEnabled => format!("{}", self.editing_config.images.enabled),
        }
    }

    pub fn cancel(&mut self) {
        self.editing_config = self.original_config.clone();
    }

    pub fn get_config(&self) -> Config {
        self.editing_config.clone()
    }

    pub fn next_button(&mut self) {
        self.focused_button = match self.focused_button {
            DialogButton::Cancel => DialogButton::Ok,
            DialogButton::Ok => DialogButton::Save,
            DialogButton::Save => DialogButton::Cancel,
        };
    }

    pub fn prev_button(&mut self) {
        self.focused_button = match self.focused_button {
            DialogButton::Cancel => DialogButton::Save,
            DialogButton::Ok => DialogButton::Cancel,
            DialogButton::Save => DialogButton::Ok,
        };
    }
}
