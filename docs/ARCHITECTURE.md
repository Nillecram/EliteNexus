# Elite Nexus — Architecture

Native Rust (edition 2024, rustc 1.99, MSVC) desktop companion for Elite Dangerous that replaces
EDDiscovery, ED Market Connector, Odyssey Materials Helper, Observatory, Exploration Buddy, EDCoPilot-style
alerts, OD Elite Tracker and the website tools (INARA/Spansh/EDSM/EDSY/Coriolis/EDAstro/ED Tools/EDRefCard)
with one fast app.

## Build

```powershell
. .\tools\env.ps1                     # puts the D:\GameDev\Tools\Rust toolchain on PATH (nothing global changed)
cargo run -p nexus-app                # debug build (deps optimised, fast enough)
cargo build --release -p nexus-app    # → target\release\EliteNexus.exe
cargo test --workspace
```

Agents/parallel builds: set your own `CARGO_TARGET_DIR` (e.g. `target-agents\<name>`) to avoid lock contention.
`EliteNexus.exe --page Exploration --screenshot out.png` renders a page after loading and saves a PNG (used for UI checks).

## Crates

| crate | role | depends on |
|---|---|---|
| `nexus-journal` | journal + companion-file I/O: typed events (`Event::parse`), `load_history` (parallel bulk load), `Watcher` (100 ms live tail), NaN-sanitising parser | — |
| `nexus-data` | static game knowledge compiled in: ranks, ships, exploration values, exobiology, commodities/modules/materials names (FDevIDs), engineering blueprints & engineers, material-trader maths | — |
| `nexus-model` | pure state machine: `Model` (all CMDRs) → `GameState` (location, fleet, inventory, missions + massacre stacks, exploration, exobio, mining, carrier, colonisation, history ledger, session, feed) + `Alerts` | journal, data |
| `nexus-galaxy` | local SQLite (WAL) galaxy/market DB: one writer thread, pooled read connections, in-memory station index; commodity search, trade routes, nearest services, hotspots, streaming Spansh dump import | journal |
| `nexus-eddn` | EDDN **subscriber** (ZeroMQ relay → decoder thread → galaxy DB), dedupe, watchdog + reconnect, stats & headlines | galaxy |
| `nexus-uplink` | **uploads**: EDDN (schema-validated, strictly compliant), EDSM journal sync, Inara, EDAstro; persistent retry queues (`nexus-data\uplink\*.jsonl`), per-service rate limits | journal |
| `nexus-spansh` | Spansh API client: neutron/galaxy/carrier/riches/exobio/trade plotters, system/station/body search | — |
| `nexus-binds` | `.binds` parser, conflicts/missing essentials, printable reference card | — |
| `nexus-app` | egui 0.36 / eframe (wgpu) GUI, services wiring, TTS, toasts | everything |

## Runtime

```
journal-watcher thread ──WatchMsg──► nexus-core thread ── write lock ──► Arc<RwLock<Model>> ◄── read lock ── UI (egui)
                                          │  alerts ──► UI toasts + TTS thread
                                          │  history sink (start-up) ──► galaxy-history thread ─┐
                                          └─ LiveMsg (Event | Companion, + EventContext)        │
                                               ├─► nexus-sync thread ──► nexus-uplink (queues)  │
                                               └─► galaxy-feed thread ──► galaxy-live thread ───┤
EDDN relay ──ZeroMQ──► socket task (tokio) ──► eddn-decoder thread ─────────────────────────────┤
Spansh dump (HTTP stream / file) ──► galaxy-import thread ──────────────────────────────────────┴─► galaxy-writer thread ──► galaxy.db
UI pages ──► background jobs (jobs.rs) ──► galaxy read pool / Spansh API (tokio)
```

* `app.rs` owns the services: `Core` (journal → model), `SyncService` (sync.rs → nexus-uplink),
  `MarketService` (market.rs → nexus-galaxy + nexus-eddn), Spansh client, TTS. `finder.rs` holds market lookups shared
  by pages (sell plan, colonisation suppliers).
* History is folded with alerts **disabled**; only live events raise alerts or get uploaded. Never upload history.
  Test runs (`--journal-dir`, `--screenshot`) never upload at all.
* The UI never blocks: long work (DB queries, HTTP) runs off-thread and results are polled each frame.
* Data dir: `nexus-data\` next to the exe (portable; the C: drive is nearly full) — fallback `%LOCALAPPDATA%\EliteNexus`, override `ELITE_NEXUS_HOME`.
  Contents: `settings.json` (API keys DPAPI-sealed), `galaxy.db` (+ WAL), `uplink\` queues and caches.
* The galaxy DB only keeps what queries use: EDDN jumps into unpopulated systems are dropped (populated systems,
  carrier jumps, docked arrivals, stations, markets and ring hotspots are kept); the commander's own journal is kept whole.

## Conventions

* Journal timestamps: `nexus_journal::Timestamp` (unix seconds). Commodity symbols: lower-case (`commodity_symbol("$gold_name;") == "gold"`).
* All serde structs for external data use `#[serde(default)]`; unknown fields ignored; never panic on input data.
* No `unwrap()` on I/O or network data. `tracing` for logs. Clippy-clean.
* Research specs live in `docs/research/*.md` (EDDN, EDSM, Inara, CAPI, Spansh, websites, journal, engineering, exploration, tool feature matrix).

## Licensing notes

* EDCD/FDevIDs CSVs (data/fdevids) have **no licence file** — fine for a private build; ask EDCD before distributing binaries.
* EDEngineer data (MIT), EDDiscovery sources (Apache-2.0), ed-journal-schemas (Apache-2.0): attribution in data/*/SOURCE.txt.
* coriolis-data / EDSY data are Frontier IP / CC BY-NC: link to the sites, don't bundle their data.
* Exobiology dataset is a factual compilation incl. facts from GPL tools — no GPL code is copied.
