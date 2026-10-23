//! Text-to-speech on a background thread. Windows: WinRT speech synthesis via the `tts` crate.
//! Linux: speech-dispatcher's `spd-say`, else `espeak-ng` or `espeak`, run as programs (nothing to link); without
//! any of them the app stays quiet and Settings says why.

use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::sync::Arc;

pub enum TtsCmd {
    Say(String),
    Rate(f32),
    Volume(f32),
}

/// What happened the last time Elite Nexus tried to speak, for Settings (under Test voice).
#[derive(Debug, Clone, Default, PartialEq)]
pub enum VoiceStatus {
    #[default]
    Untried,
    /// Spoke through this engine.
    Spoke(&'static str),
    /// No speech program was found (Linux).
    Missing,
    /// This engine failed, with what it said.
    Failed(&'static str, String),
}

#[derive(Clone)]
pub struct Speaker {
    tx: Sender<TtsCmd>,
    status: Arc<Mutex<VoiceStatus>>,
}

impl Speaker {
    pub fn start() -> Speaker {
        let (tx, rx) = crossbeam_channel::unbounded();
        let status = Arc::new(Mutex::new(VoiceStatus::Untried));
        let st = status.clone();
        let _ = std::thread::Builder::new().name("nexus-tts".into()).spawn(move || run(rx, st));
        Speaker { tx, status }
    }

    pub fn say(&self, text: impl Into<String>) {
        let _ = self.tx.send(TtsCmd::Say(text.into()));
    }

    pub fn set_rate(&self, rate: f32) {
        let _ = self.tx.send(TtsCmd::Rate(rate));
    }

    pub fn set_volume(&self, v: f32) {
        let _ = self.tx.send(TtsCmd::Volume(v));
    }

    pub fn status(&self) -> VoiceStatus {
        self.status.lock().clone()
    }
}

#[cfg(windows)]
fn run(rx: Receiver<TtsCmd>, status: Arc<Mutex<VoiceStatus>>) {
    const ENGINE: &str = "Windows speech";
    let mut engine = match tts::Tts::default() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("text-to-speech unavailable: {e}");
            // Drain so senders never block; each line says why it isn't spoken.
            while let Ok(cmd) = rx.recv() {
                if matches!(cmd, TtsCmd::Say(_)) {
                    *status.lock() = VoiceStatus::Failed(ENGINE, e.to_string());
                }
            }
            return;
        }
    };
    let (min_rate, max_rate, normal_rate) = (engine.min_rate(), engine.max_rate(), engine.normal_rate());
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCmd::Say(text) => {
                // Queue rather than interrupt, so back-to-back alerts are all heard.
                *status.lock() = match engine.speak(text, false) {
                    Ok(_) => VoiceStatus::Spoke(ENGINE),
                    Err(e) => {
                        tracing::warn!("tts speak failed: {e}");
                        VoiceStatus::Failed(ENGINE, e.to_string())
                    }
                };
            }
            TtsCmd::Rate(r) => {
                let rate = (normal_rate * r).clamp(min_rate, max_rate);
                let _ = engine.set_rate(rate);
            }
            TtsCmd::Volume(v) => {
                let _ = engine.set_volume(v.clamp(0.0, 1.0));
            }
        }
    }
}

/// Speech programs, in the order they're tried.
#[cfg(not(windows))]
const PROGRAMS: [&str; 3] = ["spd-say", "espeak-ng", "espeak"];

#[cfg(not(windows))]
fn run(rx: Receiver<TtsCmd>, status: Arc<Mutex<VoiceStatus>>) {
    let program = find_program(std::env::var_os("PATH"));
    if program.is_none() {
        tracing::warn!("text-to-speech unavailable: none of {PROGRAMS:?} found");
    }
    let (mut rate, mut volume) = (1.0, 1.0);
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCmd::Say(text) => {
                let Some(program) = program else {
                    *status.lock() = VoiceStatus::Missing;
                    continue;
                };
                *status.lock() = match speak(program, &text, rate, volume) {
                    Ok(()) => VoiceStatus::Spoke(program),
                    Err(e) => {
                        tracing::warn!("text-to-speech ({program}) failed: {e}");
                        VoiceStatus::Failed(program, e)
                    }
                };
            }
            TtsCmd::Rate(r) => rate = r,
            TtsCmd::Volume(v) => volume = v,
        }
    }
}

/// The first speech program on `PATH` (an executable file).
#[cfg(not(windows))]
fn find_program(path: Option<std::ffi::OsString>) -> Option<&'static str> {
    use std::os::unix::fs::PermissionsExt;
    let dirs: Vec<std::path::PathBuf> = path.map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let runnable = |p: std::path::PathBuf| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    PROGRAMS.into_iter().find(|p| dirs.iter().any(|d| runnable(d.join(p))))
}

/// Speaks one line and waits for it (so back-to-back alerts queue rather than interrupt), up to 2 minutes (only a
/// stuck program takes that long). The error is
/// what the program printed, or how it ended.
#[cfg(not(windows))]
fn speak(program: &str, text: &str, rate: f32, volume: f32) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut cmd = Command::new(program);
    if program == "spd-say" {
        cmd.args(["-w", "-r", &spd_rate(rate).to_string(), "-i", &spd_volume(volume).to_string(), "--", text]);
    } else {
        // espeak reads the text from stdin, so nothing in it can be taken for an option.
        cmd.args(["-s", &espeak_wpm(rate).to_string(), "-a", &espeak_amplitude(volume).to_string(), "--stdin"]).stdin(Stdio::piped());
    }
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(exit)) if exit.success() => return Ok(()),
            Ok(Some(exit)) => {
                let mut said = String::new();
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_string(&mut said);
                }
                let said = said.trim();
                return Err(if said.is_empty() { format!("it ended with {exit}") } else { said.to_string() });
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("it didn't finish within 2 minutes".into());
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// The speed setting (1.0 = normal) as spd-say's rate, -100 to 100 with 0 normal.
#[cfg(not(windows))]
fn spd_rate(rate: f32) -> i32 {
    ((rate - 1.0) * 100.0).clamp(-100.0, 100.0).round() as i32
}

/// The volume setting (0 to 1) as spd-say's, -100 (silent) to 100 (full).
#[cfg(not(windows))]
fn spd_volume(volume: f32) -> i32 {
    (volume.clamp(0.0, 1.0) * 200.0 - 100.0).round() as i32
}

/// The speed setting as espeak's words per minute (175 normal, 80 to 450).
#[cfg(not(windows))]
fn espeak_wpm(rate: f32) -> i32 {
    (175.0 * rate).clamp(80.0, 450.0).round() as i32
}

/// The volume setting as espeak's amplitude, 0 to 100 (its normal; above that it distorts).
#[cfg(not(windows))]
fn espeak_amplitude(volume: f32) -> i32 {
    (volume.clamp(0.0, 1.0) * 100.0).round() as i32
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn spd_say_ranges() {
        assert_eq!((spd_rate(1.0), spd_rate(1.5), spd_rate(3.0), spd_rate(0.5)), (0, 50, 100, -50));
        assert_eq!((spd_volume(1.0), spd_volume(0.5), spd_volume(0.0)), (100, 0, -100));
    }

    #[test]
    fn espeak_ranges() {
        assert_eq!((espeak_wpm(1.0), espeak_wpm(2.0), espeak_wpm(0.1)), (175, 350, 80));
        assert_eq!((espeak_amplitude(1.0), espeak_amplitude(0.5), espeak_amplitude(2.0)), (100, 50, 100));
    }

    #[test]
    fn speech_program_from_path() {
        let root = std::env::temp_dir().join(format!("nexus-tts-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).expect("dir");
        std::fs::create_dir_all(&b).expect("dir");
        let path = |dirs: &[&std::path::Path]| Some(std::env::join_paths(dirs).expect("PATH"));
        assert_eq!(find_program(path(&[&a, &b])), None);
        assert_eq!(find_program(None), None);
        let program = |file: std::path::PathBuf, mode: u32| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&file, "").expect("file");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).expect("mode");
        };
        program(b.join("espeak-ng"), 0o755);
        assert_eq!(find_program(path(&[&a, &b])), Some("espeak-ng"));
        // A file that can't be run doesn't count.
        program(a.join("spd-say"), 0o644);
        assert_eq!(find_program(path(&[&a, &b])), Some("espeak-ng"));
        // speech-dispatcher first, wherever it is on PATH.
        program(b.join("spd-say"), 0o755);
        assert_eq!(find_program(path(&[&a, &b])), Some("spd-say"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failing_program_says_why() {
        // `false` exits with an error and prints nothing.
        let e = speak("false", "test", 1.0, 1.0).expect_err("fails");
        assert!(e.starts_with("it ended with"), "{e}");
        assert!(speak("no-such-speech-program", "test", 1.0, 1.0).is_err());
    }
}
