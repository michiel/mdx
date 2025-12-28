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
#[cfg(feature = "git")]
pub fn open_repo_for_path(path: &Path) -> Result<Option<RepoContext>> {
    use gix::discover;

    // Get the absolute path of the file
    let abs_path = path.canonicalize().ok();
    if abs_path.is_none() {
        return Ok(None);
    }
    let abs_path = abs_path.unwrap();

    // Try to discover and open repository
    let repo = discover(&abs_path);
    if repo.is_err() {
        return Ok(None);
    }
    let repo = repo.unwrap();

    // Get working directory
    let workdir = match repo.work_dir() {
        Some(wd) => wd.to_path_buf(),
        None => return Ok(None), // Bare repo
    };

    // Compute relative path from workdir to file
    let rel_path = abs_path.strip_prefix(&workdir).ok();
    if rel_path.is_none() {
        return Ok(None);
    }
    let rel_path = rel_path.unwrap().to_path_buf();

    Ok(Some(RepoContext {
        repo,
        workdir,
        rel_path,
    }))
}

#[cfg(not(feature = "git"))]
pub fn open_repo_for_path(_path: &Path) -> Result<Option<RepoContext>> {
    Ok(None)
}

/// Read file text from HEAD
#[cfg(feature = "git")]
pub fn read_head_file_text(repo: &gix::Repository, rel_path: &Path) -> Result<Option<String>> {
    use bstr::ByteSlice;

    // Get HEAD reference
    let head = repo.head();
    if head.is_err() {
        // No HEAD (unborn)
        return Ok(None);
    }
    let mut head = head.unwrap();

    // Try to peel to commit
    let commit = head.peel_to_commit_in_place();
    if commit.is_err() {
        // Unborn HEAD or invalid
        return Ok(None);
    }
    let commit = commit.unwrap();

    // Get tree
    let tree = commit.tree();
    if tree.is_err() {
        return Ok(None);
    }
    let tree = tree.unwrap();

    // Lookup entry by path (use Path directly, not BString)
    let mut buf = Vec::new();
    let entry = tree.lookup_entry_by_path(rel_path, &mut buf);
    if entry.is_err() {
        // File not in tree (new file)
        return Ok(Some(String::new()));
    }
    let entry = entry.unwrap();

    if entry.is_none() {
        // File not found
        return Ok(Some(String::new()));
    }
    let entry = entry.unwrap();

    // Get the object
    let object = entry.object();
    if object.is_err() {
        return Ok(None);
    }
    let object = object.unwrap();

    // Read blob data
    let data = object.data.as_slice();

    // Decode as UTF-8 (lossy)
    let text = data.to_str_lossy().to_string();

    Ok(Some(text))
}

#[cfg(not(feature = "git"))]
pub fn read_head_file_text(_repo: &gix::Repository, _rel_path: &Path) -> Result<Option<String>> {
    Ok(None)
}


/// Get base text from git HEAD using gix
#[cfg(feature = "git")]
pub fn get_base_text_gix(file_path: &Path) -> Result<Option<String>> {
    // Open repository
    let repo_ctx = open_repo_for_path(file_path)?;
    if repo_ctx.is_none() {
        return Ok(None);
    }
    let repo_ctx = repo_ctx.unwrap();

    // Read file from HEAD
    read_head_file_text(&repo_ctx.repo, &repo_ctx.rel_path)
}

/// Get base text from git HEAD using subprocess (Stage 12 temporary implementation)
/// Deprecated: Use get_base_text_gix instead
#[cfg(feature = "git")]
#[allow(dead_code)]
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
