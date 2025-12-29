//! Configuration management for mdx

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: ThemeVariant,
    pub toc: TocConfig,
    pub editor: EditorConfig,
    #[cfg(feature = "watch")]
    pub watch: WatchConfig,
    #[cfg(feature = "git")]
    pub git: GitConfig,
    #[cfg(feature = "images")]
    pub images: ImageConfig,
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

#[cfg(feature = "images")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub enabled: ImageEnabled,
    pub backend: ImageBackend,
    pub max_width_percent: u8,
    pub max_height_percent: u8,
    pub allow_remote: bool,
    pub cache_dir: Option<PathBuf>,
}

#[cfg(feature = "images")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageEnabled {
    Auto,
    Always,
    Never,
}

#[cfg(feature = "images")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageBackend {
    Auto,
    Kitty,
    ITerm2,
    Sixel,
    None,
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
            #[cfg(feature = "images")]
            images: ImageConfig::default(),
        }
    }
}

impl Default for TocConfig {
    fn default() -> Self {
        Self {
            enabled: false,
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

#[cfg(feature = "images")]
impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: ImageEnabled::Auto,
            backend: ImageBackend::Auto,
            max_width_percent: 90,
            max_height_percent: 50,
            allow_remote: false,
            cache_dir: None,
        }
    }
}

impl Config {
    /// Get the platform-specific config file path
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "mdx")
            .map(|proj_dirs| proj_dirs.config_dir().join("mdx.yaml"))
    }

    /// Load configuration from file, falling back to defaults if missing
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if let Some(path) = config_path {
            if path.exists() {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config file: {}", path.display()))?;

                let config: Config = serde_yaml::from_str(&content)
                    .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

                return Ok(config);
            }
        }

        // No config file, use defaults
        Ok(Self::default())
    }

    /// Load from a specific path (for testing)
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, ThemeVariant::Dark);
        assert!(!config.toc.enabled);
        assert_eq!(config.toc.side, TocSide::Left);
        assert_eq!(config.toc.width, 32);
        assert_eq!(config.editor.command, "$EDITOR");
    }

    #[test]
    fn test_load_missing_config() -> Result<()> {
        // Loading should return defaults when file doesn't exist
        let config = Config::load()?;
        assert_eq!(config.theme, ThemeVariant::Dark);
        Ok(())
    }

    #[test]
    fn test_load_valid_yaml() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let mut yaml_content = String::from(
            "theme: Light\n\
toc:\n  enabled: false\n  side: Right\n  width: 40\n\
editor:\n  command: nvim\n  args: [\"+{line}\", \"{file}\"]\n"
        );

        if cfg!(feature = "watch") {
            yaml_content.push_str("watch:\n  enabled: true\n  auto_reload: false\n");
        }

        if cfg!(feature = "git") {
            yaml_content.push_str("git:\n  diff: true\n  base: Head\n");
        }

        if cfg!(feature = "images") {
            yaml_content.push_str("images:\n  enabled: auto\n  backend: auto\n  max_width_percent: 90\n  max_height_percent: 50\n  allow_remote: false\n");
        }

        let yaml_content = yaml_content;
        file.write_all(yaml_content.as_bytes())?;

        let config = Config::load_from(file.path())?;
        assert_eq!(config.theme, ThemeVariant::Light);
        assert!(!config.toc.enabled);
        assert_eq!(config.toc.side, TocSide::Right);
        assert_eq!(config.toc.width, 40);
        assert_eq!(config.editor.command, "nvim");

        Ok(())
    }

    #[test]
    fn test_load_partial_yaml() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let mut yaml_content = String::from(
            "theme: Light\n\
toc:\n  enabled: true\n  side: Left\n  width: 32\n\
editor:\n  command: \"$EDITOR\"\n  args: [\"+{line}\", \"{file}\"]\n"
        );

        if cfg!(feature = "watch") {
            yaml_content.push_str("watch:\n  enabled: true\n  auto_reload: false\n");
        }

        if cfg!(feature = "git") {
            yaml_content.push_str("git:\n  diff: true\n  base: Head\n");
        }

        if cfg!(feature = "images") {
            yaml_content.push_str("images:\n  enabled: auto\n  backend: auto\n  max_width_percent: 90\n  max_height_percent: 50\n  allow_remote: false\n");
        }

        file.write_all(yaml_content.as_bytes())?;

        let config = Config::load_from(file.path())?;
        assert_eq!(config.theme, ThemeVariant::Light);
        assert!(config.toc.enabled);

        Ok(())
    }

    #[test]
    fn test_load_invalid_yaml_returns_error() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"invalid: yaml: syntax:").unwrap();

        let result = Config::load_from(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_path_returns_some() {
        let path = Config::config_path();
        // Should return Some on all platforms
        assert!(path.is_some());
        if let Some(p) = path {
            assert!(p.to_string_lossy().contains("mdx"));
            assert!(p.to_string_lossy().ends_with("mdx.yaml"));
        }
    }

    #[test]
    fn test_theme_variant_serialization() -> Result<()> {
        let config = Config {
            theme: ThemeVariant::Light,
            ..Default::default()
        };

        let yaml = serde_yaml::to_string(&config)?;
        assert!(yaml.contains("Light"));

        let parsed: Config = serde_yaml::from_str(&yaml)?;
        assert_eq!(parsed.theme, ThemeVariant::Light);

        Ok(())
    }
}
