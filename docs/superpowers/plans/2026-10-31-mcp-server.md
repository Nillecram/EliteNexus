# Elite Nexus MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let any MCP client (Claude Code, Claude Desktop through `mcp-remote`, …) use the Ask assistant's tools,
served by the running app, and give Ask (and so the MCP server) ten more tools that cover what `elite-mcp` does.

**Architecture:** A new small crate `nexus-mcp` serves MCP Streamable HTTP (JSON-RPC 2.0 over POST `/mcp`, JSON
responses only) on the app's existing tokio runtime, with hand-rolled HTTP/1.1 on `tokio::net` (no new HTTP crate).
It takes any `nexus_ai::ToolHost`; `nexus-app` hands it the same `NexusTools` the Ask page builds, through a
swappable slot so a core restart reaches it. Tool calls run in `spawn_blocking`, never on the UI thread or a runtime
worker. New tools live in `ai_tools/` submodules and are appended to `ai_tools::specs()`.

**Tech Stack:** Rust 2024 (1.93), tokio 1 (`net`, `io-util`, `time`, `sync`), serde_json, parking_lot,
`getrandom` 0.3 (already in `Cargo.lock` through rustls; no new crate compiled), egui 0.36.

**Spec:** `docs/superpowers/specs/2026-10-31-mcp-server-design.md`

## Global Constraints

* AGENTS.md hard rules apply. No `unwrap()`/`expect()` on request data, files or network data; `#[serde(default)]`
  on any struct read from outside.
* The server is **off by default**. "MCP server" binds `127.0.0.1:<port>`; "Allow other devices on the network"
  binds `0.0.0.0:<port>` and requires `Authorization: Bearer <key>` from every non-loopback peer.
* Default port **47561** (`nexus_mcp::DEFAULT_PORT`), user-settable 1024–65535.
* Key: 32 random bytes as 64 lower-case hex characters, stored sealed (`crate::secrets::seal`), never logged,
  compared in constant time.
* Reject any request whose `Origin` header names a host other than `localhost`, `127.0.0.1` or `[::1]` (HTTP 403).
* Request body cap **1 MiB** (413); whole-request read timeout **10 s**; at most **16** connections at once.
* Test runs (`--journal-dir`, `--screenshot`) never start the server.
* No new write actions: the only writes stay `plan_route`/`money_routes` (route on Navigation, clipboard after jumps).
* `ai_tools::specs()`: existing entries unchanged and in order; new entries appended after `knowledge`.
* Linux/macOS parity: no `#[cfg(windows)]` in this work.
* UI from `widgets.rs`/`theme.rs` only; no text below 12 pt. No `cargo fmt` over the workspace; match the
  hand-formatted long-line style.
* Every commit: imperative subject "Area: what changed", body saying what and why. **No `Co-Authored-By` trailer and
  no "Generated with" line** (the maintainer's personal rule overrides AGENTS.md here). Commit on `feat/mcp-server`;
  never push.
* Each task ends green: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (on
  Windows use `tools\dev\cargo.ps1 -Log <name> -CargoArgs "..."` and your own `-TargetDir target-agents\<name>`).
* `docs/USER_GUIDE.md` and `CHANGELOG.md` (under "Unreleased") change in the same commit as the behaviour they
  describe.

## Review Focus

1. A client that sends `Transfer-Encoding: chunked` or no `Content-Length` on POST must get HTTP 411 at once, not
   hang until the timeout. (Task 1, `chunked_or_missing_length_is_411`)
2. A tool that panics must cost that one call (JSON-RPC `-32603`), and the next request must still be served.
   (Task 3, `panicking_tool_leaves_server_up`)
3. Tools that call `Handle::block_on` (every Spansh tool) must work when called through the server, which runs on
   the same runtime. (Task 3, `tool_may_block_on_the_runtime`)
4. Port already taken (a second Nexus, another app): the switch stays on, Settings says the port is in use, nothing
   panics. (Task 3, `second_bind_on_same_port_fails_cleanly`; Task 8, `apply_reports_busy_port`)
5. Network access on with a key that can't be unsealed (settings copied to another PC, DPAPI fails): the server
   must not start without a key; a new key is generated. (Task 8, `lan_without_readable_key_gets_new_key`)

---

### Task 1: `nexus-mcp` crate, HTTP request reading and access checks

**Files:**
- Create: `crates/nexus-mcp/Cargo.toml`, `crates/nexus-mcp/src/lib.rs`, `crates/nexus-mcp/src/http.rs`
- Modify: `Cargo.toml` (workspace `members` after `nexus-ai`; `[workspace.dependencies]` `nexus-mcp = { path = "crates/nexus-mcp" }`)
- Modify: `docs/ARCHITECTURE.md` (Crates table: a `nexus-mcp` row before `nexus-app`; "depends on: ai")
- Also commit: `docs/superpowers/specs/2026-10-31-mcp-server-design.md`, `docs/superpowers/plans/2026-10-31-mcp-server.md`

**Interfaces:**
- Produces (in `http.rs`, re-exported from `lib.rs` as `pub mod http`):
  - `pub struct Request { pub method: String, pub path: String, pub headers: Vec<(String, String)>, pub body: Vec<u8> }`
    (header names lower-cased) and `impl Request { pub fn header(&self, name: &str) -> Option<&str> }`
  - `pub enum HttpError { BadRequest, LengthRequired, TooLarge, Closed }` with `pub fn status(&self) -> u16`
    (400, 411, 413; `Closed` = peer closed before a request line, no reply)
  - `pub async fn read_request<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R, max_body: usize) -> Result<Request, HttpError>`
  - `pub fn response(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8>` (always `Connection: close`
    and `Content-Length`; reason phrases for 200, 202, 400, 401, 403, 404, 405, 411, 413, 500)
  - `pub fn check_access(req: &Request, peer: std::net::IpAddr, key: &str) -> Result<(), u16>`: 403 for a foreign
    `Origin`; for a non-loopback peer, 401 unless `Authorization` is `Bearer <key>` and `key` is non-empty
  - `pub(crate) fn ct_eq(a: &[u8], b: &[u8]) -> bool`
  - `pub const MAX_BODY: usize = 1 << 20;`

- [ ] **Step 1: Create the crate skeleton** (`Cargo.toml` with `version.workspace` etc. like `nexus-ai/Cargo.toml`;
  deps: `nexus-ai`, `serde_json`, `tokio`, `tracing`, `parking_lot` from the workspace, `getrandom = "0.3"`). Doc
  comment at the top of `lib.rs`: what the crate is and that it only serves tools.

- [ ] **Step 2: Write the failing tests** in `http.rs` (`#[cfg(test)] mod tests`, `#[tokio::test]` over
  `tokio::io::BufReader::new(&bytes[..])`):

```rust
#[tokio::test] async fn reads_a_post_with_body() // "POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\nAuthorization: Bearer k\r\n\r\n{}"
    // => method "POST", path "/mcp", header("authorization") == Some("Bearer k"), body == b"{}"
#[tokio::test] async fn chunked_or_missing_length_is_411() // POST with "Transfer-Encoding: chunked" => Err(LengthRequired); POST without Content-Length => Err(LengthRequired)
#[tokio::test] async fn oversized_body_is_413()            // Content-Length: 1048577 => Err(TooLarge), without reading the body
#[tokio::test] async fn garbage_is_400()                   // b"\x00\x01 nonsense\r\n\r\n" => Err(BadRequest); header line without ':' => Err(BadRequest)
#[tokio::test] async fn get_needs_no_length()              // "GET /mcp HTTP/1.1\r\n\r\n" => Ok, body empty
#[test] fn loopback_needs_no_key()        // peer 127.0.0.1 and ::1, no Authorization => Ok(())
#[test] fn lan_peer_needs_the_key()       // peer 192.168.1.5: none => Err(401); "Bearer wrong" => Err(401); "Bearer k" with key "k" => Ok; any header with key "" => Err(401)
#[test] fn foreign_origin_is_403()        // Origin "http://evil.example" from 127.0.0.1 => Err(403); "http://localhost:3000", "http://127.0.0.1", "http://[::1]:8080" => Ok
#[test] fn response_has_length_and_close() // response(202, &[], b"") starts "HTTP/1.1 202 Accepted\r\n", contains "Content-Length: 0" and "Connection: close"
```

- [ ] **Step 3: Run them to see them fail**: `cargo test -p nexus-mcp` → compile errors / FAIL.

- [ ] **Step 4: Implement `http.rs`.** Header lines are capped (64 headers, 8 KiB per line → `BadRequest`). Parse
  `Origin` host with plain string handling (strip scheme, port, brackets); `check_access` checks Origin before auth.

- [ ] **Step 5: Run the tests**: `cargo test -p nexus-mcp` → all PASS; clippy clean.

- [ ] **Step 6: Commit** (spec, plan, crate, ARCHITECTURE row): `MCP: add nexus-mcp crate with HTTP reading and access checks`.

---

### Task 2: JSON-RPC dispatch over a `ToolHost`

**Files:**
- Create: `crates/nexus-mcp/src/rpc.rs`
- Modify: `crates/nexus-mcp/src/lib.rs` (`pub mod rpc; pub use rpc::ServerInfo;`)

**Interfaces:**
- Consumes: `nexus_ai::{ToolHost, ToolSpec}` (`fn tools(&self) -> Vec<ToolSpec>`, `fn call(&self, &str, &Value) -> Result<Value, String>`).
**Protocol (ruling made 2026-10-31 while executing, against the live specification):** the current MCP revision is
**2026-07-28** ("modern": stateless, no `initialize`; every request carries
`params._meta["io.modelcontextprotocol/protocolVersion"]` and `["io.modelcontextprotocol/clientCapabilities"]`, and
the HTTP headers `MCP-Protocol-Version`, `Mcp-Method`, plus `Mcp-Name` for `tools/call`). Revisions **2025-11-25,
2025-06-18, 2025-03-26** ("legacy") open with `initialize`. The server is **dual-era**: it serves both on `/mcp`,
deciding per request. References: modelcontextprotocol.io/specification/2026-07-28 › basic/versioning,
basic/index (`_meta`, `resultType`, error codes), basic/transports/streamable-http (request metadata, server
validation), server/discover, server/tools.

- Produces:
  - `pub struct ServerInfo { pub version: String, pub instructions: String }`
  - `pub const MODERN_VERSIONS: &[&str] = &["2026-07-28"];`
  - `pub const LEGACY_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];` (newest first)
  - `pub struct RpcHeaders<'a> { pub protocol_version: Option<&'a str>, pub method: Option<&'a str>, pub name: Option<&'a str> }`
    (the `MCP-Protocol-Version`, `Mcp-Method`, `Mcp-Name` header values; Task 3 fills it from `Request::header`)
  - `pub struct Reply { pub status: u16, pub body: Option<Value> }` (`body: None` ⇒ HTTP 202 with no body)
  - `pub fn handle(msg: &Value, h: &RpcHeaders, host: &dyn ToolHost, info: &ServerInfo) -> Reply`

**Rules `handle` implements:**
- Not a single JSON object (array, number, …) → 400, `-32600`, `id` null.
- A message without `method` (a client response) or without `id` (a notification) → `body: None` (202).
- **Modern** when `params._meta` has `io.modelcontextprotocol/protocolVersion`, or the `MCP-Protocol-Version` header
  names a modern version:
  - `_meta` version not supported (any era) → 400, `-32022` "Unsupported protocol version",
    `data: {"supported": [MODERN…, LEGACY…], "requested": <it>}`.
  - `_meta` version or `clientCapabilities` missing → 400, `-32602`.
  - Header `MCP-Protocol-Version` ≠ `_meta` version, `Mcp-Method` missing or ≠ `method`, or (for `tools/call`)
    `Mcp-Name` missing or ≠ `params.name` → 400, `-32020` "Header mismatch: …". `Mcp-Name` of the form
    `=?base64?<b64>?=` is decoded (standard alphabet, UTF-8) before comparing (`base64` 0.23 is a workspace dep).
  - `server/discover` → 200, `{"resultType":"complete","supportedVersions":[MODERN…, LEGACY…],"capabilities":{"tools":{}},"_meta":{"io.modelcontextprotocol/serverInfo":{"name":"elite-nexus","title":"Elite Nexus","version":…}},"instructions":…}`.
  - `tools/list`, `tools/call` → 200, results as below plus `"resultType":"complete"` and the same `_meta` serverInfo.
  - Any other method → **404**, `-32601`.
- **Legacy** otherwise: an `MCP-Protocol-Version` header naming no supported version → 400, `-32600`; absent header
  = 2025-03-26. `initialize` → 200, `{"protocolVersion": <the client's if in LEGACY_VERSIONS, else LEGACY_VERSIONS[0]>, "capabilities":{"tools":{}}, "serverInfo":{"name":"elite-nexus","title":"Elite Nexus","version":…}, "instructions":…}`;
  `ping` → `{}`; `tools/list`, `tools/call` as below; other methods → 200, `-32601`. No `Mcp-Session-Id` is minted.
- `tools/list` result: `{"tools":[{"name","description","inputSchema": spec.parameters}]}` in `specs()` order.
  `tools/call`: unknown name or `arguments` not an object → `-32602` (status 200); missing `arguments` = `{}`; `Ok(v)`
  → `{"content":[{"type":"text","text": serde_json::to_string(&v)}],"isError":false}`; `Err(s)` → same with
  `text: s`, `isError: true`.
- `id` (number or string) is echoed unchanged.

- [ ] **Step 1: Write the failing tests** in `rpc.rs` with a fake host (`struct Fake;` two tools: `echo` returns its
  args, `fail` returns `Err("nope")`) and helpers `legacy(msg)` (no headers) and `modern(method, params, name)` (adds
  `_meta` with version "2026-07-28" and `clientCapabilities: {}`, and matching headers):

```rust
#[test] fn legacy_initialize_answers_the_clients_version() // "2025-06-18" => 200, result.protocolVersion == "2025-06-18", capabilities.tools object, serverInfo.name "elite-nexus", serverInfo.title "Elite Nexus", instructions == info.instructions
#[test] fn legacy_initialize_offers_newest_for_unknown()   // "1999-01-01" => result.protocolVersion == "2025-11-25"
#[test] fn notifications_and_client_responses_get_202()    // notifications/initialized => status 202, body None; {"jsonrpc":"2.0","id":1,"result":{}} => 202, None
#[test] fn ping_is_empty_result()                          // legacy id 7 => {"jsonrpc":"2.0","id":7,"result":{}}
#[test] fn tools_list_maps_specs()                         // legacy and modern: result.tools[0] == {"name":"echo","description":..,"inputSchema":<spec.parameters>}; modern adds resultType "complete"
#[test] fn call_wraps_result_as_text()                     // echo {"a":1} => content[0] == {"type":"text","text":"{\"a\":1}"}, isError false
#[test] fn tool_error_is_is_error_not_protocol_error()     // fail => isError true, content[0].text "nope", no "error" key
#[test] fn unknown_tool_and_bad_args_are_32602()           // name "nope" => -32602; arguments "x" => -32602; missing arguments => echo gets {}
#[test] fn unknown_method_legacy_200_modern_404()           // "resources/list": legacy => 200, -32601; modern => 404, -32601; id echoed
#[test] fn batches_and_non_objects_are_32600()             // [] / [{..}] / 5 => 400, -32600, id null
#[test] fn modern_discover()                               // server/discover => 200, supportedVersions == ["2026-07-28","2025-11-25","2025-06-18","2025-03-26"], resultType "complete", _meta serverInfo name, instructions
#[test] fn modern_unsupported_version_is_32022()           // _meta version "1900-01-01" (header the same) => 400, -32022, data.requested "1900-01-01", data.supported lists all four
#[test] fn modern_missing_capabilities_is_32602()          // _meta without clientCapabilities => 400, -32602
#[test] fn modern_header_mismatch_is_32020()               // Mcp-Method "tools/list" on a tools/call body => 400, -32020; Mcp-Name "other" => 400, -32020; no Mcp-Name on tools/call => 400, -32020; header version "2025-06-18" with _meta "2026-07-28" => 400, -32020
#[test] fn modern_base64_name_is_decoded()                 // Mcp-Name "=?base64?ZWNobw==?=" for tool "echo" => 200
#[test] fn legacy_bad_version_header_is_400()              // legacy message with header "1999-01-01" => 400; with "2025-06-18" => 200
```

- [ ] **Step 2: Run** `cargo test -p nexus-mcp rpc` → FAIL.

- [ ] **Step 3: Implement `handle`** following the rules above. Text content is compact JSON. No
  `structuredContent`, no pagination, no `ttlMs`/`cacheScope`.

- [ ] **Step 4: Run** `cargo test -p nexus-mcp` → PASS; clippy clean.

- [ ] **Step 5: Commit**: `MCP: JSON-RPC dispatch for MCP 2026-07-28 and the 2025 revisions`.

---

### Task 3: The server: bind, accept, serve, stop

**Files:**
- Create: `crates/nexus-mcp/src/server.rs`
- Modify: `crates/nexus-mcp/src/lib.rs` (re-exports below)
- Test: `crates/nexus-mcp/src/server.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: Task 1 `http::*`, Task 2 `rpc::handle`, `ServerInfo`.
- Produces (re-exported at the crate root):
  - `pub const DEFAULT_PORT: u16 = 47561;`
  - `pub type Host = std::sync::Arc<dyn nexus_ai::ToolHost + Send + Sync>;`
  - `pub type HostSlot = std::sync::Arc<parking_lot::RwLock<Host>>;`
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct Config { pub port: u16, pub lan: bool, pub key: String }`
  - `pub struct Server` with `pub fn start(rt: &tokio::runtime::Handle, cfg: &Config, host: HostSlot, info: ServerInfo) -> Result<Server, String>`,
    `pub fn addr(&self) -> std::net::SocketAddr`, and `impl Drop` that stops accepting and frees the port.
  - `pub fn new_key() -> Result<String, String>` (getrandom 0.3: `getrandom::fill`)
  - `pub fn lan_ip() -> Option<std::net::IpAddr>` (UDP socket "connected" to `192.0.2.1:9` and `local_addr()`; no
    packet is sent; `None` if loopback/unspecified or on error)

- [ ] **Step 1: Write the failing tests** (multi-thread runtime: `tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all()`,
  like the app's; `Handle::block_on` can't drive IO on a current-thread runtime). Port 0 binds an ephemeral port;
  the client is a raw `std::net::TcpStream` writing a request and reading to EOF. Helper `post(addr, body, extra_headers) -> (u16, String)`.

```rust
#[test] fn round_trip_legacy()                // initialize => 200, body.result.protocolVersion; tools/call echo => 200; notifications/initialized => 202, empty body
#[test] fn round_trip_modern()                // server/discover and tools/call echo with _meta and Mcp-* headers => 200, Content-Type application/json; unknown method => 404
#[test] fn get_is_405_and_other_paths_404()   // GET /mcp => 405; DELETE /mcp => 405; POST /other => 404
#[test] fn rpc_status_reaches_http()          // header "MCP-Protocol-Version: 1999-01-01" on a legacy message => 400; a modern header mismatch => 400
#[test] fn lan_without_key_refuses_to_start()  // Config { lan: true, key: "" } => Err(..)
#[test] fn second_bind_on_same_port_fails_cleanly() // start on port P, start again on P => Err(msg) and msg mentions the port
#[test] fn drop_frees_the_port()               // start, read addr().port(), drop, start again on that port => Ok
#[test] fn panicking_tool_leaves_server_up()   // host tool "boom" panics => 200 with error.code -32603; next tools/list => 200
#[test] fn tool_may_block_on_the_runtime()     // host tool calls rt_handle.block_on(tokio::time::sleep(1 ms)) => 200, isError false
#[test] fn new_key_is_64_hex()                 // len 64, all ascii hex, two keys differ
```

- [ ] **Step 2: Run** `cargo test -p nexus-mcp server` → FAIL.

- [ ] **Step 3: Implement `Server::start`.** Bind with `std::net::TcpListener` (`127.0.0.1` or `0.0.0.0`), so a busy
  port is a synchronous `Err` ("Port 47561 is in use by another program." for `AddrInUse`), then
  `set_nonblocking(true)` and `tokio::net::TcpListener::from_std` inside `rt.enter()`. The accept loop is one
  `rt.spawn` task, aborted in `Drop`. Per connection: `tokio::sync::Semaphore` (16) `try_acquire_owned` or close;
  `tokio::time::timeout(10 s, read_request(.., MAX_BODY))`; `check_access(&req, peer.ip(), &cfg.key)`; route
  (`/mcp`: POST → parse JSON (unparseable → 400, `-32700`), `tokio::task::spawn_blocking` running `rpc::handle`
  with `RpcHeaders` from the request and `host.read().clone()`, `JoinError` → 500, `-32603` "The tool stopped
  unexpectedly."; the `Reply.status` becomes the HTTP status, `body: None` → 202 with no body; GET/DELETE → 405 with
  `Allow: POST`); write the response; shut down. Ignore `Mcp-Session-Id` and `Last-Event-ID`.
  `tracing::info!` the bound address once; never log headers or bodies.

- [ ] **Step 4: Run** `cargo test -p nexus-mcp` → PASS; clippy clean.

- [ ] **Step 5: Commit**: `MCP: serve tools over Streamable HTTP, off the UI thread`.

---

### Task 4: New tools `fleet` and `progress`

**Files:**
- Create: `crates/nexus-app/src/ai_tools/cmdr.rs`
- Modify: `crates/nexus-app/src/ai_tools.rs` (`mod cmdr;` near the top, two arms in `ToolHost::call`, two
  `ToolSpec`s appended after `knowledge` in `specs()`)
- Modify: `docs/USER_GUIDE.md` (the Ask row, line ~34: add "your whole fleet and engineering, rank progress and
  engineers"), `CHANGELOG.md` (Unreleased › Added: a bullet "**Ask** knows more:" listing what this task adds;
  later tasks extend that bullet)

**Interfaces:**
- Consumes: `NexusTools` fields; `GameState.fleet: Fleet` (`ships: BTreeMap<u64, Ship>`, `current_ship_id`,
  `stored_modules`), `Ship` (`type_name`, `name`, `ident`, `location: ShipLocation`, `max_jump_range`,
  `cargo_capacity`, `rebuy`, `total_value()`, `hull_health`, `modules: Vec<ShipModule>`), `ShipModule.engineering:
  Option<ModuleEngineering>`; `Commander` (`ranks`, `progress: RankLadder`, `engineers: BTreeMap<u32,
  EngineerStatus>`, `reputation`, `powerplay`, `squadron`, `game_mode`, `game_version`, `last_login`);
  `GameState.game_closed`, `at_main_menu`; `nexus_data::outfitting::{display_name, slot_display}`;
  `nexus_data::ranks::RankKind::ALL`.
- Produces: tool `fleet` (args `{ all_modules?: bool }`), tool `progress` (no args); in `cmdr.rs`
  `impl NexusTools { pub(super) fn fleet(&self, a: &Value) -> Result<Value, String>; pub(super) fn progress(&self) -> Result<Value, String> }`.

- [ ] **Step 1: Write the failing tests** in `ai_tools.rs`'s test module, extending the `commander()` fixture with a
  second ship stored at a station (`StoredShips` line), a `Loadout` module carrying `Engineering` (FSD,
  `FSD_LongRange` grade 5, experimental `special_fsd_heavy`), `Rank` + `Progress` lines and one `EngineerProgress`:

```rust
#[test] fn fleet_lists_every_ship_with_engineering()
    // fleet {} => ships.len() == 2; the current ship has "current": true and "where".system == "Alpha";
    // the stored one has "where".station == <station>; current ship "engineered"[0] has "blueprint" == "FSD_LongRange", "grade" == 5,
    // "experimental" present; no "modules" key; fleet {"all_modules": true} => "modules" array on the current ship
#[test] fn progress_has_ranks_engineers_and_game_state()
    // progress => ranks contains {"rank":"Explore", "name":<rank_name>, "progress_pct": <n>}; engineers[0].name, .rank;
    // "game"."running" == !game_closed; "game"."at_main_menu" bool
#[test] fn new_tools_are_appended_and_old_ones_unchanged()
    // specs()[..16] names equal the 16 existing names in their current order; every spec name is unique;
    // every name in specs() is accepted by NexusTools::call (no "Unknown tool")
```

- [ ] **Step 2: Run** `cargo test -p nexus-app ai_tools` → FAIL.

- [ ] **Step 3: Implement** both tools in `cmdr.rs` with one short `model.read()` each. Descriptions (exact text):
  - `fleet`: "Every ship the commander owns: where it is (or that it's in transit and when it arrives), jump range, cargo, value, rebuy and its engineered modules (blueprint, grade, experimental effect). all_modules=true adds every module. Modules are those of the last loadout the game reported for that ship."
  - `progress`: "The commander's progress: every rank with its progress to the next, engineers (unlocked with their rank, or invited/known), superpower reputation, Powerplay, squadron, and whether the game is running or at the main menu."
  For the current ship, "where" comes from `gs.location` (its `ShipLocation` is stale while flown).

- [ ] **Step 4: Run** `cargo test -p nexus-app ai_tools` → PASS; clippy clean.

- [ ] **Step 5: Commit** (code + guide + changelog): `Ask: fleet and progress tools`.

---

### Task 5: New tools `materials`, `on_foot` and `credit_history`

**Files:**
- Modify: `crates/nexus-app/src/ai_tools/cmdr.rs`, `crates/nexus-app/src/ai_tools.rs` (arms + appended specs)
- Modify: `docs/USER_GUIDE.md` (Ask row: "materials and on-foot gear, recent credits"), `CHANGELOG.md` (extend the Ask bullet)

**Interfaces:**
- Consumes: `Inventory.materials: Materials` (`raw`/`manufactured`/`encoded`: `BTreeMap<String, i64>`,
  `display_name`, `updated`), `nexus_data::materials::{material, cap}` (`Material.grade`), `Inventory.suit_loadouts`
  (`SuitLoadoutInfo`), `current_suit_loadout`, `ship_locker`/`backpack: Locker` (`items`, `components`,
  `consumables`, `data`: `BTreeMap<String, LockerItem{name,count}>`), `History.ledger: Vec<LedgerEntry{ts,category,amount,note}>`,
  `History::totals_since`, `LedgerCategory::label`, `Commander.last_login`, `Commander.credits`.
- Produces: `materials` (args `{ category?: "raw"|"manufactured"|"encoded", name?: string }`), `on_foot` (no args),
  `credit_history` (args `{ limit?: integer, since_hours?: number }`); `pub(super) fn materials(&self, a) / on_foot(&self) / credit_history(&self, a)`.

- [ ] **Step 1: Write the failing tests** (fixture gains a `Materials` line with `iron` 300 and `polonium` 12, a
  `SuitLoadout`, a `ShipLocker` line, and two credit events after `LoadGame`, e.g. `MarketSell` and `RefuelAll`):

```rust
#[test] fn materials_have_grade_cap_and_full_flag()
    // materials {} => iron: "grade" 1, "cap" 300, "full" true; polonium: grade 4, cap 150, full false;
    // {"category":"raw","name":"polo"} => exactly polonium
#[test] fn on_foot_lists_loadouts_and_locker()
    // on_foot => loadouts[0].suit, .weapons non-empty, "current" true; locker.items/components/consumables/data present
#[test] fn credit_history_since_login_newest_first()
    // credit_history {} => "balance" == cmdr.credits; transactions[0] is the later event; amounts signed;
    // "totals" has Trade income > 0 and Fuel expense < 0; {"limit": 1} => one transaction
```

- [ ] **Step 2: Run** `cargo test -p nexus-app ai_tools` → FAIL.

- [ ] **Step 3: Implement.** `credit_history`: entries with `ts >= since` (`since` = now − `since_hours`, else
  `cmdr.last_login`), newest first, `limit` default 25 clamped 1..=100. Descriptions:
  - `materials`: "Engineering materials the commander holds (raw, manufactured, encoded), each with its grade, storage cap and whether it's full. Filter by category or by part of a name."
  - `on_foot`: "On-foot equipment: suit loadouts (suit, modifications, weapons and their modifications), the one in use, and what's in the ship locker and backpack (items, components, consumables, data)."
  - `credit_history`: "Credits earned and spent since the game was last loaded (or in the last since_hours), newest first, with totals per category. Built from the journal: a few game actions record no amount, so the list may not add up to the balance exactly."

- [ ] **Step 4: Run** → PASS; clippy clean.

- [ ] **Step 5: Commit**: `Ask: materials, on-foot gear and credit history tools`.

---

### Task 6: New tools `system_factions` and `game_files`

**Files:**
- Modify: `crates/nexus-app/src/ai_tools/cmdr.rs`, `crates/nexus-app/src/ai_tools.rs`
- Modify: `docs/USER_GUIDE.md`, `CHANGELOG.md` (extend the Ask bullet: "factions of systems you've visited, with your reputation", "the game's live files")

**Interfaces:**
- Consumes: `GameState.location.info: SystemInfo` (`factions: Vec<FactionSummary>`, `controlling_faction`,
  `conflicts`, `updated`); `nexus_model::location::SystemInfo::from_arrival(&ev::SystemArrival, Timestamp)`;
  `nexus_journal::events::SystemArrival` (`Deserialize`); `JournalIndex::{id_of, entries, read_line, files}`.
- Produces: `system_factions` (args `{ system?: string }`), `game_files` (args `{ files?: [enum] }`);
  `pub(super) const GAME_FILES: [&str; 10] = ["Status", "NavRoute", "Cargo", "Market", "Outfitting", "Shipyard", "ModulesInfo", "ShipLocker", "Backpack", "FCMaterials"];`

- [ ] **Step 1: Write the failing tests** (fixture gains an `FSDJump` into "Gamma" with two factions, one with
  `MyReputation` and `PendingStates`, then a jump back to "Alpha" whose `Location` has one faction; the test writes
  the journal lines to a temp dir and indexes them the way `ai_tools.rs` `real_data` does (~line 1964:
  `JournalIndex::add_file`, then `push(ts, file, offset, len, name)` per line);
  for `game_files`, write `Status.json` and `NavRoute.json` next to the journal):

```rust
#[test] fn factions_of_current_system_from_model()
    // system_factions {} => "system" == "Alpha", factions.len() == 1, "source" mentions the journal
#[test] fn factions_of_a_visited_system_from_journal()
    // {"system":"gamma"} => system "Gamma", factions[*].my_reputation present, pending_states non-empty, "as_of" set
#[test] fn unvisited_system_says_so()
    // {"system":"Nowhere"} => Err containing "system_info"
#[test] fn game_files_reads_allowed_files_only()
    // game_files {} => files.Status.content is the JSON written, files.NavRoute present; "game_running" bool;
    // {"files":["Market"]} with no Market.json => files.Market.missing == true;
    // {"files":["../settings"]} and {"files":["Status.json"]} => Err naming the allowed names
```

- [ ] **Step 2: Run** → FAIL.

- [ ] **Step 3: Implement.** `system_factions`: current system (name matches `gs.location.system`, or no arg) from
  the model; otherwise walk `index.entries` newest first, keep `FSDJump`/`Location`/`CarrierJump` ids, `read_line`,
  cheap `contains` on the name before `serde_json::from_str::<SystemArrival>`, stop at the first exact
  (case-insensitive) `StarSystem` match. `game_files`: journal folder = parent of the newest `index.files`; read
  `<Name>.json` only for names in `GAME_FILES`; skip a file over 512 KiB with `"too_large": true`; content parsed as
  JSON (unparseable → `"unreadable": true`). Descriptions:
  - `system_factions`: "Factions in a system as the commander last saw it (default: the current system): influence, state, active, pending and recovering states, happiness and the commander's own reputation with each. Only for systems in the commander's journal; system_info has EDDN faction data for any system, without reputation."
  - `game_files`: "The game's live companion files as written now: Status (flags, fuel, legal state, destination…), NavRoute, Cargo, Market, Outfitting, Shipyard, ModulesInfo, ShipLocker, Backpack, FCMaterials. Default: Status and NavRoute. Market, Outfitting and Shipyard are the last station screen opened. Also says whether the game is running."

- [ ] **Step 4: Run** → PASS; clippy clean.

- [ ] **Step 5: Commit**: `Ask: system factions from the journal and the game's live files`.

---

### Task 7: New tools `find_ship`, `find_module`, `find_material_body`

**Files:**
- Create: `crates/nexus-app/src/ai_tools/outfit.rs`
- Modify: `crates/nexus-app/src/ai_tools.rs` (`mod outfit;`, arms, appended specs)
- Modify: `docs/USER_GUIDE.md`, `CHANGELOG.md` (extend the Ask bullet: "where to buy a ship or module (the station's
  catalogue, not its stock), where to find a raw material")

**Interfaces:**
- Consumes: `NexusTools::{me, origin}`, `SPANSH_TIMEOUT`, `nexus_spansh::SearchRequest` (`near`, `filter`,
  `sort_by`, `size`, `page`), `Spansh::{search_stations, search_bodies}`, `StationRecord` (`name`, `system_name`,
  `distance`, `distance_to_arrival`, `has_large_pad`, `medium_pads`, `ships: Vec<ShipyardEntry>`,
  `shipyard_updated_at`, `modules: Vec<OutfittingEntry>`, `outfitting_updated_at`), `BodyRecord` (`name`,
  `system_name`, `distance`, `distance_to_arrival`, `gravity`, `materials: Vec<NameShare>`, `signals`),
  `nexus_data::ships::all()`, `nexus_data::materials::material` (`kind == MaterialKind::Raw`).
- Produces: `find_ship` (`{ ship: string, near_system?, limit? }`), `find_module` (`{ module: string, class?: integer,
  rating?: string, near_system?, limit? }`), `find_material_body` (`{ material: string, near_system?, max_gravity_g?:
  number, max_distance_ls?: number }`); `pub(super) fn resolve_ship(input: &str) -> Result<String, Vec<String>>`
  (the Spansh/FDevIDs name, or suggestions).

- [ ] **Step 1: Write the failing tests** (offline ones in the test module; live ones `#[ignore]`d like the
  existing internet tests):

```rust
#[test] fn resolve_ship_normalises_spelling()
    // "krait mk2", "Krait MkII", "krait mk ii" => Ok("Krait Mk II"); "asp x" => Err(suggestions incl. "Asp Explorer")
#[test] fn find_material_body_rejects_non_raw()
    // {"material":"Biotech Conductors"} => Err mentioning "raw"; {"material":"Unobtainium"} => Err
#[test] #[ignore] fn live_find_ship_and_module()
    // near "Deciat": find_ship "Krait Mk II" => stations non-empty, "note" mentions catalogue; find_module "Fuel Scoop" class 5 rating "A" => matches non-empty
#[test] #[ignore] fn live_find_material_body()
    // near "Deciat": find_material_body "Polonium" => bodies sorted by share desc, each with system, gravity_g, share_pct, "searched_bodies" <= 500
```

- [ ] **Step 2: Run** `cargo test -p nexus-app ai_tools` → offline tests FAIL.

- [ ] **Step 3: Implement.** Station search as elite-mcp does: filter `"ships": [{"name": [ship]}]` or
  `"modules": [{"name": [module], "class": ["5"], "rating": ["A"]}]`, sorted by distance, `limit` default 10
  clamped 1..=25, every call wrapped in `tokio::time::timeout(SPANSH_TIMEOUT, …)` under `self.rt.block_on`.
  `find_material_body`: Spansh body search `is_landable` true and gravity `0..=max_gravity_g` (default 1.0) sorted
  by distance, 100 per page, at most 5 pages (500 bodies; stop early at 10 hits), keep bodies whose `materials` hold
  the material and that meet `max_distance_ls`, sort by share. Results carry `"searched_bodies"` and the cap.
  Descriptions:
  - `find_ship`: "Nearest stations whose shipyard lists a ship type, from Spansh (EDDN data): price and when the shipyard was last reported. It is the station's catalogue, not its stock: a listed ship can be sold out. Never call it in stock."
  - `find_module`: "Nearest stations whose outfitting lists a module (English name, e.g. Fuel Scoop, Frame Shift Drive), optionally of a class (1-8) and rating (A-I), from Spansh (EDDN data): the catalogue, not the stock."
  - `find_material_body`: "Landable bodies near a system that hold a raw material (e.g. Polonium), richest first, from Spansh. Searches the 500 nearest landable bodies at most (a cap that keeps the answer quick); max_gravity_g defaults to 1.0."

- [ ] **Step 4: Run** offline tests → PASS; then once `cargo test -p nexus-app live_find -- --ignored --nocapture` → PASS
  (needs the internet; report its output). Clippy clean.

- [ ] **Step 5: Commit**: `Ask: find ships, modules and raw-material bodies through Spansh`.

---

### Task 8: Settings, the app-side service and wiring

**Files:**
- Create: `crates/nexus-app/src/mcp.rs`
- Modify: `crates/nexus-app/Cargo.toml` (`nexus-mcp.workspace = true`), `crates/nexus-app/src/main.rs` (`mod mcp;`)
- Modify: `crates/nexus-app/src/settings.rs` (`McpSettings`, `Settings.mcp`)
- Modify: `crates/nexus-app/src/app.rs` (field `mcp`, `fn tools(&self) -> NexusTools`, Ask uses it, start, `after_settings`)

**Interfaces:**
- Consumes: Task 3 `nexus_mcp::{Config, Server, ServerInfo, HostSlot, DEFAULT_PORT, new_key, lan_ip}`.
- Produces:
  - `settings.rs`: `#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(default)] pub struct McpSettings { pub enabled: bool, pub lan: bool, pub port: u16, pub key: String /* sealed */ }`
    (`Default`: off, off, `DEFAULT_PORT`, empty); `impl McpSettings { pub fn ensure_key(&mut self) -> bool /* true if a key was made */; pub fn config(&self) -> Option<nexus_mcp::Config> /* None when off */ }`
  - `mcp.rs`: `pub struct McpService` with
    `pub fn new(host: NexusTools, disabled: bool) -> Self`,
    `pub fn apply(&mut self, s: &crate::settings::McpSettings, rt: &tokio::runtime::Handle)`,
    `pub fn set_host(&self, host: NexusTools)`,
    `pub fn view(&self) -> McpView`;
    `#[derive(Clone, Debug, Default)] pub struct McpView { pub disabled: bool, pub listening: Option<std::net::SocketAddr>, pub error: Option<String>, pub lan_ip: Option<std::net::IpAddr> }`;
    `pub fn claude_code_command(url: &str, key: Option<&str>) -> String`;
    `pub fn claude_desktop_config(url: &str, key: Option<&str>) -> String`;
    `pub const INSTRUCTIONS: &str` (sent in `initialize`).
  - `app.rs`: `fn tools(&self) -> crate::ai_tools::NexusTools` (the struct literal now in `Page::Ask`, moved here).

- [ ] **Step 1: Write the failing tests** in `mcp.rs` and `settings.rs`:

```rust
#[test] fn off_by_default()                       // McpSettings::default(): enabled false, lan false, port 47561, config() == None
#[test] fn lan_without_readable_key_gets_new_key() // lan on, key "dpapi:garbage" (secrets::open gives "") => ensure_key() == true, config().unwrap().key.len() == 64
#[test] fn disabled_service_never_starts()        // McpService::new(_, disabled = true).apply(enabled settings) => view().listening == None, view().disabled
#[test] fn apply_starts_restarts_and_stops()       // port 0 → listening Some; apply same settings again → same addr (no restart); enabled false → listening None
#[test] fn apply_reports_busy_port()               // occupy a port with std TcpListener, apply on it → listening None, error Some(contains port)
#[test] fn set_host_reaches_running_server()       // start, set_host(new tools whose model has CMDR "Second"), POST tools/call commander → text contains "Second"
#[test] fn setup_snippets()
    // claude_code_command("http://127.0.0.1:47561/mcp", None) == "claude mcp add --transport http elite-nexus http://127.0.0.1:47561/mcp"
    // with Some("k") ends with " --header \"Authorization: Bearer k\""
    // claude_desktop_config(url, None) parses as JSON: mcpServers."elite-nexus".command == "npx", args == ["-y","mcp-remote",url]
    // with Some("k"): args == ["-y","mcp-remote",url,"--allow-http","--header","Authorization:${AUTH_HEADER}"], env.AUTH_HEADER == "Bearer k"
```

  (Check the `mcp-remote` README before coding the Desktop snippet: `--allow-http` for a non-localhost `http://`
  URL, and the `${AUTH_HEADER}` env form that avoids Windows quoting trouble. Change the test if the README differs.)

- [ ] **Step 2: Run** `cargo test -p nexus-app mcp` → FAIL.

- [ ] **Step 3: Implement.** `McpService` holds `HostSlot`, `Option<(Config, Server)>`, `last_error`, `lan_ip`
  (looked up when a LAN config starts) and `disabled`. `apply`: when `disabled` do nothing; when `config()` equals the
  running one do nothing; else drop the old server, then `Server::start` (keep the error text on failure).
  `INSTRUCTIONS`: "Elite Nexus is a companion app for Elite Dangerous running on the player's PC. Its tools read the commander's live state (journal, companion files), Elite Nexus's market database (EDDN) and Spansh. plan_route and money_routes also put the route on Elite Nexus's Navigation page. The tools can't change the game, its files or the player's settings." In `app.rs`: build `McpService::new(tools, ov.journal_dir.is_some() || shot.is_some())` in `NexusApp::new`, call `apply(&settings.mcp, rt.handle())` once there; in `after_settings`, when `changed`, `self.settings.mcp.ensure_key()` then `self.mcp.apply(..)`; when `restart`, `self.mcp.set_host(self.tools())`. Assert `NexusTools: Send + Sync` with a test `fn assert_send_sync<T: Send + Sync>() {}`; if `GalaxyDb` makes it fail, wrap the host in a `Mutex` adapter inside `mcp.rs` and say so in the commit body.

- [ ] **Step 4: Run** `cargo test -p nexus-app` → PASS; clippy clean; `cargo build -p nexus-app`.

- [ ] **Step 5: Commit**: `MCP: settings and app service; Ask and the server share one tool set`.

---

### Task 9: Settings › AI agents (MCP) section and the docs

**Files:**
- Modify: `crates/nexus-app/src/pages/settings.rs` (`SettingsCtx.mcp: crate::mcp::McpView`; `fn agents(ui, cx)`
  called in the second card after `overlay(ui, cx)`)
- Modify: `crates/nexus-app/src/app.rs` (pass `mcp: self.mcp.view()` in `SettingsCtx`)
- Modify: `crates/nexus-app/src/pages/ask.rs` (one footnote under the setup card: "Prefer your own AI? Settings › AI agents (MCP) connects Claude Code, Claude Desktop and other MCP clients to the same tools.")
- Modify: `docs/USER_GUIDE.md` (new "AI agents (MCP)" part of the Settings section), `CHANGELOG.md` (Added: MCP
  server bullet), `site/companion.html` (an `#agents` section after `#providers` and its page-jump link),
  `site/llms.txt` (one line under the AI features), `docs/ARCHITECTURE.md` (Runtime: the `nexus-mcp` accept task on
  `nexus-net` and `spawn_blocking` tool calls; "its 16 tools" → 26 and "also served over MCP")

**Interfaces:**
- Consumes: Task 8 `McpView`, `McpSettings`, `claude_code_command`, `claude_desktop_config`, `nexus_mcp::new_key`.

- [ ] **Step 1: Build the section** with `section(ui, "AI agents (MCP)")` and `group`:
  - `g.switch(&mut s.mcp.enabled, "MCP server", "Lets an AI agent on this PC (Claude Code, Claude Desktop, …) use the same tools as Ask while Elite Nexus is open. Agents can read your commander and the market data, and put routes on Navigation; they can't change the game or your settings.")`
  - When enabled: `g.switch(&mut s.mcp.lan, "Allow other devices on the network", "For an agent on another PC. Every request from another device must carry the access key. The connection isn't encrypted: use it on your home network only, never open the port to the internet. Windows may ask to let Elite Nexus through the firewall.")`
  - A "Port" row (`egui::DragValue`, 1024..=65535; changes saved like other fields).
  - A "Status" row: "Listening on 127.0.0.1:47561" / "… and on 192.168.1.20:47561 for other devices" (`pal::GOOD`),
    the error text (`pal::BAD`), or "Not started: test run" when `disabled`.
  - "Claude Code" row: `widgets::button` "Copy command" → `ui.ctx().copy_text(claude_code_command(..))`.
  - "Claude Desktop" row: "Copy configuration" → `claude_desktop_config(..)`; detail: "Paste it into claude_desktop_config.json. Claude Desktop reaches Elite Nexus through the mcp-remote bridge, which needs Node.js."
  - When LAN is on: "Access key" row with "Copy key" (copies `secrets::open(&s.mcp.key)`) and "New key" (on
    `Ok(k)` from `new_key()`, `s.mcp.key = secrets::seal(&k)`; on `Err`, show the text in `pal::BAD` under the row;
    detail: "A new key disconnects agents that use the old one."). The key is never shown on screen.
  - Every copy button flashes "Copied" the way the page's other copy actions do (or uses a tooltip, matching the file).
  - Set `cx.changed = true` on every change.

- [ ] **Step 2: Check the UI**: `powershell -File tools\dev\shot.ps1 -Page Settings -Out mcp-narrow -Size 1000x760`
  and `-Out mcp-wide -Size 1600x900`; open both PNGs; the section must not truncate titles or overflow the card.
  (Screenshot runs never start the server: the status row reads "Not started: test run".)

- [ ] **Step 3: Write the docs** (all user-facing text in plain words, saying what agents can't do, that it works only
  while Elite Nexus is open, the Node.js need for Claude Desktop, the unencrypted-home-network warning and the
  firewall prompt). CHANGELOG bullet starts "**Settings › AI agents (MCP)** (off by default)".

- [ ] **Step 4: Manual check with Claude Code** (on the dev PC, real app, not a test run): switch the server on, run
  the copied command, start `claude`, ask "Where are my ships and what's engineered on them?" → it calls `fleet`;
  ask it to plan a route → the route appears on Navigation. Measure the cost and put it in the commit body: with the
  switch off, no listener and no extra thread (`netstat -ano | findstr 47561` shows nothing); with it on and idle,
  the app's CPU in Task Manager is unchanged and the thread count is unchanged (the accept loop is a task on the
  existing `nexus-net` runtime). Then `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` → green; Linux: `wsl -d EliteNexusLinux -u root -- bash tools/dev/linux.sh cargo clippy --workspace --all-targets -j 4 -- -D warnings`.

- [ ] **Step 5: Commit**: `Settings: AI agents (MCP) section; guide, site and changelog`.
