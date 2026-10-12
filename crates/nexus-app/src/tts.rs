//! Text-to-speech on a background thread (Windows: WinRT speech synthesis via the `tts` crate).

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
