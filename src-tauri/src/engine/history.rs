//! Persistent operation history (`history.jsonl`) plus per-job output logs in `jobs/`.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use super::types::HistoryEntry;
use crate::error::{AppError, AppResult};

/// Default number of entries kept; configurable in Settings.
const KEEP: usize = 500;

pub struct History {
    path: PathBuf,
    logs_dir: PathBuf,
    entries: Mutex<VecDeque<HistoryEntry>>,
    limit: std::sync::atomic::AtomicUsize,
}

impl History {
    pub fn load(path: PathBuf, logs_dir: PathBuf) -> Self {
        let mut entries: VecDeque<HistoryEntry> = VecDeque::new();
        if let Ok(f) = File::open(&path) {
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                if let Ok(e) = serde_json::from_str::<HistoryEntry>(&line) {
                    entries.push_back(e);
                }
            }
        }
        let history = Self { path, logs_dir, entries: Mutex::new(VecDeque::new()), limit: KEEP.into() };
        let overflow = entries.len() > KEEP;
        while entries.len() > KEEP {
            if let Some(old) = entries.pop_front() {
                history.remove_log(&old.id);
            }
        }
        *history.entries.lock().expect("fresh lock") = entries;
        if overflow {
            let _ = history.rewrite();
        }
        history
    }

    fn remove_log(&self, id: &str) {
        // Only ever deletes the exact log file this app created for that job id.
        if id.chars().all(|c| c.is_ascii_digit() || c == '-') {
            let _ = std::fs::remove_file(self.logs_dir.join(format!("{id}.jsonl")));
        }
    }

    fn rewrite(&self) -> std::io::Result<()> {
        let entries = self.entries.lock().map_err(|_| std::io::Error::other("lock"))?;
        let mut f = File::create(&self.path)?;
        for e in entries.iter() {
            writeln!(f, "{}", serde_json::to_string(e)?)?;
        }
        Ok(())
    }

    /// Changes how many entries are kept, dropping (and deleting the logs of) the oldest ones.
    pub fn set_limit(&self, limit: usize) {
        let limit = limit.clamp(50, 5000);
        self.limit.store(limit, std::sync::atomic::Ordering::Relaxed);
        let dropped: Vec<HistoryEntry> = {
            let Ok(mut entries) = self.entries.lock() else { return };
            let n = entries.len().saturating_sub(limit);
            entries.drain(..n).collect()
        };
        if !dropped.is_empty() {
            for old in &dropped {
                self.remove_log(&old.id);
            }
            let _ = self.rewrite();
        }
    }

    pub fn append(&self, entry: &HistoryEntry) {
        let limit = self.limit.load(std::sync::atomic::Ordering::Relaxed);
        let overflow: Vec<HistoryEntry> = {
            let Ok(mut entries) = self.entries.lock() else { return };
            entries.push_back(entry.clone());
            let n = entries.len().saturating_sub(limit);
            entries.drain(..n).collect()
        };
        if !overflow.is_empty() {
            for old in &overflow {
                self.remove_log(&old.id);
            }
            let _ = self.rewrite();
        } else if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            if let Ok(line) = serde_json::to_string(entry) {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    pub fn recent(&self, limit: usize) -> Vec<HistoryEntry> {
        self.entries
            .lock()
            .map(|e| e.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.lock().map(|e| e.iter().any(|x| x.id == id)).unwrap_or(false)
    }

    pub fn clear(&self) -> AppResult<()> {
        let ids: Vec<String> = {
            let mut entries = self.entries.lock().map_err(|_| AppError::Other("history lock poisoned".into()))?;
            entries.drain(..).map(|e| e.id).collect()
        };
        for id in ids {
            self.remove_log(&id);
        }
        File::create(&self.path)?;
        Ok(())
    }
}
