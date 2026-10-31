# Elite Nexus as an MCP server — design

Date: 2026-10-31. Status: draft for approval.

## Intent and why

Elite Nexus's **Ask** page is an AI co-pilot: a model (Gemini by default, or Claude, Ollama, LM Studio) answers
questions with tools that read the commander's live state (`crates/nexus-app/src/ai_tools.rs`, `NexusTools`). Some
players already run their own AI agent next to the game (Claude Code, Claude Desktop, another MCP client) and keep
its knowledge of their commander in hand-written notes, which go stale after every session. The maintainer does
exactly that today with a separate Python MCP server (`elite-mcp`) that re-reads the journal.

Elite Nexus already knows all of this live. The goal is to let any MCP client use **the same tools Ask uses**, served
by the running app, so a player chooses: Ask inside Nexus, or their own agent plugged into Nexus. Both doors open onto
one tool catalogue, and the catalogue grows with what `elite-mcp` offers that Ask lacks, so Ask gains those too.

Success: with Nexus open and the switch on, a player connects Claude Code in one command, asks "where are my ships
and what's engineered on them?" and gets the live answer; asks it to plan a route and sees it appear on Navigation.

## Goals

1. **An MCP server inside the running app**, exposing the `ToolHost` catalogue (`nexus_ai::agent::ToolHost`,
   implemented by `NexusTools`): same tool names, descriptions and JSON schemas as Ask (`ai_tools::specs()`).
2. **More tools in that catalogue** (so Ask gets them too), covering what `elite-mcp` does and Ask doesn't. Nexus's
   model already tracks almost all of it (`nexus-model`: `fleet.rs`, `commander.rs`, `inventory.rs`, `bgs.rs`,
   companion files in `GameState`):

   | Need (elite-mcp tool) | Source in Nexus |
   |---|---|
   | Full briefing: position, credits, ranks and progress, engineers and their ranks, fleet with where each ship is, jump range, rebuy, engineered modules (`briefing`) | `Commander` (ranks, engineers, credits), `Fleet` (`Ship`, `ShipLocation`, `ShipModule`, `ModuleEngineering`), `Location` |
   | All modules of any ship, not only the current one (`briefing with_modules`) | `Fleet` ships' modules |
   | Engineering materials with grade and cap (`materials`) | `Inventory` materials |
   | On-foot suits, weapons, loadouts (`on_foot_loadout`) | `Inventory` locker / suit loadouts |
   | Faction state of a system with the commander's reputation and active, pending and recovering states (`system_factions`) | the journal history: last `FSDJump`/`Location`/`CarrierJump` carrying `Factions` for that system (current system by default), via `JournalIndex`; covers visited systems only, and says so. `nexus_model::bgs::Bgs` holds only the commander's own actions, and `system_info` already gives EDDN faction data for any system without reputation, so this is the complement, not a duplicate. *(Builder's call, made by Claude.)* |
   | Credits history: the last transactions and how the balance was reached (`credit_balance`) | journal (`JournalIndex`) since the last `LoadGame`; the balance itself stays in `commander` *(Claude's call)* |
   | Whether the game is running (`commander_status`) | `GameState::game_closed` / `at_main_menu`, added to the briefing *(Claude's call)* |
   | Companion files: Status, Cargo, Market, NavRoute, Outfitting, Shipyard, ModulesInfo, ShipLocker, Backpack, FCMaterials (`journal_snapshot`) | `GameState` (`status`, `market`, `outfitting`, `shipyard`, `modules_info`, `fc_materials`, `nav_route`, inventory) |
   | Stations selling a ship or module near a system (`find_ship`, `find_module`) | the galaxy database (`nexus-galaxy`, `services.rs`) or Spansh; it is the station's **catalogue**, not its stock: say so in the tool description and result |
   | Landable bodies richest in a raw material (`find_material_body`) | Spansh body search (`nexus-spansh`) |

   Already covered by Ask and not duplicated: neutron and exact routes (`plan_route`), exobiology and road-to-riches
   trips (`money_routes`, `untouched_biology`), system info, journal search, the credit balance (`commander`).

   Deliberately left out *(Claude's call)*: `spansh_raw` (forwards any Spansh API path: with network access on it would
   make Nexus an open proxy to Spansh, and the Spansh features agents need are wrapped by the tools above) and
   `journal_files` (a debugging aid; `journal_search` covers what an agent needs from the journal).
3. **Off by default, two switches.** "MCP server" listens on `127.0.0.1` only. "Allow other devices on the network",
   usable only when the first is on, listens on all interfaces so an agent on another PC can connect.
4. **An access key on the network.** When network access is on, Nexus generates a random key and every request from a
   non-loopback address must carry it (`Authorization: Bearer <key>`); it can be copied and regenerated. Requests
   from the same PC need no key, so a local agent connects without friction.
5. **Setup help in the app**: the address to use, and **Copy** buttons for the Claude Code command and the Claude
   Desktop configuration (with the key when network access is on). Status says whether it is listening, on which
   address, or why it isn't (port busy, …).
   * Claude Code connects over HTTP directly: `claude mcp add --transport http <name> http://<address>:<port>/mcp`
     (plus `--header "Authorization: Bearer <key>"` over the network).
   * Claude Desktop's `claude_desktop_config.json` starts local stdio servers, and its remote connectors are reached
     from Anthropic's cloud, which can't see `127.0.0.1` or a home network. So the Desktop button produces a config
     entry that runs the `mcp-remote` bridge (`npx -y mcp-remote <url>`, plus `--header` with the key over the
     network), the way the maintainer already connects NVIDIA App's MCP server. The UI and the guide say it needs
     Node.js. (Agreed in the conversation: "Claude Desktop needs a small bridge".) A stdio bridge built into the
     Nexus executable is not in scope.
6. **Honest UI and docs** (AGENTS.md): say the connection is not encrypted and is meant for a trusted home network,
   never the internet; that Windows may ask to allow Nexus through the firewall; that the server only works while
   Nexus is open; what agents can and cannot do.

## Non-goals

* Working while Nexus is closed (the maintainer confirmed: only when open).
* New write actions. The only writes are those Ask already has: `plan_route` and `money_routes` put a route on the
  Navigation page and copy the next system to the clipboard after each jump. Nothing else (confirmed: "rien de plus").
* MCP resources, prompts, sampling or other capabilities beyond tools (optional: `instructions` text, see below).
* TLS, user accounts, or exposure to the internet.
* Retiring the maintainer's personal `elite-mcp` or its Claude Code hooks (outside this repo; a later, personal step).
* Changing how Ask itself works, beyond gaining the new tools.

## Anti-goals (failure even if it "works")

* Any cost when the switch is off, and any per-frame cost when it is on: no polling, no work in the UI thread, an
  idle server sleeps on its socket (the app's first goal is to be fast and light: measure and report the cost).
* Listening by default, or on the network without the key.
* An agent able to change the game, its files, the player's settings, or to switch on or trigger an upload (EDDN,
  EDSM, Inara). The AGENTS.md hard rules apply to tool calls from outside exactly as to clicks.
* A tool call blocking or stalling the UI.
* A panic on malformed input from the network (no `unwrap` on request data).
* A heavy dependency tree for what is a small JSON-RPC endpoint.

## Constraints

* AGENTS.md hard rules, Linux/macOS parity (no Windows-only code without a counterpart; `tokio` `net` is portable),
  `widgets.rs`/`theme.rs` for the UI, no text below 12 pt, no `cargo fmt` over the workspace.
* Secrets: the network key is stored sealed like the other keys (`secrets.rs`), never logged.
* Test runs (`--journal-dir`, `--screenshot`) never start the server.
* `ai_tools::specs()` is "kept stable (providers cache it)": adding tools changes it once; keep the existing entries
  unchanged and append the new ones.
* Update `docs/USER_GUIDE.md` (it feeds the in-app assistant's knowledge), `CHANGELOG.md` (Unreleased), `site/` and
  `site/llms.txt`, and `docs/ARCHITECTURE.md` (a new thread/listener) in the same commits as the behaviour.
* CI: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` on Windows, Linux, macOS.

## Decided how

* **In the app process**, sharing the `NexusTools` the Ask page builds (`app.rs`, `Page::Ask`): same `model`, `db`,
  `index`, `spansh`, `rt`, and the same `routes` slot, so a route an agent plans shows on Navigation exactly like one
  from Ask.
* **Transport: MCP Streamable HTTP**, JSON-RPC 2.0 over HTTP POST on one endpoint (e.g. `/mcp`), JSON responses (no
  server-initiated streaming needed). Methods: `initialize`, `notifications/initialized`, `ping`, `tools/list`,
  `tools/call`; anything else returns the JSON-RPC "method not found" error. *(Updated while executing, Claude's
  call: the current revision, 2026-07-28, is stateless (no `initialize`; version and headers on every request,
  `server/discover`). The server is dual-era: 2026-07-28 and the 2025 handshake revisions on the same endpoint, so
  any current client connects. Details in the plan, Task 2.)* Tool errors (`Err(String)` from
  `ToolHost::call`) become results with `isError: true`, not protocol errors.
* **Security**: loopback bind unless network access is on; bearer key required for non-loopback peers (constant-time
  comparison); validate the `Origin` header as the MCP spec requires against DNS rebinding (reject browser origins
  that aren't localhost); cap request body size.
* **Tool calls run off the UI thread** (blocking pool or a dedicated thread), with locks held as briefly as Ask holds
  them.
* **Settings** gain the two switches, the port, and the sealed key; changes apply without restarting the app (start,
  stop or rebind the listener).

## Left to the builder

* Where the code lives (a module of `nexus-app`, or a small `nexus-mcp` crate taking a `ToolHost`).
* Hand-rolled HTTP on `tokio::net` versus a small HTTP crate; justify any new dependency by size and build time.
* The default port (fixed and configurable) and the Settings group it sits in, with wording that follows the app.
* Exact names, arguments and result shapes of the new tools, following the style of the existing `specs()` (short,
  honest descriptions that say where data comes from and what it can't show).
* Whether `initialize` returns `instructions` (e.g. built from `ai_tools::context_line`) to orient the agent.
* Which MCP protocol versions to accept (check the current specification; accept the client's version when
  supported, otherwise answer with the newest one implemented).
* The test set: protocol handling, auth and origin checks, a round trip with a real HTTP client, and the new tools
  against a replayed journal; plus a manual check with Claude Code.
