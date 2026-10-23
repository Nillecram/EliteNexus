//! Text-to-speech on a background thread. Windows: WinRT speech synthesis via the `tts` crate.
//! Linux: speech-dispatcher's `spd-say`, else `espeak-ng` or `espeak`, run as programs (nothing to link); without
//! any of them the app stays quiet and Settings says why.
//! macOS: the system's `say`, in the voice chosen in System Settings › Accessibility › Spoken Content.

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
    /// No speech program was found (Linux; macOS always has `say`, and Windows reports its speech failing instead).
    #[cfg_attr(windows, allow(dead_code))]
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

/// Linux speech programs, in the order they're tried.
#[cfg(not(windows))]
const LINUX_PROGRAMS: [&str; 3] = ["spd-say", "espeak-ng", "espeak"];

/// This system's speech programs, in the order they're tried. macOS's own `say` by its full path: an app started
/// from Finder gets a minimal PATH.
#[cfg(not(windows))]
const PROGRAMS: &[&str] = if cfg!(target_os = "macos") { &["/usr/bin/say"] } else { &LINUX_PROGRAMS };

#[cfg(not(windows))]
fn run(rx: Receiver<TtsCmd>, status: Arc<Mutex<VoiceStatus>>) {
    let program = find_program(std::env::var_os("PATH"), PROGRAMS);
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

/// The first of `programs` that can be run: a full path as it is, a name on `PATH`.
#[cfg(not(windows))]
fn find_program<'a>(path: Option<std::ffi::OsString>, programs: &[&'a str]) -> Option<&'a str> {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    let dirs: Vec<PathBuf> = path.map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let runnable = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    programs.iter().copied().find(|p| if Path::new(p).is_absolute() { runnable(Path::new(p)) } else { dirs.iter().any(|d| runnable(&d.join(p))) })
}

/// How a speech program gets one line: its arguments, and what it reads on standard input (`None`: nothing, the text
/// is an argument). Text on standard input can't be taken for an option, even when it starts with "-".
#[cfg(not(windows))]
fn invocation(program: &str, text: &str, rate: f32, volume: f32) -> (Vec<String>, Option<String>) {
    match program {
        // -w waits until the line is spoken, so back-to-back alerts queue rather than interrupt.
        "spd-say" => (vec!["-w".into(), "-r".into(), spd_rate(rate).to_string(), "-i".into(), spd_volume(volume).to_string(), "--".into(), text.into()], None),
        // macOS: say returns once the line is spoken.
        "say" => (say_rate(rate).map(|wpm| vec!["-r".into(), wpm.to_string()]).unwrap_or_default(), Some(say_input(text, volume))),
        _ => (vec!["-s".into(), espeak_wpm(rate).to_string(), "-a".into(), espeak_amplitude(volume).to_string(), "--stdin".into()], Some(text.into())),
    }
}

/// Speaks one line and waits for it (so back-to-back alerts queue rather than interrupt), up to 2 minutes (only a
/// stuck program takes that long). The error is what the program printed, or how it ended.
#[cfg(not(windows))]
fn speak(program: &str, text: &str, rate: f32, volume: f32) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let name = std::path::Path::new(program).file_name().and_then(|n| n.to_str()).unwrap_or(program);
    let (args, input) = invocation(name, text, rate, volume);
    let mut cmd = Command::new(program);
    cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Written and closed before waiting: say and espeak read to the end of their input before they speak. A failed
    // write (the program quit early) still leaves its exit status to wait for.
    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), input) {
        let _ = stdin.write_all(input.as_bytes());
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

/// The speed setting (1.0 = normal) as `say`'s words per minute; `None` at normal speed, which keeps the rate set
/// for the voice in System Settings.
#[cfg(not(windows))]
fn say_rate(rate: f32) -> Option<u32> {
    /// About the rate macOS voices speak at by default.
    const NORMAL_WPM: f32 = 180.0;
    ((rate - 1.0).abs() > 0.01).then(|| (NORMAL_WPM * rate).clamp(90.0, 720.0).round() as u32)
}

/// The line for `say`: below full volume it starts with the `[[volm]]` speech command (0 to 1). Brackets in the
/// text itself are loosened so a name can't be read as a speech command.
#[cfg(not(windows))]
fn say_input(text: &str, volume: f32) -> String {
    let text = text.replace("[[", "[ [").replace("]]", "] ]");
    let volume = volume.clamp(0.0, 1.0);
    if volume < 0.995 { format!("[[volm {volume:.2}]] {text}") } else { text }
}

// Linux CI runs these too, the macOS ones included (say through a stand-in script).
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
    fn say_rate_and_volume() {
        assert_eq!((say_rate(1.0), say_rate(1.5), say_rate(0.5), say_rate(10.0)), (None, Some(270), Some(90), Some(720)));
        assert_eq!(say_input("Hull critical", 1.0), "Hull critical");
        assert_eq!(say_input("Hull critical", 0.5), "[[volm 0.50]] Hull critical");
        assert_eq!(say_input("Body [[rate 1]] A 1", 1.0), "Body [ [rate 1] ] A 1");
    }

    #[test]
    fn speech_program_from_path() {
        let root = std::env::temp_dir().join(format!("nexus-tts-{}", std::process::id()));
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).expect("dir");
        std::fs::create_dir_all(&b).expect("dir");
        let path = |dirs: &[&std::path::Path]| Some(std::env::join_paths(dirs).expect("PATH"));
        assert_eq!(find_program(path(&[&a, &b]), &LINUX_PROGRAMS), None);
        assert_eq!(find_program(None, &LINUX_PROGRAMS), None);
        let program = |file: std::path::PathBuf, mode: u32| {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&file, "").expect("file");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).expect("mode");
        };
        program(b.join("espeak-ng"), 0o755);
        assert_eq!(find_program(path(&[&a, &b]), &LINUX_PROGRAMS), Some("espeak-ng"));
        // A file that can't be run doesn't count.
        program(a.join("spd-say"), 0o644);
        assert_eq!(find_program(path(&[&a, &b]), &LINUX_PROGRAMS), Some("espeak-ng"));
        // speech-dispatcher first, wherever it is on PATH.
        program(b.join("spd-say"), 0o755);
        assert_eq!(find_program(path(&[&a, &b]), &LINUX_PROGRAMS), Some("spd-say"));
        // A full path is taken as it is, with or without a PATH.
        let say = root.join("say");
        program(say.clone(), 0o755);
        let full = say.to_string_lossy();
        assert_eq!(find_program(None, &[&*full]), Some(&*full));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `say` gets the rate as arguments and the line on standard input (a stand-in script records both).
    #[test]
    fn say_gets_the_line_on_stdin() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("nexus-say-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let (script, out) = (dir.join("say"), dir.join("heard.txt"));
        std::fs::write(&script, format!("#!/bin/sh\nprintf '%s|' \"$@\" > '{}'\ncat >> '{}'\n", out.display(), out.display())).expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        speak(&script.to_string_lossy(), "-1 jump left", 1.5, 0.25).expect("spoken");
        assert_eq!(std::fs::read_to_string(&out).expect("heard"), "-r|270|[[volm 0.25]] -1 jump left");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failing_program_says_why() {
        // `false` exits with an error and prints nothing.
        let e = speak("false", "test", 1.0, 1.0).expect_err("fails");
        assert!(e.starts_with("it ended with"), "{e}");
        assert!(speak("no-such-speech-program", "test", 1.0, 1.0).is_err());
    }
}
