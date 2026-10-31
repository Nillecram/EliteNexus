//! The MCP server inside the app: starts, restarts and stops `nexus_mcp::Server` as the settings say, and keeps the
//! tool set it serves current. Nothing here runs per frame: `apply` is called at start and when settings change.

use crate::ai_tools::NexusTools;
use crate::settings::McpSettings;
use nexus_mcp::{Config, Host, HostSlot, Server, ServerInfo};
use parking_lot::RwLock;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

/// Sent to the outside agent in `initialize`.
pub const INSTRUCTIONS: &str = "Elite Nexus is a companion app for Elite Dangerous running on the player's PC. Its tools read the commander's live state (journal, companion files), Elite Nexus's market database (EDDN) and Spansh. plan_route and money_routes also put the route on Elite Nexus's Navigation page. The tools can't change the game, its files or the player's settings.";

/// The name the setup snippets register the server under.
const SERVER_NAME: &str = "elite-nexus";

pub struct McpService {
    /// The tools being served; swapped by [`McpService::set_host`] while the server runs.
    slot: HostSlot,
    running: Option<(Config, Server)>,
    last_error: Option<String>,
    /// This PC's address on the network, looked up when a LAN server starts.
    lan_ip: Option<IpAddr>,
    /// Test runs (`--journal-dir`, `--screenshot`) never start the server.
    disabled: bool,
}

/// What Settings › MCP server shows.
#[derive(Clone, Debug, Default)]
pub struct McpView {
    pub disabled: bool,
    pub listening: Option<SocketAddr>,
    pub error: Option<String>,
    pub lan_ip: Option<IpAddr>,
}

impl McpService {
    pub fn new(host: NexusTools, disabled: bool) -> Self {
        let host: Host = Arc::new(host);
        McpService { slot: Arc::new(RwLock::new(host)), running: None, last_error: None, lan_ip: None, disabled }
    }

    /// Makes the server match the settings: starts it, restarts it when the port or the network option changed
    /// (or the key), stops it when switched off. Does nothing when nothing changed. Call from the UI thread:
    /// dropping the old server waits (a few ms) for its port to be free again.
    pub fn apply(&mut self, s: &McpSettings, rt: &tokio::runtime::Handle) {
        if self.disabled {
            return;
        }
        let want = s.config();
        if want.as_ref() == self.running.as_ref().map(|(c, _)| c) {
            if want.is_none() {
                self.last_error = None;
            }
            return;
        }
        self.running = None;
        self.last_error = None;
        self.lan_ip = None;
        let Some(cfg) = want else { return };
        let info = ServerInfo { version: env!("CARGO_PKG_VERSION").to_string(), instructions: INSTRUCTIONS.to_string() };
        match Server::start(rt, &cfg, self.slot.clone(), info) {
            Ok(server) => {
                if cfg.lan {
                    self.lan_ip = nexus_mcp::lan_ip();
                }
                self.running = Some((cfg, server));
            }
            Err(e) => {
                tracing::warn!("MCP server not started: {e}");
                self.last_error = Some(e);
            }
        }
    }

    /// Serves `host` from the next request on (the journal reader restarted, so the old tools read stale state).
    pub fn set_host(&self, host: NexusTools) {
        let host: Host = Arc::new(host);
        *self.slot.write() = host;
    }

    pub fn view(&self) -> McpView {
        McpView { disabled: self.disabled, listening: self.running.as_ref().map(|(_, s)| s.addr()), error: self.last_error.clone(), lan_ip: self.lan_ip }
    }
}

/// The command that adds the server to Claude Code. The key goes in a header, never in the URL.
pub fn claude_code_command(url: &str, key: Option<&str>) -> String {
    let mut cmd = format!("claude mcp add --transport http {SERVER_NAME} {url}");
    if let Some(key) = key {
        cmd += &format!(" --header \"Authorization: Bearer {key}\"");
    }
    cmd
}

/// The `claude_desktop_config.json` entry: Claude Desktop reaches an HTTP server through `mcp-remote`. With a key,
/// the header's value comes from an environment variable: Claude Desktop on Windows doesn't escape the space in
/// `Bearer <key>` inside `args` (mcp-remote's README), and `--allow-http` lets it use a plain-http network address.
pub fn claude_desktop_config(url: &str, key: Option<&str>) -> String {
    use serde_json::json;
    let mut server = json!({ "command": "npx", "args": ["-y", "mcp-remote", url] });
    if let Some(key) = key {
        server["args"] = json!(["-y", "mcp-remote", url, "--allow-http", "--header", "Authorization:${AUTH_HEADER}"]);
        server["env"] = json!({ "AUTH_HEADER": format!("Bearer {key}") });
    }
    serde_json::to_string_pretty(&json!({ "mcpServers": { SERVER_NAME: server } })).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_journal::{Event, EventCtx, raw::scan_line};
    use nexus_model::{Alerts, Model};
    use parking_lot::{Mutex, RwLock};
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::runtime::{Builder, Runtime};

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_tools_can_sit_in_the_server_slot() {
        assert_send_sync::<NexusTools>();
    }

    fn runtime() -> Runtime {
        Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("runtime")
    }

    /// A model with commander `name` loaded.
    fn model(name: &str) -> Model {
        let now = nexus_journal::Timestamp::now().to_iso();
        let lines = [
            format!(r#"{{ "timestamp":"{now}", "event":"Fileheader", "part":1, "language":"English/UK", "Odyssey":true, "gameversion":"4.4.1.1", "build":"r1 " }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"Commander", "FID":"F1", "Name":"{name}" }}"#),
            format!(r#"{{ "timestamp":"{now}", "event":"LoadGame", "FID":"F1", "Commander":"{name}", "Horizons":true, "Odyssey":true, "Ship":"Anaconda", "ShipID":1, "ShipName":"", "ShipIdent":"", "FuelLevel":32.0, "FuelCapacity":32.0, "GameMode":"Solo", "Credits":1000, "Loan":0 }}"#),
        ];
        let mut m = Model::default();
        let mut quiet = Alerts::disabled();
        for line in &lines {
            let (ts, ev_name) = scan_line(line).expect("line");
            let ev = Event::parse(ev_name, line).expect("event");
            m.apply(&EventCtx { ts, name: ev_name, line, live: false }, &ev, &mut quiet);
        }
        m
    }

    fn tools(rt: &Runtime, commander: &str) -> NexusTools {
        NexusTools {
            model: Arc::new(RwLock::new(model(commander))),
            db: None,
            index: Arc::new(RwLock::new(Default::default())),
            spansh: nexus_spansh::Spansh::new(),
            rt: rt.handle().clone(),
            routes: Arc::new(Mutex::new(None)),
        }
    }

    /// On, on loopback, on a port the system picks.
    fn on() -> McpSettings {
        McpSettings { enabled: true, port: 0, ..Default::default() }
    }

    fn post(addr: std::net::SocketAddr, body: &str) -> String {
        let mut s = std::net::TcpStream::connect(addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
        let req = format!("POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        s.write_all(req.as_bytes()).expect("write");
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn disabled_service_never_starts() {
        let rt = runtime();
        let mut svc = McpService::new(tools(&rt, "First"), true);
        svc.apply(&on(), rt.handle());
        let v = svc.view();
        assert!(v.listening.is_none() && v.error.is_none());
        assert!(v.disabled);
    }

    #[test]
    fn apply_starts_restarts_and_stops() {
        let rt = runtime();
        let mut svc = McpService::new(tools(&rt, "First"), false);
        assert!(svc.view().listening.is_none() && !svc.view().disabled);
        svc.apply(&McpSettings::default(), rt.handle());
        assert!(svc.view().listening.is_none(), "off by default");
        svc.apply(&on(), rt.handle());
        let addr = svc.view().listening.expect("listening");
        assert!(addr.ip().is_loopback() && addr.port() != 0);
        assert!(svc.view().lan_ip.is_none(), "loopback only: no network address to show");
        // The same settings again leave the running server alone.
        svc.apply(&on(), rt.handle());
        assert_eq!(svc.view().listening, Some(addr));
        // A change (here: the port, now fixed to the one it got) restarts it, and the port is free again at once.
        let list = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        assert!(post(addr, list).contains("200 OK"));
        svc.apply(&McpSettings { port: addr.port(), ..on() }, rt.handle());
        assert_eq!(svc.view().listening, Some(addr), "same address after the restart");
        assert!(post(addr, list).contains("200 OK"));
        // Off stops it and frees the port.
        svc.apply(&McpSettings { enabled: false, ..on() }, rt.handle());
        assert!(svc.view().listening.is_none() && svc.view().error.is_none());
        assert!(std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_err());
    }

    #[test]
    fn apply_reports_busy_port() {
        let rt = runtime();
        let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = taken.local_addr().expect("addr").port();
        let mut svc = McpService::new(tools(&rt, "First"), false);
        svc.apply(&McpSettings { port, ..on() }, rt.handle());
        let v = svc.view();
        assert!(v.listening.is_none());
        assert!(v.error.is_some_and(|e| e.contains(&port.to_string())));
        // Switching it off clears the message.
        svc.apply(&McpSettings { enabled: false, port, ..on() }, rt.handle());
        assert!(svc.view().error.is_none());
    }

    #[test]
    fn set_host_reaches_running_server() {
        let rt = runtime();
        let mut svc = McpService::new(tools(&rt, "First"), false);
        svc.apply(&on(), rt.handle());
        let addr = svc.view().listening.expect("listening");
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"commander","arguments":{}}}"#;
        let first = post(addr, call);
        assert!(first.contains("First") && !first.contains("Second"), "{first}");
        svc.set_host(tools(&rt, "Second"));
        let second = post(addr, call);
        assert!(second.contains("Second") && !second.contains("First"), "{second}");
    }

    #[test]
    fn setup_snippets() {
        let url = "http://127.0.0.1:47561/mcp";
        assert_eq!(claude_code_command(url, None), "claude mcp add --transport http elite-nexus http://127.0.0.1:47561/mcp");
        assert_eq!(claude_code_command(url, Some("k")), "claude mcp add --transport http elite-nexus http://127.0.0.1:47561/mcp --header \"Authorization: Bearer k\"");
        let plain: serde_json::Value = serde_json::from_str(&claude_desktop_config(url, None)).expect("json");
        let server = &plain["mcpServers"]["elite-nexus"];
        assert_eq!(server["command"], "npx");
        assert_eq!(server["args"], serde_json::json!(["-y", "mcp-remote", url]));
        assert!(server.get("env").is_none());
        let keyed: serde_json::Value = serde_json::from_str(&claude_desktop_config(url, Some("k"))).expect("json");
        let server = &keyed["mcpServers"]["elite-nexus"];
        assert_eq!(server["args"], serde_json::json!(["-y", "mcp-remote", url, "--allow-http", "--header", "Authorization:${AUTH_HEADER}"]));
        assert_eq!(server["env"]["AUTH_HEADER"], "Bearer k");
    }

    #[test]
    fn instructions_say_what_the_tools_cannot_do() {
        assert!(INSTRUCTIONS.starts_with("Elite Nexus is a companion app for Elite Dangerous"));
        assert!(INSTRUCTIONS.contains("can't change the game"));
    }
}
