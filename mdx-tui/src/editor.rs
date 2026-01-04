//! External editor integration

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Resolve the editor command from config or environment
pub fn resolve_editor_command(config_command: &str) -> String {
    if config_command == "$EDITOR" {
        // Try environment variable
        std::env::var("EDITOR").unwrap_or_else(|_| {
            // Fallback chain: nvim -> vim -> nano
            if which("nvim") {
                "nvim".to_string()
            } else if which("vim") {
                "vim".to_string()
            } else {
                "nano".to_string()
            }
        })
    } else {
        config_command.to_string()
    }
}

/// Check if a command exists in PATH
fn which(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Expand template variables in editor arguments
pub fn expand_template(template: &str, file_path: &Path, line: usize) -> String {
    let file_str = file_path.to_string_lossy();
    template
        .replace("{file}", &file_str)
        .replace("{line}", &line.to_string())
}

/// Launch an external editor with the given file and line
pub fn launch_editor(command: &str, args: &[String], file_path: &Path, line: usize) -> Result<()> {
    // Suspend the terminal (will be done by caller)
    // The caller should call terminal::restore() before this and terminal::init() after

    // Expand template variables in all arguments
    let expanded_args: Vec<String> = args
        .iter()
        .map(|arg| expand_template(arg, file_path, line))
        .collect();

    // Spawn the editor process
    let status = Command::new(command)
        .args(&expanded_args)
        .status()
        .with_context(|| format!("Failed to launch editor: {}", command))?;

    if !status.success() {
        anyhow::bail!("Editor exited with status: {}", status);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_expand_template() {
        let path = PathBuf::from("/tmp/test.md");
        let result = expand_template("+{line} {file}", &path, 42);
        assert_eq!(result, "+42 /tmp/test.md");
    }

    #[test]
    fn test_expand_template_vscode() {
        let path = PathBuf::from("/home/user/doc.md");
        let result = expand_template("--goto {file}:{line}:0", &path, 10);
        assert_eq!(result, "--goto /home/user/doc.md:10:0");
    }

    #[test]
    fn test_resolve_editor_command_literal() {
        let result = resolve_editor_command("nvim");
        assert_eq!(result, "nvim");
    }

    #[test]
    fn test_resolve_editor_command_env() {
        // If $EDITOR is set, it should use it
        // Otherwise falls back to available editors
        let result = resolve_editor_command("$EDITOR");
        // Result depends on environment, just check it's not empty
        assert!(!result.is_empty());
    }
}
