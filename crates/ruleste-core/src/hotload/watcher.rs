//! Mtime-based hot-reload watchers for maps, resources and Wasm plugins.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Tracks the modification times of a set of files and reports changes.
#[derive(Debug, Default)]
pub struct MtimeWatcher {
    files: HashMap<PathBuf, Option<SystemTime>>,
}

impl MtimeWatcher {
    pub fn new() -> MtimeWatcher {
        MtimeWatcher::default()
    }

    /// Adds a file to watch. Returns the previous mtime if the file was
    /// already tracked.
    pub fn watch(&mut self, path: impl Into<PathBuf>) -> bool {
        let path = path.into();
        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        match self.files.insert(path, mtime) {
            Some(prev) => prev != mtime,
            None => true,
        }
    }

    /// Marks a file as up-to-date at its current mtime.
    pub fn mark_seen(&mut self, path: &Path) {
        let mtime = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
        self.files.insert(path.to_path_buf(), mtime);
    }

    /// Returns the set of watched files whose mtime changed since last seen.
    pub fn changed(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|(path, seen)| {
                let current = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
                **seen != current
            })
            .map(|(path, _)| path.clone())
            .collect()
    }
}
