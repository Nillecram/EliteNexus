//! The main window: sidebar, HUD header, page routing, toasts, voice.

use crate::core::{Core, Phase};
use crate::pages::{self, Page};
use crate::settings::Settings;
use crate::theme::{display, pal};
use crate::tts::Speaker;
use crate::widgets;
use egui::{Align, Align2, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Stroke, Ui, Vec2};
use egui_phosphor::regular as icon;
use nexus_model::{Alert, Priority};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Toast {
    alert: Alert,
    born: Instant,
}

/// `--screenshot <file>`: render once the journals are loaded, save a PNG, exit (for docs/tests).
/// Command-line overrides (testing / special setups).
#[derive(Default)]
pub struct Overrides {
    pub journal_dir: Option<PathBuf>,
    /// Never speak (tests).
    pub mute: bool,
    /// Sub-tab to open on the start page (screenshots), e.g. `sell` for Market.
    pub tab: Option<String>,
}

pub struct ScreenshotJob {
    pub path: PathBuf,
    pub frames: u32,
    pub requested: bool,
    pub delay: Duration,
    pub started: Instant,
}

impl ScreenshotJob {
    pub fn new(path: PathBuf, delay: Duration) -> Self {
        ScreenshotJob { path, frames: 0, requested: false, delay, started: Instant::now() }
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) {
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    match image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, bytes) {
        Some(buf) => match buf.save(path) {
            Ok(()) => tracing::info!(path = %path.display(), "screenshot saved"),
            Err(e) => tracing::error!(path = %path.display(), "screenshot save failed: {e}"),
        },
        None => tracing::error!("screenshot buffer size mismatch"),
    }
}

pub struct NexusApp {
    core: Core,
    settings: Settings,
    data_dir: PathBuf,
    page: Page,
    toasts: Vec<Toast>,
    alert_log: VecDeque<Alert>,
    speaker: Speaker,
    applied_scale: f32,
    settings_dirty: Option<Instant>,
    explo: pages::exploration::ExplorationUi,
    fleet: pages::fleet::FleetUi,
    eng: pages::engineering::EngineeringUi,
    log: pages::logbook::LogbookUi,
    map: pages::galaxy_map::MapUi,
    binds: pages::bindings::BindsUi,
    bgs: pages::bgs::BgsUi,
    nav: pages::navigation::NavUi,
    /// Shared async runtime for network services (Spansh, EDDN, uploads).
    rt: tokio::runtime::Runtime,
    spansh: nexus_spansh::Spansh,
    accounts: pages::accounts::AccountsUi,
    /// EDDN / EDSM / Inara / EDAstro uploads (all off until enabled in Accounts).
    sync: crate::sync::SyncService,
    /// Local galaxy/market database + live EDDN feed.
    market: crate::market::MarketService,
    market_ui: pages::market::MarketUi,
    colo: pages::colonisation::ColonisationUi,
    // Command-line overrides: applied at runtime, never written to settings.json.
    overrides: Overrides,
    shot: Option<ScreenshotJob>,
    last_jump_count: Option<usize>,
    clipboard: Option<arboard::Clipboard>,
    compact: bool,
    compact_request: bool,
}

impl NexusApp {
    pub fn new(cc: &eframe::CreationContext<'_>, data_dir: PathBuf, start_page: Option<Page>, shot: Option<ScreenshotJob>, ov: Overrides) -> Self {
        crate::theme::install(&cc.egui_ctx);
        let settings = Settings::load(&data_dir);
        let page = start_page.or_else(|| cc.storage.and_then(|s| eframe::get_value(s, "page"))).unwrap_or(Page::Dashboard);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("nexus-net").enable_all().build().expect("tokio runtime");
        let mut market = crate::market::MarketService::open(&data_dir, &cc.egui_ctx);
        market.set_watches(&settings.price_watches);
        let core = start_core(&cc.egui_ctx, ov.journal_dir.as_ref().or(settings.journal_dir.as_ref()), market.history_sink());
        market.attach(&core);
        market.set_listen(settings.accounts.eddn_listen, rt.handle());
        // Test runs (CLI journal folder, screenshots) never upload: replayed journals must not reach the services.
        let sync = if ov.journal_dir.is_none() && shot.is_none() { crate::sync::SyncService::start(&core, &settings.accounts, &data_dir, rt.handle()) } else { crate::sync::SyncService::disabled() };
        let mut market_ui = pages::market::MarketUi::default();
        if let Some(t) = ov.tab.as_deref().and_then(pages::market::Tab::from_name) {
            market_ui.tab = t;
        }
        let speaker = Speaker::start();
        speaker.set_rate(settings.tts_rate);
        speaker.set_volume(settings.tts_volume);
        NexusApp {
            core,
            settings,
            data_dir,
            page,
            toasts: Vec::new(),
            alert_log: VecDeque::new(),
            speaker,
            applied_scale: 0.0,
            settings_dirty: None,
            explo: Default::default(),
            fleet: Default::default(),
            eng: Default::default(),
            log: Default::default(),
            map: pages::galaxy_map::MapUi { show_route: true, show_labels: true, show_landmarks: false, show_assets: true },
            binds: Default::default(),
            bgs: Default::default(),
            nav: Default::default(),
            rt,
            spansh: nexus_spansh::Spansh::new(),
            accounts: Default::default(),
            sync,
            market,
            market_ui,
            colo: Default::default(),
            overrides: ov,
            shot,
            last_jump_count: None,
            clipboard: None,
            compact: false,
            compact_request: false,
        }
    }

    /// After every jump, put the next route system on the clipboard (paste into the galaxy map).
    fn auto_copy_next(&mut self) {
        let next = {
            let model = self.core.model.read();
            let Some(gs) = model.active() else { return };
            let jumps = gs.history.jumps.len();
            let prev = self.last_jump_count.replace(jumps);
            if !(self.settings.auto_copy_next_system && prev.is_some_and(|p| jumps > p)) {
                return;
            }
            match pages::navigation::active_route(gs, self.nav.planned.as_ref()).first() {
                Some(w) => w.system.clone(),
                None => return,
            }
        };
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            match cb.set_text(next.clone()) {
                Ok(()) => self.toasts.push(Toast {
                    alert: Alert::new(nexus_journal::Timestamp::now(), nexus_model::AlertKind::Navigation, Priority::Low, format!("Next: {next}")).detail("Copied to clipboard"),
                    born: Instant::now(),
                }),
                Err(e) => tracing::warn!("clipboard: {e}"),
            }
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(job) = self.shot.as_mut() else { return };
        job.frames += 1;
        let live = matches!(self.core.status.read().phase, Phase::Live);
        if live && !job.requested && job.frames > 40 && job.started.elapsed() >= job.delay {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            job.requested = true;
        }
        let img = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = img {
            save_png(&job.path, &img);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }

    fn drain_alerts(&mut self) {
        // Journal alerts first, then price alerts from the market watcher.
        let mut incoming: Vec<Alert> = self.core.alerts.try_iter().collect();
        incoming.extend(self.market.alerts.try_iter());
        for a in incoming {
            let prio = match a.priority {
                Priority::Low => 0,
                Priority::Normal => 1,
                Priority::High => 2,
                Priority::Critical => 3,
            };
            if self.settings.tts_enabled && !self.overrides.mute && prio >= self.settings.tts_min_priority
                && let Some(s) = &a.speech {
                    self.speaker.say(s.clone());
                }
            if self.alert_log.len() >= 200 {
                self.alert_log.pop_front();
            }
            self.alert_log.push_back(a.clone());
            let dup = self.toasts.iter().rev().take(4).any(|t| t.alert.title == a.title && t.alert.detail == a.detail && t.born.elapsed() < Duration::from_secs(20));
            if !dup {
                self.toasts.push(Toast { alert: a, born: Instant::now() });
            }
            if self.toasts.len() > 5 {
                self.toasts.remove(0);
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.label(RichText::new(icon::DIAMOND).size(24.0).color(pal::ACCENT));
            ui.vertical(|ui| {
                ui.label(RichText::new("ELITE NEXUS").font(display(19.0)).color(pal::TEXT));
                ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).small().color(pal::FAINT));
            });
        });
        ui.add_space(12.0);
        egui::ScrollArea::vertical().id_salt("nav").show(ui, |ui| {
            for (group, items) in Page::GROUPS {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new(*group).font(display(11.0)).color(pal::FAINT));
                });
                for p in *items {
                    if nav_item(ui, *p, self.page == *p) {
                        self.page = *p;
                    }
                }
            }
            ui.add_space(12.0);
        });
    }

    fn header(&mut self, ui: &mut Ui) {
        let status = self.core.status.read().clone();
        let model = self.core.model.read();
        let gs = model.active();
        let mut pause_toggled = false;
        ui.horizontal_centered(|ui| {
            ui.add_space(4.0);
            match gs {
                Some(gs) => {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("CMDR {}", gs.cmdr.name)).font(display(18.0)).color(pal::TEXT));
                        let ship = gs.fleet.current().map(|s| if s.name.trim().is_empty() { s.type_name.clone() } else { format!("{} · {}", s.name, s.type_name) }).unwrap_or_default();
                        ui.label(RichText::new(ship).small().color(pal::DIM));
                    });
                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{} {}", icon::MAP_PIN, gs.location.system)).font(display(17.0)).color(pal::ACCENT_SOFT));
                        ui.label(RichText::new(gs.location.situation()).small().color(pal::DIM));
                    });
                    if let Some(s) = gs.fleet.current().filter(|s| s.fuel_main_capacity > 0.0) {
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.set_width(130.0);
                            let f = (gs.fleet.fuel_main / s.fuel_main_capacity) as f32;
                            let col = if f < 0.25 { pal::BAD } else if f < 0.5 { pal::WARN } else { pal::CYAN };
                            ui.label(RichText::new(format!("{} {:.1}/{:.0} t", icon::GAS_PUMP, gs.fleet.fuel_main, s.fuel_main_capacity)).small().color(pal::DIM));
                            widgets::bar(ui, f, col, 6.0, None);
                        });
                    }
                }
                None => {
                    ui.label(RichText::new("Waiting for journal data…").font(display(18.0)).color(pal::DIM));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(8.0);
                let (_live, label, col) = match &status.phase {
                    Phase::Live => {
                        let fresh = status.last_live.is_some_and(|t| t.elapsed() < Duration::from_secs(120));
                        let playing = gs.is_some_and(|g| !g.game_closed);
                        if fresh || playing { (true, "LIVE", pal::GOOD) } else { (false, "OFFLINE", pal::FAINT) }
                    }
                    Phase::Loading { .. } | Phase::Starting => (false, "LOADING", pal::WARN),
                    Phase::Error(_) => (false, "ERROR", pal::BAD),
                };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 22.0), Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::same(11), col.gamma_multiply(0.15));
                ui.painter().circle_filled(rect.left_center() + Vec2::new(12.0, 0.0), 4.0, col);
                ui.painter().text(rect.left_center() + Vec2::new(22.0, 0.0), Align2::LEFT_CENTER, label, display(12.0), col);
                ui.add_space(10.0);
                let acc = &mut self.settings.accounts;
                if service_indicators(ui, &self.sync, &self.market, &mut self.page, &mut self.market_ui.tab, acc.eddn_upload, &mut acc.eddn_paused) {
                    pause_toggled = true;
                }
                if ui.button(RichText::new(format!("{} HUD", icon::PICTURE_IN_PICTURE)).color(pal::ACCENT_SOFT)).on_hover_text("Compact always-on-top HUD for a second screen").clicked() {
                    let ctx = ui.ctx().clone();
                    self.compact_request = true;
                    ctx.request_repaint();
                }
                ui.add_space(10.0);
                if let Some(gs) = gs {
                    ui.label(RichText::new(widgets::credits(gs.cmdr.credits)).font(display(19.0)).color(pal::GOLD));
                    ui.label(RichText::new(icon::COINS).size(18.0).color(pal::GOLD));
                }
            });
        });
        drop(model);
        if pause_toggled {
            self.sync.apply(&self.settings.accounts);
            self.settings_dirty = Some(Instant::now());
        }
    }

    fn content(&mut self, ui: &mut Ui) {
        let status = self.core.status.read().clone();
        match &status.phase {
            Phase::Loading { done, total } => {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Reading your journals…").font(display(22.0)));
                    ui.add_space(8.0);
                    ui.scope(|ui| {
                        ui.set_max_width(400.0);
                        widgets::bar(ui, *done as f32 / (*total).max(1) as f32, pal::ACCENT, 10.0, None);
                    });
                    ui.label(RichText::new(format!("{done} / {total} files")).color(pal::DIM));
                });
                return;
            }
            Phase::Error(e) => {
                widgets::empty(ui, icon::WARNING, "Journal problem", e);
                return;
            }
            Phase::Starting => return,
            Phase::Live => {}
        }
        let model = self.core.model.read();
        let Some(gs) = model.active() else {
            widgets::empty(ui, icon::USER, "No commander found", "Start Elite Dangerous — Elite Nexus picks everything up live.");
            return;
        };
        let (title, subtitle) = page_heading(self.page, gs);
        widgets::page_title(ui, self.page.icon(), &title, &subtitle);
        let scroll = !matches!(self.page, Page::Exploration | Page::Logbook | Page::GalaxyMap | Page::Fleet | Page::Navigation | Page::Bindings | Page::Market);
        let page = self.page;
        let high_value = self.settings.high_value_threshold;
        let mut settings_changed = false;
        let mut test_voice = false;
        let mut restart = false;
        let mut body = |ui: &mut Ui| match page {
            Page::Dashboard => pages::dashboard::show(ui, gs),
            Page::Exploration => pages::exploration::show(ui, gs, &mut self.explo, high_value),
            Page::Exobiology => pages::exobio::show(ui, gs),
            Page::GalaxyMap => pages::galaxy_map::show(ui, gs, &mut self.map),
            Page::Mining => pages::mining::show(ui, gs),
            Page::Colonisation => pages::colonisation::show(ui, gs, &self.market, &mut self.colo),
            Page::Carrier => pages::carrier::show(ui, gs),
            Page::Engineering => settings_changed |= pages::engineering::show(ui, gs, &mut self.eng, &mut self.settings.wishlist),
            Page::Fleet => pages::fleet::show(ui, gs, &mut self.fleet),
            Page::Missions => pages::missions::show(ui, gs),
            Page::Bgs => pages::bgs::show(ui, gs, &mut self.bgs),
            Page::Logbook => pages::logbook::show(ui, gs, &mut self.log, &self.core.index),
            Page::Statistics => pages::stats::show(ui, gs),
            Page::Settings => {
                let mut cx = pages::settings::SettingsCtx {
                    settings: &mut self.settings,
                    status: &status,
                    data_dir: &self.data_dir,
                    changed: false,
                    test_voice: false,
                    restart_core: false,
                };
                pages::settings::show(ui, &mut cx);
                settings_changed = cx.changed;
                test_voice = cx.test_voice;
                restart = cx.restart_core;
            }
            Page::Navigation => {
                settings_changed |= pages::navigation::show(ui, gs, &mut self.nav, &mut self.settings.auto_copy_next_system, &self.spansh, self.rt.handle())
            }
            Page::Bindings => pages::bindings::show(ui, &mut self.binds, &self.data_dir),
            Page::Accounts => {
                let name = gs.cmdr.name.clone();
                let status = pages::accounts::StatusView { eddn_feed: self.market.feed_summary().map(|(_, line)| line), uplink: self.sync.status() };
                settings_changed |= pages::accounts::show(ui, &mut self.settings.accounts, &mut self.accounts, &name, &status);
                if self.sync.any_enabled() {
                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                }
            }
            Page::Market => {
                settings_changed |= pages::market::show(ui, gs, &mut self.market, &mut self.market_ui, self.rt.handle(), &mut self.settings.price_watches);
                // Feed counters and headlines move every second or so.
                ui.ctx().request_repaint_after(Duration::from_millis(1000));
            }
        };
        if scroll {
            egui::ScrollArea::vertical().id_salt(("page", page as u8)).auto_shrink([false, false]).show(ui, |ui| body(ui));
        } else {
            body(ui);
        }
        drop(model);
        if settings_changed {
            self.settings_dirty = Some(Instant::now());
            self.speaker.set_rate(self.settings.tts_rate);
            self.speaker.set_volume(self.settings.tts_volume);
            self.sync.apply(&self.settings.accounts);
            self.market.set_watches(&self.settings.price_watches);
            self.market.set_listen(self.settings.accounts.eddn_listen, self.rt.handle());
        }
        if test_voice {
            self.speaker.say("Elite Nexus voice check. All systems nominal, commander.");
        }
        if restart {
            self.core = start_core(ui.ctx(), self.overrides.journal_dir.as_ref().or(self.settings.journal_dir.as_ref()), self.market.history_sink());
            self.sync.attach(&self.core);
            self.market.attach(&self.core);
        }
    }

    fn set_compact(&mut self, ctx: &egui::Context, on: bool) {
        self.compact = on;
        if on {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(340.0, 300.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(420.0, 640.0)));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(Vec2::new(1100.0, 680.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1480.0, 920.0)));
        }
    }

    fn compact_ui(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default().frame(Frame::new().fill(pal::BG1).inner_margin(Margin::same(10))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon::DIAMOND).color(pal::ACCENT));
                ui.label(RichText::new("ELITE NEXUS HUD").font(display(13.0)).color(pal::DIM));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button(format!("{} Full view", icon::ARROWS_OUT)).clicked() {
                        let ctx = ui.ctx().clone();
                        self.set_compact(&ctx, false);
                    }
                });
            });
            ui.add_space(4.0);
            let model = self.core.model.read();
            match model.active() {
                Some(gs) => {
                    let alerts: Vec<Alert> = self.alert_log.iter().cloned().collect();
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| pages::compact::show(ui, gs, &alerts));
                }
                None => {
                    ui.label(RichText::new("Waiting for journal data…").color(pal::DIM));
                }
            }
        });
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let life = Duration::from_secs_f32(self.settings.toast_seconds.max(1.0));
        self.toasts.retain(|t| t.born.elapsed() < life);
        if self.toasts.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_max_width(380.0);
                for t in self.toasts.iter().rev() {
                    let age = t.born.elapsed().as_secs_f32();
                    let fade = ((life.as_secs_f32() - age) / 0.6).clamp(0.0, 1.0);
                    let col = match t.alert.priority {
                        Priority::Critical => pal::BAD,
                        Priority::High => pal::ACCENT,
                        Priority::Normal => pal::CYAN,
                        Priority::Low => pal::DIM,
                    };
                    Frame::new()
                        .fill(pal::CARD.gamma_multiply(fade))
                        .stroke(Stroke::new(1.0, col.gamma_multiply(fade)))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::same(12))
                        .shadow(egui::Shadow { offset: [0, 4], blur: 18, spread: 0, color: Color32::from_black_alpha((120.0 * fade) as u8) })
                        .show(ui, |ui| {
                            ui.set_width(356.0);
                            ui.label(RichText::new(t.alert.kind.label().to_uppercase()).font(display(11.0)).color(col.gamma_multiply(fade)));
                            ui.label(RichText::new(&t.alert.title).font(display(16.0)).color(pal::TEXT.gamma_multiply(fade)));
                            if !t.alert.detail.is_empty() {
                                ui.label(RichText::new(&t.alert.detail).color(pal::DIM.gamma_multiply(fade)));
                            }
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

fn start_core(ctx: &egui::Context, journal_dir: Option<&PathBuf>, history: Option<crate::core::HistorySink>) -> Core {
    let repaint_ctx = ctx.clone();
    Core::start(journal_dir.cloned(), move || repaint_ctx.request_repaint(), history)
}

/// Live feed and upload state, right-to-left in the header. Click to open the details. Returns true
/// when the EDDN pause button was toggled.
fn service_indicators(
    ui: &mut Ui,
    sync: &crate::sync::SyncService,
    market: &crate::market::MarketService,
    page: &mut Page,
    market_tab: &mut pages::market::Tab,
    eddn_upload: bool,
    eddn_paused: &mut bool,
) -> bool {
    let mut toggled = false;
    if eddn_upload {
        let (label, col, tip) = if *eddn_paused {
            (format!("{} Resume EDDN", icon::PLAY), pal::WARN, "Sharing with EDDN is paused. Click to share again.")
        } else {
            (format!("{} Pause EDDN", icon::PAUSE), pal::DIM, "Stop sharing with EDDN, e.g. to keep a market you just found to yourself (prices are only sent when you undock).")
        };
        if ui.add(egui::Button::new(RichText::new(label).small().color(col))).on_hover_text(tip).clicked() {
            *eddn_paused = !*eddn_paused;
            toggled = true;
        }
        ui.add_space(8.0);
    }
    if sync.any_enabled() {
        let s = sync.status();
        let all = [&s.eddn, &s.edsm, &s.inara, &s.edastro];
        let sent: u64 = all.iter().map(|x| x.sent).sum();
        let queued: usize = all.iter().map(|x| x.queued).sum();
        let trouble = all.iter().any(|x| x.paused.is_some() || (x.last_error.is_some() && x.last_success.is_none()));
        let col = if trouble { pal::WARN } else { pal::GOOD };
        let text = if queued > 0 { format!("{} {sent} · {queued} queued", icon::CLOUD_ARROW_UP) } else { format!("{} {sent}", icon::CLOUD_ARROW_UP) };
        if ui.add(egui::Label::new(RichText::new(text).small().color(col)).sense(Sense::click())).on_hover_text("Uploads sent this session (EDDN / EDSM / Inara / EDAstro)").clicked() {
            *page = Page::Accounts;
        }
        ui.add_space(10.0);
    }
    if let Some((connected, per_min)) = market.feed_state() {
        let (col, text) = if connected { (pal::GOOD, format!("{} EDDN {per_min}/min", icon::BROADCAST)) } else { (pal::WARN, format!("{} EDDN…", icon::BROADCAST)) };
        if ui.add(egui::Label::new(RichText::new(text).small().color(col)).sense(Sense::click())).on_hover_text("Live market feed. Click for the market database.").clicked() {
            *page = Page::Market;
            *market_tab = pages::market::Tab::Database;
        }
        ui.add_space(10.0);
    }
    toggled
}

fn nav_item(ui: &mut Ui, page: Page, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 29.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect.shrink2(Vec2::new(6.0, 1.0)), CornerRadius::same(7), pal::ACCENT_BG);
        p.rect_filled(egui::Rect::from_min_size(rect.left_top() + Vec2::new(6.0, 6.0), Vec2::new(3.0, 17.0)), CornerRadius::same(2), pal::ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect.shrink2(Vec2::new(6.0, 1.0)), CornerRadius::same(7), pal::CARD_HI);
    }
    let col = if selected { pal::ACCENT } else if resp.hovered() { pal::TEXT } else { pal::DIM };
    p.text(rect.left_center() + Vec2::new(20.0, 0.0), Align2::LEFT_CENTER, page.icon(), egui::FontId::proportional(17.0), col);
    p.text(
        rect.left_center() + Vec2::new(46.0, 0.0),
        Align2::LEFT_CENTER,
        page.label(),
        egui::FontId::proportional(14.0),
        if selected { pal::TEXT } else { col },
    );
    resp.clicked()
}

fn page_heading(page: Page, gs: &nexus_model::GameState) -> (String, String) {
    let sub = match page {
        Page::Dashboard => gs.location.situation(),
        Page::Exploration => format!("{} systems explored", gs.exploration.systems.len()),
        Page::Exobiology => format!("Exobiologist: {}", gs.cmdr.rank_name(nexus_data::ranks::RankKind::Exobiologist)),
        Page::Fleet => format!("{} ships", gs.fleet.ships.len()),
        Page::Missions => format!("{} active", gs.missions.active.len()),
        _ => String::new(),
    };
    (page.label().to_string(), sub)
}

impl eframe::App for NexusApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.screenshot_tick(ctx);
        self.drain_alerts();
        self.auto_copy_next();
        let here = self.core.model.read().active().and_then(|g| g.location.star_pos);
        self.market.set_origin(here);
        if (self.settings.ui_scale - self.applied_scale).abs() > f32::EPSILON {
            ctx.set_zoom_factor(self.settings.ui_scale);
            self.applied_scale = self.settings.ui_scale;
        }
        if let Some(t) = self.settings_dirty
            && t.elapsed() > Duration::from_millis(600) {
                self.settings.save(&self.data_dir);
                self.settings_dirty = None;
            }
        // Keep clocks/countdowns ticking even without journal activity.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if std::mem::take(&mut self.compact_request) {
            let ctx = ui.ctx().clone();
            self.set_compact(&ctx, true);
        }
        if self.compact {
            self.compact_ui(ui);
            let ctx = ui.ctx().clone();
            self.toasts(&ctx);
            return;
        }
        egui::Panel::left("nav")
            .exact_size(212.0)
            .resizable(false)
            .frame(Frame::new().fill(pal::BG0).inner_margin(Margin::symmetric(6, 0)).stroke(Stroke::new(1.0, pal::BORDER)))
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::top("hud")
            .exact_size(58.0)
            .frame(Frame::new().fill(pal::BG1).inner_margin(Margin::symmetric(14, 6)).stroke(Stroke::new(1.0, pal::BORDER)))
            .show(ui, |ui| self.header(ui));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(pal::BG1).inner_margin(Margin::same(16)))
            .show(ui, |ui| self.content(ui));
        if !self.settings.welcomed && self.shot.is_none() && matches!(self.core.status.read().phase, Phase::Live) {
            let files = self.core.status.read().files;
            let ctx = ui.ctx().clone();
            let model = self.core.model.read();
            let res = pages::welcome::show(&ctx, &mut self.settings, model.active(), files);
            drop(model);
            match res {
                pages::welcome::WelcomeResult::Open => {}
                pages::welcome::WelcomeResult::Done => {
                    self.settings.welcomed = true;
                    self.settings_dirty = Some(Instant::now());
                }
                pages::welcome::WelcomeResult::GoToAccounts => {
                    self.settings.welcomed = true;
                    self.settings_dirty = Some(Instant::now());
                    self.page = Page::Accounts;
                }
            }
        }
        let ctx = ui.ctx().clone();
        self.toasts(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "page", &self.page);
        self.settings.save(&self.data_dir);
    }

    fn on_exit(&mut self) {
        // Close open batches; whatever the rate limits hold back stays queued on disk.
        self.sync.flush();
    }
}
