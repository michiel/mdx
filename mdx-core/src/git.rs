//! Git integration using gix

use anyhow::Result;
use std::path::{Path, PathBuf};

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

    // Try to discover and open repository (use parent directory for discovery)
    let discover_path = abs_path.parent().unwrap_or(&abs_path);
    let repo = discover(discover_path);
    if repo.is_err() {
        return Ok(None);
    }
    let repo = repo.unwrap();

    // Get working directory
    let workdir = match repo.workdir() {
        Some(wd) => wd.to_path_buf(),
        None => return Ok(None), // Bare repo
    };

    // Compute relative path from workdir to file
    let rel_path = abs_path.strip_prefix(&workdir).ok();
    if rel_path.is_none() {
        return Ok(None);
    }
    let rel_path = rel_path.unwrap().to_path_buf();

    // Check if file is gitignored
    if is_path_ignored(&repo, &rel_path) {
        return Ok(None);
    }

    Ok(Some(RepoContext {
        repo,
        workdir,
        rel_path,
    }))
}

/// Check if a path should be excluded from git diff display
///
/// Returns true if the file should be skipped.
/// For simplicity and reliability, we only show git diff for tracked files (in the index).
/// Untracked files are treated as if they were ignored for diff purposes.
#[cfg(feature = "git")]
fn is_path_ignored(repo: &gix::Repository, rel_path: &Path) -> bool {
    use bstr::ByteSlice;

    // Check if path is in index (tracked files are never ignored)
    if let Ok(index) = repo.index() {
        // Convert Path to BStr for gix API
        let path_str = rel_path.to_string_lossy();
        let path_bytes = path_str.as_bytes();
        if index.entry_by_path(path_bytes.as_bstr()).is_some() {
            // File is tracked in git index - show diff
            return false;
        }
    }

    // File is not tracked - don't show diff for untracked files
    // This is a simple, conservative approach that works reliably without
    // needing complex .gitignore pattern matching
    true
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
    let commit = head.peel_to_commit();
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
    let entry = tree.lookup_entry_by_path(rel_path);
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

