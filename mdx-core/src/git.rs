//! Git integration using gix

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Repository context for a file
#[derive(Debug)]
pub struct RepoContext {
    pub repo: gix::Repository,
    pub workdir: PathBuf,
    pub rel_path: PathBuf,
}

/// Open repository containing the given path
pub fn open_repo_for_path(_path: &Path) -> Result<Option<RepoContext>> {
    // TODO: Implementation in Stage 13
    // This will walk up parents looking for .git
    Ok(None)
}

/// Read file text from HEAD
pub fn read_head_file_text(_repo: &gix::Repository, _rel_path: &Path) -> Result<Option<String>> {
    // TODO: Implementation in Stage 13
    // This will:
    // 1. Resolve HEAD -> commit -> tree
    // 2. Lookup entry by path
    // 3. Read blob data
    // 4. Decode as UTF-8
    Ok(None)
}

/// Get base text from git HEAD using subprocess (Stage 12 temporary implementation)
/// This will be replaced with gix in Stage 13
#[cfg(feature = "git")]
pub fn get_base_text_subprocess(file_path: &Path) -> Result<Option<String>> {
    // Check if we're in a git repo by trying to find the root
    let repo_root = find_git_root(file_path)?;
    if repo_root.is_none() {
        return Ok(None);
    }

    let repo_root = repo_root.unwrap();

    // Get relative path from repo root
    let rel_path = file_path
        .strip_prefix(&repo_root)
        .context("Failed to compute relative path")?;

    // Try to get file from HEAD using git show
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .arg("show")
        .arg(format!("HEAD:{}", rel_path.display()))
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
            } else {
                // File not in HEAD (new file)
                Ok(Some(String::new()))
            }
        }
        Err(_) => {
            // Git command failed
            Ok(None)
        }
    }
}

/// Find git repository root by walking up parents
fn find_git_root(start_path: &Path) -> Result<Option<PathBuf>> {
    let mut current = start_path
        .parent()
        .unwrap_or(start_path)
        .to_path_buf();

    loop {
        let git_dir = current.join(".git");
        if git_dir.exists() {
            return Ok(Some(current));
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Ok(None),
        }
    }
}
