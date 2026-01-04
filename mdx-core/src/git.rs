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
    let abs_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    // Try to discover and open repository (use parent directory for discovery)
    let discover_path = abs_path.parent().unwrap_or(&abs_path);
    let repo = match discover(discover_path) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    // Get working directory
    let workdir = match repo.workdir() {
        Some(wd) => wd.to_path_buf(),
        None => return Ok(None), // Bare repo
    };

    // Compute relative path from workdir to file
    let rel_path = match abs_path.strip_prefix(&workdir) {
        Ok(p) => p.to_path_buf(),
        Err(_) => return Ok(None),
    };

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
    let mut head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(None), // No HEAD (unborn)
    };

    // Try to peel to commit
    let commit = match head.peel_to_commit() {
        Ok(c) => c,
        Err(_) => return Ok(None), // Unborn HEAD or invalid
    };

    // Get tree
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    // Lookup entry by path (use Path directly, not BString)
    let entry = match tree.lookup_entry_by_path(rel_path) {
        Ok(Some(e)) => e,
        Ok(None) => return Ok(Some(String::new())), // File not found
        Err(_) => return Ok(Some(String::new())),   // File not in tree (new file)
    };

    // Get the object
    let object = match entry.object() {
        Ok(obj) => obj,
        Err(_) => return Ok(None),
    };

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
    let repo_ctx = match open_repo_for_path(file_path)? {
        Some(ctx) => ctx,
        None => return Ok(None),
    };

    // Read file from HEAD
    read_head_file_text(&repo_ctx.repo, &repo_ctx.rel_path)
}
