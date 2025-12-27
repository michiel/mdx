//! Configuration management for mdx

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: ThemeVariant,
    pub toc: TocConfig,
    pub editor: EditorConfig,
    #[cfg(feature = "watch")]
    pub watch: WatchConfig,
    #[cfg(feature = "git")]
    pub git: GitConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeVariant {
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocConfig {
    pub enabled: bool,
    pub side: TocSide,
    pub width: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TocSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[cfg(feature = "watch")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    pub enabled: bool,
    pub auto_reload: bool,
}

#[cfg(feature = "git")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub diff: bool,
    pub base: GitBase,
}

#[cfg(feature = "git")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitBase {
    Head,
    Index,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeVariant::Dark,
            toc: TocConfig::default(),
            editor: EditorConfig::default(),
            #[cfg(feature = "watch")]
            watch: WatchConfig::default(),
            #[cfg(feature = "git")]
            git: GitConfig::default(),
        }
    }
}

impl Default for TocConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            side: TocSide::Left,
            width: 32,
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: "$EDITOR".to_string(),
            args: vec!["+{line}".to_string(), "{file}".to_string()],
        }
    }
}

#[cfg(feature = "watch")]
impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_reload: false,
        }
    }
}

#[cfg(feature = "git")]
impl Default for GitConfig {
    fn default() -> Self {
        Self {
            diff: true,
            base: GitBase::Head,
        }
    }
}
