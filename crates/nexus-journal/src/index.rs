//! Compact index of every journal event (EDDiscovery-style journal grid) — 24 bytes per event;
//! the JSON itself stays on disk and is read on demand.

use crate::time::Timestamp;
use rustc_hash::FxHashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct IndexEntry {
    pub ts: i64,
    pub file: u32,
    pub offset: u32,
    pub len: u32,
    pub name: u16,
}

#[derive(Debug, Default)]
pub struct JournalIndex {
    pub files: Vec<PathBuf>,
    pub entries: Vec<IndexEntry>,
    names: Vec<String>,
    name_ids: FxHashMap<String, u16>,
    counts: Vec<u32>,
}

impl JournalIndex {
    pub fn add_file(&mut self, path: PathBuf) -> u32 {
        if let Some(i) = self.files.iter().position(|p| *p == path) {
            return i as u32;
        }
        self.files.push(path);
        (self.files.len() - 1) as u32
    }

    fn intern(&mut self, name: &str) -> u16 {
        if let Some(&id) = self.name_ids.get(name) {
            return id;
        }
        let id = self.names.len().min(u16::MAX as usize) as u16;
        self.names.push(name.to_string());
        self.counts.push(0);
        self.name_ids.insert(name.to_string(), id);
        id
    }

    pub fn push(&mut self, ts: Timestamp, file: u32, offset: u64, len: usize, name: &str) {
        let id = self.intern(name);
        if let Some(c) = self.counts.get_mut(id as usize) {
            *c += 1;
        }
        self.entries.push(IndexEntry { ts: ts.0, file, offset: offset as u32, len: len as u32, name: id });
    }

    pub fn name(&self, id: u16) -> &str {
        self.names.get(id as usize).map(String::as_str).unwrap_or("?")
    }

    /// Event names with counts, most frequent first.
    pub fn name_counts(&self) -> Vec<(u16, &str, u32)> {
        let mut v: Vec<(u16, &str, u32)> = self.names.iter().enumerate().map(|(i, n)| (i as u16, n.as_str(), self.counts[i])).collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(b.1)));
        v
    }

    pub fn id_of(&self, name: &str) -> Option<u16> {
        self.name_ids.get(name).copied()
    }

    /// Reads the raw JSON line of an entry from disk.
    pub fn read_line(&self, e: &IndexEntry) -> Option<String> {
        let path = self.files.get(e.file as usize)?;
        let mut f = std::fs::File::open(path).ok()?;
        f.seek(SeekFrom::Start(u64::from(e.offset))).ok()?;
        let mut buf = vec![0u8; e.len as usize];
        f.read_exact(&mut buf).ok()?;
        Some(String::from_utf8_lossy(&buf).into_owned())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
