//! Background diff computation worker thread

use crossbeam_channel::{Receiver, Sender};
use mdx_core::diff::DiffGutter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// Request to compute diff for a document
#[derive(Debug, Clone)]
pub struct DiffRequest {
    pub doc_id: usize,
    pub path: PathBuf,
    pub rev: u64,
    pub current_text: String,
}

/// Result of diff computation
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub doc_id: usize,
    pub rev: u64,
    pub gutter: DiffGutter,
}

/// Diff worker handle
pub struct DiffWorker {
    request_tx: Sender<DiffRequest>,
    result_rx: Receiver<DiffResult>,
    _worker_thread: thread::JoinHandle<()>,
}

impl DiffWorker {
    /// Spawn a new diff worker thread
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = crossbeam_channel::unbounded();
        let (result_tx, result_rx) = crossbeam_channel::unbounded();

        let worker_thread = thread::spawn(move || {
            worker_loop(request_rx, result_tx);
        });

        Self {
            request_tx,
            result_rx,
            _worker_thread: worker_thread,
        }
    }

    /// Send a diff request
    pub fn request_diff(&self, req: DiffRequest) {
        let _ = self.request_tx.send(req);
    }

    /// Try to receive a diff result (non-blocking)
    pub fn try_recv_result(&self) -> Option<DiffResult> {
        self.result_rx.try_recv().ok()
    }
}

/// Worker thread main loop
fn worker_loop(request_rx: Receiver<DiffRequest>, result_tx: Sender<DiffResult>) {
    let mut pending: HashMap<usize, DiffRequest> = HashMap::new();
    let mut last_process = Instant::now();
    let coalesce_window = Duration::from_millis(75);

    loop {
        // Try to receive requests with timeout
        match request_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(req) => {
                // Coalesce: keep only the latest request per doc_id
                pending.insert(req.doc_id, req);
                last_process = Instant::now();
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Timeout - check if we should process pending requests
                if !pending.is_empty() && last_process.elapsed() >= coalesce_window {
                    // Process all pending requests
                    for (_doc_id, req) in pending.drain() {
                        if let Some(result) = compute_diff(req) {
                            let _ = result_tx.send(result);
                        }
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                // Channel closed, exit
                break;
            }
        }
    }
}

/// Compute diff for a request
fn compute_diff(req: DiffRequest) -> Option<DiffResult> {
    #[cfg(feature = "git")]
    {
        use mdx_core::diff::{diff_gutter_from_text, DiffGutter};
        use mdx_core::git::get_base_text_gix;

        // Get base text from git
        let base_text = match get_base_text_gix(&req.path) {
            Ok(Some(text)) => text,
            Ok(None) | Err(_) => {
                // Not in git or error - return empty gutter
                let line_count = req.current_text.lines().count().max(1);
                return Some(DiffResult {
                    doc_id: req.doc_id,
                    rev: req.rev,
                    gutter: DiffGutter::empty(line_count),
                });
            }
        };

        // Compute diff
        let gutter = diff_gutter_from_text(&base_text, &req.current_text);

        Some(DiffResult {
            doc_id: req.doc_id,
            rev: req.rev,
            gutter,
        })
    }

    #[cfg(not(feature = "git"))]
    {
        use mdx_core::diff::DiffGutter;
        let line_count = req.current_text.lines().count().max(1);
        Some(DiffResult {
            doc_id: req.doc_id,
            rev: req.rev,
            gutter: DiffGutter::empty(line_count),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_worker_spawns() {
        let _worker = DiffWorker::spawn();
        // Worker should spawn without crashing
    }

    #[test]
    fn test_worker_processes_request() {
        let worker = DiffWorker::spawn();

        let req = DiffRequest {
            doc_id: 0,
            path: PathBuf::from("/tmp/test.md"),
            rev: 1,
            current_text: "line 1\nline 2\n".to_string(),
        };

        worker.request_diff(req);

        // Wait a bit for processing
        thread::sleep(Duration::from_millis(200));

        // Should receive a result
        let result = worker.try_recv_result();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.doc_id, 0);
        assert_eq!(result.rev, 1);
    }

    #[test]
    fn test_worker_coalesces_requests() {
        let worker = DiffWorker::spawn();

        // Send multiple requests for the same doc
        for i in 1..=5 {
            let req = DiffRequest {
                doc_id: 0,
                path: PathBuf::from("/tmp/test.md"),
                rev: i,
                current_text: format!("revision {}\n", i),
            };
            worker.request_diff(req);
        }

        // Wait for coalescing window + processing
        thread::sleep(Duration::from_millis(200));

        // Should receive only one result (the latest)
        let mut count = 0;
        let mut last_rev = 0;

        while let Some(result) = worker.try_recv_result() {
            count += 1;
            last_rev = result.rev;
        }

        // Due to coalescing, we should get only 1 result
        assert_eq!(count, 1);
        // And it should be the latest revision
        assert_eq!(last_rev, 5);
    }
}
