use crate::core::{CoreStatus, Phase};
use crate::settings::Settings;
use crate::theme::pal;
use crate::widgets::{card, kv};
use egui::{Grid, RichText, Ui};

pub struct SettingsCtx<'a> {
    pub settings: &'a mut Settings,
    pub status: &'a CoreStatus,
    pub data_dir: &'a std::path::Path,
    pub changed: bool,
    pub test_voice: bool,
    pub restart_core: bool,
}

pub fn show(ui: &mut Ui, cx: &mut SettingsCtx<'_>) {
    ui.columns(2, |c| {
        card(&mut c[0], "Journal", |ui| {
            let st = cx.status;
            Grid::new("jgrid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                kv(ui, "Folder", st.journal_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "not found".into()));
                kv(ui, "Journal files", st.files.to_string());
                kv(ui, "Size", format!("{:.1} MB", st.bytes as f64 / 1e6));
                kv(ui, "History load time", format!("{:.0} ms", st.load_ms));
                kv(ui, "Tailing", st.current_file.clone().unwrap_or_default());
                kv(ui, "Live events this run", st.live_events.to_string());
                let phase = match &st.phase {
                    Phase::Starting => "starting".to_string(),
                    Phase::Loading { done, total } => format!("loading {done}/{total}"),
                    Phase::Live => "live".to_string(),
                    Phase::Error(e) => format!("error: {e}"),
                };
                kv(ui, "State", phase);
            });
            ui.add_space(6.0);
            ui.label(RichText::new("Custom journal folder (leave empty to auto-detect):").color(pal::DIM));
            let mut text = cx.settings.journal_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY)).changed() {
                cx.settings.journal_dir = if text.trim().is_empty() { None } else { Some(text.trim().into()) };
                cx.changed = true;
            }
            if ui.button("Reload journals").clicked() {
                cx.restart_core = true;
            }
            if !st.errors.is_empty() {
                ui.add_space(4.0);
                for e in st.errors.iter().rev().take(5) {
                    ui.label(RichText::new(e).small().color(pal::BAD));
                }
            }
        });
        card(&mut c[1], "Voice & alerts", |ui| {
            let s = &mut *cx.settings;
            cx.changed |= ui.checkbox(&mut s.tts_enabled, "Speak alerts (text-to-speech)").changed();
            ui.horizontal(|ui| {
                ui.label("Speak from priority");
                egui::ComboBox::from_id_salt("ttsprio")
                    .selected_text(["Low", "Normal", "High", "Critical"][s.tts_min_priority.min(3) as usize])
                    .show_ui(ui, |ui| {
                        for (i, n) in ["Low", "Normal", "High", "Critical"].iter().enumerate() {
                            cx.changed |= ui.selectable_value(&mut s.tts_min_priority, i as u8, *n).changed();
                        }
                    });
            });
            cx.changed |= ui.add(egui::Slider::new(&mut s.tts_rate, 0.5..=2.0).text("speech rate")).changed();
            cx.changed |= ui.add(egui::Slider::new(&mut s.tts_volume, 0.0..=1.0).text("volume")).changed();
            cx.changed |= ui.add(egui::Slider::new(&mut s.toast_seconds, 2.0..=30.0).text("toast seconds")).changed();
            if ui.button("Test voice").clicked() {
                cx.test_voice = true;
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Exploration").color(pal::ACCENT));
            let mut hv = s.high_value_threshold as f64 / 1e6;
            if ui.add(egui::Slider::new(&mut hv, 0.1..=10.0).text("high-value threshold (M Cr)")).changed() {
                s.high_value_threshold = (hv * 1e6) as i64;
                cx.changed = true;
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Display").color(pal::ACCENT));
            cx.changed |= ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).text("UI scale")).changed();
        });
    });
    ui.add_space(10.0);
    card(ui, "About", |ui| {
        ui.label(RichText::new(format!("Elite Nexus {} — native Rust, no Python, no Node.js.", env!("CARGO_PKG_VERSION"))).color(pal::TEXT));
        ui.label(RichText::new(format!("Data folder: {}", cx.data_dir.display())).color(pal::DIM));
        ui.label(RichText::new("Elite Dangerous is a trademark of Frontier Developments plc. Elite Nexus is an unofficial community tool.").small().color(pal::FAINT));
    });
}
