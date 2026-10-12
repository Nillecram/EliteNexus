//! Live tailing of the journal and companion files.
//!
//! Plain polling on a dedicated thread: the game keeps the journal open and appends to it, and
//! directory-change notifications on Windows are unreliable for that pattern. A stat call every
//! 100 ms on a dozen files is negligible CPU and gives ~100 ms worst-case latency, which beats
//! every tool that waits for change notifications or polls once a second.

use crate::companion::{Companion, CompanionKind};
use crate::events::Event;
use crate::paths::{list_journal_files, JournalFile};
use crate::raw::{LineSource, RawEvent};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

/// A journal event observed live (written after the watcher started).
#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub raw: RawEvent,
    pub event: Event,
}

#[derive(Debug, Clone)]
pub enum WatchMsg {
    Event(LiveEvent),
    Companion(Companion),
    /// The watcher started tailing a (new) journal file.
    FileOpened(JournalFile),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub dir: PathBuf,
    pub poll: Duration,
    /// Continue tailing this file from this byte offset (where the history load stopped).
    pub resume: Option<(PathBuf, u64)>,
    /// Emit every companion file once at start-up so the consumer has the current state.
    pub emit_initial_companions: bool,
}

impl WatcherConfig {
    pub fn new(dir: PathBuf) -> Self {
        WatcherConfig { dir, poll: Duration::from_millis(100), resume: None, emit_initial_companions: true }
    }
}

pub struct Watcher {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Watcher {
    pub fn spawn(cfg: WatcherConfig, tx: Sender<WatchMsg>) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = std::thread::Builder::new()
            .name("journal-watcher".into())
            .spawn(move || run(cfg, tx, stop2))?;
        Ok(Watcher { stop, handle: Some(handle) })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Tail {
    info: JournalFile,
    file: File,
    offset: u64,
    pending: Vec<u8>,
    file_index: u32,
}

impl Tail {
    fn open(info: JournalFile, offset: u64, file_index: u32) -> std::io::Result<Self> {
        let file = File::open(&info.path)?;
        Ok(Tail { info, file, offset, pending: Vec::new(), file_index })
    }

    /// Reads whatever the game appended and returns complete lines with their byte offsets.
    fn read_lines(&mut self) -> std::io::Result<Vec<(u64, String)>> {
        let len = self.file.metadata()?.len();
        if len < self.offset {
            // File was truncated/replaced (should not happen with journals) — start over.
            self.offset = 0;
            self.pending.clear();
        }
        if len == self.offset {
            return Ok(Vec::new());
        }
        self.file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::with_capacity((len - self.offset) as usize);
        (&mut self.file).take(len - self.offset).read_to_end(&mut buf)?;
        let start_of_pending = self.offset - self.pending.len() as u64;
        self.offset += buf.len() as u64;
        self.pending.extend_from_slice(&buf);

        let mut out = Vec::new();
        let mut line_start = 0usize;
        while let Some(nl) = self.pending[line_start..].iter().position(|&b| b == b'\n') {
            let end = line_start + nl;
            let line = String::from_utf8_lossy(&self.pending[line_start..end]).trim_end_matches('\r').to_string();
            if !line.trim().is_empty() {
                out.push((start_of_pending + line_start as u64, line));
            }
            line_start = end + 1;
        }
        self.pending.drain(..line_start);
        Ok(out)
    }
}

#[derive(Default)]
struct CompanionTracker {
    seen: HashMap<CompanionKind, (SystemTime, u64)>,
}

impl CompanionTracker {
    /// Reads the file if it changed since we last parsed it successfully.
    fn check(&mut self, dir: &std::path::Path, kind: CompanionKind, force: bool) -> Option<Companion> {
        let path = dir.join(kind.file_name());
        let meta = std::fs::metadata(&path).ok()?;
        let stamp = (meta.modified().ok()?, meta.len());
        if !force && self.seen.get(&kind) == Some(&stamp) {
            return None;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let parsed = Companion::parse(kind, &text)?;
        // Only remember the stamp once the content parsed, so a half-written file is retried.
        self.seen.insert(kind, stamp);
        Some(parsed)
    }

    fn prime(&mut self, dir: &std::path::Path) {
        for kind in CompanionKind::ALL {
            let path = dir.join(kind.file_name());
            if let Ok(meta) = std::fs::metadata(&path)
                && let Ok(m) = meta.modified() {
                    self.seen.insert(kind, (m, meta.len()));
                }
        }
    }
}

fn run(cfg: WatcherConfig, tx: Sender<WatchMsg>, stop: Arc<AtomicBool>) {
    let mut companions = CompanionTracker::default();
    if cfg.emit_initial_companions {
        for kind in CompanionKind::ALL {
            if let Some(c) = companions.check(&cfg.dir, kind, true)
                && tx.send(WatchMsg::Companion(c)).is_err() {
                    return;
                }
        }
    } else {
        companions.prime(&cfg.dir);
    }

    let mut tail: Option<Tail> = None;
    if let Some((path, offset)) = &cfg.resume {
        let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some((started, part)) = crate::paths::parse_journal_file_name(&file_name) {
            let info = JournalFile { path: path.clone(), file_name, started, part, size: 0 };
            match Tail::open(info, *offset, u32::MAX) {
                Ok(t) => tail = Some(t),
                Err(e) => {
                    let _ = tx.send(WatchMsg::Error(format!("cannot open {}: {e}", path.display())));
                }
            }
        }
    }

    let mut last_scan = Instant::now() - Duration::from_secs(60);

    while !stop.load(Ordering::Relaxed) {
        // 1. Newer journal file?
        if last_scan.elapsed() >= Duration::from_millis(1000) {
            last_scan = Instant::now();
            match list_journal_files(&cfg.dir) {
                Ok(files) => {
                    let newest_index = files.len().saturating_sub(1) as u32;
                    if let Some(newest) = files.last() {
                        let is_new = match &tail {
                            None => true,
                            Some(t) => newest.path != t.info.path && (newest.started, newest.part) > (t.info.started, t.info.part),
                        };
                        if is_new {
                            // Drain the old file completely before switching.
                            if let Some(t) = tail.as_mut() {
                                emit_lines(t, &tx, &cfg, &mut companions);
                            }
                            let offset = if tail.is_none() && cfg.resume.is_none() { newest.size } else { 0 };
                            match Tail::open(newest.clone(), offset, newest_index) {
                                Ok(t) => {
                                    let _ = tx.send(WatchMsg::FileOpened(t.info.clone()));
                                    tail = Some(t);
                                }
                                Err(e) => {
                                    let _ = tx.send(WatchMsg::Error(format!("cannot open journal: {e}")));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(WatchMsg::Error(format!("cannot list journal folder {}: {e}", cfg.dir.display())));
                }
            }
        }

        // 2. New journal lines.
        if let Some(t) = tail.as_mut()
            && !emit_lines(t, &tx, &cfg, &mut companions) {
                return;
            }

        // 3. Companion files changed on their own (Status.json mostly).
        for kind in CompanionKind::ALL {
            if let Some(c) = companions.check(&cfg.dir, kind, false)
                && tx.send(WatchMsg::Companion(c)).is_err() {
                    return;
                }
        }

        std::thread::sleep(cfg.poll);
    }
}

/// Returns false when the receiver hung up.
fn emit_lines(t: &mut Tail, tx: &Sender<WatchMsg>, cfg: &WatcherConfig, companions: &mut CompanionTracker) -> bool {
    let lines = match t.read_lines() {
        Ok(l) => l,
        Err(e) => {
            let _ = tx.send(WatchMsg::Error(format!("read error on {}: {e}", t.info.path.display())));
            return true;
        }
    };
    for (offset, line) in lines {
        let Some(raw) = RawEvent::new(&line, LineSource { file: t.file_index, offset }) else { continue };
        let event = match crate::raw::parse_event_lenient(&raw.name, &raw.line) {
            Ok(e) => e,
            Err(err) => {
                tracing::debug!(%err, event = %raw.name, "live journal line failed typed parse");
                Event::Other
            }
        };
        let companion = CompanionKind::from_event_name(&raw.name);
        if tx.send(WatchMsg::Event(LiveEvent { raw, event })).is_err() {
            return false;
        }
        // The game writes the companion file just before the journal event; read it right away
        // so consumers see Market.json etc. together with the event that announced it.
        if let Some(kind) = companion
            && let Some(c) = companions.check(&cfg.dir, kind, true)
                && tx.send(WatchMsg::Companion(c)).is_err() {
                    return false;
                }
    }
    true
}
