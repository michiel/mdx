//! Configuration management for mdx

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::security::SecurityEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderConfig {
    pub use_utf8_graphics: bool,
    pub show_scrollbar: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            use_utf8_graphics: true,
            show_scrollbar: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: ThemeVariant,
    pub toc: TocConfig,
    pub editor: EditorConfig,
    pub security: SecurityConfig,
    pub render: RenderConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    pub safe_mode: bool,
    pub no_exec: bool,
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
#[serde(default)]
pub struct ImageConfig {
    pub enabled: bool,
    pub allow_absolute: bool,
    pub allow_remote: bool,
    pub max_bytes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeVariant::Dark,
            toc: TocConfig::default(),
            editor: EditorConfig::default(),
            security: SecurityConfig::default(),
            render: RenderConfig::default(),
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

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            safe_mode: true,
            no_exec: true,
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
            enabled: false,
            allow_absolute: false,
            allow_remote: false,
            max_bytes: 10 * 1024 * 1024,
        }
    }
}

impl Config {
    /// Get the platform-specific config file path
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "mdx")
            .map(|proj_dirs| proj_dirs.config_dir().join("mdx.toml"))
    }

    /// Load configuration from file, falling back to defaults if missing
    /// Returns (Config, Vec<SecurityEvent>) where events track security-related settings
    pub fn load() -> Result<(Self, Vec<SecurityEvent>)> {
        let warnings = Vec::new();
        let config_path = Self::config_path();

        if let Some(path) = config_path {
            if path.exists() {
                // Check config file permissions (Unix only)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let metadata = std::fs::metadata(&path)?;
                    let perms = metadata.permissions();
                    if perms.mode() & 0o002 != 0 {
                        anyhow::bail!(
                            "Config file {} is world-writable (insecure permissions)",
                            path.display()
                        );
                    }
                }

                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config file: {}", path.display()))?;

                let config: Config = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

                #[cfg(feature = "images")]
                {
                    let mut config = config;
                    if config.security.safe_mode {
                        config.images.enabled = false;
                    }
                    return Ok((config, warnings));
                }

                #[cfg(not(feature = "images"))]
                return Ok((config, warnings));
            }
        }

        // No config file, use defaults
        let config = Self::default();
        #[cfg(feature = "images")]
        {
            let mut config = config;
            if config.security.safe_mode {
                config.images.enabled = false;
            }
            return Ok((config, warnings));
        }

        #[cfg(not(feature = "images"))]
        Ok((config, warnings))
    }

    /// Load from a specific path (for testing)
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        // Check config file permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path)?;
            let perms = metadata.permissions();
            if perms.mode() & 0o002 != 0 {
                anyhow::bail!(
                    "Config file {} is world-writable (insecure permissions)",
                    path.display()
                );
            }
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        #[cfg(feature = "images")]
        {
            let mut config = config;
            if config.security.safe_mode {
                config.images.enabled = false;
            }
            return Ok(config);
        }

        #[cfg(not(feature = "images"))]
        Ok(config)
    }

    /// Write the default configuration to the default config file path
    /// Returns an error if the file already exists or cannot be written
    pub fn write_default() -> Result<PathBuf> {
        let config_path = Self::config_path().context("Could not determine config file path")?;

        // Check if file already exists
        if config_path.exists() {
            anyhow::bail!(
                "Config file already exists at: {}\nRemove it first if you want to reinitialize.",
                config_path.display()
            );
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        // Serialize default config to TOML
        let config = Self::default();
        let toml_string =
            toml::to_string_pretty(&config).context("Failed to serialize config to TOML")?;

        // Write to file
        std::fs::write(&config_path, toml_string)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        // Set proper permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&config_path)?.permissions();
            perms.set_mode(0o644); // rw-r--r--
            std::fs::set_permissions(&config_path, perms)?;
        }

        Ok(config_path)
    }

    /// Save configuration to the default config file path
    /// Overwrites the existing file if it exists
    pub fn save_to_file(config: &Self) -> Result<()> {
        let config_path = Self::config_path().context("Could not determine config file path")?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        // Serialize config to TOML
        let toml_string =
            toml::to_string_pretty(config).context("Failed to serialize config to TOML")?;

        // Write to file
        std::fs::write(&config_path, toml_string)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        // Set proper permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&config_path)?.permissions();
            perms.set_mode(0o644); // rw-r--r--
            std::fs::set_permissions(&config_path, perms)?;
        }

        Ok(())
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
    fn security_defaults() {
        let config = Config::default();
        assert!(config.security.safe_mode);
        assert!(config.security.no_exec);
        if cfg!(feature = "images") {
            assert!(!config.images.enabled);
            assert!(!config.images.allow_absolute);
            assert!(!config.images.allow_remote);
            assert!(config.images.max_bytes > 0);
        }
    }

    #[test]
    fn test_load_missing_config() -> Result<()> {
        // Loading should return defaults when file doesn't exist
        let (config, _warnings) = Config::load()?;
        assert_eq!(config.theme, ThemeVariant::Dark);
        Ok(())
    }

    #[test]
    fn test_load_valid_toml() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let mut toml_content = String::from(
            "theme = \"Light\"\n\
\n\
[toc]\n\
enabled = false\n\
side = \"Right\"\n\
width = 40\n\
\n\
[editor]\n\
command = \"nvim\"\n\
args = [\"+{line}\", \"{file}\"]\n",
        );

        if cfg!(feature = "watch") {
            toml_content.push_str("\n[watch]\nenabled = true\nauto_reload = false\n");
        }

        if cfg!(feature = "git") {
            toml_content.push_str("\n[git]\ndiff = true\nbase = \"Head\"\n");
        }

        if cfg!(feature = "images") {
            toml_content.push_str("\n[images]\nenabled = true\nallow_absolute = true\nallow_remote = false\nmax_bytes = 2048\n");
        }

        file.write_all(toml_content.as_bytes())?;

        let config = Config::load_from(file.path())?;
        assert_eq!(config.theme, ThemeVariant::Light);
        assert!(!config.toc.enabled);
        assert_eq!(config.toc.side, TocSide::Right);
        assert_eq!(config.toc.width, 40);
        assert_eq!(config.editor.command, "nvim");
        assert!(config.images.allow_absolute);
        assert!(!config.images.allow_remote);
        assert_eq!(config.images.max_bytes, 2048);

        Ok(())
    }

    #[test]
    fn test_load_partial_toml() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let mut toml_content = String::from(
            "theme = \"Light\"\n\
\n\
[toc]\n\
enabled = true\n\
side = \"Left\"\n\
width = 32\n\
\n\
[editor]\n\
command = \"$EDITOR\"\n\
args = [\"+{line}\", \"{file}\"]\n",
        );

        if cfg!(feature = "watch") {
            toml_content.push_str("\n[watch]\nenabled = true\nauto_reload = false\n");
        }

        if cfg!(feature = "git") {
            toml_content.push_str("\n[git]\ndiff = true\nbase = \"Head\"\n");
        }

        if cfg!(feature = "images") {
            toml_content.push_str("\n[images]\nenabled = true\nallow_absolute = true\nallow_remote = false\nmax_bytes = 2048\n");
        }

        file.write_all(toml_content.as_bytes())?;

        let config = Config::load_from(file.path())?;
        assert_eq!(config.theme, ThemeVariant::Light);
        assert!(config.toc.enabled);
        assert!(config.images.allow_absolute);
        assert!(!config.images.allow_remote);
        assert_eq!(config.images.max_bytes, 2048);

        Ok(())
    }

    #[test]
    fn security_safe_mode_disables_images() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        let toml_content = "theme = \"Dark\"\n\
\n\
[security]\n\
safe_mode = true\n\
no_exec = false\n\
\n\
[toc]\n\
enabled = false\n\
side = \"Left\"\n\
width = 32\n\
\n\
[editor]\n\
command = \"$EDITOR\"\n\
args = [\"+{line}\", \"{file}\"]\n\
\n\
[images]\n\
enabled = true\n\
allow_absolute = true\n\
allow_remote = true\n\
max_bytes = 2048\n";

        file.write_all(toml_content.as_bytes())?;

        let config = Config::load_from(file.path())?;
        assert!(config.security.safe_mode);
        assert!(!config.images.enabled);

        Ok(())
    }

    #[test]
    fn test_load_invalid_toml_returns_error() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"invalid toml [[[syntax").unwrap();

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
            assert!(p.to_string_lossy().ends_with("mdx.toml"));
        }
    }

    #[test]
    fn test_theme_variant_serialization() -> Result<()> {
        let config = Config {
            theme: ThemeVariant::Light,
            ..Default::default()
        };

        let toml_str = toml::to_string(&config)?;
        assert!(toml_str.contains("Light"));

        let parsed: Config = toml::from_str(&toml_str)?;
        assert_eq!(parsed.theme, ThemeVariant::Light);

        Ok(())
    }
}
