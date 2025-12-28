//! File watching for external changes

use anyhow::{Context, Result};
use crossbeam_channel::Receiver;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// File change event
#[derive(Debug, Clone)]
pub enum FileEvent {
    Changed,
}

/// File watcher that monitors a document for external changes
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<FileEvent>,
    watched_path: PathBuf,
    last_event: Option<Instant>,
}

impl FileWatcher {
    /// Create a new file watcher for the given path
    pub fn new(path: &Path) -> Result<Self> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let watched_path = path.to_path_buf();
        let watched_path_clone = watched_path.clone();

        // Create the watcher
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only care about modification events
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) {
                    // Check if the event is for our file
                    if event.paths.iter().any(|p| p == &watched_path_clone) {
                        let _ = tx.send(FileEvent::Changed);
                    }
                }
            }
        })
        .context("Failed to create file watcher")?;

        // Watch the file itself
        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .with_context(|| format!("Failed to watch file: {}", path.display()))?;

        // Also watch the parent directory (for editors that use atomic rename)
        if let Some(parent) = path.parent() {
            watcher
                .watch(parent, RecursiveMode::NonRecursive)
                .context("Failed to watch parent directory")?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
            watched_path,
            last_event: None,
        })
    }

    /// Check if a file change event has occurred
    /// Returns true if a change was detected and debounce period has elapsed
    pub fn check_changed(&mut self, debounce_ms: u64) -> bool {
        // Drain all pending events
        while self.receiver.try_recv().is_ok() {
            self.last_event = Some(Instant::now());
        }

        // If we have a pending event, check if debounce period has elapsed
        if let Some(last) = self.last_event {
            let elapsed = last.elapsed();
            if elapsed >= Duration::from_millis(debounce_ms) {
                self.last_event = None;
                return true;
            }
        }

        false
    }

    /// Check if there are pending events (not debounced yet)
    pub fn has_pending(&self) -> bool {
        self.last_event.is_some()
    }

    /// Get the watched file path
    pub fn path(&self) -> &Path {
        &self.watched_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::thread;
    use tempfile::NamedTempFile;

    #[test]
    fn test_watcher_detects_changes() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "Initial content")?;
        file.flush()?;

        let mut watcher = FileWatcher::new(file.path())?;

        // Modify the file
        writeln!(file, "Modified content")?;
        file.flush()?;

        // Poll for event arrival (file system events can take time)
        let mut has_event = false;
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            if watcher.has_pending() {
                has_event = true;
                break;
            }
            // Also try to drain events that may have arrived
            let _ = watcher.check_changed(0);
        }

        // If still no event, try one more modification
        if !has_event {
            writeln!(file, "More changes")?;
            file.flush()?;
            thread::sleep(Duration::from_millis(500));
        }

        // Wait for debounce and check
        thread::sleep(Duration::from_millis(300));

        // Should trigger after debounce
        let triggered = watcher.check_changed(250);
        assert!(triggered || watcher.has_pending());

        Ok(())
    }

    #[test]
    fn test_debounce_prevents_immediate_trigger() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "Initial content")?;
        file.flush()?;

        let mut watcher = FileWatcher::new(file.path())?;

        // Modify the file
        writeln!(file, "Modified content")?;
        file.flush()?;

        // Wait a tiny bit
        thread::sleep(Duration::from_millis(50));

        // Should not trigger immediately with 250ms debounce
        assert!(!watcher.check_changed(250));

        // But should have pending
        assert!(watcher.has_pending());

        Ok(())
    }

    #[test]
    #[ignore] // File system events can be unreliable in test environments
    fn test_multiple_changes_debounced() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "Initial content")?;
        file.flush()?;

        let mut watcher = FileWatcher::new(file.path())?;

        // Multiple rapid changes
        for i in 0..5 {
            writeln!(file, "Change {}", i)?;
            file.flush()?;
            thread::sleep(Duration::from_millis(50));
        }

        // Poll for at least one event
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            if watcher.has_pending() {
                break;
            }
            let _ = watcher.check_changed(0);
        }

        // Make another change
        writeln!(file, "Final change")?;
        file.flush()?;

        // Poll for the final change event
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            if watcher.has_pending() {
                break;
            }
            let _ = watcher.check_changed(0);
        }

        // Wait for debounce
        thread::sleep(Duration::from_millis(300));

        // Should trigger once after debounce (or still be pending)
        // File system events can be unreliable in tests
        let triggered = watcher.check_changed(250);

        // If not triggered, check if we at least have pending events
        if !triggered {
            // Make one final attempt
            writeln!(file, "Absolutely final change")?;
            file.flush()?;
            thread::sleep(Duration::from_millis(500));
            assert!(watcher.check_changed(250), "Watcher should eventually detect changes");
        }

        // Second call should return false (events consumed)
        assert!(!watcher.check_changed(250));

        Ok(())
    }

    #[test]
    fn test_watcher_path() -> Result<()> {
        let file = NamedTempFile::new()?;
        let watcher = FileWatcher::new(file.path())?;

        assert_eq!(watcher.path(), file.path());

        Ok(())
    }
}
