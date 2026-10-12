//! The "Ask" assistant: answers questions on a worker thread with `nexus-ai`, using tools over the
//! app's own data. The UI only polls for progress.

use crate::ai_tools::NexusTools;
use crossbeam_channel::Receiver;
use nexus_ai::{AiError, Answer, ProviderConfig, Remote, Step, Turn, Usage};
use parking_lot::Mutex;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// One question and its answer.
pub struct Exchange {
    pub question: String,
    /// None while the assistant is working.
    pub answer: Option<Result<String, String>>,
    pub tools: Vec<String>,
    pub usage: Usage,
}

enum Event {
    Step(Step),
    Done(Result<Answer, AiError>, Option<String>),
}

pub struct AssistantService {
    pub exchanges: Vec<Exchange>,
    /// "Searching markets…" while working.
    pub status: Option<String>,
    /// The model that answered last.
    pub model: Option<String>,
    conv: Arc<Mutex<Vec<Turn>>>,
    rx: Option<Receiver<Event>>,
    /// (provider fingerprint, model) so the model list is fetched once per provider/key.
    resolved: Arc<Mutex<Option<(u64, String)>>>,
    http: reqwest::Client,
}

impl Default for AssistantService {
    fn default() -> Self {
        AssistantService {
            exchanges: Vec::new(),
            status: None,
            model: None,
            conv: Arc::new(Mutex::new(Vec::new())),
            rx: None,
            resolved: Arc::new(Mutex::new(None)),
            http: reqwest::Client::builder().user_agent(concat!("EliteNexus/", env!("CARGO_PKG_VERSION"))).build().unwrap_or_default(),
        }
    }
}

impl AssistantService {
    /// A sample conversation for screenshot runs (`--demo`): long lines, a list and a pending answer.
    pub fn demo() -> Self {
        let mut s = AssistantService { model: Some("gemini-3.5-flash-lite".into()), ..Default::default() };
        s.exchanges.push(Exchange {
            question: "Are you aware of the first footfall status of planets on the route we're planning for exobiology? I need planets where I can find expensive biology that nobody has walked on yet.".into(),
            answer: Some(Ok("Yes. The exobiology planner (powered by Spansh) targets bodies that typically carry high-value biology such as **Stratum Tectonicas**, worth about 19 M Cr base or about 95 M Cr with the first-logged bonus.\n\nTo get the ×5 bonus:\n- Pick systems a little off the beaten path, where no commander has stamped their name.\n- When you scan a body with the FSS or DSS, check its footfall status before you land; bodies nobody has walked on in unpopulated systems qualify.\n  - Populated systems never pay the bonus, even when the body shows no footfall.\n\nYour route is loaded on the Navigation page. Happy hunting!".into())),
            tools: vec!["money_routes".into(), "knowledge".into()],
            usage: Usage::default(),
        });
        s.exchanges.push(Exchange { question: "Which of those bodies are still unfootfalled?".into(), answer: None, tools: Vec::new(), usage: Usage::default() });
        s.status = Some("Checking the planets…".into());
        s
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    /// Starts a new conversation.
    pub fn clear(&mut self) {
        if !self.busy() {
            self.exchanges.clear();
            self.conv.lock().clear();
        }
    }

    pub fn ask(&mut self, question: String, cfg: ProviderConfig, tools: NexusTools, context: String, ctx: &egui::Context) {
        let question = question.trim().to_string();
        if self.busy() || question.is_empty() {
            return;
        }
        self.exchanges.push(Exchange { question: question.clone(), answer: None, tools: Vec::new(), usage: Usage::default() });
        self.status = Some("Thinking…".into());
        let (tx, rx) = crossbeam_channel::unbounded();
        self.rx = Some(rx);
        let (conv, resolved, http, ctx) = (self.conv.clone(), self.resolved.clone(), self.http.clone(), ctx.clone());
        let spawned = std::thread::Builder::new().name("assistant".into()).spawn(move || {
            let send = |e: Event| {
                let _ = tx.send(e);
                ctx.request_repaint();
            };
            let rt = tools.rt.clone();
            let fp = fingerprint(&cfg);
            let cached = resolved.lock().as_ref().filter(|(f, _)| *f == fp).map(|(_, m)| m.clone());
            let model = cached.or_else(|| {
                let listed = nexus_ai::list_models(&http, &rt, &cfg).unwrap_or_default();
                let m = nexus_ai::resolve_model(&cfg, &listed)?;
                *resolved.lock() = Some((fp, m.clone()));
                Some(m)
            });
            let Some(model) = model else {
                send(Event::Done(Err(AiError::NoModel), None));
                return;
            };
            let backend = Remote { http: &http, rt: &rt, cfg: &cfg, model: model.clone() };
            let mut conv = conv.lock();
            nexus_ai::trim_history(&mut conv, 6);
            let q = nexus_ai::with_context(&context, &question);
            let res = nexus_ai::ask(&backend, nexus_ai::SYSTEM_PROMPT, &tools, &mut conv, q, |s| send(Event::Step(s)));
            if res.is_err() {
                // A wrong model name or key: look the model up again next time.
                *resolved.lock() = None;
            }
            send(Event::Done(res, Some(model)));
        });
        if spawned.is_err() {
            self.rx = None;
            self.status = None;
            if let Some(last) = self.exchanges.last_mut() {
                last.answer = Some(Err("Could not start the assistant.".into()));
            }
        }
    }

    /// Call every frame.
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut done = false;
        while let Ok(e) = rx.try_recv() {
            match e {
                Event::Step(Step::Thinking) => self.status = Some("Thinking…".into()),
                Event::Step(Step::Tool(name)) => {
                    self.status = Some(tool_label(&name).into());
                    if let Some(last) = self.exchanges.last_mut()
                        && !last.tools.contains(&name)
                    {
                        last.tools.push(name);
                    }
                }
                Event::Done(res, model) => {
                    if model.is_some() {
                        self.model = model;
                    }
                    if let Some(last) = self.exchanges.last_mut() {
                        match res {
                            Ok(a) => {
                                last.answer = Some(Ok(a.text));
                                last.usage = a.usage;
                                last.tools = a.tools_used;
                            }
                            Err(e) => last.answer = Some(Err(friendly(&e))),
                        }
                    }
                    done = true;
                }
            }
        }
        if done {
            self.rx = None;
            self.status = None;
        }
    }
}

fn fingerprint(cfg: &ProviderConfig) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (format!("{:?}", cfg.kind), cfg.base_url(), &cfg.api_key, &cfg.model).hash(&mut h);
    h.finish()
}

pub fn tool_label(name: &str) -> &'static str {
    match name {
        "commander" => "Checking your ship…",
        "find_commodity" => "Searching markets…",
        "price_deals" => "Looking for deals…",
        "sell_my_cargo" => "Planning your cargo sale…",
        "trade_routes" => "Finding trade routes…",
        "nearest_service" => "Finding stations…",
        "mining_hotspots" => "Finding hotspots…",
        "system_info" => "Looking up the system…",
        "system_bodies" => "Checking the planets…",
        "plan_route" => "Plotting a route with Spansh…",
        "money_routes" => "Planning a money route with Spansh…",
        "exobiology_here" => "Checking the biology here…",
        "journal_search" => "Searching your journal…",
        "knowledge" => "Reading up…",
        _ => "Working…",
    }
}

/// Short noun for the "used: markets · deals" line under an answer.
pub fn tool_noun(name: &str) -> &'static str {
    match name {
        "commander" => "your ship",
        "find_commodity" => "markets",
        "price_deals" => "deals",
        "sell_my_cargo" => "your cargo",
        "trade_routes" => "trade routes",
        "nearest_service" => "stations",
        "mining_hotspots" => "hotspots",
        "system_info" => "system data",
        "system_bodies" => "planets",
        "plan_route" | "money_routes" => "Spansh",
        "exobiology_here" => "biology",
        "journal_search" => "your journal",
        "knowledge" => "knowledge base",
        _ => "tools",
    }
}

fn friendly(e: &AiError) -> String {
    match e {
        e if e.is_auth() => "The model service rejected the API key. Check it with the gear button.".into(),
        AiError::Status { code: 429, .. } => "The model service is busy or the free quota is used up. Try again in a minute.".into(),
        AiError::Status { code: 404, message } => format!("The model service doesn't know that model ({message})."),
        AiError::NoModel => "No model is available for this service.".into(),
        AiError::Http(m) => format!("Can't reach the model service: {m}"),
        other => other.to_string(),
    }
}
