//! Persistent FIFO queues: one append-only JSON-lines log per service in `data_dir\uplink\`.
//!
//! Every push/update/remove appends one small record, so a crash loses at most the record being
//! written. The log is replayed and compacted when opened, and compacted again whenever dead
//! records outnumber live ones. If the file cannot be written the queue keeps working in memory.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(tag = "op")]
enum Record<T> {
    #[serde(rename = "add")]
    Add { id: u64, item: T },
    #[serde(rename = "upd")]
    Update { id: u64, item: T },
    #[serde(rename = "del")]
    Del { id: u64 },
}

#[derive(Serialize)]
#[serde(tag = "op")]
enum RecordRef<'a, T> {
    #[serde(rename = "add")]
    Add { id: u64, item: &'a T },
    #[serde(rename = "upd")]
    Update { id: u64, item: &'a T },
    #[serde(rename = "del")]
    Del { id: u64 },
}

/// A queued item with its stable id.
#[derive(Debug, Clone)]
pub(crate) struct Entry<T> {
    pub id: u64,
    pub item: T,
}

pub(crate) struct DiskQueue<T> {
    path: Option<PathBuf>,
    items: VecDeque<Entry<T>>,
    next_id: u64,
    log: Option<BufWriter<File>>,
    dead_records: usize,
    max_items: usize,
}

impl<T: Serialize + DeserializeOwned> DiskQueue<T> {
    /// In-memory only (tests, unwritable data dir).
    pub fn memory(max_items: usize) -> Self {
        DiskQueue { path: None, items: VecDeque::new(), next_id: 1, log: None, dead_records: 0, max_items: max_items.max(1) }
    }

    /// Opens (or creates) the queue file at `path`, replaying whatever was queued before.
    pub fn open(path: PathBuf, max_items: usize) -> Self {
        let mut q = Self::memory(max_items);
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            tracing::warn!(dir = %dir.display(), %e, "uplink: cannot create queue directory, queue is memory-only");
            return q;
        }
        q.load(&path);
        q.path = Some(path);
        q.compact();
        q
    }

    fn load(&mut self, path: &Path) {
        let Ok(file) = File::open(path) else { return };
        let mut bad = 0usize;
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Record<T>>(&line) {
                Ok(Record::Add { id, item }) => {
                    self.next_id = self.next_id.max(id + 1);
                    self.items.push_back(Entry { id, item });
                }
                Ok(Record::Update { id, item }) => {
                    if let Some(e) = self.items.iter_mut().find(|e| e.id == id) {
                        e.item = item;
                    }
                }
                Ok(Record::Del { id }) => self.items.retain(|e| e.id != id),
                Err(_) => bad += 1,
            }
        }
        if bad > 0 {
            tracing::warn!(file = %path.display(), bad, "uplink: skipped unreadable queue records");
        }
        while self.items.len() > self.max_items {
            self.items.pop_front();
        }
    }

    /// Rewrites the log with only the live items.
    fn compact(&mut self) {
        let Some(path) = self.path.clone() else { return };
        self.log = None; // close before replacing the file (Windows)
        let tmp = path.with_extension("tmp");
        let written = (|| -> std::io::Result<()> {
            let mut w = BufWriter::new(File::create(&tmp)?);
            for e in &self.items {
                serde_json::to_writer(&mut w, &RecordRef::Add { id: e.id, item: &e.item })?;
                w.write_all(b"\n")?;
            }
            w.flush()?;
            drop(w);
            std::fs::rename(&tmp, &path)
        })();
        if let Err(e) = written {
            tracing::warn!(file = %path.display(), %e, "uplink: cannot write queue file, queue is memory-only");
            let _ = std::fs::remove_file(&tmp);
            self.path = None;
            return;
        }
        self.dead_records = 0;
        match OpenOptions::new().append(true).open(&path) {
            Ok(f) => self.log = Some(BufWriter::new(f)),
            Err(e) => {
                tracing::warn!(file = %path.display(), %e, "uplink: cannot open queue file, queue is memory-only");
                self.path = None;
            }
        }
    }

    fn append(&mut self, rec: RecordRef<'_, T>) {
        if self.log.is_none() {
            return;
        }
        match serde_json::to_string(&rec) {
            Ok(line) => self.append_line(&line),
            Err(e) => tracing::warn!(%e, "uplink: cannot serialise queue record"),
        }
    }

    fn append_line(&mut self, line: &str) {
        let Some(w) = self.log.as_mut() else { return };
        let res = w.write_all(line.as_bytes()).and_then(|_| w.write_all(b"\n")).and_then(|_| w.flush());
        if let Err(e) = res {
            tracing::warn!(%e, "uplink: queue write failed, queue is memory-only from now on");
            self.log = None;
            self.path = None;
        }
    }

    fn maybe_compact(&mut self) {
        if self.path.is_some() && self.dead_records > 64 && self.dead_records > self.items.len() * 2 {
            self.compact();
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entry<T>> {
        self.items.iter()
    }

    pub fn front(&self) -> Option<&Entry<T>> {
        self.items.front()
    }

    /// Appends an item; returns how many old items were evicted to respect the size limit.
    pub fn push(&mut self, item: T) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.append(RecordRef::Add { id, item: &item });
        self.items.push_back(Entry { id, item });
        let mut evicted = 0;
        while self.items.len() > self.max_items {
            if let Some(old) = self.items.pop_front() {
                self.append(RecordRef::Del { id: old.id });
                self.dead_records += 2;
                evicted += 1;
            }
        }
        self.maybe_compact();
        evicted
    }

    pub fn remove(&mut self, id: u64) -> Option<T> {
        let pos = self.items.iter().position(|e| e.id == id)?;
        let e = self.items.remove(pos)?;
        self.append(RecordRef::Del { id });
        self.dead_records += 2;
        self.maybe_compact();
        Some(e.item)
    }

    pub fn update(&mut self, id: u64, f: impl FnOnce(&mut T)) {
        let Some(pos) = self.items.iter().position(|e| e.id == id) else { return };
        f(&mut self.items[pos].item);
        if self.log.is_some() {
            match serde_json::to_string(&RecordRef::Update { id, item: &self.items[pos].item }) {
                Ok(line) => self.append_line(&line),
                Err(e) => tracing::warn!(%e, "uplink: cannot serialise queue record"),
            }
        }
        self.dead_records += 1;
        self.maybe_compact();
    }

    /// Keeps only items for which `keep` returns true; returns the removed items.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) -> Vec<T> {
        let mut removed = Vec::new();
        let mut kept = VecDeque::with_capacity(self.items.len());
        let items = std::mem::take(&mut self.items);
        for e in items {
            if keep(&e.item) {
                kept.push_back(e);
            } else {
                removed.push(e);
            }
        }
        self.items = kept;
        for e in &removed {
            self.append(RecordRef::Del { id: e.id });
            self.dead_records += 2;
        }
        self.maybe_compact();
        removed.into_iter().map(|e| e.item).collect()
    }

    pub fn clear(&mut self) -> usize {
        let n = self.items.len();
        self.items.clear();
        self.dead_records += n * 2;
        self.compact();
        n
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A unique scratch directory for one test (removed by the guard).
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "nexus-uplink-test-{}-{}-{}",
                tag,
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn survives_reopen() {
        let dir = TempDir::new("queue");
        let path = dir.0.join("q.jsonl");
        {
            let mut q: DiskQueue<String> = DiskQueue::open(path.clone(), 100);
            q.push("a".into());
            q.push("b".into());
            q.push("c".into());
            let id_b = q.iter().nth(1).unwrap().id;
            q.remove(id_b);
            let id_c = q.iter().nth(1).unwrap().id;
            q.update(id_c, |s| s.push('!'));
        }
        let q: DiskQueue<String> = DiskQueue::open(path.clone(), 100);
        let items: Vec<_> = q.iter().map(|e| e.item.clone()).collect();
        assert_eq!(items, vec!["a".to_string(), "c!".to_string()]);
        // compacted on open: exactly one record per live item
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn bounded_and_tolerates_garbage() {
        let dir = TempDir::new("queue2");
        let path = dir.0.join("q.jsonl");
        std::fs::write(&path, "garbage\n{\"op\":\"add\",\"id\":7,\"item\":\"x\"}\n").unwrap();
        let mut q: DiskQueue<String> = DiskQueue::open(path, 2);
        assert_eq!(q.len(), 1);
        q.push("y".into());
        assert_eq!(q.push("z".into()), 1);
        let items: Vec<_> = q.iter().map(|e| e.item.clone()).collect();
        assert_eq!(items, vec!["y".to_string(), "z".to_string()]);
        assert!(q.iter().all(|e| e.id > 7));
        let removed = q.retain(|s| s != "y");
        assert_eq!(removed, vec!["y".to_string()]);
        assert_eq!(q.clear(), 1);
        assert!(q.is_empty());
    }
}
