//! Text-to-speech on a background thread. Windows: WinRT speech synthesis via the `tts` crate.
//! Linux: speech-dispatcher's `spd-say`, which desktop distributions ship; without it the app stays quiet.

use crossbeam_channel::{Receiver, Sender};

pub enum TtsCmd {
    Say(String),
    Rate(f32),
    Volume(f32),
}

#[derive(Clone)]
pub struct Speaker {
    tx: Sender<TtsCmd>,
}

impl Speaker {
    pub fn start() -> Speaker {
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = std::thread::Builder::new().name("nexus-tts".into()).spawn(move || run(rx));
        Speaker { tx }
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
}

#[cfg(windows)]
fn run(rx: Receiver<TtsCmd>) {
    let mut engine = match tts::Tts::default() {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("text-to-speech unavailable: {e}");
            // Drain so senders never block or error noisily.
            while rx.recv().is_ok() {}
            return;
        }
    };
    let (min_rate, max_rate, normal_rate) = (engine.min_rate(), engine.max_rate(), engine.normal_rate());
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCmd::Say(text) => {
                // Queue rather than interrupt, so back-to-back alerts are all heard.
                if let Err(e) = engine.speak(text, false) {
                    tracing::warn!("tts speak failed: {e}");
                }
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

#[cfg(not(windows))]
fn run(rx: Receiver<TtsCmd>) {
    let (mut rate, mut volume) = (1.0, 1.0);
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TtsCmd::Say(text) => {
                // -w waits until the line is spoken, so back-to-back alerts queue rather than interrupt.
                let spoken = std::process::Command::new("spd-say")
                    .args(["-w", "-r", &spd_rate(rate).to_string(), "-i", &spd_volume(volume).to_string(), "--", &text])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                if let Err(e) = spoken {
                    tracing::warn!("text-to-speech unavailable (spd-say, from speech-dispatcher): {e}");
                    while rx.recv().is_ok() {}
                    return;
                }
            }
            TtsCmd::Rate(r) => rate = r,
            TtsCmd::Volume(v) => volume = v,
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

#[cfg(all(test, not(windows)))]
mod tests {
    #[test]
    fn spd_say_ranges() {
        assert_eq!((super::spd_rate(1.0), super::spd_rate(1.5), super::spd_rate(3.0), super::spd_rate(0.5)), (0, 50, 100, -50));
        assert_eq!((super::spd_volume(1.0), super::spd_volume(0.5), super::spd_volume(0.0)), (100, 0, -100));
    }
}
