//! The MCP endpoint's lifecycle: bind, accept, serve each connection, stop.
//!
//! One request per connection (see `http`). Reading and answering HTTP is async; `rpc::handle` is synchronous and
//! tools may block for a long time (Spansh over the network through `Handle::block_on`), so it runs in
//! `spawn_blocking`, never on a runtime worker. The tools themselves come from a `HostSlot` that the app can swap
//! while the server runs.

use crate::http::{HttpError, MAX_BODY, Request, check_access, read_request, response};
use crate::rpc::{self, RpcHeaders, ServerInfo};
use nexus_ai::ToolHost;
use parking_lot::RwLock;
use serde_json::{Value, json};
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdListener, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// The port the server listens on unless the commander picks another.
pub const DEFAULT_PORT: u16 = 47561;
/// Connections served at once; one more is closed without an answer.
const MAX_CONNECTIONS: usize = 16;
/// How long a peer gets to send its whole request (head and body), and to take the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// The only path that speaks MCP.
const ENDPOINT: &str = "/mcp";

/// The tools served, shared with the app.
pub type Host = Arc<dyn ToolHost + Send + Sync>;
/// The tools in use right now; the app replaces the host inside while the server runs.
pub type HostSlot = Arc<RwLock<Host>>;

/// How the server listens.
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    /// Listen on every interface instead of loopback only; every non-loopback peer then needs `key`.
    pub lan: bool,
    pub key: String,
}

/// Written by hand so that logging a `Config` never prints the access key.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = if self.key.is_empty() { "<empty>" } else { "<set>" };
        f.debug_struct("Config").field("port", &self.port).field("lan", &self.lan).field("key", &format_args!("{key}")).finish()
    }
}

/// What a connection needs, shared by all of them.
struct Ctx {
    key: String,
    host: HostSlot,
    info: Arc<ServerInfo>,
}

/// A running server. Dropping it stops accepting and frees the port.
///
/// Drop it from a thread that is not a runtime thread (the UI thread is fine): `Drop` then waits until the port is
/// free, so starting again on the same port right away works. Dropped from a runtime or `spawn_blocking` thread, the
/// wait is skipped (blocking there could stall the runtime) and an immediate restart on the same port may fail.
pub struct Server {
    addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    /// Binds synchronously (a busy port is an `Err` right here), then accepts on `rt`.
    pub fn start(rt: &tokio::runtime::Handle, cfg: &Config, host: HostSlot, info: ServerInfo) -> Result<Server, String> {
        if cfg.lan && cfg.key.is_empty() {
            return Err("Allowing other devices needs an access key.".to_string());
        }
        let ip = if cfg.lan { Ipv4Addr::UNSPECIFIED } else { Ipv4Addr::LOCALHOST };
        let std_listener = StdListener::bind((ip, cfg.port)).map_err(|e| match e.kind() {
            ErrorKind::AddrInUse => format!("Port {} is in use by another program.", cfg.port),
            _ => format!("Could not listen on port {}: {e}", cfg.port),
        })?;
        let fail = |e: std::io::Error| format!("Could not listen on port {}: {e}", cfg.port);
        std_listener.set_nonblocking(true).map_err(fail)?;
        let addr = std_listener.local_addr().map_err(fail)?;
        let listener = {
            let _guard = rt.enter();
            TcpListener::from_std(std_listener).map_err(fail)?
        };
        tracing::info!("MCP server listening on {addr}");
        let ctx = Arc::new(Ctx { key: cfg.key.clone(), host, info: Arc::new(info) });
        let task = rt.spawn(accept_loop(listener, ctx));
        Ok(Server { addr, task })
    }

    /// The address actually bound (the port is the real one when `Config::port` was 0).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Aborting drops the accept loop, its listener and its open connections, but on a runtime worker, a moment
        // later. Wait for that (briefly) so that starting again on the same port right away works, unless this runs
        // on a runtime thread, which must not block.
        self.task.abort();
        if tokio::runtime::Handle::try_current().is_err() {
            for _ in 0..200 {
                if self.task.is_finished() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

async fn accept_loop(listener: TcpListener, ctx: Arc<Ctx>) {
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    // Connections live in the set, so stopping the server (dropping this task) drops them too.
    let mut conns = JoinSet::new();
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("MCP accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        while conns.try_join_next().is_some() {}
        let Ok(permit) = permits.clone().try_acquire_owned() else { continue }; // too many: the stream drops, closed
        let ctx = ctx.clone();
        conns.spawn(async move {
            let _permit = permit;
            serve(stream, peer, &ctx).await;
        });
    }
}

/// Reads one request, answers it, closes. Nothing is logged from the request.
async fn serve(stream: TcpStream, peer: SocketAddr, ctx: &Ctx) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let reply = match tokio::time::timeout(IO_TIMEOUT, read_request(&mut reader, MAX_BODY)).await {
        Err(_) | Ok(Err(HttpError::Closed)) => return, // too slow, or nobody there: no answer
        Ok(Err(e)) => response(e.status(), &[], b""),
        Ok(Ok(req)) => route(req, peer.ip(), ctx).await,
    };
    let _ = tokio::time::timeout(IO_TIMEOUT, async {
        write_half.write_all(&reply).await?;
        write_half.shutdown().await
    })
    .await;
}

fn json_response(status: u16, body: &Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(body).unwrap_or_else(|_| b"null".to_vec());
    response(status, &[("Content-Type", "application/json")], &bytes)
}

fn rpc_error(status: u16, code: i64, message: &str) -> Vec<u8> {
    json_response(status, &json!({"jsonrpc": "2.0", "id": null, "error": {"code": code, "message": message}}))
}

async fn route(req: Request, peer: IpAddr, ctx: &Ctx) -> Vec<u8> {
    match check_access(&req, peer, &ctx.key) {
        Ok(()) => {}
        Err(401) => return response(401, &[("WWW-Authenticate", "Bearer")], b""),
        Err(status) => return response(status, &[], b""),
    }
    let path = req.path.split('?').next().unwrap_or("");
    if path != ENDPOINT {
        return response(404, &[], b"");
    }
    if req.method != "POST" {
        return response(405, &[("Allow", "POST")], b"");
    }
    let Ok(msg) = serde_json::from_slice::<Value>(&req.body) else {
        return rpc_error(400, -32700, "Parse error");
    };
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let protocol_version = req.header("mcp-protocol-version").map(str::to_string);
    let method = req.header("mcp-method").map(str::to_string);
    let name = req.header("mcp-name").map(str::to_string);
    let host = ctx.host.read().clone();
    let info = ctx.info.clone();
    let done = tokio::task::spawn_blocking(move || {
        let headers = RpcHeaders { protocol_version: protocol_version.as_deref(), method: method.as_deref(), name: name.as_deref() };
        rpc::handle(&msg, &headers, &*host, &info)
    })
    .await;
    match done {
        Ok(reply) => match reply.body {
            Some(body) => json_response(reply.status, &body),
            None => response(202, &[], b""),
        },
        Err(_) => json_response(500, &json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32603, "message": "The tool stopped unexpectedly."}})),
    }
}

/// A fresh access key: 32 random bytes as 64 lower-case hex characters.
pub fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("Could not get random bytes for the key: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// This PC's address on the local network, to show next to the port. A UDP socket "connected" to a documentation
/// address (RFC 5737) only asks the OS which interface it would use: nothing is sent.
pub fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_ai::ToolSpec;
    use std::io::{Read, Write};
    use tokio::runtime::{Builder, Handle, Runtime};

    /// Tools `echo`, `boom` (panics) and `wait` (blocks on the runtime like the real tools do).
    struct TestHost {
        rt: Handle,
    }

    impl ToolHost for TestHost {
        fn tools(&self) -> Vec<ToolSpec> {
            ["echo", "boom", "wait"].into_iter().map(|name| ToolSpec { name, description: "test tool", parameters: json!({"type": "object"}) }).collect()
        }
        fn call(&self, name: &str, args: &Value) -> Result<Value, String> {
            match name {
                "echo" => Ok(args.clone()),
                "boom" => panic!("boom: this panic is expected by the test"),
                "wait" => {
                    self.rt.block_on(tokio::time::sleep(Duration::from_millis(1)));
                    Ok(json!("waited"))
                }
                _ => Err("unknown".to_string()),
            }
        }
    }

    /// A running server on an ephemeral port. `server` is first so it drops before the runtime.
    struct Fixture {
        server: Server,
        rt: Runtime,
    }

    fn runtime() -> Runtime {
        Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
    }

    fn slot(rt: &Runtime) -> HostSlot {
        let host: Host = Arc::new(TestHost { rt: rt.handle().clone() });
        Arc::new(RwLock::new(host))
    }

    fn info() -> ServerInfo {
        ServerInfo { version: "9.9.9".into(), instructions: "Use the tools.".into() }
    }

    fn cfg(port: u16) -> Config {
        Config { port, lan: false, key: String::new() }
    }

    fn fixture() -> Fixture {
        let rt = runtime();
        let server = Server::start(rt.handle(), &cfg(0), slot(&rt), info()).unwrap();
        Fixture { server, rt }
    }

    struct Resp {
        status: u16,
        /// Status line and headers, lower-cased.
        head: String,
        body: String,
    }

    /// Writes `raw`, reads until the server closes.
    fn send(addr: SocketAddr, raw: &[u8]) -> Resp {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(raw).unwrap();
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        Resp { status, head: head.to_ascii_lowercase(), body: body.to_string() }
    }

    fn post_full(addr: SocketAddr, body: &str, extra: &[(&str, &str)]) -> Resp {
        let mut req = format!("POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n", body.len());
        for (k, v) in extra {
            req.push_str(&format!("{k}: {v}\r\n"));
        }
        req.push_str("\r\n");
        req.push_str(body);
        send(addr, req.as_bytes())
    }

    fn post(addr: SocketAddr, body: &str, extra: &[(&str, &str)]) -> (u16, String) {
        let r = post_full(addr, body, extra);
        (r.status, r.body)
    }

    fn json_of(body: &str) -> Value {
        serde_json::from_str(body).unwrap_or(Value::Null)
    }

    fn modern_call(method: &str, params: Value) -> String {
        let mut params = params;
        params["_meta"] = json!({"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientCapabilities": {}});
        json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string()
    }

    #[test]
    fn round_trip_legacy() {
        let f = fixture();
        let addr = f.server.addr();
        let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}});
        let (status, body) = post(addr, &init.to_string(), &[]);
        assert_eq!(status, 200);
        assert_eq!(json_of(&body)["result"]["protocolVersion"], "2025-06-18");
        let call = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "echo", "arguments": {"a": 1}}});
        let (status, body) = post(addr, &call.to_string(), &[]);
        assert_eq!(status, 200);
        assert_eq!(json_of(&body)["result"]["isError"], false);
        let (status, body) = post(addr, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(), &[]);
        assert_eq!(status, 202);
        assert!(body.is_empty(), "{body}");
    }

    #[test]
    fn round_trip_modern() {
        let f = fixture();
        let addr = f.server.addr();
        let h = |method: &'static str| [("MCP-Protocol-Version", "2026-07-28"), ("Mcp-Method", method)];
        let r = post_full(addr, &modern_call("server/discover", json!({})), &h("server/discover"));
        assert_eq!(r.status, 200, "{}", r.body);
        assert!(r.head.contains("content-type: application/json\r\n"), "{}", r.head);
        assert!(json_of(&r.body)["result"]["supportedVersions"].is_array());
        let mut headers = h("tools/call").to_vec();
        headers.push(("Mcp-Name", "echo"));
        let (status, body) = post(addr, &modern_call("tools/call", json!({"name": "echo", "arguments": {"a": 1}})), &headers);
        assert_eq!(status, 200, "{body}");
        assert_eq!(json_of(&body)["result"]["isError"], false);
        let (status, _) = post(addr, &modern_call("nope/nothing", json!({})), &h("nope/nothing"));
        assert_eq!(status, 404);
    }

    #[test]
    fn get_is_405_and_other_paths_404() {
        let f = fixture();
        let addr = f.server.addr();
        let r = send(addr, b"GET /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert_eq!(r.status, 405);
        assert!(r.head.contains("allow: post"), "{}", r.head);
        assert_eq!(send(addr, b"DELETE /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n").status, 405);
        assert_eq!(send(addr, b"POST /other HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n{}").status, 404);
    }

    #[test]
    fn rpc_status_reaches_http() {
        let f = fixture();
        let addr = f.server.addr();
        let legacy = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string();
        assert_eq!(post(addr, &legacy, &[("MCP-Protocol-Version", "1999-01-01")]).0, 400);
        // A modern message whose Mcp-Method header disagrees with its body.
        let modern = modern_call("server/discover", json!({}));
        assert_eq!(post(addr, &modern, &[("MCP-Protocol-Version", "2026-07-28"), ("Mcp-Method", "tools/list")]).0, 400);
    }

    #[test]
    fn http_level_failures_have_their_status() {
        let f = fixture();
        let addr = f.server.addr();
        let (status, body) = post(addr, "{not json", &[]);
        assert_eq!(status, 400);
        assert_eq!(json_of(&body), json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}));
        assert_eq!(post(addr, "{}", &[("Origin", "http://evil.example")]).0, 403);
        assert_eq!(post(addr, "{}", &[("Origin", "http://evil.example\\@localhost")]).0, 403);
        assert_eq!(send(addr, b"POST /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n").status, 411);
        assert_eq!(send(addr, b"POST /mcp HTTP/1.1\r\nContent-Length: 1048577\r\n\r\n").status, 413);
        assert_eq!(send(addr, b"garbage\r\n\r\n").status, 400);
        // A page served from localhost is welcome.
        assert_eq!(post(addr, &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(), &[("Origin", "http://localhost:3000")]).0, 202);
    }

    #[test]
    fn config_debug_never_shows_the_key() {
        let secret = "0123456789abcdef-the-secret";
        let shown = format!("{:?}", Config { port: 47561, lan: true, key: secret.to_string() });
        assert!(!shown.contains(secret), "{shown}");
        assert!(shown.contains("<set>") && shown.contains("47561"), "{shown}");
        assert!(format!("{:?}", cfg(1)).contains("<empty>"));
    }

    /// A `Ctx` built the way `start` builds it, with a LAN key.
    fn lan_ctx(rt: &Runtime) -> Ctx {
        Ctx { key: "k3y".to_string(), host: slot(rt), info: Arc::new(info()) }
    }

    fn route_req(headers: &[(&str, &str)]) -> Request {
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string().into_bytes();
        Request { method: "POST".into(), path: "/mcp".into(), headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), body }
    }

    fn lan_route(rt: &Runtime, headers: &[(&str, &str)]) -> (u16, String) {
        let peer: IpAddr = "192.168.1.5".parse().unwrap();
        let bytes = rt.block_on(route(route_req(headers), peer, &lan_ctx(rt)));
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let status = text.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (status, text)
    }

    #[test]
    fn lan_peer_without_the_key_gets_401() {
        let rt = runtime();
        let (status, text) = lan_route(&rt, &[]);
        assert_eq!(status, 401, "{text}");
        assert!(text.contains("WWW-Authenticate: Bearer\r\n"), "{text}");
        assert_eq!(lan_route(&rt, &[("authorization", "Bearer wrong")]).0, 401);
    }

    #[test]
    fn lan_peer_with_the_key_is_served() {
        let rt = runtime();
        let (status, text) = lan_route(&rt, &[("authorization", "Bearer k3y")]);
        assert_eq!(status, 200, "{text}");
        assert!(text.contains("\"tools\""), "{text}");
    }

    #[test]
    fn foreign_origin_is_refused_even_with_the_key() {
        let rt = runtime();
        let (status, text) = lan_route(&rt, &[("authorization", "Bearer k3y"), ("origin", "http://evil.example")]);
        assert_eq!(status, 403, "{text}");
    }

    #[test]
    fn lan_without_key_refuses_to_start() {
        let rt = runtime();
        let r = Server::start(rt.handle(), &Config { port: 0, lan: true, key: String::new() }, slot(&rt), info());
        assert!(r.is_err());
    }

    #[test]
    fn second_bind_on_same_port_fails_cleanly() {
        let f = fixture();
        let port = f.server.addr().port();
        let second = Server::start(f.rt.handle(), &cfg(port), slot(&f.rt), info());
        let msg = second.err().unwrap_or_default();
        assert!(msg.contains(&port.to_string()), "{msg}");
        assert!(msg.contains("in use"), "{msg}");
    }

    #[test]
    fn drop_frees_the_port() {
        let rt = runtime();
        let server = Server::start(rt.handle(), &cfg(0), slot(&rt), info()).unwrap();
        let port = server.addr().port();
        drop(server);
        let again = Server::start(rt.handle(), &cfg(port), slot(&rt), info());
        assert!(again.is_ok(), "{:?}", again.err());
    }

    #[test]
    fn panicking_tool_leaves_server_up() {
        let f = fixture();
        let addr = f.server.addr();
        let boom = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "boom"}});
        let (status, body) = post(addr, &boom.to_string(), &[]);
        // The panic is a JoinError: a 500 carrying a JSON-RPC internal error for the same id.
        assert_eq!(status, 500, "{body}");
        assert_eq!(json_of(&body)["error"]["code"], -32603);
        assert_eq!(json_of(&body)["id"], 3);
        let (status, body) = post(addr, &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}).to_string(), &[]);
        assert_eq!(status, 200);
        assert_eq!(json_of(&body)["result"]["tools"].as_array().map(Vec::len), Some(3));
    }

    #[test]
    fn tool_may_block_on_the_runtime() {
        let f = fixture();
        let wait = json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "wait"}});
        let (status, body) = post(f.server.addr(), &wait.to_string(), &[]);
        assert_eq!(status, 200, "{body}");
        assert_eq!(json_of(&body)["result"]["isError"], false);
    }

    #[test]
    fn the_host_slot_can_be_swapped_while_running() {
        let f = fixture();
        struct Empty;
        impl ToolHost for Empty {
            fn tools(&self) -> Vec<ToolSpec> {
                Vec::new()
            }
            fn call(&self, _: &str, _: &Value) -> Result<Value, String> {
                Err("none".to_string())
            }
        }
        let slot = slot(&f.rt);
        let server = Server::start(f.rt.handle(), &cfg(0), slot.clone(), info()).unwrap();
        let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string();
        assert_eq!(json_of(&post(server.addr(), &list, &[]).1)["result"]["tools"].as_array().map(Vec::len), Some(3));
        let empty: Host = Arc::new(Empty);
        *slot.write() = empty;
        assert_eq!(json_of(&post(server.addr(), &list, &[]).1)["result"]["tools"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn a_seventeenth_connection_is_closed() {
        let f = fixture();
        let addr = f.server.addr();
        // Sixteen peers that connect and say nothing hold every slot.
        let idle: Vec<_> = (0..MAX_CONNECTIONS).map(|_| std::net::TcpStream::connect(addr).unwrap()).collect();
        let extra = send(addr, b"GET /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert_eq!(extra.status, 0, "an answer came back: {}", extra.head);
        drop(idle);
    }

    #[test]
    fn new_key_is_64_hex() {
        let (a, b) = (new_key().unwrap(), new_key().unwrap());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn lan_ip_is_a_real_address_or_nothing() {
        if let Some(ip) = lan_ip() {
            assert!(!ip.is_loopback() && !ip.is_unspecified(), "{ip}");
        }
    }
}
