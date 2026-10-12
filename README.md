# Elite Nexus

**One native, fast companion for Elite Dangerous** — replacing the pile of tools commanders run side by side
(EDDiscovery, ED Market Connector, Odyssey Materials Helper, Observatory + BioInsights, Exploration Buddy,
EDCoPilot-style voice alerts, OD Elite Tracker) and the websites you alt-tab to (INARA, Spansh, EDSM, EDSY/Coriolis,
EDAstro, ED Tools, EDRefCard).

Pure Rust. No Python, no Node.js, no browser engine. One executable, GPU-rendered UI (egui/wgpu).

## Why it's faster

| | Elite Nexus |
|---|---|
| Full journal history load | **~40–60 ms** for 16.5k events (parallel parse, ~260 MB/s, 600k events/s) — EDD takes minutes on big histories |
| Live latency | journal + Status.json polled every **100 ms** (no missed writes, no notification lag) |
| Market data | **direct EDDN firehose** (~550 msgs/min) into a local SQLite DB — prices the moment any commander docks, before websites ingest them |
| Market queries | best price within 50 ly in **~1 ms**, galaxy-wide best price 0.2 ms, trade routes in 8–22 ms on an 80k-station / 4.4M-price database |
| Startup to usable | about one second |

## What works now

* **Dashboard** — commander, ranks & progress, credits, legal state, ship (fuel/hull/cargo/jump range/rebuy), location
  & system politics, session stats, current-system exploration value, holdings, fleet carrier, colonisation, activity feed.
* **Exploration** — every body as you FSS it: type, distance, value now vs. mapped (MattG formula, Odyssey bonus, first
  discovery/mapping, efficiency), gravity/temp/atmosphere, bio/geo signals, and Observatory-style highlights (ELW/WW/AW,
  terraformable, landable atmosphere, first footfall, close/wide rings, shepherd & nested moons, fast rotation/orbit,
  jumponium…). Unsold data, data the game refused to buy, codex history.
* **Exobiology** — live **sampling radar**: previous sample points around you (oriented to your heading) with colony-range
  circles and a big "OK to sample / too close" readout; bio bodies with DSS genera; unsold Vista Genomics data with
  first-logged bonus (5×). **Species predictions** from the body's atmosphere, gravity, temperature, volcanism and parent
  stars before you land, with expected value (checked against your own history: for 15 of the 16 species you sampled,
  the top prediction for that genus was the right one).
* **Market** — a local galaxy database fed live by EDDN, your own journal and (optionally) Spansh's daily dump:
  find the best/nearest place to buy or sell any commodity, **sell my cargo** (best single stop for the whole hold),
  trade routes for your ship and capital, nearest material traders / brokers / Interstellar Factors / Vista Genomics,
  mining hotspots with best sell prices, **price alerts** that fire seconds after any commander's market update
  matches your rule, and the full price list of the station you're docked at.
* **Navigation** — in-game route with scoopable/neutron/white-dwarf flags, non-scoopable streak warning, and **next
  system auto-copied to the clipboard after every jump**. Spansh planners built in: neutron, exact galaxy plotter
  (your ship's real FSD), fleet carrier (tritium), road to riches, exobiology.
* **Fleet** — all ships, where they are (incl. in-transit), full loadouts with engineering & modifier deltas; one-click
  **Open in EDSY / Coriolis** with the exact build.
* **Engineering** — blueprint planner with a wishlist (or "upgrade to G5" from your ship), shopping list, material-trader
  plan, engineers (unlocked first, by distance), Odyssey suits & weapons, synthesis, materials and ship locker.
* **Missions** — active missions with expiry, cargo/kill progress, and a **massacre-stack tracker** (kills count once per
  issuing faction; shows exactly how many kills remain).
* **Mining** — live sessions: tonnes/hour, prospector quality, motherlodes.
* **Fleet carrier** — tritium, finances, capacity, crew/services, orders, jump schedule & history.
* **Colonisation** — construction depots with remaining tonnage per commodity, what's already in your hold, and
  **where to buy** each commodity nearest the site with stock for a full hold.
* **BGS & Powerplay** — what you did for which faction since the tick.
* **Logbook / Statistics / Galaxy map** — activity, every journal event with full-text search, travel, docking, a credits
  ledger by category, credit history, jumps per day, in-game statistics, and a map of everywhere you've been.
* **Bindings** — your active presets, conflicts, missing essentials, printable reference card.
* **Accounts & sync** — strictly compliant EDDN contribution, EDSM journal sync, Inara profile sync, EDAstro. All opt-in;
  only live events (never history); persistent retry queues that respect each service's rate limits.
* **Alerts & voice** — toasts plus Windows speech for high-value bodies, likely high-value biology, sample spacing,
  motherlodes, non-scoopable stars when fuel is low, interdictions, hull critical, mission completion, carrier jumps…
* **Compact HUD** — one click turns the window into a small always-on-top panel for a second screen.

## Build & run

Download the latest `EliteNexus-…-windows-x64.zip` from the GitHub Releases page, unzip anywhere, run `EliteNexus.exe`.

To build it yourself (Windows, [Rust](https://rustup.rs) stable):

```powershell
cargo run -p nexus-app                 # debug
cargo build --release -p nexus-app     # target\release\EliteNexus.exe
```

On the original dev machine the toolchain is not on PATH: run `. .\tools\env.ps1` first. Pushing a tag such as `v0.1.0`
makes GitHub Actions build the exe and publish a Release (`.github/workflows/release.yml`).

Command-line options: `--page <name>`, `--journal-dir <dir>`, `--mute`, `--screenshot <png> [--shot-delay <s>]`.
Data (settings, DB, logs) is kept in `nexus-data\` next to the executable.

Dev tools: `cargo run --release -p nexus-journal --example scan_journals` (parse stats),
`cargo run --release -p nexus-model --example summary` (everything the model knows),
`cargo run --release -p nexus-journal --example replay -- <journal> <dir> [speed]` (simulate a live session),
`cargo run --release -p nexus-model --example predictions` (exobiology predictions vs. what you sampled),
`cargo run --release -p nexus-uplink --example validate` (every upload your journals would produce, checked against the
EDDN schemas — sends nothing), `cargo run --release -p nexus-galaxy --example seed_journal -- <db>` (galaxy DB from
your journals), `cargo run -p nexus-eddn --example live -- <db> <seconds>` (watch the live EDDN feed).

## Accounts you may want

* **EDSM** API key: https://www.edsm.net/en/settings/api
* **Inara** API key: https://inara.cz/elite/cmdr-settings-api/ (Inara also asks app authors to have the app name
  whitelisted before public release — keep "Developer mode" on until then; see docs/research/inara.md)
* **Frontier Companion API (CAPI)** needs a client ID approved by Frontier (docs/research/capi.md) — optional; the
  journal already provides almost everything.

Keys are stored encrypted with Windows DPAPI.

## Docs

* `docs/ARCHITECTURE.md` — crates, threads, conventions.
* `docs/research/` — verified specs: EDDN (live service differs from GitHub!), EDSM, Inara, CAPI, Spansh, websites,
  journal format, engineering data, exploration/exobiology science, and a 167-feature competitive matrix.

*Elite Dangerous is a trademark of Frontier Developments plc. Elite Nexus is an unofficial community tool.*
