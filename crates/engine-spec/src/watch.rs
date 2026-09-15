//! §16.4's own text: "`engine-spec` owns file-watching directly (a
//! `notify`-crate watcher, no `pyo3` needed to detect a file change)."
//!
//! `ViewWatcher` wraps a real `notify::RecommendedWatcher` on one file
//! path, delivering change notifications through a channel a caller
//! polls. §16.4's "triggers reconciliation on the main thread between
//! frames" means the *caller's* own frame loop calls `poll_changed()`
//! once per tick -- this module owns detecting the change, not a loop
//! of its own; wiring that poll into a real running `engine-py` `App`
//! is deliberately deferred (see this step's own `LOG.md`).

use std::path::Path;
use std::sync::mpsc::{Receiver, channel};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

pub struct ViewWatcher {
    // Held only to keep the OS-level watch alive -- notify stops
    // delivering events as soon as its `Watcher` is dropped.
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<notify::Event>>,
}

impl ViewWatcher {
    /// Starts watching `path` (a single file, non-recursive -- a
    /// `view.yaml` is one file, not a directory tree).
    pub fn watch(path: &Path) -> notify::Result<Self> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })?;
        watcher.watch(path, RecursiveMode::NonRecursive)?;
        Ok(Self {
            _watcher: watcher,
            events: rx,
        })
    }

    /// Drains every filesystem event queued since the last call and
    /// reports whether at least one real (non-error) event arrived --
    /// a non-blocking channel drain, cheap to call once per frame even
    /// when nothing has changed.
    pub fn poll_changed(&self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            if event.is_ok() {
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{Duration, Instant};

    /// Real proof, not "the API compiles": actually writes to a real
    /// file on disk and confirms the watcher notices -- filesystem
    /// watchers deliver asynchronously, so this polls in a bounded loop
    /// rather than a single fixed sleep, failing definitively (not
    /// flakily) if no event ever arrives within a generous window.
    #[test]
    fn watcher_detects_a_real_write_to_the_watched_file() {
        let path = std::env::temp_dir().join(format!(
            "engine_spec_watch_test_{}_{}.yaml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, "id: root\nkind: Container\n").expect("create test file");

        let watcher = ViewWatcher::watch(&path).expect("failed to start watching a real file");

        // No event yet -- the file hasn't changed since watching started.
        std::thread::sleep(Duration::from_millis(50));
        assert!(
            !watcher.poll_changed(),
            "poll_changed reported a change before any write happened"
        );

        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("reopen test file for writing");
            writeln!(file, "# a real change").expect("write to test file");
        }

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut detected = false;
        while Instant::now() < deadline {
            if watcher.poll_changed() {
                detected = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = std::fs::remove_file(&path);
        assert!(
            detected,
            "watcher never reported the real write within 5s -- file-watching isn't working"
        );
    }
}
