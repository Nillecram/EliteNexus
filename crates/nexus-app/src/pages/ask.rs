//! "Ask": the built-in assistant. One question box; setup is a single pasted key (other providers
//! and the model choice are tucked away under a disclosure).

use crate::assistant::{AssistantService, tool_noun};
use crate::secrets;
use crate::settings::AiSettings;
use crate::theme::{display, pal};
use egui::text::LayoutJob;
use egui::{Align, Button, CornerRadius, FontId, Frame, Layout, Margin, RichText, ScrollArea, TextEdit, TextFormat, Ui};
use egui_phosphor::regular as icon;
use nexus_ai::ProviderKind;

#[derive(Default)]
pub struct AskUi {
    pub input: String,
    key_input: String,
    show_setup: bool,
    show_other: bool,
}

pub enum AskAction {
    None,
    Ask(String),
    SettingsChanged,
}

const SUGGESTIONS: &[&str] = &[
    "Where can I make the most money in the next hour?",
    "Where should I sell my cargo?",
    "What's the best deal near me right now?",
    "Plan an exobiology trip near me worth a billion credits",
    "Where's the nearest raw material trader for my ship?",
];

pub fn show(ui: &mut Ui, svc: &mut AssistantService, st: &mut AskUi, ai: &mut AiSettings) -> AskAction {
    if !ai.is_ready() || st.show_setup {
        return setup(ui, st, ai);
    }
    chat(ui, svc, st)
}

fn short_name(k: ProviderKind) -> &'static str {
    match k {
        ProviderKind::Gemini => "Gemini",
        ProviderKind::Claude => "Claude",
        ProviderKind::Local => "local",
        ProviderKind::Custom => "API",
    }
}

fn setup(ui: &mut Ui, st: &mut AskUi, ai: &mut AiSettings) -> AskAction {
    let mut action = AskAction::None;
    ScrollArea::vertical().id_salt("ask-setup").auto_shrink([false, false]).show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.set_max_width(560.0);
            ui.add_space(28.0);
            ui.label(RichText::new(icon::SPARKLE).size(42.0).color(pal::ACCENT));
            ui.label(RichText::new("Ask Elite Nexus").font(display(26.0)));
            ui.add_space(4.0);
            ui.label(
                RichText::new("An AI co-pilot that knows your ship, cargo, journal and the live galaxy market. Ask things like “Where can I make the most money in the next hour?”")
                    .color(pal::DIM),
            );
            ui.add_space(18.0);
            let kind = ai.provider;
            if kind.needs_key() {
                if let Some(url) = kind.key_url() {
                    ui.hyperlink_to(RichText::new(format!("{} Get a free {} key", icon::ARROW_SQUARE_OUT, short_name(kind))).font(display(16.0)), url);
                    ui.add_space(8.0);
                }
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - 440.0) / 2.0).max(0.0));
                    let field = ui.add(TextEdit::singleline(&mut st.key_input).password(true).hint_text("Paste your key here").desired_width(320.0));
                    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (ui.add(Button::new(RichText::new("Connect").color(pal::ACCENT))).clicked() || enter) && !st.key_input.trim().is_empty() {
                        ai.api_key = secrets::seal(st.key_input.trim());
                        st.key_input.clear();
                        st.show_setup = false;
                        action = AskAction::SettingsChanged;
                    }
                });
                if !ai.api_key.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - 260.0) / 2.0).max(0.0));
                        ui.label(RichText::new(format!("{} A key is saved (encrypted)", icon::LOCK_KEY)).small().color(pal::GOOD));
                        if ui.small_button("Remove").clicked() {
                            ai.api_key.clear();
                            action = AskAction::SettingsChanged;
                        }
                    });
                }
            } else {
                ui.label(RichText::new("Uses a model running on this PC: start Ollama or LM Studio with a model that supports tools.").color(pal::DIM));
                if ui.add(Button::new(RichText::new("Connect").color(pal::ACCENT))).clicked() {
                    st.show_setup = false;
                    action = AskAction::SettingsChanged;
                }
            }
            ui.add_space(10.0);
            let note = match kind {
                ProviderKind::Gemini => "Your key is stored encrypted on this PC. On Google's free tier, questions may be used to improve Google's models; paid use is not.",
                ProviderKind::Claude => "Your key is stored encrypted on this PC. Claude Haiku costs about a tenth of a cent per question.",
                ProviderKind::Local => "Nothing leaves this PC.",
                ProviderKind::Custom => "Your key is stored encrypted on this PC.",
            };
            ui.label(RichText::new(note).small().color(pal::FAINT));
            ui.add_space(14.0);

            // Hidden depth: other providers, model, address.
            let caret = if st.show_other { icon::CARET_DOWN } else { icon::CARET_RIGHT };
            if ui.add(Button::new(RichText::new(format!("{caret} Other providers and model")).small().color(pal::DIM)).frame(false)).clicked() {
                st.show_other = !st.show_other;
            }
            if st.show_other {
                Frame::new().fill(pal::CARD).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                    egui::Grid::new("ai-advanced").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label(RichText::new("Provider").color(pal::DIM));
                        egui::ComboBox::from_id_salt("ai-provider").selected_text(ai.provider.label()).width(280.0).show_ui(ui, |ui| {
                            for k in ProviderKind::ALL {
                                if ui.selectable_value(&mut ai.provider, k, k.label()).changed() {
                                    action = AskAction::SettingsChanged;
                                }
                            }
                        });
                        ui.end_row();
                        ui.label(RichText::new("Model").color(pal::DIM));
                        let auto = ai.provider.preferred_models().first().map(|m| format!("automatic ({m})")).unwrap_or_else(|| "automatic".into());
                        if ui.add(TextEdit::singleline(&mut ai.model).hint_text(auto).desired_width(280.0)).changed() {
                            action = AskAction::SettingsChanged;
                        }
                        ui.end_row();
                        if matches!(ai.provider, ProviderKind::Local | ProviderKind::Custom) {
                            ui.label(RichText::new("Address").color(pal::DIM));
                            let hint = match ai.provider.default_base_url() {
                                "" => "https://…/v1/",
                                u => u,
                            };
                            if ui.add(TextEdit::singleline(&mut ai.base_url).hint_text(hint).desired_width(280.0)).changed() {
                                action = AskAction::SettingsChanged;
                            }
                            ui.end_row();
                        }
                    });
                });
            }
            if ai.is_ready() && st.show_setup {
                ui.add_space(10.0);
                if ui.button("Done").clicked() {
                    st.show_setup = false;
                }
            }
        });
    });
    action
}

fn chat(ui: &mut Ui, svc: &mut AssistantService, st: &mut AskUi) -> AskAction {
    let mut action = AskAction::None;
    ui.horizontal(|ui| {
        if let Some(m) = &svc.model {
            ui.label(RichText::new(m).small().color(pal::FAINT));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Button::new(icon::GEAR_SIX).frame(false)).on_hover_text("Assistant settings").clicked() {
                st.show_setup = true;
            }
            if ui.add_enabled(!svc.busy() && !svc.exchanges.is_empty(), Button::new(format!("{} New chat", icon::NOTE_PENCIL))).clicked() {
                svc.clear();
            }
        });
    });
    let input_h = 54.0;
    ScrollArea::vertical()
        .id_salt("ask-chat")
        .max_height((ui.available_height() - input_h).max(120.0))
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if svc.exchanges.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(icon::SPARKLE).size(34.0).color(pal::ACCENT));
                    ui.label(RichText::new("What do you want to do, Commander?").font(display(22.0)));
                    ui.add_space(12.0);
                    for s in SUGGESTIONS {
                        if ui.add(Button::new(RichText::new(*s).color(pal::ACCENT_SOFT)).fill(pal::CARD)).clicked() {
                            action = AskAction::Ask((*s).to_string());
                        }
                        ui.add_space(4.0);
                    }
                });
            }
            let width = ui.available_width();
            for ex in &svc.exchanges {
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    Frame::new().fill(pal::ACCENT_BG).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.set_max_width(width * 0.7);
                        ui.label(RichText::new(&ex.question).color(pal::TEXT));
                    });
                });
                ui.add_space(6.0);
                match &ex.answer {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(svc.status.clone().unwrap_or_else(|| "Thinking…".into())).color(pal::DIM));
                        });
                    }
                    Some(Ok(text)) => {
                        markdown(ui, text, width - 8.0);
                        if !ex.tools.is_empty() {
                            let mut nouns: Vec<&str> = ex.tools.iter().map(|t| tool_noun(t)).collect();
                            nouns.dedup();
                            ui.label(RichText::new(format!("used {}", nouns.join(" · "))).small().color(pal::FAINT)).on_hover_text(format!(
                                "{} input tokens ({} cached), {} output tokens",
                                ex.usage.input_tokens, ex.usage.cached_tokens, ex.usage.output_tokens
                            ));
                        }
                    }
                    Some(Err(e)) => {
                        ui.label(RichText::new(format!("{} {e}", icon::WARNING)).color(pal::BAD));
                    }
                }
            }
        });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 96.0).max(200.0);
        let field = ui.add_sized(
            [w, 36.0],
            TextEdit::singleline(&mut st.input).hint_text("Ask anything, e.g. where should I sell my cargo?").font(FontId::proportional(16.0)).margin(Margin::symmetric(10, 8)),
        );
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let send = ui.add_enabled(!svc.busy(), Button::new(RichText::new(format!("{} Ask", icon::PAPER_PLANE_RIGHT)).color(pal::ACCENT)).min_size(egui::vec2(84.0, 36.0))).clicked();
        if (enter || send) && !svc.busy() && !st.input.trim().is_empty() {
            action = AskAction::Ask(std::mem::take(&mut st.input));
            field.request_focus();
        }
    });
    action
}

/// Simple Markdown: headings, bullet and numbered lists, **bold**.
fn markdown(ui: &mut Ui, text: &str, width: f32) {
    for line in text.lines() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            ui.add_space(5.0);
            continue;
        }
        let trimmed = t.trim_start();
        let indent = ((t.len() - trimmed.len()) as f32 * 4.0).min(32.0);
        let (heading, body, prefix) = if let Some(h) = trimmed.strip_prefix("### ").or_else(|| trimmed.strip_prefix("## ")).or_else(|| trimmed.strip_prefix("# ")) {
            (true, h, String::new())
        } else if let Some(b) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("• ")) {
            (false, b, "•  ".to_string())
        } else if let Some((n, rest)) = trimmed.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() <= 2 && n.chars().all(|c| c.is_ascii_digit())) {
            (false, rest, format!("{n}.  "))
        } else {
            (false, trimmed, String::new())
        };
        let size = if heading { 17.0 } else { 15.0 };
        let mut job = LayoutJob::default();
        job.wrap.max_width = (width - indent).max(100.0);
        let normal = TextFormat { font_id: if heading { display(size) } else { FontId::proportional(size) }, color: pal::TEXT, ..Default::default() };
        let bold = TextFormat { font_id: display(size), color: pal::ACCENT_SOFT, ..Default::default() };
        if !prefix.is_empty() {
            job.append(&prefix, 0.0, TextFormat { color: pal::ACCENT, ..normal.clone() });
        }
        for (i, part) in body.split("**").enumerate() {
            if !part.is_empty() {
                job.append(part, 0.0, if i % 2 == 1 { bold.clone() } else { normal.clone() });
            }
        }
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.label(job);
        });
    }
}
