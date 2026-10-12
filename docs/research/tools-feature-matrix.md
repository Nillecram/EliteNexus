# Elite Nexus: Competitive Feature Audit & Merged Feature Matrix

> **Snapshot date:** 2026-10-12 · **Scope:** the third-party tools Elite Nexus aims to replace (EDDiscovery, ED Market Connector + popular plugins, Odyssey Materials Helper, Observatory Core + BioInsights/Evaluator, Exploration Buddy, EDCoPilot, OD Elite Tracker) and adjacent tools (SrvSurvey, EDEngineer, colonisation planners/Raven Colonial, EDDI, COVAS:NEXT, EDSY/Coriolis, BGS/Powerplay/carrier/mining/trade tools, and 2026 newcomers).
>
> **Method:** nine parallel research passes over GitHub repos (READMEs, wikis, release notes, source files, issue trackers), official sites and docs, EDCodex, Steam discussions and Reddit (via an archive API). Frontier forum pages, inara.cz and edsm.net blocked automated fetching, so forum evidence comes from search snippets only.
>
> **Caveats:**
> - GitHub reactions are tiny in this niche (the top EDMC issue ever has 15 thumbs-up), so "most requested" is judged by comment counts and repeated reports.
> - Some release years were inferred where GitHub omits them.
> - Licensing notes are not legal advice.
> - Items that could not be verified are marked as such.

## 0. TL;DR

1. **The ecosystem is sprawling.**
   - EDCodex lists roughly 260–315 tools.
   - A typical 2025–26 player runs 4–8 of them at once. Commonly reported examples:
     - EDMC + plugins
     - EDCoPilot
     - Observatory/BioInsights
     - SrvSurvey
     - Exploration Buddy
     - EDOMH
     - EDDiscovery
     - Spansh, Inara, EDSY in the browser
   - Players say so explicitly and ask for "one cross-platform, plugin-based app" (r/EliteDangerous, Mar 2025 and Oct 2026). Several are now building their own all-in-ones.
2. **The incumbents each own one job.** All are very active in 2026, and nearly all are **single-maintainer, Windows-first** codebases. Linux support is via Proton/Mono/Wine, except EDMC and EDOMH.

   | Tool | Job it owns | Signature strength / weakness |
   |---|---|---|
   | **EDDiscovery** | The everything-app | "Time-machine" history and ~60 panels; slow at scale, fragile OpenGL map |
   | **EDMC** | The upload backbone + plugin host | Plugins break on host upgrades |
   | **EDOMH** | Engineering / materials / ship-builder reference | Java memory hog |
   | **EDCoPilot** | Voice + second-screen/VR cockpit | Closed source, needs paid VoiceAttack for voice input |
   | **Observatory + BioInsights / SrvSurvey / EDEB / BioScan** | Exploration & exobiology | Split across four tools |
   | **ODET** | Activity tracker: massacre, BGS, Powerplay, colonisation, carrier | No license, Windows only |
   | **Raven Colonial** | Colonisation hub | — |
3. **What nobody does well yet** (Nexus's opening):
   - A **single local data layer**: one event store, one EDDN-fed galaxy/market index, every module reading it.
   - **Reliable overlays + VR + LAN tablet UI from one app**.
   - **End-to-end colonisation**: target, claim, plan, haul, carrier, team.
   - **Carrier finance**, **passenger missions**, an **EDDB-class trade solver**, a **bindings editor/reference**, a **journal-fed OBS overlay**.
   - **A speech arbiter** so voice tools stop talking over each other.
   - **Native Linux**.
4. **2026 forces to design for:**
   - **Game updates**: Operations mode (`GameModeChange`), Rhino surface mining, Nomad/Lynx/Kestrel hulls, squadron carriers, colonisation rule churn.
   - **AI copilots**: COVAS:NEXT, Elite Intel, MCP bridges.
   - **Expectations**: LAN/tablet companions, and "local-only" privacy as a selling point.
5. **Direct all-in-one competitors exist:**
   - **EDNexus** (.NET/Avalonia, "does-it-all commander console"). **Its name nearly collides with "Elite Nexus"; check naming.**
   - **EDCAS** (Rust TUI + Rust server).
   - **EDLD**, **ED Colony Architect** and **Elite Dangerous Warboard**.
6. **Data constraints shape the architecture.**
   - **EDDN**:
     - strict uploader rules;
     - no colonisation schema;
     - a ~25 msg/s firehose that's easy to subscribe to.
   - **CAPI**:
     - client registration;
     - ~25-day tokens;
     - about 1 request/min.
   - **EDSM**: had a 33 h outage in Mar 2026.
   - **Spansh**:
     - undocumented API;
     - 10k-result cap;
     - ~110 GiB galaxy dump.
   - **Inara**: blocks scraping.
   - **Elite BGS**: ticks stale since Jul 2026.
7. **Licensing matters early.**
   - Safe to port: EliteDangerousCore (EDD, Apache-2.0), post-Apr-2026 EDOMH (MIT), Observatory (MIT), the `ed-journals` crates (MIT).
   - Must be clean-room: GPL tools (EDMC, BioScan, SrvSurvey, Canonn).
   - Off-limits: **EDSY data** (CC BY-NC).
8. **Recommended MVP** = the **40 P0 features** in §4. Together they replace EDMC (uploads), core EDD/EDOMH (state, history, materials, engineering), the exploration/exobiology stack, Spansh routing, massacre stacking and colonisation depot tracking, on Windows + Linux from day one.

## How to read this document

- **§1 Landscape at a glance**: one-line facts per tool.
- **§2 Context**: game and data-service changes 2024–2026 that every module must handle.
- **§3 Tool profiles**: purpose, exhaustive features, UX ideas worth copying, data sources, tech/license/platform, pain points and requests, maintenance.
- **§4 Merged feature matrix**: 167 de-duplicated features in 20 areas, with Elite Nexus priority and a "how to do it better" note for each.
- **§5 Top 15 pain points** Elite Nexus should explicitly solve.
- **Appendices**:
  - A. Data-source and integration rules
  - B. Reusable data and licensing
  - C. Rust ecosystem
  - D. Sources

**Related docs in this folder.** For protocol and data deep-dives beyond Appendix A's summary, see:
- `journal.md`
- `eddn.md`
- `capi.md`
- `edsm.md`
- `inara.md`
- `spansh.md`
- `websites.md`
- `engineering.md`
- `exploration.md`

**Matrix column codes**

| Code | Tool |
|---|---|
| EDD | EDDiscovery |
| EDMC | ED Market Connector (core; `P <name>` = provided by that plugin) |
| OMH | Odyssey Materials Helper (EDOMH) |
| OBS | Observatory Core (`P <name>` = via plugin, e.g. BioInsights, Evaluator) |
| EXB | Exploration Buddy (EDEB) |
| ECP | EDCoPilot |
| ODT | OD Elite Tracker |
| SRV | SrvSurvey (+ Raven Colonial where noted) |
| Others | Other tools/web services with the feature |

**Cell legend**

| Mark | Meaning |
|---|---|
| `Y` | Built in |
| `P …` | Via the named plugin |
| `~` | Partial or basic |
| (blank) | Not present |

**Priority legend**

| Priority | Meaning |
|---|---|
| **P0** | Core MVP, needed to be someone's daily driver |
| **P1** | Next wave |
| **P2** | Later / niche |

---


## 1. Landscape at a glance (Oct 2026)

| Tool | Role | Tech | License | Platforms | Latest release | Activity / team | Popularity signal |
|---|---|---|---|---|---|---|---|
| **EDDiscovery** | All-in-one captain's log, ~60 panels, 3D map, routing, voice/actions, web server | C# .NET Fx 4.8 WinForms, OpenGL, SQLite | Apache-2.0 | Win (Linux via Mono, reduced) | 19.1.11 (2026-09-12) | Active; 1 lead + 2–3 | 899 stars; #2 EDDN uploader |
| **ED Market Connector** | Uploader (EDDN/EDSM/Inara/EDAstro), CAPI, plugin host | Python 3.13 + Tk | GPL-2.0+ | Win, Linux (Flatpak) | 6.1.2 (2026-01-29); 6.1.3-b3 (05-30) | Active; small team | 1,275 stars; #1 EDDN uploader |
| ↳ **BGS-Tally** | BGS/PP/colonisation/FC squadron tracker + Discord | EDMC plugin | MIT | as EDMC | 5.6.1 (2026-09-12) | Very active | 62 stars |
| ↳ **BioScan / Pioneer / ExploData** | Exobio prediction, exploration value, shared DB | EDMC plugins | GPL-2.0+ | as EDMC | 2.12.2 / 2.4.2 (Jan 2026) | Active | 48 stars / 29 stars |
| ↳ **EDMC-Canonn** | Citizen science + exploration HUD | EDMC plugin | GPL-3.0 | as EDMC | 7.8.0 (Sep 2026) | Active | 107 stars |
| ↳ **NeutronDancer** (SpanshRouter successor) | All Spansh routes in EDMC | EDMC plugin | MIT | as EDMC | 2.1.2 (2026-10-06) | Very active | 28 stars (SpanshRouter archived) |
| ↳ **EDMCModernOverlay** | Cross-platform overlay host | EDMC plugin + PyQt6 | GPL-3.0 | Win, Linux | 0.9.2 (2026-09-09) | Active | 57 stars |
| ↳ **EDMC-Massacres** | Massacre stacking | EDMC plugin | — | as EDMC | 2.0.5 (2023) | Dormant (forks) | 33 stars |
| **Odyssey Materials Helper** | Materials, engineering, ship editor, colonisation, CGs, AR overlay | Java 25 + JavaFX, SQLite | MIT source (since 2026-04-24); proprietary binary parts | Win, Linux, macOS | 3.15.6 (2026-09-16) | Extremely active; 1 dev | 399 stars; #3 EDDN uploader |
| **Observatory Core** | Plugin host: exploration notifications, criteria, voice | C# .NET 8 WinForms | MIT | Win (Wine builds) | 1.5.2 (2026-10-01) | Very active | 134 stars |
| ↳ **BioInsights / Evaluator** (MattG) | Exobio species/variant prediction; mapping value + order | Observatory plugins | Closed freeware | Win | 1.3.444 (2026-09-22) | Active | "Observatory + BioInsights" widely recommended |
| **Exploration Buddy (EDEB)** | Second-screen exploration companion + transparent HUD | .NET 8 | Closed freeware | Win (Proton w/ workarounds) | 2.7.9 (2026) | Active; 1 dev | Frequently recommended |
| **EDCoPilot** | Voice/"ship AI" + 40+ panel second screen/VR + automation | Closed (Windows) | Proprietary freeware (Patreon perks) | Win | 1.11.762 (2026-10-11) | Very active; 1 dev | Most-mentioned companion on Reddit (2025) |
| **OD Elite Tracker** | Massacre/trade/BGS/PP/mining/FC/colonisation tracker + overlays | C# WPF .NET 9, EF Core/SQLite | None (all rights reserved) | Win (Avalonia Linux beta) | 1.5.32 (2026-10-04) | Very active; 1 dev | 43 stars |
| **SrvSurvey** (+ **Raven Colonial**) | Contextual overlays (exobio, FSS, Guardian, settlements), colonisation hub | C# .NET 9 WinForms; React web | GPL-3.0 | Win (Proton); web | 2.0.95.48 (2026-09-05) | Extremely active | 174 stars |
| **EDEngineer** | Blueprint/material tracker + local REST | C# WPF + F# | MIT | Win | 1.1.6.26 (2023) | **Archived 2025-07-04** | 524 stars |
| **EDDI** | Speech responder + VoiceAttack bridge + uploads | C# .NET 8 | Apache-2.0 | Win | 5.0.5 (2026-08-03) | Steady | 526 stars |
| **COVAS:NEXT** | LLM copilot (STT/LLM/TTS, actions, VR, web UI) | Python + Electron | MIT | Win, Linux | 4.3.0 (2026-09-17) | Very active | 116 stars; the 2025–26 trend |
| **EDSY** | Most accurate ship builder + retrofit planner | Plain JS web | CC BY-NC 4.0 | Web | 4.24.1.2 (2026-09-23) | Very active; 1 dev | De-facto standard |
| **Coriolis** (+ CMDR-Coriolis) | Combat-analysis ship builder; account/journal API | React 19 web | MIT (code) | Web | 4.1.12 (2026-09-26) | Revived 2024–26 | 284 stars |

---


## 2. Context: what changed in the game and the data ecosystem, 2024–2026

Tools live or die by how fast they absorb game updates. These are the updates every Elite Nexus module must handle on day one.

| Date | Update | What it means for tools |
|---|---|---|
| 2024 | Python Mk II, Type-8 (ARX early access, credits later) | Start of the "pay early" ship pattern: **new ship and module IDs show up in journals before data tables have them** |
| 31 Oct 2024 | **Ascendancy, Powerplay 2.0**, Mandalay | Powerplay 1 tools obsolete. Merits and rank tracking needed. Powerplay state now in FSDJump/Location. Weekly cycle ends Thu 07:00 UTC |
| 12 Dec 2024 | Cobra Mk V | New hull |
| 26 Feb 2025 | **Trailblazers: system colonisation** (beta) | New Colonisation* events. A wave of new tools (Raven Colonial, BGS-Tally colonisation, Architect Tracker…) |
| 19 Mar 2025 | Trailblazers Update 2 | Powerplay journal additions; CarrierLocation |
| 8 Apr 2025 | Gutamaya Corsair | Ship builders lacked it at launch (Reddit, 9 Apr 2025); EDSY and Coriolis added it on 10 Apr |
| 27 Apr 2025 | Colonisation "Update 3": strong/weak economy links | Planners had to rework their economy models |
| 22 Jul 2025 | Panther Clipper Mk II | New hull |
| 19 Aug 2025 | **Vanguards: squadron overhaul** (squadron carriers, Treasury, Bank) | Squadron events; squadron carriers in carrier tools; CATS and ODET journal fixes |
| ~Sep 2025 | Type-11 Prospector (ARX early access; EDSY added it 26 Sep 2025) | Mining hull; tools added it in their next point releases |
| Oct 2025 | Colonisation demolition/cancellation (4.2.1.0); anti-sniping (4.2.1.2) | Planners must model refunds and negative points |
| 10–11 Nov 2025 | **"Dodec" update**: colonisation out of beta, Dodec T3 station | New station types; score bonuses and penalties |
| 3 Dec 2025 | Caspian Explorer | **Supercruise Overcharge** (more jump range when scooping at neutron/WD stars). Routers must model it |
| Jan 2026 | Type-11 Prospector purchasable for credits | Mining hull |
| 24 Feb 2026 | Kestrel Mk II (4.3.1.0) | New hull; many journal fields became optional around this time |
| 28 Apr 2026 | **Lynx Highliner** (225 passengers) + passenger missions (4.3.3.0); **Journal Manual v38 published** | Passenger mission tracking. First official journal docs update since v37 (May 2023) |
| 30 Jun 2026 | **Operations (4.4.0.0)**: 4-player instanced squad operations, Mercenary and Powerplay modes, **Merc Coins**, Nomad ship-launched vessel, megaship interiors | **New `GameModeChange` event** (MainGame / Operation / ProvingGrounds). Tools must keep Operations data out of real history. EDD launched with "no Frontier info on Operations entries". ODET and SrvSurvey broke |
| 2 Sep 2026 | **Rhino SRV surface mining** (4.4.1.x) | `LaunchVessel` event; surface deposits and rigs; new commodities. New tools (EDSMT, Rhino Surface Mapper) appeared within weeks |
| Rest of 2026 | Roadmap: one vessel in autumn, one in winter, plus an unnamed "core feature reboot" | Expect more new events and IDs. **Hot-updatable data packs** are essential |

**Data ecosystem facts that shape the architecture.** Details are in Appendix A.
- **EDDN**
  - Healthy: 18 live schemas.
  - **No colonisation schema.**
  - The relay restarts every Thursday at 07:06 UTC.
  - Since Mar 2026 the relay may no longer de-duplicate, so listeners must.
  - EDMC is by far the largest uploader, then EDDiscovery, then Odyssey Materials Helper.
- **CAPI**
  - Undocumented and tolerated.
  - Client ID registration takes time.
  - About 25-day refresh-token life; at most ~1 request/min; 15 min for `/fleetcarrier`.
  - Common failures: Steam/Epic linking and `customer_id` mismatch.
- **EDSM**
  - Alive but slower, now behind Cloudflare.
  - **33 h outage in Mar 2026.**
  - Bodies dump covers only 7 days.
- **Spansh**
  - Indispensable: every route type, body/system searches, dumps.
  - Undocumented API; 10k-result cap; occasional slowness.
  - Full galaxy dump is about 110 GiB gzipped.
- **Inara**: blocks automated access. API uploads only through registered apps with per-user keys, and only Live-galaxy data.
- **Ardent Insight**
  - Open trade and station API with no rate limits.
  - Weekly SQLite dumps.
  - About 36M orders.
- **Elite BGS**: in maintenance mode. Its tick feed has been stale since Jul 2026. **Per-system tick inference** (edservertick/EDCAS) is the new state of the art.
- **Official journal docs lag gameplay by months to years** (v37 in May 2023, then v38 in Apr 2026). The de-facto references are community schema projects (jixxed/ed-journal-schemas, @kayahr/ed-journal, the Rust `ed-journals` crate) and a regression corpus of real journals.

## 3. Tool profiles

### 3.1 EDDiscovery (EDD)

| | |
|---|---|
| Repo / wiki | https://github.com/EDDiscovery/EDDiscovery · wiki (108 pages, reorganised in 19.1). Org has 17 repos: EliteDangerousCore, ActionLanguage, ExtendedControls, BaseUtilities, CAPI submodules; EDDLite; EDDCanonn; EDDiscoveryData (packs) |
| Maintainers | Effectively one lead (Robby, robbyxp1) plus Eahlstan, klightspeed and Bravada. "Only two people have repository access" (#3871, Aug 2026). High bus factor |
| Tech | C# / **.NET Framework 4.8 WinForms** (in-house ExtendedControls). OpenTK/OpenGL 3D map (needs OpenGL 4.6 + bindless textures). SQLite (`EDDUser.sqlite`, `EDDSystem.sqlite`). WebView2 panels. NetMQ (Python plugins). System.Speech (SAPI TTS and Windows speech recognition). In-house HTTP/WebSocket server |
| License | Apache-2.0 (all repos) |
| Platforms | Windows 10/11. Linux "with reservations" via Mono, built from source, with no speech, web panels or 3D maps. Wine/Proton needs CAPI workarounds (#3746). macOS legacy only |
| Latest | **19.1.11 (2026-09-12)**, the 300th release. About 23 releases in 2024, about 19 in 2025, about 9 so far in 2026. Master has v20-dev features (Docking, Travel, Bindings editor panels). Installer ~39 MB, unsigned |
| Popularity | 899 stars. **Second-largest EDDN uploader** after EDMC |
| Status | **Active but strained.** Fast point releases for game updates; aggressive issue triage (3 open of ~2,071) |

**Purpose.** A "captain's log" that ingests every journal entry into SQLite and can rebuild the full game state at any point in history. About 58 panel types (61 on master) cover the data, plus 3D/2D maps, routing (its own and Spansh's), overlays, a scripting and voice system, a tablet web server, and uploads (EDSM, EDDN, Inara, EDAstro; Canonn via a plugin). It is **the most feature-complete all-in-one in the ecosystem, and the heaviest.**

**Complete panel inventory (19.1.11)**

`[OV]` marks panels documented for use as a transparent in-game overlay.

- **History group**
  - **Log**.
  - **History grid**: master event grid.
    - Filters: time, text with `system:`/`body:`/`station:` wildcards, event types, field-condition filters, filter groups.
    - Marks, trip start/stop markers, colouring and outlining.
    - CSV/Excel export.
    - Right-click: notes, bookmarks, 3D map, EDSM, raw JSON, move entries between commanders.
  - **Journal**: expanded grid.
  - **Visited Stars**: notable-body flags and a jumponium indicator.
- **Current state** (every panel follows the history cursor, so you can view state at any past moment)
  - **Materials `[OV]`**, **Commodities `[OV]`**, **Micro Resources `[OV]`**, **Resources `[OV]`**. Each has a "Wanted" column that builds an overlay shopping list, with EDSY-retrofit import.
  - **Ledger** with a credit chart.
  - **Missions**.
  - **Factions**: per-faction interactions (missions, trades, bounties, bonds, vouchers, fines, data links).
  - **Ships/Loadout**:
    - Engineering-aware stats said to match EDSY, with power toggles and a module map.
    - SLEF import/export, EDSY/Coriolis export, saved loadouts, FSD range including SCO.
  - **Statistics**: General; Travel (today, week, month, since dock, since trip, all-time); Scan; In-game; Ships; Ranks, Ledger and Combat charts; Thargoid, organics, on-foot and squadron data.
  - **Suits & Weapons**.
  - **Carrier**: journal plus CAPI (status, services, goods, finances, itinerary, jump timing).
  - **Squadron/Carrier** (19.1, via CAPI): members and items.
  - **Colonisation** (19.0): claimed and visited systems, per-station required/provided/your deposits/remaining, hide completed.
  - **Powerplay Merits** (19.1): by cycle, session or system.
- **Station data**
  - **Market Data**: compare station A with B, profit per ton both ways, auto-swap for loops.
  - **Ship Yards** and **Outfitting**: "where was this ship or module last seen, at what price and distance".
  - **Spansh Stations**: filters by type, commodities, services, economy, ships, modules and Ls; up to 3 commodity price columns; cross-system search.
- **Engineering**
  - **Synthesis** and **Engineering** planners with **drag-to-prioritise, top-down material allocation**.
  - **Shopping List `[OV]`**: when landed, shows material % on the current body; in flight, lists bodies in the system with what you need.
  - **Material Trader** simulator (queued trades).
  - **Engineers**: access state, unlock requirements, recipes.
- **Scans & stars**
  - **Scan**: graphical system diagram.
    - Landable outlines, rings, barycentres, habitable-zone band, atmosphere halos, settlements.
    - Icons for terraformable, volcanism, high value, signals, mapped by you or others, discovered by others, bio/geo, organics.
    - EDSM/Spansh fill-in.
  - **Scan Grid**: system value and an FSD-injection progress bar.
  - **EDSM / Spansh / Inara** embedded web panels (Windows).
  - **Nearest Stars**.
  - **Estimated Values**: current, base, mapped, first-mapped, first-discovered+mapped; efficient vs basic.
  - **Search**:
    - Stars: fuzzy name; proximity via the local DB, EDSM or Spansh.
    - Materials/Commodities: where in your history you found items.
    - **Scans**: an expression query language.
  - **Trilateration** (re-added 19.1.5).
  - **2D Map**, **3D Map**, **Local 3D Map**.
  - **Organic Scans `[OV]`**: current sampling progress plus history with estimated value.
  - **Discoveries**: 60+ built-in saved searches run over your scan history.
- **Bookmarks & logs**
  - **Bookmarks**: system, region and planet surface points; send to Compass.
  - **Captain's Log**: calendar diary.
  - **Notes**.
- **Combat**: "campaigns" over a period with an optional target faction; HUD mode `[OV]`.
- **Routes**
  - **Route Finder**: EDD's own direct-line router over the local star DB (cargo-aware range, avoid permit systems), plus **all Spansh routers**: neutron, R2R, ammonia, Earth-like, Exomastery, trade, fleet carrier (squadron carrier aware), galaxy plotter with supercharge/injection/refuel from ship data.
  - **Expedition**: waypoints imported from CSV, NavRoute, Spansh and others; shown on the 3D map.
- **Overlay panels**
  - **System Information `[OV]`**: Ctrl-drag layout builder, click any field to copy, fuel and range warnings.
  - **Summary `[OV]`**: compact live event list.
  - **Surveyor / Route Tracker `[OV]`**: exploration HUD with values, mapped/organics markers and saved-search hits (text or voice per search); tracks a NavRoute or expedition; auto-hides when game panels open.
  - **Notes `[OV]`**.
  - **Compass `[OV]`**: bearing, glide slope, drift arrows.
  - **Mission Overlay `[OV]`**.
  - **Mining Overlay `[OV]`**: per-material refined/collected, avg/min/max %, motherlodes, high/medium/low counts, a **cumulative-frequency chart**, CSV export.
  - v20-dev only: **Docking `[OV]`** (flashing assigned pad + orientation guide) and **Travel `[OV]`**.
- **Containers & settings**
  - **Grid** (free-form, nestable) and **Splitter** containers.
  - **Screen Shot**.
  - **Settings**.
  - v20-dev only: **Bindings editor** (conflicts per UI mode, unplugged-device warnings, multiple bind files with `.bak`, friendly names exported to Elite's DeviceButtonMaps).

**Platform features**
- **Overlay pop-out modes**: T (transparent until hovered), Tc (transparent, controls live), Tf (transparent and inert until a hotkey). Caption, taskbar and top-most toggles.
- **Profiles**: named layouts that **auto-switch on triggers** (docked/undocked, a specific ship, custom conditions).
- **Action language**: event-driven scripts in "packs".
  - Triggers: every journal event, Status UI events, timers, keys, joystick input, voice input, menu items.
  - Commands: Say, Play, Key, Dialog, bookmarks, captain's log and more.
  - There is a GUI editor.
  - Packs: Voice Output (EN/DE/ES), Free Voice Control (Windows speech recognition → key presses), Key Pack (hotkeys), JKM (speaks the function of each bound button), SoundBlaster, **Chat console** (`/ln`, `/nn`, `/ts` commands typed in in-game chat), **OBS pack** (one-line text files).
- **Extensibility**: C#/C++ DLL plugin API with callbacks into history, loadout, carrier, scans, Spansh dumps and custom panels. **Python plugins** (19.0+) run as subprocesses over ZeroMQ.
- **Web server** (port 6502):
  - HTTP + WebSocket push (journal, status, scan image, missions, texts).
  - A Roccat-style **tablet key grid**.
- **Settings & recovery**
  - Theme editor with gradient themes; community translations.
  - **Safe Mode** (hold Shift): reset layout, theme or language; remove plugins; rebuild or move DBs.
  - Admin menu: rescan, rebuild indexes, fix coordinates, de-duplicate jumps.
- **Star DB**: Spansh dump (default since 18.1) or EDSM, with delta updates and a "galaxy select" preset (All, Bubble, Bubble+Colonia, …). Sizes are undocumented; anecdotally ~7 GB.
- **EDDLite**: minimal uploader (<50 MB RAM, no star DB). Stalled at 3.3.0 (May 2025).

**UX ideas worth copying**
1. **History cursor as a time machine.** Every state panel shows the state at the selected past event.
2. **Overlay behaviour**:
   - Per-window overlay modes (T/Tc/Tf).
   - Panels that collapse to their "essential sub-view" when transparent.
3. **Wanted-list workflow**:
   - Wanted counts in any inventory grid become an overlay shopping list.
   - The shopping list also shows **"% on this body / bodies in this system that have it"**.
4. **Priority-ordered material allocation** in engineering and synthesis planning.
5. **Expression-based saved searches** that drive both a background "Discoveries" list and live callouts, with a voice/text toggle per search.
6. **Profiles that auto-switch** on game state or ship.
7. **Navigation & docking aids**:
   - Docking pad overlay.
   - Compass with glide slope.
   - System Information as a click-to-copy layout builder.
8. **Station history**:
   - Market A/B compare with auto-swap.
   - "Last seen at" for ships and modules, from your own docking history.
9. **Mining analytics**: a CFD chart ("chance of >X% content") from your own prospecting history.
10. **No-alt-tab inputs**:
    - Chat-console commands.
    - JKM (speak what a button does).
11. **Recovery**: Safe Mode for self-service recovery.
12. **Negative lesson.** Game data is hard-coded, so every new module, actor or station type produces "unknown" spam until a release ships. The most-commented issues since 2023 are these report threads: #3544 (115 comments), #3635 (101), #3522, #3753, #3849.

**Data sources & integrations**
- **Reads**:
  - Journals (polled every 100 ms; full scan on first run).
  - Status.json (56 UI event types).
  - NavRoute, Market, Outfitting, Shipyard, ModulesInfo, Cargo, ShipLocker, Backpack, FCMaterials.
  - `.binds`, DeviceMappings, screenshots folder.
- **Writes**:
  - **EDDN** (22 event types; CAPI market tagged Live/Legacy).
  - **EDSM** (journal sync; flight-log backfill; notes).
  - **Inara** (full profile sync, historic fleet).
  - **EDAstro** (asks the server which events it accepts, then batches).
  - Canonn via the EDDCanonn DLL.
- **Frontier CAPI**: market, shipyard and outfitting after docking; fleet and squadron carriers; console commanders' journals.
- **Reads (web)**:
  - Spansh: star DB, bodies (7-day cache), stations, routers.
  - EDSM: bodies, GMOs.
  - EDAstro: GEC objects and map images.
  - GitHub: packs and release checks.

**Pain points & requests**
- **Scale**
  - All history is loaded into memory at start-up.
  - #3629 (Jan 2025): ~1.75 M entries, minutes per load stage, crashes, "Application Hang".
  - The wiki's official advice is to load fewer tabs or less history.
  - Older reports: #2716 (5 min UI freeze), #2654 (15 min of HDD thrash).
- **3D map instability** (OpenGL 4.6 / bindless textures / Intel iGPU and Arc / APU selection / sleep)
  - Pinned issue #3271 (open since 2022).
  - Crash reports in 2024–25: #3789, #3646, #3511, #3498.
- **Star DB and SQLite fragility**
  - Download timeouts (#3744: "EDSM not a good option in 2025").
  - "Database is locked" (#3464); corruption on exit (#3766, #3799).
  - Duplicate jumps via junction paths (#3833).
  - User settings lost after update (#3870).
- **Hard-coded game data**: the "unknown module" treadmill described above. Game updates break features: Caspian jump range (#3857); 19.1.7 withdrawn.
- **Platform**
  - Linux is build-from-source with missing features.
  - Unsigned binaries are blocked by Smart App Control (#3871). Maintainers decline to code-sign.
- **Complexity**
  - ~60 panels, plus splitters, grids, profiles and packs.
  - Terse or stale wiki pages.
  - Users get lost in layouts (#3085).
  - Requests: JSON viewer (#3801); per-voice settings (#3486); subfolder journals not monitored (#3740).
  - **The project ships EDDLite as its own "too heavy" escape hatch.**
- **Missing**: no tick-aware BGS reporting; no exobiology prediction or colony-range helper; no massacre stacking; no carrier finance projection.

### 3.2 ED Market Connector (EDMC), core app

| | |
|---|---|
| Repo | https://github.com/EDCD/EDMarketConnector (EDCD). Original author Jonathan Harris ("Marginal"). Current lead maintainer: Rixxan |
| Tech | Python 3.13 + Tkinter/ttk, frozen exe. SQLite (EDDN queue), TOML config (6.0+), WinSparkle updater |
| License | GPL-2.0-or-later |
| Platforms | Windows 10/11 x64 (primary; x86 still built). Linux via Flatpak (Flathub) or from source. macOS dropped in 5.11 (WINE or source only) |
| Latest | 6.1.2 stable (2026-01-29). 6.1.3-beta3 (2026-05-30). 6.2.0 in alpha. 6.0.0 major (2025-12-13) |
| Popularity | ~1,275 stars, 180 forks, ~109 open issues; last push 2026-10-01 |
| Status | **Active but a small team.** The de-facto uploader and plugin host |

**Purpose.** A small always-on-top companion window. It reads the Journal and companion files, optionally queries Frontier's Companion API (CAPI), and forwards data to EDDN, EDSM, Inara and EDAstro. It also writes trade and loadout files and links out to web tools. Most of its user-facing value now comes from its Python plugin host.

**Feature list**
- **Commander/Dashboard**
  - Main window with one row per fact: Cmdr (shows private group / multicrew context), Ship (click opens the loadout in EDSY or Coriolis), Suit (Odyssey), System, Station.
  - System and Station are hyperlinks to a user-chosen provider (EDSM, Inara or Spansh). An EDSM status icon shows whether the system is new to EDSM.
  - Update button for a manual CAPI fetch, with a live "cooldown Ns" countdown. A status line shows the last update and errors (plugins can post here too).
  - File > Status window (from CAPI /profile): balance, loan, nine ranks, and a ship list with location and value. Click any value to copy it.
  - Context menus; Ctrl+C copies the system and station names.
- **History & Stats**: none. EDMC keeps no history database; plugins fill the gap.
- **Trade & Markets**
  - Auto-fetches CAPI market, shipyard and outfitting on dock.
  - Market CSV export in several formats (CSV, TAB, PIPE, SEMICOLON); Trade Dangerous `.prices` file; Save Raw Data (last CAPI JSON).
  - `EDMC.exe` CLI: market, outfitting, shipyard, status, ship locations and loadout exports, send-to-EDDN, documented exit codes.
- **Ship builds**
  - Shipyard provider links (EDSY default, or Coriolis with Normal/Beta URLs); an "alternate URL method" writes a local HTML file to get around URL-length limits.
  - Loadout file written after every outfitting change; Inara SLEF export (5.12).
  - ships.json / modules.json data auto-update at runtime (6.1). New 2024–26 hulls are added as they ship (Kestrel Mk II and Lynx in 6.1.3-beta).
- **Fleet Carrier**: CAPI `/fleetcarrier` queries, triggered by carrier events, with a ~15-minute cooldown. EDDN `fcmaterials_capi` and `fcmaterials_journal`. A `capi_fleetcarrier` plugin hook.
- **Website sync**
  - **EDDN:**
    - commodity/3, outfitting/2, shipyard/2
    - journal/1 (FSDJump, Location, Docked, Scan, SAASignalsFound, CarrierJump)
    - fssdiscoveryscan, navbeaconscan, codexentry, scanbarycentre, navroute, dockingdenied, dockinggranted, approachsettlement, fssallbodiesfound, fssbodysignals, fsssignaldiscovered, fcmaterials
    - Handling: durable SQLite queue with gzip and retries; "delay sending until docked"; privacy stripping (`_Localised` keys, personal faction fields); BodyID cross-check against Status.json; Live/Legacy tagging; beta builds go to `/test` schemas. EDDN outfitting v3 is planned for 6.2.0.
  - **EDSM:** batched journal upload, filtered by EDSM's discard list; per-CMDR API key; respects rate-limit headers.
  - **Inara:** about 40 event types (ranks incl. Powerplay, reputation, ships and loadouts, storage, cargo and materials, ShipLocker and suit loadouts, travel, missions, combat, community goals, friends), batched every ~35 s per credential set.
  - **EDAstro** (6.0+): CarrierStats, CarrierJumpRequest and ScanOrganic.
- **Overlay/second screen**: Default, Dark and Transparent themes (transparent is Windows only); HUD-matching text colours; transparency slider; always on top; UI scale 0–400%; tray icon. **No in-game overlay in core.**
- **Bindings**: one global hotkey (Windows only) to trigger a CAPI update. Plugin hotkeys need the third-party EDMCHotkeys plugin.
- **Misc/Platform**
  - CAPI OAuth with PKCE via an `edmc://` protocol handler or a localhost callback; per-CMDR tokens; Frontier forces re-auth about every 25 days; separate Live and Legacy hooks.
  - **Remote killswitches** (`killswitches_v2.json`): disable EDDN events, plugins, CAPI endpoints or exports per EDMC version, or redact/modify fields, without shipping a release.
  - Diagnostics: rotating debug log, System Profiler, `--reset-ui`, many `--trace` and debug flags.
  - `config.toml` with backup and auto-recovery (6.1.1).
  - Crowdin translations.
  - Privacy toggles (hide private group name and multicrew captain), useful for streamers.
- **Plugin API** (Python)
  - Hooks: `journal_entry`, `dashboard_entry` (Status.json), `cmdr_data` / `cmdr_data_legacy` (CAPI), `capi_fleetcarrier`, plus `plugin_app` and `plugin_prefs` for UI and the EDSM/Inara notify hooks.
  - A shared `state` dict: commander, ship, cargo, materials, ShipLocker, NavRoute, suits, and so on.
  - Everything runs on the single Tk main thread.
  - Plugin management: enable/disable (6.0). A read-only **Plugin Browser** (6.1) lists the EDCD registry (26 plugins) with a "last tested EDMC version" badge, a VirusTotal link and "Report plugin". **No install, update or dependency handling.**

**UX ideas worth copying**
- One-row-per-fact window where every name is a hyperlink to the user's preferred site.
- A visible API cooldown on the Update button.
- "Delay uploads until docked."
- A durable upload queue.
- Remote killswitches with field redaction.
- Tested-version and VirusTotal badges for extensions.
- Streamer privacy toggles.
- HUD-coloured text.
- Support tooling in the Help menu (log folder, profiler, reset UI).

**Data sources & integrations**
- Reads: Journal, Status.json, Cargo.json, NavRoute.json, ModulesInfo.json, Backpack.json, ShipLocker.json, Market.json, Outfitting.json, Shipyard.json, FCMaterials.json; CAPI `/profile`, `/market`, `/shipyard`, `/fleetcarrier`.
- Writes: EDDN, EDSM, Inara, EDAstro.
- Links only: EDSY, Coriolis, Spansh.

**Pain points & requests**
- **Authentication and Frontier-side failures dominate history.** #432 "Credentials don't seem to be saved", #242 "Invalid Credentials" (64 comments), "Frontier server is down" issues.
- **Plugin fragility.** Every major EDMC version marks some plugins "broken" (Pioneer #5, BioScan #28, PowerPlayProgress #61, InaraProgress, Dec 2025). Plugins share one Tk thread, so a slow plugin stalls the journal monitor ("Awaiting CMDR Login"; #2590).
- **Plugin management.** Manual zip installs (#513 "Plugin management & update API"). No way to re-order plugin panels (#1792): the GalaxyGPS README tells users to rename the plugin folder `!GalaxyGPS` to sort it first.
- **Requests:**
  - #889 click-through ("inert") window
  - #2701 multiple CMDR API logins (Aug 2026)
  - #2172 fit to screen
  - #1422 accessible alert log
  - #2116 TTK overhaul
- **2026 reliability.**
  - Crash reports #2705, #2707, #2717, plus TclErrors.
  - Flatpak install broken (#2678, Jun 2026).
  - Windows/Linux journal event discrepancies (#2709).
  - An SLV rename sets the current ship to Nomad (#2689).
  - Smart App Control false positive (#2663).
  - Dark-mode windows could not be resized for 5+ years (fixed 6.1.0).

**Maintenance.**
- Active, but a small team.
- Triage happens in 2026, with a steady patch cadence.
- 6.2.0 brings the EDDN v3 outfitting schema and a WinSparkle refactor.

### 3.3 The EDMC plugin ecosystem (the features most users actually run)

**Popularity snapshot.** GitHub stars from the `edmc` / `edmc-plugin` topic pages, Oct 2026. This is a weak signal in a niche community.

| Plugin | Stars | Status (Oct 2026) | Area |
|---|---|---|---|
| EDMC-Canonn | 107 | active (7.8.0, Sep 2026) | exploration/science |
| ED Recon (EDR) | 102 | active (3.4.2, Jun 2026) | intel/combat/assistant |
| EDMCOverlay (legacy) | 85 | stale (last commit 2024-02) | overlay |
| EDMC-Discord-Presence | 64 | stale (last commit 2024-11) | social |
| BGS-Tally | 62 | very active (5.6.1, 2026-09-12) | BGS/PP/colonisation/FC |
| EDMCModernOverlay | 57 | active (0.9.2, 2026-09-09) | overlay |
| LandingPad | 51 | active (2.5.7) | docking |
| BioScan | 48 | active (2.12.2, Jan 2026; commits to Sep 2026) | exobiology |
| EDMC_SpanshRouter | 48 | **archived 2025-03-18** | routing |
| EDMC-Screenshot | 45 | maintained | screenshots |
| ArtemisScannerTracker | 40 | maintained (Jul 2026) | exobiology |
| EDSM-RSE | 35 | stale | exploration |
| HITS | 34 | dead (2020) | danger warnings |
| EDMC-Massacres | 33 | dormant (2023); forks active | missions |
| HabZone | 32 | dead (2019) | exploration |
| Pioneer | 29 | active (2.4.2) | exploration value |
| NeutronDancer | 28 | very active (2.1.2, 2026-10-06) | routing |
| Mining-Analytics | 27 | active (1.2.0) | mining |
| PowerPlayProgress | 18 | active (0.9.15) | Powerplay |

The official EDCD Plugin Registry lists 26 plugins:
- **Exploration & navigation:** BioScan, Pioneer, ExploData, ExplorerLite, NavRoute, NeutronDancer, SpanshTools, GalaxyGPS, EDPlanetNavigator, Route Scoop
- **Activity tracking:** BGS-Tally, Architect Tracker, Mining-Analytics, LimpetTimer, PowerPlayProgress
- **Overlay & input:** EDMCModernOverlay, EDMCHotkeys
- **Trade, cargo & docking:** UpdateTD, Cargo Manifest, LandingPad, Docking Denied Reason
- **Utilities:** LogEventMiner, Screenshot-Linux, ServerStatus, EDR, EDSpec

Canonn, the legacy overlays, Discord Presence and Massacres are **not** in the registry.

#### BGS-Tally (aussig): MIT, Python
The de-facto squadron tracker for BGS, Powerplay, colonisation, fleet carrier and (legacy) Thargoid War, with Discord-ready reports.

- **BGS** (per tick, per system and faction):
  - Mission INF +1..+5, with primary/secondary split and election/war mission counts.
  - Trade profit by supply/demand bracket; black market tracked separately.
  - Cartography and exobiology sales; bounties; combat bonds; failed missions; murders.
  - Ground CZs (with settlement names) and space CZs with side objectives.
  - Megaship scenarios; search-and-rescue hand-ins; manual adjusters.
- **Ticks:** tick detection via Zoy's tick detector, plus per-system tick times (5.0) and Force Tick. Elections are highlighted orange, wars red.
- **Favourite factions:** post only favourite factions, or only systems containing them.
- **Thargoid War (legacy):** passenger, cargo and evacuation missions; massacre kills by vessel type; SAR items.
- **Powerplay:** merits per tick and per day, with their own Discord channel.
- **Colonisation:**
  - Plans window, one tab per plan. Build tables show tier points, prerequisites, commodity cost, pad size, economy and system impact.
  - Progress panel shows required, delivered, remaining, cargo and carrier stock, in tonnes or ship-loads.
  - Colour code: grey = delivered, green = covered by cargo plus carrier, blue = sold at this market, gold = sold on your carrier.
  - Click a commodity to open market lookups.
  - **Two-way real-time Raven Colonial sync.**
- **Fleet carrier** (five tabs):
  - Summary, cargo, and locker/bartender.
  - Itinerary using the Spansh FC router, with auto-copy of the next hop.
  - Shipyard with transfer time and cost.
  - Discord posts for orders and jumps; overlay countdowns.
- **Squadron objectives API** and a CMDR interaction log (targeted, interdicted, killed by, friend requests).
- **Overlay** (via ModernOverlay): tick alerts, system summary on jump, colonisation table, objectives.
- **Discord:** multiple webhooks split per activity, edit-on-repost, and a preview with Discord colouring.
- **BGS-Tally API** (`/discovery`, `/activities`, `/events`, `/objectives`): squadron servers declare which events they want.
- **Requests:** multi-CMDR separation (#182, #275), live refresh (#227), system history display (#475), carrier API (#488), and a faster table widget (#254). The window was historically very slow with many systems (#35).

#### EDMC-Canonn ("Project Athens"): GPL-3.0
Citizen-science collector plus an exploration HUD.

- **Galaxy-map targeting:** shows whether the targeted system is known to Spansh (red/amber/green).
- **Codex icon strip** with tooltips: bio, geo, Guardian, Thargoid, high-value bodies, rings, tritium hotspots, jumponium.
- **"Tourist" badges:** close orbits, collision prediction, shepherd moons.
- **Organic distance** to the nearest previous sample.
- **Nearest-POI navigation:** patrols, Guardian/Thargoid sites, personal POI CSVs, nearest stored ship; plus a surface bearing command.
- **Chat-command "search nearest":** services, commodities, economies; results copy to the clipboard.
- **Data capture:**
  - Captures codex, NHSS, FSS signals, hyperdictions and Thargoid kills to Canonn.
  - Bookmark capture from Status.json.
- **Other:** news ticker; self-updater; automatic error reporting.
- **Pain points:** Linux load failures (#74, #211 Bazzite, Mar 2026); bio/geo signal mixing (#88); duplicate system names (#124).
- **Requests:** bio value-range predictions (#199) and a green gas giant predictor (#222).

#### BioScan + Pioneer + ExploData (Silarn): GPL-2.0-or-later
**BioScan** is the most complete exobiology species-deduction engine:
- **Predictions:**
  - Genus and species predictions with value ranges after FSS/DSS, narrowing after each sample.
  - Codex flags: new for the region, never logged.
  - Body shorthand list; gravity flags (≥1 G; ≥2.7 G too heavy on foot).
- **Sampling:**
  - Required sample distance, with a live minimum distance to previous samples.
  - Waypoints from composition scans, with distance, heading and turn direction.
  - Heading-relative **radar overlay** whose ring turns green when you are far enough.
- **Values and bonuses:**
  - First-footfall and first-logged bonus detection.
  - Populated systems are excluded from footfall bonuses.
  - Unsold value that accounts for sold and lost data.
- **Data and settings:**
  - Persistent per-CMDR database with a resumable background journal import.
  - One EDSM fetch per system.
  - Ship whitelist (show only in chosen ships); translations.

**Pioneer** handles exploration value:
- Values account for previously discovered and mapped status, the terraforming bonus, mapping efficiency, first-fully-scanned and first-fully-mapped bonuses, and the honk value.
- Each body shows current, maximum and honk value.
- Emoji status grammar, a high-value threshold list, and unsold, sold and lost data tracking.

**ExploData** is the shared SQLAlchemy database for both: systems, stars, planets, rings, flora, signals, codex, sales and deaths.

**Pain points:**
- Emoji glyphs are missing on Linux and X11 overlays (#27, 47 comments).
- Requests:
  - Hide BioScan while mining (#9, 24 comments)
  - Hide low-value signals (#34)
  - Bearing hint when close to a waypoint (#26)
  - Habitable-zone calculations (Pioneer #1)
  - Ring hotspot tracking (Pioneer #3)
- **Lock-step database versions:** all three plugins must be upgraded together.
- Breakage after EDMC updates (BioScan #14, #28; Pioneer #5).
- Database migrations failing (BioScan #4, #7, #17).

#### Massacre stacking: EDMC-Massacres (CMDR-WDX), dormant since 2023, with forks
- Per-faction table: kills required, reward (wing-shareable portion in brackets), delta to the highest stack, and a sum row.
- **Stack ratio** = total mission kills ÷ required kills.
- Reward per kill.
- Updates live on accept, abandon and complete.
- **Limitations:**
  - Reads only the last 2 weeks of journals.
  - Requires a relog if EDMC starts after the game.
- **Status:** the original repo is dormant; forks (ckx000, AlphaConqueror) keep it alive.

#### Spansh routing inside EDMC
- **EDMC_SpanshRouter** (archived 2025-03-18) was the classic plugin:
  - Plots or imports a Spansh route and copies the next waypoint to the clipboard on each jump.
  - EDMC #2318 "SpanshRouter plugin no longer works".
- **EDMC-NeutronDancer** (dwomble, MIT) is the active successor.
  - **Route types:** Galaxy Plotter with the exact loadout, Neutron, Road to Riches, Expressway to Exomastery, Trade Planner, Tourist, Fleet Carrier, Boxel Surveyor.
  - **Per-jump table:** fuel and tritium use, refuel and neutron flags.
  - **Persistence:** routes persist and "catch up" after restarts.
  - **Input/output:** autocomplete; CSV import/export.
  - **Overlays:** a galaxy-map-only "next destination" overlay and a carrier overlay.
  - **Control:** chat commands (`!nd next`), hotkeys, and a public route-event API.
  - **Pain points:**
    - Linux clipboard backends (#8, 22 comments; #23, 16).
    - Galaxy Plotter ship detection (#53, 24).
    - Supercharge flag not passed to Spansh (#51).
    - Upgrading with no recorded ship breaks the plugin and takes EDMC down with it (#62).
- **SpanshTools** (wuuthradd) adds more plotters (Ammonia, Earth-like, Rocky/HMC), a spreadsheet-style route viewer, and SLEF ship import.
- **GalaxyGPS** adds a CAPI multi-carrier manager: tritium in tank and cargo, restock warnings, cargo values, stored ships and modules.

#### Overlays: EDMCOverlay (legacy) → EDMCModernOverlay
- **Legacy EDMCOverlay:**
  - Windows helper `edmcoverlay.exe` that listens on `127.0.0.1:5010` for newline-delimited JSON: text and rectangles on a virtual 1280×960 canvas, with a TTL.
  - Many plugins standardised on it.
  - Limits: Windows only, borderless/windowed only, no VR, no click-through guarantees.
- **EDMCModernOverlay** (GPL-3.0, PyQt6; README says ~99.999% Codex-written) is the backwards-compatible cross-platform successor.
  - Windows plus Linux (X11 and Wayland, packages for many distros).
  - Per-plugin overlay groups that the user positions, anchors and themes; profiles; font scaling; title-bar compensation; an off-screen nudge.
  - Experimental standalone window for SteamVR.
  - Shape API: rect and circle.
  - **Pain points:**
    - Controller fails to launch (#29, 74 comments); overlay not showing (#28, 52).
    - Mouse capture broken (#121, 30); text "crunchy" (#169).
    - Not capturable by streaming software (#108).
    - Broken when launched via MinEDLauncher (#211).
    - Visible borders (#226, #228).
    - Wayland focus, input and multi-monitor issues.
    - VR roadmap (#105) is the top-voted issue.

#### ED Recon (EDR, LeKeno): Apache-2.0
A crowd-sourced intel and assistant plugin.
- **Commander intel:**
  - `!who` profiles with karma, scan and bounty history; automatic outlaw alerts with bounty and distance thresholds.
  - Personal tags; OPSEC on by default.
  - System sitreps, NOTAMs, `!distance`.
- **Navigation and services:**
  - Surface navigation `!nav lat lon`; route HUD; Spansh journeys.
  - Docking pad schematics.
  - Nearest-service commands: IF, material traders, brokers, staging, `!parking` for FC slots, `!ship` (where a ship is parked, with ETA).
- **Materials:** material farming search; Odyssey settlement search; `!eval` of backpack and locker.
- **Mining:** mining assistance and limpet reminders.
- **Exploration:** top planets on honk.
- **Overlay:** in-game or standalone window (VR/second monitor).
- **Alerts and sharing:** ~15 audio cues; Discord chat forwarding; 20 macro slots.

#### Smaller but popular plugins (one line each)
- **EDMC-PowerPlayProgress:** rank progress; session merits; merits per system with control/reinforcement/undermining trends; **merits by activity with an explicit "Unknown" bucket**; PP commodity tracking; templated clipboard.
  - Pain points: Frontier journal bugs (#22) and attribution errors (#58, #68).
- **Colonisation plugins:**
  - **Architect Tracker:** required/provided/needed per site, carrier cargo, preferred-market column, slim mode for VR.
  - **ConstructionTracker:** carrier stock from CAPI, then FCMaterials; cargo-transfer reconciliation; 46 unit tests.
  - **ravencolonial_edmc:** link projects to Raven Colonial; "track all"; trip estimates from cargo capacity.
- **EDMC-Mining-Analytics:**
  - Live tons/hour and refinements per minute; prospector duplicate detection; yield histograms.
  - Session archive with ring "Analysis/Compare" reports.
  - Spansh hotspot finder and sell-price search; Discord run summaries.
- **LimpetTimer:** active collector limpet lifetime.
- **Exobiology alternatives:**
  - **ArtemisScannerTracker:** scan progress, sold/unsold value, colony-distance colouring, codex window.
  - **ExplorerLite:** next-step guidance "Honk → FSS → DSS/Sample → Done", genus predictions, history browser with unsold filter.
- **Navigation:**
  - **NavRoute** (Silarn): remaining jumps, fuel-star flags, off-route detection.
  - **Route Scoop:** route pips by star type and fuel reach; tank widget.
  - **EDPlanetNavigator:** planet coordinate overlay.
- **LandingPad:** station diagram with your pad highlighted. One of the most-loved small features (EDR has one too).
- **Cargo and trade:**
  - **Cargo Manifest:** cargo by category with value and icons; prices from daily EDDN averages.
  - **UpdateTD:** writes market data to TradeDangerous.
  - **EDHauler:** scrapes Inara carrier pages; fragile.
  - Income trackers (HourlyIncome, StreamHUD for OBS).
- **Fleet carrier:**
  - **FCMS:** carrier jumps reported to fleetcarrier.space and Discord.
  - **FCDN**, FC-monitor and FCParkingFinder (parking slots via EDSM).
- **Utilities:**
  - **EDMCHotkeys:** a global hotkey broker for plugins, with X11 and Wayland backends.
  - **EDMC-Screenshot:** BMP to PNG, names from system and body, timed shots.
  - **ServerStatus:** CAPI health.
  - **LogEventMiner:** journal logger.
- **Legacy, still recommended** (mostly dead):
  - **HITS:** danger reports.
  - **HabZone:** habitable-zone ranges.
  - **Discord Presence.**
  - **EDMFAT:** simple BGS tally.
  - **EDMC-Triumvirate:** Russian community; incompatible with Canonn.
  - **InaraProgress:** rank and awards progress; broke with EDMC 6.

**What the plugin ecosystem tells Elite Nexus**
1. The real "EDMC feature set" is core uploads plus roughly 10 plugins: BGS-Tally, BioScan/Pioneer, Canonn, NeutronDancer, ModernOverlay, Massacres, Mining-Analytics, PowerPlayProgress, LandingPad, colonisation trackers.
2. These plugins each re-parse journals and keep their own SQLite/JSON stores, with lock-step database versions. Users ask for unified history and multi-CMDR separation.
3. Breakage on host upgrades is constant. Linux friction (clipboard, fonts/emoji, Wayland overlays) and overlay reliability are the top comment-heavy pain points.
4. UX patterns users love:
   - icon/emoji status grammar (with a no-emoji fallback)
   - progressive prediction narrowing
   - sampling radar
   - galaxy-map-only next-jump overlay
   - chat-command palettes
   - Discord previews
   - colour-coded colonisation sourcing

### 3.4 Odyssey Materials Helper (EDOMH)

| | |
|---|---|
| Repo / site | https://github.com/jixxed/ed-odyssey-materials-helper · https://edomh.nl. Companion sites: pinplanner.edomh.nl, schemas.edomh.nl, math.edomh.nl |
| Author | jixxed (effectively a single maintainer); community translators on Crowdin |
| Tech | Java 25 + JavaFX (JMetro, ControlsFX, ChartFX), Ebean ORM over SQLite, Jackson, Tesseract/OpenCV for OCR. Packaged as jlink images (MSI, deb, rpm, DMG, zip). Memory capped at 2 GB |
| License | **Source is MIT since 2026-04-24** (it was GPL-3.0 before that). The official binaries include proprietary modules under an EULA (Oct 2025). Self-builds must be labelled "unofficial" |
| Platforms | Windows (signed MSI, auto-update), Linux (deb/rpm), macOS Apple Silicon (unsigned DMG, May 2026) |
| Latest | **v3.15.6 (2026-09-16).** 436+ releases: about 145 in 2025, about 89 in 2026. Several releases per week; same-day hotfixes |
| Popularity | 399 stars. **The third-largest EDDN uploader** (after EDMC and EDD) |
| Status | **Extremely active.** Funded through GitHub Sponsors, with a sponsor early-access tier since Jun 2026 |

**Purpose.** A real-time materials and engineering hub. It tracks every Odyssey and Horizons material and shows what each one is needed for: engineer unlocks, suit/weapon upgrades, ship blueprints, experimental effects, synthesis. It plans the engineering itself (wishlists, engineer routes, loadouts, ship builds). Its scope has grown well beyond materials: colonisation, Powerplay, Community Goals, permits, carriers, an AR overlay and EDDN uploads.

**Feature list.** The app has three tab groups. Odyssey: materials, blueprints, engineers, loadout, wishlist, bartender. Horizons: materials, commodities, engineers, blueprints, ship builder, wishlist. Other: colonisation, Community Goals, permits, Powerplay.

- **Commander/Dashboard**
  - **Multi-commander**, with Live and Legacy kept separate. Each commander has its own wishlists, loadouts, favourites and pins. Renamed or reset commanders are hidden automatically.
  - Commander stats (credits, Arx, ranks, reputation).
  - **Status bar:**
    - system, with lat/long on touchdown;
    - carrier name and location;
    - **per-endpoint CAPI status** (fleetcarrier, squadron, commander);
    - **game-server downtime indicator**;
    - **EDDN queue**;
    - "out of date" warning.
  - **"What's New since the version you last ran."**
- **History & Stats**
  - Daily commodity statistics.
  - CG history back to 2024.
  - No journal browser (requested in #326).
- **Navigation & Routing**
  - **Engineer shortest-path planner** for Horizons and Odyssey wishlists. It accounts for pinned blueprints, the remote workshop, engineer-unlock visits and blueprint priorities.
  - **Nearest material trader and tech broker**, aware of trader/broker type, Ls distance, engineer unlocks and **permits**.
  - **"Plot on Spansh"** sends the ship to Spansh as SLEF.
  - **Nearby selling stations** for commodities and CGs, filtered by pad size and cargo capacity.
  - Permit-required flags.
- **Trade & Markets**
  - Horizons commodities tab:
    - search;
    - rare-goods indicator;
    - refinable conversion info;
    - outline when the current market buys or sells the item;
    - stock brackets;
    - carrier and squadron-carrier stock.
  - **Community Goals tab:**
    - current and historical CGs, crowd-sourced through EDOMH users;
    - personal progress;
    - CG commodities with seller lookup;
    - **tier/band chart with optimistic and pessimistic end predictions**.
  - Retired: a player-to-player material trading marketplace (2021–2024).
- **Mining**: mining info and POIs for ring mineables; mining modules in the ship editor.
- **Powerplay**: tab showing unlock and rank-decal progress, every power's perks, live merits and rank; PP goods and data styling with barter prices; AR highlight; EDDN Powerplay data.
- **Colonisation**
  - Projects tracked automatically from depot events, with build-type and layout selectors.
  - Candidate build types pre-filtered by material requirements; known Trailblazer locations.
  - Choose which carrier or squadron-carrier stock counts toward a project.
  - Hide, search and sort projects; exclude projects from the overview.
  - Issues: tracking resets after a CAPI refresh (#643, #695).
- **Fleet Carrier**
  - Stock and orders via CAPI (polled hourly) plus live market/trade-order events; fuel; **debt state**; pending decommission.
  - Stale-data warning when CAPI is down.
  - **Yellow "you have enough, but some is on the carrier" state** in wishlists.
  - **Squadron carrier**: bank, orders, and **squadron perks read from CAPI that adjust blueprint costs**.
- **Engineering & Materials (core)**
  - **Odyssey materials:**
    - cards with backpack, locker, carrier and squadron-carrier counts;
    - relevance colouring (engineer unlock / blueprint / irrelevant, with manual override);
    - filters: irrelevant, illegal, favourites, carrier-only, **"surplus vs wishlist"**, Powerplay, collector mode;
    - tooltips: blueprint usage, barter value, **data-port spawn chances**, settlement/POI locations;
    - **pickup notifications** (relevant / irrelevant / wishlist), including wing pickups;
    - **"point" emote lookup**: point at an item in game to get a notification about it;
    - **unknown materials auto-reported** to the developer.
  - **Odyssey engineers and blueprints:** unlock prerequisites that turn green as you meet them, unlock tips, search by specialisation.
  - **Wishlists:** several per commander plus an aggregated "ALL" view; add a whole loadout; progress bars; TXT/CSV/XLSX export including carrier amounts.
  - **Bartender** simulator, with fuzzy search.
  - **Horizons materials:**
    - grades and caps; collector mode;
    - **colour-coded trade suggestions** (trade-down / cross / trade-up), limited to blueprints you can see;
    - nearest trader;
    - **spawn locations**: crashed ships, about 21k brain trees and crystal shards, sensor fragments, Guardian beacons.
  - **Horizons engineers:** unlock order, reputation tips, **pinned blueprints**, Colonia/Bubble filter, permit warnings, external pin planner with round-trip import.
  - **Horizons wishlists:**
    - grade sliders with min/current/max materials;
    - quantities and experimentals per blueprint;
    - priorities; blocks invalid combinations;
    - synthesis and tech-broker unlocks included;
    - open the build in Coriolis or the EDOMH builder.
  - **Data packs:** traders and brokers fetched from GitHub daily.
  - Coverage includes 2026 content: Merc modules (Merc Coins), Vessel Hangars, Rhino-era modules.
- **Ship builds** (Horizons Ship Editor, 2024–2026)
  - **Builds:**
    - live current ship (cargo, pips, fuel);
    - saved builds and favourite modules;
    - engineering kept across resize;
    - changed-module highlights;
    - validation (e.g. no two shield generators);
    - slot positions drawn on ship images.
  - **Stats:**
    - weapon groups and DPS (burst-aware), falloff, breach, thermals;
    - acceleration and handling charts, supercruise profile;
    - **jump range matching the in-game panel**, plus neutron/WD boosts and SCO fuel;
    - power groups with 20/40/50% lines; silent-running time; emissions; mass lock; passengers.
  - Synthesis effects; **squadron and Powerplay perks applied**.
  - **Share and import:** SLEF import, paste tab, **ship links v2 with Discord rich previews**, Plot on Spansh.
  - Colourblind styling.
  - Missing (requested): build comparison (#983), damage over range (#857), staying power (#863), refuel time (#749).
- **Odyssey suits/weapons**: loadout editor (upgrades, modifications, handling and damage stats), clone, current in-game loadout view, mark owned mods, add loadout to wishlist, share links.
- **Voice/TTS & alerts**: sound plus popup notifications (pickups, wishlist, unknown materials, Powerplay, CAPI re-auth), with custom sounds, a per-type toggle, monitor choice and a sound queue. **No TTS.**
- **Overlay/second screen ("Augmented Reality")**
  - **OCR overlays** read the data-port download list and the bartender screens and colour items by wishlist, relevance and Powerplay.
  - Persistent mode for OBS.
  - Requires in-game blur off.
  - Issues: game freeze/crash with AR on (#920); OCR crashes.
- **Website sync**
  - **EDDN upload (opt-in):** commodity, outfitting (v3 with merc prices), shipyard, journal, FSSSignalDiscovered, ApproachSettlement, CodexEntry, Docking*, FCMaterials, ScanOrganic, Powerplay. Nothing is sent outside the main game (e.g. during Operations).
  - No Inara or EDSM upload.
- **Misc/Platform**
  - `edomh://` URL scheme with a **global Ctrl+V import plus an accept/refuse preview**.
  - Crowdin translations (9 languages).
  - Dark and colourblind themes; **user CSS**; tray icon.
  - **Support-package button** with Sentry reporting; refuses to run elevated.

**UX ideas worth copying**
- **Imports and links**
  - Global paste-to-import with a preview.
  - Shareable links that unfurl richly in Discord.
- **Materials**
  - The "you have it, but it's on your carrier" state.
  - A surplus finder.
  - Trade suggestions filtered by reachability and permits.
- **Engineering**: an engineer route planner aware of pins, the remote workshop, unlock visits and priorities.
- **Status & updates**
  - Per-endpoint CAPI health and game-downtime indicators.
  - "What's new since your last version."
- **Robustness**
  - Remote data packs (traders, brokers).
  - **Automatic, opt-in reporting of unknown journal events and fields**, so the app adapts quickly when Frontier changes the journal.
- **Community Goals**: tier predictions with confidence bounds.

**Data sources & integrations**
- **Local files**: Journal (Live and Legacy), Status.json, Backpack.json, ShipLocker.json, Cargo, Market.json, FCMaterials.json; Outfitting and Shipyard (inferred).
- **CAPI** (profile, fleetcarrier, squadron; Epic login supported); client ID delivered by remote config.
- **Uploads and services**: EDDN; its own edomh.nl backend (shortlinks, CG aggregation, market lookups); AWS Lambda for unknown-event reports; Sentry; GitHub.
- **Imports**: EDSY and Coriolis wishlist links, SLEF, pin planner.
- **Exports**: links, shortlinks, TXT/CSV/XLSX, SLEF to Spansh. No generic JSON/SLEF export (#514).

**Pain points & requests**
- **Performance**
  - **#923 "uses nearly as much memory as the base game"** (Jun 2026).
  - **#939 CPU above 50%** (Jul 2026); #722 high CPU.
  - #991 TLS busy-loop on VPN/proxy.
- **Elevated-privilege refusal**: #670 (30 comments, locked "too heated"), #676.
- **Updates and URL handling**
  - Launch/update failures: #266 (16 comments), #844, broken auto-updater in v3.15.1–3.15.4.
  - `edomh://` handler fails with national characters in the path (#312, 18 comments).
  - Linux KDE scaling (#685, 16 comments).
- **Accounts and data**
  - Cannot delete commanders (#686, 16 comments).
  - Squadron carrier not shown for alt commanders (#747).
  - CAPI 406 errors (#925).
- **Open requests**
  - **#967 a local API / bot integration** (maintainer: "single player focused, no endpoints").
  - #983 build comparison; #977 edit carrier cargo; #934 merge wishlists; #946 optimal collection strategy; #440 return-to-start route; #514 JSON/SLEF export; #739/#612 compact lists; #203 choose the bartender's wishlist.

### 3.5 Observatory Core (+ BioInsights, Evaluator and the plugin ecosystem)

| | |
|---|---|
| Repo / docs | https://github.com/Xjph/ObservatoryCore · https://observatory.xjph.net |
| Author | CMDR Vithigar (GitHub Xjph). Contributors include fredjk-gh, MattG (matt-g-dev) and flusedev (Wine build) |
| Tech | C# / .NET 8, **Windows Forms** since v1.0 (2024-09). Earlier v0.x used cross-platform Avalonia. NAudio, System.Speech; NLua for criteria |
| License | MIT (Core and first-party plugins). MattG's plugins are closed-source freeware |
| Platforms | Windows. Linux via Wine 10+ builds (speech replaced by chimes; theming quirks). A "v2.0 with native Linux" is announced but no branch exists yet |
| Latest | **v1.5.2 (2026-10-01).** Cadence: 1.5.1 May 2026, 1.5.0 Apr 2026, 1.4.0 Dec 2025, 1.3.0 Sep 2025 |
| Popularity | 134 stars. Recommended widely on Steam/Reddit as "Observatory + BioInsights" |
| Status | **Very active** |

**Purpose.** Observatory is a generic journal reader that "does essentially nothing without plugins". It provides plumbing for plugins: live and batch ("Read All") journal processing, typed events, popup and voice notifications, data grids, generated settings UIs, themes and export. Its plugin ecosystem is almost entirely exploration and exobiology.

**Core features**
- **Journal reading**
  - Live monitor or **Read All** batch over every journal, with progress and cancel.
  - Start-on-launch options; `/nomin` flag for min-ed-launcher; optional polled mode.
- **PreRead on start.** When monitoring starts, Observatory reads the last two journals and keeps only what follows the last FSDJump/CarrierJump, plus the LoadGame, Statistics, Loadout and CarrierLocation headers. Plugins therefore know the commander, ship and system instantly.
- **Low-latency pickup.** Every 250 ms it opens and closes the newest journal to force Windows to report the change. Without this, popups could lag up to 30–40 s (#80).
- **Main window**
  - One tab per plugin, drag-reorderable.
  - Any tab can **pop out** into its own window, which remembers its position and can be always-on-top.
  - Export the current tab to CSV or XLSX.
  - Built-in light and dark themes plus user JSON themes.
- **Notifications**
  - **Popups:** monitor, corner or centre position, scale, font, duration, colour, acrylic blur.
    - Popups can be updated in place by GUID (for "Sample 2/3" progress) and can persist until cancelled.
    - Plugins can position them by % of the screen.
  - **Voice:** Windows SAPI/OneCore with SSML (so "AB 1 a" is pronounced correctly), duplicate-title suppression, a 6-sound **chime mode** as an alternative to speech, output-device choice.
  - **Routing:** flags send each notification to popup, voice and/or notifier plugins. Notifications are suppressed during Read All unless a plugin opts in.
- **Plugins**
  - Single-file `.eop` bundles: double-click to install.
  - Built-in update check and plugin self-update.
  - Settings screens are generated from code annotations.
  - Plugin types: worker (journal and status events) and notifier (takes over popups and voice).
  - Inter-plugin messaging; per-plugin storage; shared HTTP client.
- **Journal model.** A full typed model of journal events, up to Odyssey 4.4.1. It covers Colonisation, Powerplay 2.0, Trailblazers, Vanguards, and the `WasFootfalled` / `WasLogged` flags.

**First-party plugins**
- **Explorer**
  - **23 built-in body criteria:**
    - landability: terraformable, atmosphere, high-g, large, ringed
    - orbits and rings: close orbit, shepherd moon, wide ring, close and colliding binaries, ring proximity, nested moon, small object, fast rotation and orbit, high eccentricity
    - signals and materials: diverse life, good FSD injection, all FSD or all surface materials in system
    - discovery: high-value body (prefixed "Undiscovered" or "Unmapped"), uncommon secondary star, codex discoveries
  - **Custom criteria in Lua:**
    - Block types: Simple (one-line expression plus detail), Complex, Global, and per-event blocks (Jump, AllBodies, BodySignals, Discovery).
    - Inputs: `scan`, `parents` and `system` tables, plus helper iterators and unit converters.
    - `notify()` sends custom alerts from a script.
    - Script errors appear in the grid with the scan JSON.
    - A shareable community pack exists ("Custom Criteria For Everyone").
- **Botanist:** per-body bio list with analysed status.
  - Sampling popup that updates in place: "Sample N of 3", the genus colony range, and a live great-circle distance to earlier samples (from Status.json lat/long and the body radius).
  - Hard-coded colony ranges per genus. No credit values.
- **Herald:** cloud TTS through Vithigar's Azure service (requires a Patreon pledge), your own Azure key, or an OpenAI-compatible endpoint. Local voice cache. English only.
- **Photographer** (2026)
  - Imports screenshots automatically while monitoring, or from history via Read All.
  - Converts to PNG, JPG, WEBP or HEIC, with resizing.
  - Names and folders built from a template of metadata (system, body, coordinates, heading…).
  - Embeds the metadata in the image.
  - Captions and watermarks with automatic placement; presets.
- **Communicator:** chat log with per-channel filters and notifications.

**Key third-party plugins**
- **MattG's plugins** are closed-source freeware from edjp.colacube.net, versioned 1.3.444 in Sep 2026.
  - **BioInsights:**
    - Predicts genus, species and colour variant per body from the FSS, and narrows the prediction after DSS and samples.
    - Grid columns: signals, high-value and new-codex flags, genus (crossed out if absent after DSS), species with a Seen yes/no mark, samples, type (star- or material-dependent variant), possible variants, base value and colony distance.
    - Per-body value **range** that narrows as signals are confirmed.
    - Configurable "never scanned" markers.
    - Needs a Read All on first run.
    - Reputed to be slightly more accurate than BioScan in edge cases.
  - **Evaluator:**
    - Per-body current and maximum (if mapped) value; flags bodies **worth mapping**.
    - Distance thresholds per body class; a "Calculate Efficient Route" visiting order; notifications.
    - **The exploration value formula used by Pioneer and SrvSurvey is credited to MattG.**
  - **StellarOverlay:** a notifier that replaces native popups and voice with "highly customizable" notifications.
  - **AstroAnalytica:** notable stellar phenomena, unregistered beacons, listening posts. Replaces SignalMonitor, GEC and CodexEntries.
  - BoxelStats, Colliders and Hecate (undocumented); legacy Telegram notifier.
- **GeoPredictor** (mcmuttons, MIT): predicts geological signals before landing, flags possible new codex entries per region, can add bodies to Evaluator's route.
- **fredjk-gh suite** (MIT):
  - **Aggregator:** one row per body with an icon summary (new, all-found, scoopable, high-value, landable, bio/geo counts) and every plugin's notifications grouped beneath it. An "interest checklist" ticks bodies automatically on FSS target, then DSS or landing.
  - **Archivist:** a database of exploration journal entries, plus **synthetic entries rebuilt from Spansh**; system search; notification log; address cache with ID64 decoding.
  - **Fleet Commander:** tritium, capacity and balance; jump and cancel timers with pop-out countdowns; Spansh carrier routes with next-jump clipboard; **"carrier runs out of funds within 30 days" warning**; squadron carriers; inventory and bartender tracker (beta).
  - **Helm:**
    - Cards for commander (Realtime vs **Detached** history browsing), system, planet, route, ship, cargo, prospector and messages.
    - Alerts: low fuel with scoopable stars, secondary stars to refuel from while neutron jumping, high-g on approach, no limpets, "undiscovered system not fully scanned".
  - **Prospector:** ring commodity callouts in the FSS; hotspot callouts after DSS; per-rock callouts above a threshold or with a core; limpet warnings; session stats.
  - **Stat Scanner:** compares each body to EDAstro galactic records and your personal bests; near-record and tie alerts with rarity thresholds; per-region codex tracking.

**UX ideas worth copying**
- **The `NotificationArgs` model:**
  - title and detail, each with optional SSML;
  - routing flags (popup, voice, plugins);
  - position and timeout (0 = until cancelled);
  - update-in-place by GUID;
  - a grouping ID per body;
  - "extended details" kept for logs rather than speech.
- **Lua criteria** with simple/complex/event blocks and shareable packs.
- **Aggregator's body-centric** "one row per body" consolidation.
- **Read All and PreRead:** a cancellable Read All with progress, and the PreRead "instant state" trick.
- **Pop-out tabs** for second screens.
- **Annotation-generated settings UIs.**

**Data sources & integrations**
- **Files watched:** Journal, Status.json, Cargo.json.
- **Files read on their event:** NavRoute, Market, Outfitting, Shipyard, Backpack, FCMaterials, ModulesInfo, ShipLocker (with retries).
- **Uploads:** Core uploads nothing. The README still lists CAPI, EDDN and IGAU as "planned".
- **Plugin services:** Spansh, EDAstro, EdGIS, Telegram, Azure/OpenAI (Herald).

**Pain points & requests**
- **Plugin breakage**
  - Third-party plugins break on Core updates: most-reacted issue #193 "Core 1.2.1 requires bioinsights update which doesn't exist".
  - Breaking releases: 0.2.22347, 1.0 and 1.2.
- **Install, start-up and Linux**
  - Start-up failures: #234 (Dec 2025), #100.
  - Wine problems: #202, #217; #255 Linux install missing core plugins (Aug 2026).
- **Overlay and display**
  - Popups do not show in exclusive fullscreen (#254).
  - Multi-monitor popup issues: #48, #61; #206 multi-line popups overlap.
  - Taskbar interference: #204.
- **Open requests**
  - #230 voice on/off per criterion.
  - #17 disable individual criteria.
  - #28 font size and column widths (open since 2021).
  - #62 Read All for a date range.
  - #243 export BioInsights results so COVAS:NEXT or EDCoPilot can speak them (VR).
- **Other limits**
  - Read All takes minutes on large journal sets.
  - Herald is paywalled and English-only.

### 3.6 Exploration Buddy (EDEB, "Elite Dangerous Exploration Buddy")

| | |
|---|---|
| Site | https://www.panostrede.de/EDEB/ (changelog at /changelog.html). EDCodex entry 585 |
| Author | CMDR Panostrede |
| Tech | .NET 8 (since 2.3.0), Inno Setup installer, local database (engine not stated), Windows SAPI voice. Not code-signed |
| License | Freeware, closed source. No public issue tracker; feedback goes through an in-app menu and Discord |
| Platforms | Windows. Works under Wine/Proton with workarounds (Proton #9735: TTS voice-files path, May 2026) |
| Latest | **2.7.9** (current website build; 2.7.6 was 2026-01-18) |
| Status | **Active.** Steady releases. Commonly listed next to SrvSurvey as an essential Odyssey exploration tool |

> Name note: other tools with similar names exist (Sextant VoiceAttack profile, EDScout, Caspian Surveyor, ScanDeck, MattG's EDJP). "Exploration Buddy" in community usage means EDEB.

**Purpose.** A second-monitor companion for Odyssey (Live mode only). It shows the plotted route, the current system's bodies and biologicals with values, predictions and history, using local journals plus EDSM. A compact transparent HUD covers single-monitor players.

**Feature list**
- **Commander/Dashboard:** activity state; current system's total exploration value; trip progress.
- **History & Stats**
  - Permanent exploration history plus a separate **current trip**, which can be reset after selling.
  - Estimated value across cartography and Vista Genomics, with first-discovery bonus ranges.
  - Star-class and biology statistics; EDSM totals.
  - Old-journal import.
  - Double-click copies a value; Ctrl+double-click copies the raw number.
- **Navigation & Routing**
  - **Route view** refreshed on every plot. Each hop shows:
    - star class, distance (and from the previous hop), scoopability
    - exploration status and EDSM estimated value
    - counts of valuable, terraformable and landable planets
    - **discoverer**, with you and your teammates highlighted
    - population, and an "EDSM-only" flag
  - **Route lock:** keeps a route even when the game re-plots. Locked routes export and import for teams.
  - In-app Spansh route generator that auto-fills your ship (2.7.6).
  - Auto-copy of the next system to the clipboard, plus a hotkey.
  - Bodies view opens automatically while the FSD charges.
  - "Surroundings" tab: the 25 nearest systems (EDSM).
- **Exploration & Exobiology**
  - **Bodies view:**
    - type, distance, terraform state, landability (icon changes after touchdown)
    - atmosphere, temperature and gravity; ring details; inclination and parent
    - bio and geo counts; DSS state in 4 colours
    - first discoverer (journal or EDSM); cartographic value
    - duplicate-name-safe identification by ID
  - **Biologicals tab** (opens automatically on approach):
    - genus and species, scans done, colony range, Vista Genomics value plus first-discovery bonus
    - colour variants
    - **species prediction from Canonn statistics** (needs at least 3 recorded occurrences; narrows after DSS)
    - real-time **inside/outside colony range** indicator with an altitude threshold
  - **"Planets of Interest"** rules built in a **form** (no scripting), in tabs, with colourable icons and optional speech.
  - Value thresholds for bodies (2 M default) and species (8 M default).
- **Voice/TTS & alerts:** speaks chosen events through placeholder phrase templates, with correct pronunciation of designations; voice, speed and volume settings; cancel-speech hotkey.
- **Overlay/second screen**
  - Main window designed for a second monitor.
  - **HUD:** transparent and compact, with configurable views and columns. Auto-hides in the galaxy map, codex and on foot. Click-through mode (2.4.4). Hotkeys to show, switch mode and open tabs.
  - Every colour is customisable.
- **Website sync:** read-only EDSM. Recommends EDMC/EDDLite plus the Canonn plugin for uploads.
- **Not covered:** maps, trade, mining, missions, BGS, Powerplay, colonisation, carrier, engineering, builds, suits, bindings.

**UX ideas worth copying**
- **Route planning:** hop-by-hop route summary showing "is it worth going / already discovered by whom"; route lock with team sharing.
- **Teams:** highlighting of teammates' discoveries.
- **Rules:** form-based POI rules for non-programmers, with spoken templates.
- **HUD:** context auto-hide and click-through.
- **Value tracking:** "?" when EDSM lacks bio/geo counts; separate trip and lifetime tallies.

**Data sources & integrations:** journal (Status.json for the live colony-range check is likely but not documented), EDSM API, bundled Canonn statistics, Vista Genomics price table, Spansh routes.

**Pain points**
- **Display & input**
  - The HUD needs borderless mode (Steam, Oct 2024).
  - The mouse escaped to the overlay (fixed by click-through in 2.4.4).
- **Stability & data safety**
  - Crashes on window state changes.
  - **Data or settings lost on upgrades:** 0.9.9 wiped locked routes, 1.5.9 reset history, 2.3.0 reset speech settings.
- **Platform & data**
  - EDSM dependency: values depend on other players uploading.
  - Linux TTS breakage (Proton #9735).
  - No public tracker.

### 3.7 EDCoPilot

| | |
|---|---|
| Site / downloads | https://www.razzafrag.com (feature list, voice commands, blog). Wiki: razzserver.com/dokuwiki. Installer releases: https://github.com/Razzafrag/EDCoPilot-Installer |
| Author | CMDR Razzafrag (Razzafrag Technology Inc., Canada). Sole developer, project about 7 years old |
| Tech | Closed source. Components: `EDCoPilot.exe` (server), `EDCoPilotGUI2.exe` (GUI, also used as a remote client), INI config, Python `edge_tts` for Edge voices, MSI installer. Implementation language not published |
| License | Proprietary freeware (EULA). Patreon from $2.50/month or a $20+ donation buys immersion extras only; "all game aids are free" |
| Platforms | Windows only. VR through overlay tools. Linux unofficially via the community EDCoLauncher (Proton). Not usable with GeForce NOW |
| Latest | **v1.11.762 (2026-10-11).** About 40 releases from Oct 2025 to Oct 2026, with same-day hotfixes |
| Status | **Very active** |

**Purpose.** A "talking ship AI" plus a 40+ panel second-screen/VR UI plus input automation.
- It announces events, generates immersive station, crew and NPC radio chatter, and plots courses by driving the galaxy map with simulated key presses.
- It was built originally so VR pilots never need to remove the headset.

**Feature list**
- **Commander/Dashboard (panel catalogue)**
  - No single dashboard. Panels are reached through Activity, Information and Management tabs.
  - **Logs:** Voice Log, Event Log, System History.
  - **Session Progress:** combat, mining, on-foot and colonisation layouts.
  - **Navigation:** Bookmarks and Bookmark Groups, Location Search, Places of Interest, Guidance Computer.
  - **System:** System Information (factions, conflicts, Powerplay, surface-mining totals; auto-shows on jump), Body Data, Bodies (with R2R values and landing counts), Stations, Station Facts (Inara Galpedia plus architect-written pages).
  - **Ships and engineering:** Show My Ships, Stored Modules, Blueprints.
  - **Economy:** Mining Prices and Mining Stats, Trading Table and Trading Tables (material trader), Commodities, Trade Route.
  - **Odyssey:** Ship Locker/Backpack, Suit Loadouts, Suits, Weapons.
  - **Other panels:**
    - Mission List
    - Discovery Estimator and Prospector Announcements
    - Messages (predefined chat) and Permits
    - Biologicals and Biologicals In System
    - Faction Systems
    - Command Reference
    - Controls button grids (flight, combat, SRV, on-foot, camera)
    - MiniBar and Subtitles bar
    - Time Trial leaderboard
- **History & Stats**
  - History Refresh re-processes all journals. It is mandatory at setup and confuses new users.
  - Session and lifetime stats.
  - Combat totals by ship type, ship, system and faction, with milestone congratulations.
  - Per-system and per-station visit counts, announced on arrival.
  - Per-ship stats: jumps, distance, discoveries, kills.
  - Mining stats per system.
- **Maps**: no galaxy or system renderer. The Guidance Computer gives surface bearing and distance to coordinates from notes, history, POIs or bio samples.
- **Navigation & Routing**
  - **Bookmarks:** unlimited, grouped, sortable and filterable (discovered, IF, mined, first footfalls, biologicals), with notes and a per-system command sub-menu. Optional auto-bookmark of every plotted system.
  - **Voice course plotting:** "Set course for Sol" opens the galaxy map and plots. The last 120 destinations are available by voice.
  - **Spansh routes:** Neutron (auto-detects the Mk II FSD supercharge range), Road to Riches, Fleet Carrier, Exo Mastery, and Galaxy Plotter CSV import.
    - Routes become **bookmark groups**, with "next jump" commands and neutron auto-plot after supercharge.
  - **Location Finder** returns **one recommended result**, weighing jumps against distance from the star. It covers:
    - commodities to buy or sell, material traders, tech brokers, module and ship sellers
    - hotspots and surface mining sites
    - Vista Genomics, Universal Cartographics, Bartenders, Pioneer Supplies, vehicle hangars (Sep 2026)
  - Scoopable and neutron-ahead announcements.
  - Landing-pad directions, also drawn through the EDMC overlay.
- **Exploration & Exobiology**
  - **Discovery Estimator:** last scanned body, new terraformables, HGE candidate announcements, R2R progress with auto-plot.
  - **Biologicals panel:** species, then colour variant, then system and body.
  - **Biologicals In System:** values, remaining vs scanned credits, "scanned before" flags.
  - **On arrival:** ranks bodies by payout using Spansh/Canonn data.
  - **Sampling callouts:** sample 1/2/3, colony distance, "moved far enough", perimeter warnings; then suggests the next body or plots the next system.
  - **Exo Mastery route:** filtered by your own scan history.
  - FSS bio-count cross-check against Spansh/Canonn.
  - Samples lost on death are reset.
- **Trade & Markets**
  - Spansh Trade Planner routes (1–10 hops) with next-hop navigation and **live market re-checks** that suggest a better or replacement purchase.
  - Commodity best buy/sell lookups.
- **Mining**
  - Session tracker with quantities, value and live prospector reports. Announced prospector percentages for chosen materials, plus "lack of cores" comments.
  - Mining Prices panel with links to hotspot and sell finders; mining stats by system.
  - **Rhino SRV surface-mining tracking with a manual tagging editor** (Oct 2026).
- **Missions**: Mission List with reminders 30 minutes before expiry and when ready to hand in; sound cue when you scan a mission target. **No massacre stack tracking.**
- **BGS**: Faction Systems panel (your faction's systems kept as a bookmark group, with rival faction states); tick announcement; faction and conflict data, sourced from Spansh since Oct 2025 (was EliteBGS).
- **Powerplay**: Powerplay info in System Info (Jan 2026). Further PP2 details unverified.
- **Colonisation**
  - Colonisation Progress layout: depot remaining, carrier stock and hold.
  - Reload button that pulls in deliveries **made by other EDCoPilot users** (crowd-shared progress).
  - Supporter perk: **architect-published system greetings, broadcasts and Station Facts pages** that every EDCoPilot user in that system sees or hears.
- **Fleet Carrier**: Spansh carrier route as a bookmark group, with "next carrier jump" and "Set FC Destination"; carrier greetings. No finance or tritium panel.
- **Engineering & Materials**
  - Blueprints panel (engineer, then module, then blueprint) showing needed vs owned, with engineer status (unlocked, known, invited).
  - Crafting lists, material inventory, trader tables.
  - "Material category nearly full" reminder at docking.
- **Ship builds**: My Ships (stats and loadouts), Stored Modules, per-ship minimum-limpet "nag mode". No build planner or EDSY export.
- **Odyssey suits/weapons**: locker/backpack, suit loadouts, suits and weapons panels; on-foot session progress; auto-shields when on foot; radio filter on the AI voice outside the ship; on-foot Time Trials.
- **Voice/TTS & alerts**
  - **TTS providers:** free Edge neural voices (default), Windows SAPI (including CereProc), Google Cloud (Chirp3-HD), Amazon Polly, Azure, ElevenLabs (including cloned voices). All cloud voices use **the user's own accounts**.
  - **Translation:** Google or DeepL, with proper nouns excluded automatically.
  - **Audio:** radio effects; an interruptible speech queue; per-provider pronunciation fixes; choice of output device.
  - **Personality knobs:** AI name, how it addresses you, nicknames, a profanity probability, sweetheart or all-business modes, "dislikes mining/trading", chit-chat interval.
  - **Speech Extensions:** per-event phrase files with tokens, filters (`(operations)`, `(not-station)`, `(flirt)`…) and a REPLACE directive.
  - **Space Chatter:** station traffic, NPC/local/commander chat, carrier greetings, crew chatter from five simulated crew, deep-space chatter.
  - **Announcements:** navigation, docking, limpets, threats, friends, Galnet, prospector, HGE, BGS tick and more.
  - **Sounds:** fixed cues plus per-event custom sound folders.
  - **Music:** event-driven player with m3u playlists and internet radio.
  - **Context awareness:** knows Main, Operations or CQC mode, and goes quiet at Thargoid Spire sites.
- **Speech input / AI**
  - **No built-in speech recognition.** Voice control needs paid VoiceAttack plus EDCoPilot's profile.
  - Commands such as "panel 3", "save layout 2", "Show Display".
  - LLM chat: originally via an OpenAI VoiceAttack plug-in. Since June 2025 the author recommends the **COVAS:NEXT integration** instead.
- **Overlay/second screen**
  - VR through Oculus Dash, Desktop+, OVRDrop, OVRToolkit or OpenKneeboard.
  - Single monitor with show/hide hotkey and transparency.
  - Dual monitor with **auto-refocus of the game window after a click**.
  - Up to 10 voice-addressable panel layouts; multiple UI instances.
  - Tablet through SpaceDesk; a **remote Windows client** connecting by IP.
  - Layout scales with window shape; HUD colour matching; MiniBar; subtitles bar.
  - **No web UI.** The third-party **EDCoPTER** fills the gap: a local web server rebuilding panels for any browser, custom button panels, and MQTT/Home Assistant hooks.
- **Website sync**: read-only. No uploads to EDDN, EDSM or Inara. Its own backend handles Time Trials, crowd-shared colonisation deliveries and architect content. Screenshots to a Discord webhook.
- **Bindings**
  - Parses `.binds` to drive key-press automation; keybind check at setup.
  - **Automatic backup of keybinds on every change.**
- **Misc**
  - SRV, ship, SLF and on-foot **Time Trials** with a global leaderboard and track publishing.
  - Screenshot conversion and renaming.
  - Telemetry.

**UX ideas worth copying**
- **Panels and layouts**
  - Voice-addressable multi-window layouts.
  - Remote thin client, with auto-refocus of the game window.
- **Exobiology flow**: rank bodies on arrival, then guide to the sample, then call out distance, then announce 1/2/3, then plot the next target.
- **Routes and finding**
  - Routes become bookmark groups.
  - A single recommended answer from the location finder.
  - Trade route with live market re-checks.
- **Reminders**
  - Docking checklists: minimum limpets, full material categories, cargo.
  - Situational suppression: game mode, Spire sites, on foot.
- **Content and community**
  - User-generated phrase packs with tokens and filters.
  - Crowd-shared colonisation progress.
  - Architect-published system content.
- **Safety net**: automatic keybind backups.

**Data sources & integrations**
- Reads: Journal (live plus full history), Status.json, NavRoute, `.binds`, screenshots. Market, Cargo and ShipLocker files are inferred from panel contents.
- External: Spansh (routes, trade, exo, system lookups, and since Oct 2025 faction and station data), EDSM fallback, Canonn bio data, Inara Galpedia, Google and DeepL translation, TTS clouds, Discord, VoiceAttack and HCS, COVAS:NEXT, SRVTracker, its own backend.

**Pain points & requests**
- **Setup**
  - Voice input requires paid VoiceAttack.
  - Settings are buried in INI files (e.g. the UI toggle hotkey).
  - History Refresh confuses first-time users.
  - VR overlay setup is hard (OpenKneeboard/OpenComposite workarounds).
- **Reliability**
  - Antivirus false positives.
  - **Edge TTS fragility:** Microsoft's free endpoint started returning 403 around 2026-10-10, forcing three hotfixes in two days. This is a **systemic risk for any tool defaulting to Edge TTS.**
  - Regressions: speech-queue rewrite reverted, bookmark and GUI crashes in 2026.
- **Coexistence**: users run EDCoPilot, EDDI, HCS and COVAS at once and **they talk over each other**.
- **Gaps**: no web UI, no uploads, no massacre stacking, no carrier finance or tritium tools, Windows-only.

**Maintenance.**
- Very active: about one release every 10 days through 2026.
- Version scheme changed to 1.11.7xx in May 2026.

### 3.8 OD Elite Tracker (ODET)

| | |
|---|---|
| Repo | https://github.com/WarmedxMints/ODEliteTracker. Author WarmedxMints ("WarmedMints" on the Frontier forums) |
| Lineage | Replaces **ODMissionStacker** (archived 2025-10-23) and OD-Trade-Mission-Tracker. The author's separate **OD-Explorer** (73 stars) covers exploration/exobiology |
| Tech | C# / **WPF**, `net9.0-windows`. EF Core 9 + SQLite (per-CMDR DB in `%LocalAppData%`), NAudio, NLog, toast notifications. Some helper projects (ODCapi, ODCompass, ODJournalDatabase, ODMvvm) are **not public**, so it cannot be built from source (#3, #32) |
| License | **No license file** (all rights reserved) |
| Platforms | Windows only. Wine is unreliable (#43). An **Avalonia rewrite for native Linux** was decided in Jan 2026; a beta zip is attached to #1, with no public branch |
| Latest | **1.5.32 (2026-10-04)**. About 45 releases since v1.0 (2025-05-02). New hulls are added promptly |
| Popularity | 43 stars (the ODMissionStacker predecessor had 53) |
| Status | **Very active, single maintainer** |

**Purpose.** A journal-driven activity tracker. It turns massacre stacks, trade missions, colonisation depots, BGS, Powerplay 2.0, mining and fleet carrier activity into live tables, click-through overlays and clipboard-ready Discord reports.

**Feature list**
- **Commander/Dashboard**
  - Views: Massacre, Trade, Colonisation (with Route Planner), BGS, Powerplay, Mining, Fleet Carrier, Spansh CSV, Settings.
  - Status bar shows system, station and ship. System and station link to Inara or Spansh; the ship links to EDSY or Coriolis.
  - **Multi-commander**:
    - per-CMDR journal folders;
    - hide or delete commanders;
    - **"Migrate" a renamed CMDR's history**;
    - Legacy commanders kept separate.
  - Journal history age setting (7 days … 8 years or All).
  - Theme, UI scale, resizable splitters.
- **History & Stats**: per-CMDR SQLite ingest at start-up. Keeps completed massacre and trade missions, mining sessions, BGS per tick, and the current and previous Powerplay cycles. No general stats.
- **Navigation & Routing**
  - **Spansh CSV route follower** (R2R, carrier, neutron, galaxy, world-type, tourist, exobiology, colonisation):
    - shows the current and next system;
    - previous/next controls and auto-copy;
    - carrier ETA;
    - refuel notification.
  - **Colonisation Route Planner**:
    - imports the live NavRoute, Spansh colonisation JSON or a saved plan;
    - mark systems as **Hub** (resets hop counts) or **Built** (auto-detected when a system has more than one faction);
    - distances per hop, since the last hub, from the hub and from the start;
    - body counts from your own scans or the Spansh dump API;
    - clipboard automation on arrival;
    - export to JSON or CSV.
  - **Straight Line tool** and overlay: lateral, vertical and depth offset from a line toward a deep-space target.
  - **Planetary compass** overlay with importable bookmarks.
- **Missions: massacre stacker**
  - **Missions table**: origin, issuer, target faction and system, kills, live expiry, reward, wing flag.
  - **Stacks** grouped by issuer × target faction × target system, with:
    - kills and remaining;
    - **Diff** (how far behind the top stack);
    - **kills to next completion**;
    - kill ratio;
    - average reward per kill and per mission;
    - stack value and **shareable (wing) value**;
    - turn-in value.
  - Highlights missions ready to hand in when you dock at their origin.
  - Hover a faction to highlight its missions.
  - Overlay; Operations-mode fixes in 2026.
- **Missions: trade**
  - Collect, Delivery, Mining and Altruism missions, grouped by station with commodity totals.
  - CargoDepot progress, expiry, rewards.
  - **Carrier stock and diff** columns.
- **Trade & Markets**
  - **Market Watchlist**: pin a market (usually a carrier being loaded). Remaining demand drops with your sells, and changes in the carrier's Market.json reveal fills by other players. Overlay; total value.
  - Purchase history per commodity and station.
- **Mining**
  - Sessions start and stop automatically: SupercruiseExit at a ring or cluster starts one; jump, supercruise or menu ends it.
  - Session stats:
    - tonnes per hour;
    - refined, prospected and cracked counts;
    - prospectors fired and collectors deployed;
    - low/medium/high content distribution.
  - Per-commodity min/max %, motherlodes, limpets.
  - Latest-prospector overlay and history.
  - **No sales or profit tracking.**
- **BGS**
  - Per-tick ledger by system and faction:
    - INF from missions;
    - bounties and bonds;
    - trade spend and sales;
    - cartography and SAR;
    - ship and on-foot murders (victim faction resolved through ShipTargeted);
    - space and ground CZs at low/medium/high intensity;
    - settlement activity;
    - megaship data uplinks.
  - Conflict views.
  - Ticks auto-pulled from EliteBGS and the Infomancer galtick feed; manual add/delete; hide detected ticks.
  - **One-click Discord post** (markdown with a native `<t:unix:f>` timestamp), with per-system ignore toggles.
  - Outstanding bounty vouchers.
  - Nearest material traders.
  - Settlement Activity overlay.
- **Powerplay 2.0**
  - Pledge, rank, total and cycle merits.
  - Per-system merits per weekly cycle (current and previous).
  - **Merit attribution heuristic**: a merit event is credited to the activity seen in the previous 5 s; early merits are stored and applied later.
  - Goods collected and delivered.
  - Discord post.
  - **Gaps**: pledge changes not handled (#40); weekly-assignment merits not attributed (#41).
- **Colonisation**
  - Depots (from ColonisationConstructionDepot) show progress %, remaining tonnes and required/provided/remaining by category.
  - **Build type inferred from the commodity profile**, because the journal does not record it.
  - Commander systems tracked from the claim, release and beacon events.
  - Columns:
    - ship cargo;
    - carrier stock from CAPI (stolen goods excluded);
    - **Carrier Diff**;
    - current market availability;
    - **"previously purchased from" with distance**.
  - Trip estimate.
  - Multi-depot **Shopping List** overlay.
  - Build totals and a **Build Wishlist** (future builds × count gives commodity totals).
  - Build Stats reference table.
  - Discord/ASCII posts and CSV export.
- **Fleet Carrier**
  - Personal and squadron carriers: fuel, balance, access, services, location, destination.
  - **Cooldown timers** with a toast and sound.
  - Stock and sell orders, rebuilt from CargoTransfer, own-carrier sales, trade orders and Market.json.
  - CAPI refresh with a 15-minute cooldown.
  - **No finance projection and no native route planner.**
- **Voice/TTS & alerts**
  - No TTS.
  - **Toasts**: system on jump, station on dock (with pads), wanted/enemy ship scanned, copied to clipboard, carrier ready, docking denied with reason, crime, refuel.
  - Toast options: size, monitor- or app-relative placement with X/Y offsets for triple monitors, maximum visible, themes, click-through.
- **Overlay/second screen**: **11 pop-out overlays** (Massacre, Trade, Colonisation, Shopping List, Watchlist, Mining, Prospector, Settlement, Route, Compass, Straight Line). Each has:
  - opacity that goes opaque on hover;
  - always-on-top and click-through;
  - per-overlay UI scale;
  - multiple instances.

  A central "active overlays" list manages them. No web or remote view (#28 requested).
- **Website sync**: **none outbound** (no EDDN, EDSM or Inara). Discord output is clipboard-only.

**UX ideas worth copying**
- **Massacre metrics**: Diff, kills-to-next-completion, shareable value, and hand-in highlighting at the origin station.
- **Discord posts**: one-click creation with native timestamps and per-system include toggles.
- **Colonisation**: Carrier Diff and trip estimates on every list; "previously purchased from" built from your own MarketBuy history; build-type inference.
- **Watchlist**: shows fills by other players by diffing the carrier market.
- **Route planner and straight-line tool**: hubs and auto-"Built" detection in the planner; the straight-line tool for deep-space bearings.
- **Powerplay**: merit attribution using a time window.
- **Overlays**: per-overlay hover-to-opaque, click-through and scale, plus a central list of active overlays.

**Data sources & integrations**
- **Journal events**: missions, combat, market, mining, BGS, Powerplay, colonisation, carrier, scans.
- **Companion files**: Market.json, Cargo.json, NavRoute.json and probably Status.json.
- **Frontier CAPI**: carrier cargo and orders; Frontier, Steam and Epic logins.
- **Web APIs**: EDSM, EliteBGS ticks, Infomancer galtick, Spansh dump API.
- **Outbound**: links only, to Inara, Spansh, EDSY and Coriolis.

**Pain points & requests** (ranked by comments; reactions are all 0)
- **CAPI & multi-account**
  - #13 carrier cargo not updating via CAPI (21 comments).
  - #15, #22 CAPI login loops; no Steam login (fixed in 1.5.1).
  - #2, #33, #34 multiple accounts, and running Steam and Epic at the same time.
- **Platform**: #1 **native Linux** (15 comments, open).
- **Overlay & notifications**
  - #36 overlay flicker at 4K.
  - #45 notification settings don't apply.
  - #27 overlay controls unclickable at reduced scale.
  - #26 massacre overlay grouped by faction.
- **Massacre accuracy**
  - #46 remove expired missions from stacks.
  - #20, #24, #29, #49 Diff and kill-count accuracy.
  - #23 manual kill correction for wingmates' kills.
  - #50, #51 the Operations update broke massacre tracking.
- **Remote monitoring**: #28 **web server for remote monitoring, plus a ship-destroyed-while-AFK alert**.
- **Other**
  - #40, #41 Powerplay attribution gaps.
  - #16, #19 settings lost on update; SQLite start-up crash.

**Adjacent competitor: Elite Dangerous Warboard** (Mirooz; MIT; JavaFX; Windows and Linux; 44 stars). "Stack pirate massacre missions in one place", plus mining sessions with a prospector overlay, exobiology prediction, Spansh routes, colonisation (architect panel, multi-commodity market search, carrier CAPI grid) and optional EDDN upload.

### 3.9 SrvSurvey (+ Raven Colonial)

| | |
|---|---|
| Repo | https://github.com/njthomson/SrvSurvey. Author CMDR Grinning2001 (GitHub njthomson). Companion website **Raven Colonial** (ravencolonial.com), same author |
| Tech | C# / .NET 9 WinForms x64. SharpDX (D3D11, DirectInput, XInput), OVRSharp (OpenVR), LuaCSharp. Ships as a signed MS Store build or an unsigned GitHub zip with auto-update |
| License | GPL-3.0 (SrvSurvey and RavenColonialWeb) |
| Platforms | Windows. Linux only by launching inside the game's Proton prefix via min-ed-launcher. Steam Deck has quirks. A community **SrvSurvey-XP** fork (.NET 10, Avalonia 12, SkiaSharp, SDL3) produces a Linux AppImage; it works on X11/XWayland only |
| Latest | **2.0.95.48 (2026-09-05)**: Rhino support and FC route following. About 250 releases, often weekly |
| Popularity | 174 stars, 159 open issues. Steam users call SrvSurvey + Raven Colonial "great" |
| Status | **Extremely active** |

**Purpose.** It began as a Guardian-ruins survey aid. It is now a set of 27 context-triggered overlays covering:
- exobiology, FSS/DSS values, jump previews and ground navigation;
- Guardian sites and Ram Tah missions;
- human settlements and on-foot combat zones;
- colonisation logistics, via Raven Colonial.

It stays hidden until game events make an overlay relevant.

**Feature list**
- **Exploration & Exobiology**
  - **System bio overlay**: one row per body with an estimated reward and one block per genus.
    - Yellow blocks mark species or variants you have never scanned.
    - Value bars use 3 / 7 / 12 M bands.
    - Completed bodies are struck through.
    - Galactic and regional first-discovery flags.
  - **Body approach overlay**: possible species and variants per genus. After DSS it shows analysed counts; after a sample it shows sample circles, reward, and the distance still needed. Canonn reference images.
  - **Ground overlay**:
    - **radar** of previous samples, ship and SRV, with circles that change colour when you are far enough;
    - auto-tracked **Composition Scanner** scans;
    - named trackers created by chat command (`+bac` makes a 500 m tracker);
    - 8 bookmark key-chords;
    - **prior scans by other players** (from Canonn);
    - targets ordered by reward;
    - glide **attitude indicator** (ideal -30° to -50° descent);
    - per-ship cockpit-offset correction.
  - **Bio prediction engine**: JSON rule trees per genus, species and variant (ranges, atmosphere %, regions and arms, nebula distance, Guardian bubble, materials, star types), tested automatically against Canonn data. Rewritten Apr 2026.
  - **FSS overlays**: last scanned body with value with and without DSS; full body list with FSS/DSS values including the efficiency bonus, plus undiscovered/terraformable/landable markers; DSS skip filters.
    - Experimental: samples screen pixels to detect whether the FSS is still scanning, to fill a journal gap.
  - **Visiting-order optimiser** (TSP over orbital elements) in progress (#875).
  - **Body info**, system status after the honk, **galaxy-map preview** (EDSM discoverer and date, bodies, genus count), **jump info** (route progress, scoopable and neutron hops, traffic, stations).
  - Green gas giant detector.
  - **Codex Bingo**: personal discovery tree per region.
  - **Old Journal Processor**: rebuild first discoveries from a chosen date.
  - **Journeys**: named expeditions with stats, notes and screenshot albums; they follow carrier jumps.
- **Guardian / Odyssey**
  - **Guardian ruins and structures**: interactive photogrammetry maps; logs presence of relics and items using fire-group and cockpit-mode toggles as input; heading measurement and aerial alignment; site database with filters.
  - **Ram Tah**: mission log checklists and an obelisk map that shows needed items.
  - **Human settlements and combat zones**: settlement maps (medkits, batteries, terminals); material-collection surveys; foot CZ checkpoints. Overlays hide for Combat/Maverick suits.
- **Navigation & Routing**
  - Lat/long ground target with heading arrow.
  - **Follows Spansh routes** (neutron, tourist, galaxy, **fleet carrier** since Sep 2026), auto-copying the next hop.
  - Sphere and boxel search helpers.
  - "Nearest systems".
  - High-gravity warning.
- **Colonisation (with Raven Colonial)**
  - Projects created **automatically from depot and contribution events**; shared and primary projects.
  - **Shopping overlay** (Need / FCs / Ship):
    - markers for: not sold here, enough, ship carries too much, and a finish-flag hint when your ship completes the carrier stock;
    - scope adapts to "this site", "N local projects" or "N total projects";
    - hotkeys.
  - Fleet and **squadron** carrier cargo tracked against a baseline and synced.
  - System updater that identifies built facilities by triangulating the landing pad.
  - Nicknames; demolished-site handling.
- **Raven Colonial website**
  - **System planner** (map and table views; Spansh body import; issues panel for tier points and prerequisites; pin a site to see its impact; camera snapshots to compare plans; economy predictions; slot heuristics; build order; barycentre parents).
  - Projects with commodity assignment per commander and hourly delivery charts.
  - Groups.
  - **Fleet carrier**: CAPI refresh **anyone can trigger** (once per 5 min per carrier) while the owner's token is under 30 days old; a yellow reminder at 28 days.
  - **"Where to buy"**: the 5 nearest markets per commodity, with FC-diff.
  - **Nexus**: multi-system bridge/chain allocator on a 3D map.
  - Buildables table.
  - Public **REST API** (also used by BGS-Tally and EDMC plugins).
- **Mining**: Rhino mining-rig trackers with 6 slots (Sep 2026); material count alert.
- **Missions**: massacre stack overlay; "cargo remaining" alerts.
- **Overlay/second screen**
  - Each overlay is anchored, offset and given an opacity in `plotters.json`, with live reload.
  - Hover-to-hide; toggle-all hotkey.
  - **OpenVR overlays** (Oct 2025) with **per-ship layouts**.
  - Streamer mode; multi-boxing support.
- **Bindings/input**
  - Key-chords from the keyboard or DirectInput/XInput (HOTAS).
  - **Chat commands** (`.note`, `.target`, `.kill`…).
  - Game controls used as signals.
  - Lua "Quests" (hidden).
- **Website sync**
  - Raven Colonial (two-way).
  - **Inara upload** (opt-in, Aug 2026).
  - Canonn reads, plus opt-in green gas giant uploads.
  - **EDDN upload code is present but disabled** (`Year > 3000` guard).

**UX ideas worth copying**
- **Context-triggered overlays** that are invisible until relevant, each user-anchored.
- **Exobiology**: value bars and first-discovery flag colours; a sample radar with colour-changing circles; glide angle indicator; auto-tracking Composition Scanner scans.
- **Colonisation**: a scope-aware shopping list with finish-flag ("one load completes it") hints.
- **Input**: chat and game-control "side channels" for input when no API exists.
- **Overlay layouts**: per-ship VR layouts.
- **History**: old-journal reprocessing from a date; Journeys with albums; Codex Bingo.
- Caution: pixel sampling is fragile.

**Data sources & integrations**
- **Journal and companion files**: Journal, Status.json (lat/long, heading, altitude, flags), NavRoute.json, Cargo.json, ShipLocker.json, Market, screenshots.
- **Web services**: EDSM, Spansh (dumps, stations, plotters), Canonn (bio statistics, prior scans, codex images, settlement types), EdGalaxy StarCache, Raven Colonial, Inara.
- **Reference data**: pulled from GitHub raw files.

**Pain points & requests**
- **Crashes & overlay glitches**
  - Repeated exobiology crash "Why no this.systemBody?" (#993, #1009, #1011).
  - Overlays freezing (#958).
  - The Operations screen kills the overlays (#1041).
- **Server-side & sync**
  - **Raven Colonial server errors**: #520, #648, #906.
  - **Carrier sync issues**: #525, #560, #222 (20 comments).
- **Prediction accuracy**: codex flags disagree with the game (#605); wrong economy predictions (#325, #355).
- **Linux**: #886 (Bazzite, 10 comments), #224 Wine.
- **Install & network**
  - Antivirus false positives (#137, #235).
  - Auto-update deleted the install folder (#917).
  - Fails when raw.githubusercontent is blocked (#197).
- **Requests**
  - Localisation: #132 (25 comments).
  - FC jump countdown overlay (#989).
  - Visiting order (#875).
  - VR for Meta Horizon Link (#516).
  - Bigger text (#1071).
  - Hide API key (#1069).
  - Architect systems table (#982).
- **Coexistence**: you must disable colonisation in EDMC plugins to avoid conflicts.

### 3.10 EDEngineer (archived)

| | |
|---|---|
| Repo | https://github.com/msarilar/EDEngineer: 524 stars, MIT |
| Tech | C# WPF (.NET Framework) with an F# Suave local web server. Windows only |
| Status | **Dead.** Last release v1.1.6.26 (2023-03-18), last commit 2024-03-22, **archived 2025-07-04**. No data for 2024–26 content (AGZR, SCO engineering, PP2, merc modules) |

**Features (historical)**
- **Blueprint grid:** sortable and filterable, covering ship blueprints, grades, experimentals, synthesis, tech-broker and engineer-unlock recipes, and Odyssey suit/weapon blueprints. Favourite and ignore flags.
- **Material management:** **per-material thresholds** with a thresholds-only view; "top source systems and settlements"; **shopping list** with aggregated ingredients, saved to file.
- **Notifications:** Windows toast when a **favourite blueprint becomes craftable**; cargo-full alerts.
- **Overlay:** transparent always-on-top window with a global show/hide hotkey, lock and opacity controls.
- **Commanders:** multiple commanders supported.
- **Local REST API** (loopback by default; JSON, CSV or XML; CORS):
  - read endpoints: `/commanders`, `/{cmdr}/materials`, `/{cmdr}/shopping-list`, operations, chart data;
  - **`PATCH /{cmdr}/shopping-list`**, which **lets Coriolis push a shopping list into the desktop app**.

**Pain points:**
- Linux support (#596, the most-reacted issue).
- EDSY import (#576).
- "Suggest engineers / fewest visits" (#543).

**Legacy for Elite Nexus:**
- The **local REST API with a write endpoint** is the idea worth reviving.
- EDOMH, EDD and Inara replaced the rest.


### 3.11 Colonisation tools (post-Trailblazers, 2025–2026)

**What a colonisation tool must model.** Sources: the "Colonization Mega Guide" v2.3.0 and the HerzbubeWiki (edited Oct 2026). Frontier's patch notes could not be fetched, so 2026 rule details are second-hand.

- **Claims and deadlines**
  - Claim a system within ~15 ly (some tools use 16) for 25 MCr. Deploy the beacon within 24 h. Finish the Primary Port within 4 weeks.
  - After the Primary Port is built, up to 5 projects can run concurrently. Completing the Primary Port makes you the permanent **System Architect**.
  - Anti-sniping: the architect, then their squadron, has exclusive rights to claim from the new port for 24 h (4.2.1.2).
- **Facilities and points**
  - About 55 facility types, each with tier, orbital/surface placement, pad size, layout subtype, points needed and given, economy influence, and effects on score (security, tech, wealth, standard of living, development). Each also has prerequisites and approximate haul tonnage.
  - **Construction-point escalation**: T2 ports cost 3, 3, 5, 7, 9… T2 points; T3 ports (including Dodec) cost 6, 6, 12, 18, 24… T3 points. This is why guides say "build T3 first".
- **Economy**: **strong links** within a body; **weak links** at a flat 5% to other bodies. Port "conversion" when several ports share a body. A system resource level.
- **Slot counts are not in the journal.** Tools estimate them heuristically.
- **Demolition and change history**: demolition and cancellation since 4.2.1.0 (Oct 2025). Dodec stations and "out of beta" in Nov 2025. Rules changed again in Jan and May 2026. Facilities can orbit barycentres since summer 2026.
- **Journal events and blind spots**
  - Events: `ColonisationSystemClaim`, `…ClaimRelease`, `…BeaconDeployed`, `ColonisationConstructionDepot` (every ~15 s while docked: progress, `ResourcesRequired[]` with required, provided and payment), `ColonisationContribution`.
  - **The facility build type is not logged.**
  - Carrier stock is only fully visible via CAPI. Squadron-carrier transfers are not journaled.

**Tool landscape**

| Tool | Type | Status (Oct 2026) | Key features |
|---|---|---|---|
| **Raven Colonial** (+ SrvSurvey) | Web hub + Windows overlay client; GPL-3.0 | Very active | System planner (map/table, Spansh import, issues panel, pin-impact, snapshots, economy prediction, slot heuristics, build order). Projects with commodity assignments and delivery charts. Carrier CAPI refresh **anyone can trigger** while the owner's token is valid. "Where to buy" (5 nearest markets per commodity, FC-diff). **Nexus** multi-system chain allocator. REST API. Self-reported scale (Jul 2025): ~1,700 active and ~6,800 completed projects, ~1,360 CMDRs, ~900 carriers |
| **BGS-Tally** (EDMC plugin) | Plugin; MIT | Very active (5.6.1) | Plans window. Progress window with click-to-choose columns, units in tonnes or ship loads, context-aware sort, colour legend (delivered / covered / sold here / sold on your FC). Two-way Raven Colonial sync. Overlay. Discord |
| **ODET** | Desktop (WPF) | Very active (1.5.32) | Depots with build-type inference, Carrier Diff, "previously purchased from", trip estimate. Shopping list overlay. Build wishlist. **Colonisation route planner** with hubs and auto "Built" detection |
| **EDCPS** (gaborauth/ed-colonisation-planner) | Web, client-side; GPL-3.0 | Active (Aug 2026) | **MILP optimiser** (HiGHS compiled to WASM) that picks which facilities to build: objectives, economy preferences, min/max per building, build order with running T2/T3 totals. Imports from journal, Spansh or Raven Colonial; exports Raven Colonial plan sites |
| **Spreadsheets/guides** | Google Sheets/Docs | Still used | DaftMav's construction sheet v3 (hauling tab with OCR); CMDR Mechan's Mega Guide (v2.3.0, 2.4 WIP); AetherWave reference tables |
| **Spansh Colonisation Plotter** | Web | Active | Route between two systems (claim chain), CSV/JSON output. Spansh also supplies body and carrier data to most tools |
| **ravencolonial_edmc** (Fenris159) | EDMC plugin; MIT | Active (1.8.3) | Links projects from the depot; "track all"; trip estimate; carrier countdown; three privacy toggles |
| **Architect Tracker** (kfpopeye) | EDMC plugin; GPL-2.0 | Active (v1.8, Sep 2026) | Required/provided/needed per site; preferred market (cheapest/closest/alternate); carrier cargo with pause sync; **slim mode for VR** |
| **EDMC-ConstructionTracker** (Greybaer) | EDMC plugin | Mar 2026 | CAPI carrier stock, then FCMaterials fallback; CargoTransfer reconciliation; orange "covered by carrier but not delivered" state; carrier free space |
| **ED Colony Architect** (ChITty98) | Node local server + React UI; MIT | Very active (v1.58.2, Sep 2026) | **LAN UI for iPad/phone**; expansion scouting scores; chain planner (≤15 ly hops); system orrery and 2D map; carrier cargo ledger replayed from journals; **EDDN proximity radar / threat watch**; mining; voiced personas (optionally generated by a local LLM CLI) |
| **EDCT** (Kailoren) | WPF; MIT | Brand new (2026-10-10) | System plan with slots; cargo hauling with trips at ship and carrier capacity; best-stocked hub station from EDDN data; transparent overlay with Loading/Unloading modes; imports DaftMav sheets and Raven Colonial JSON |
| **Elite Dangerous Warboard** (Mirooz) | JavaFX; MIT; Windows + Linux | 2026 | Architect panel on an orrery; **optimal multi-commodity market search**; carrier CAPI grid aligned to needs; colonisation target finder |
| **anthonylangsworth/Colonisation** | .NET CLI; GPL-3.0 | 2025–26 | Ranks colonisation targets for a minor faction from EDSM dumps (body, landable, ring and terraformable scoring) |
| EDDiscovery 19.x | Desktop panel | Active | Colonisation panel (progress %, hide completed) |
| EDCoPilot | Desktop | Active | Colonisation Progress layout; crowd-shared deliveries; architect-published system content |

**Gaps across the landscape (opportunity for Elite Nexus)**
1. **No tool covers the whole loop.** Target search, claim chain, planning/optimisation, hauling logistics, carrier stock and team sync are split across Raven Colonial, EDCPS, Spansh, sheets and 3–4 trackers. **Running two trackers at once conflicts** (the Raven Colonial About page says to disable colonisation in one of them).
2. **Build type and slot counts are guessed.**
   - ODET infers type from the commodity profile; Raven Colonial uses a slot heuristic.
   - A shared, versioned rules dataset (facility catalogue + point escalation + economy links) with community corrections is a clear need.
3. **Carrier stock drift.**
   - CAPI tokens last ~25–30 days and are rate-limited; squadron-carrier transfers are invisible.
   - This is the top complaint class (ODET #13, SrvSurvey #525/#560/#222, BGS-Tally stale cargo).
4. **Open requests**:
   - per-project × per-carrier commodity grid (RC #5);
   - burn-up chart (#17);
   - prominent unlock/effect display (#19);
   - "what-if" economy prediction (#2);
   - remote/web monitoring;
   - native Linux;
   - robustness to new game screens (Operations broke ODET and SrvSurvey).
5. **Target finding is tedious.** Players cross-reference Spansh system and body searches, or parse Spansh dumps by hand. Spansh caps results at 10,000.

### 3.12 Voice and AI assistants beyond EDCoPilot

#### EDDI (EDCD/EDDI)
| | |
|---|---|
| Repo | https://github.com/EDCD/EDDI (526 stars). Main maintainer in 2025–26: Tkael |
| Tech | C# / .NET 8 (since 5.0), a separate process from VoiceAttack. Cottle templating, NAudio |
| License | Apache-2.0 |
| Platforms | Windows only. A Linux request (#1976) was closed; a fork discussion (#2773, Jan 2026) is open |
| Latest | **5.0.5 (2026-08-03)**. 5.0.6 is on develop (Rhino SRV, GameModeChange, material editor). 5.x requires **VoiceAttack 2** |

**What it does:** an event-driven speech responder and VoiceAttack data bridge.
- **Speech responder**
  - Per-event scripts in **Cottle templates**, grouped into shareable **personalities**. Customised scripts survive upgrades as backups.
  - 60+ helper functions (SystemDetails, RouteDetails, SpellOut, `PronounceForContext`…). SSML; **lexicons and phonetics**.
  - TTS: Windows voices, **Amazon Polly or Azure** (user keys), web speech providers (5.0.3), output-device choice.
- **Monitors**: Cargo, Crime, Material (**min/desired thresholds per material** with alerts), Mission (expiry warnings, mission routes), Ship (fleet, loadouts, stored modules, Coriolis/EDSY export URIs), Status, Navigation (bookmarks, route types such as nearest scoopable / encoded trader / guardian broker / farthest mission; Spansh lookups since 4.1), Galnet, Fleet Carrier (including squadron carriers).
- **Coverage**: Powerplay 2.0 events, colonisation events, a factions table.
- **Uploads**: EDDN, EDSM and Inara responders; CAPI OAuth.
- **VoiceAttack bridge**: hundreds of variables plus `((EDDI <event>))` commands.
- **Most-requested (open)**:
  - internationalisation (#523);
  - **AI/neural voices** (#2691; PR #2843 adds an OpenAI-compatible TTS endpoint);
  - more event data (#2580);
  - **science responder using Canonn data** (#1340);
  - multiple game accounts (#505);
  - BGS monitor (#177, open since 2017);
  - journal rescan (#2548; no history backfill today);
  - carrier cargo (#2418);
  - closed captions (#2519).
- **Friction**:
  - The VoiceAttack 1→2 break.
  - CAPI callback launching a second instance (#2850).
  - Double speech when run both standalone and as a plugin.

#### COVAS:NEXT ("Elite Dangerous AI Integration")
| | |
|---|---|
| Repo | https://github.com/RatherRude/Elite-Dangerous-AI-Integration (116 stars). Docs on GitHub Pages |
| Tech | Python backend (PyInstaller) + **Electron** UI; Linux via Flatpak (experimental) |
| License | MIT |
| Latest | **v4.3.0 (2026-09-17)**. About 40 releases in 16 months; last push 2026-10-11 |
| Status | **The de-facto LLM "AI COVAS" of 2025–26.** EDCoPilot's author recommends it, and the two integrate both ways |

- **Pipeline**: STT (voice activation or push-to-talk; OpenAI or **local Whisper/Parakeet**) → LLM (OpenAI default; Gemini, OpenRouter, Mistral, **Ollama/LM Studio**) → TTS (**Edge free default**, OpenAI, OS voices, Kokoro/Supertonic/Pocket-TTS clones, ElevenLabs).
  - The **fully local "AIServer"** is in beta.
  - Typical cloud cost ≈ $0.10/hour.
- **Agent design**:
  - A main LLM plus a **search sub-agent** with typed tools:
    - external: Galnet, Spansh system/station/body finders;
    - internal: engineers, blueprints, materials, stored ships and modules, carriers.
  - Long-term memory "Logbook" (embeddings).
  - Exposes LLM **cost and latency metrics** to the user.
- **~80 actions**: ship, SRV, on-foot and SLF controls; galaxy-map plotting; chat; "get visuals" screenshot to a vision model.
  - Bindings parser with collision detection.
  - HOTAS-only binds are unusable.
- **Reactions matrix**: every event is set to *speak*, *know silently* or *hide*, per character, with bulk editing and context suppression (mining, on foot, supercruise). An idle-commentary timer.
- **Surfaces**:
  - Desktop overlay.
  - **Native VR overlay via a custom OpenXR API layer.**
  - **Remote web UI with QR pairing.**
  - **Generative HUD widgets** created by the LLM.
- **Extensibility**: Python plugin API (events, actions, tools, model providers); Quests editor.
- **Pain points**:
  - **Cost and confusing API keys** (a ChatGPT subscription does not cover the API).
  - Weak free tiers; latency.
  - Tool-calling unreliability with non-OpenAI models (#214, #272, #297).
  - Antivirus false positives.
  - Event spam (#179).
  - **Stuck keys (#206)**.
  - Mic feedback loop.
  - **v4.2.0 turned all actions on by default** (a safety footgun).

#### Others in the voice/AI space (2025–2026)
- **VoiceAttack v2** (Steam, Mar 2025, ~$12) + **HCS VoicePacks** (ASTRA 2.0, ~£22): still the speech-input layer for EDCoPilot and EDDI. Windows only. **Users complain that these voice tools talk over each other.**
- **Elite Intel ("Vega")** (SudoKrondor; Java 21; ~141 stars; V1.0 June 2026, near-daily builds):
  - Offline or hybrid LLM ship computer (Parakeet STT, Kokoro TTS, LM Studio or cloud LLMs).
  - **Per-ship voice and personality.**
  - **Typed fallback "@Vega …" in in-game chat** when STT garbles names.
  - Features: massacre stacking, carrier management, colonisation hauling.
  - License conflicting (CC0 vs CC BY-NC-SA).
- **Directive 47 (d47)**: .NET 10 + Avalonia, MIT. Its issue #11 is a useful mission-UX competitive analysis that proposes a "speaking mission board":
  - hand-in reminders;
  - redirect "plot it";
  - feasibility check on accept (cargo, cabins);
  - delivery checklist ticked by CargoDepot;
  - 1 h / 10 min expiry warnings.
- **Copiloto Elite** (Spanish): offline wake word (Whisper/Vosk), optional LLM, **local web panel reachable from a phone**.
- **EliteVA → EliteAPI** (Somfic): VoiceAttack plugin, archived Dec 2025 and merged into EliteAPI (MIT .NET journal library).
- **Stream Deck**: the original mhwlng plugin was archived Mar 2025. Active forks (SlayerNZ) add Spansh neutron buttons, planetary heading readouts and an exobiology 3-scan tracker on Stream Deck+ dials.
- **EDNexus (Signal-Thread-LLC)**: see section 3.15. It is a **direct "does-it-all" competitor with a near-identical name.**

**Takeaways for Elite Nexus voice design**
1. **Use a hybrid voice architecture.**
   - Deterministic, scripted callouts (cheap, instant, predictable) come first.
   - An optional LLM layer goes behind an OpenAI-compatible provider abstraction, with cost and latency shown.
2. **Own speech input.**
   - Bundle local STT (Parakeet/Whisper via ONNX) with a grammar fast path for commands and system names.
   - Add a typed chat fallback.
   - This removes the paid-VoiceAttack barrier.
3. **Own TTS supply.**
   - Ship a bundled local neural voice (Piper/Kokoro class) and treat cloud voices as optional.
   - The free Edge endpoint broke in Oct 2026.
4. **Use one speech arbiter.**
   - A priority queue with ducking.
   - A three-state reaction matrix per event: speak, know silently, hide.
   - Situational suppression (game mode, Operations, combat, mining, on foot).
   - Expose it as a local API so other assistants can route speech through Nexus instead of talking over it.
5. **Keep actions opt-in and safe.**
   - Key presses come only from `.binds`-derived mappings.
   - Each action is enabled individually, with visible acknowledgement and a stuck-key watchdog.

### 3.13 Ship builders: EDSY and Coriolis

**EDSY (edsy.org; taleden; plain-JS single-page app; CC BY-NC 4.0; v4.24.1.2, 2026-09-23)** is the accuracy leader. **New hulls usually arrive within days:** Corsair 2 days after launch; Lynx 5 days *before* launch; merc modules in Aug–Sep 2026.

- **Builds:**
  - every hull;
  - saved builds and a saved-module library in browser storage;
  - **built-in pre-engineered, CG, merc and tech-broker modules**;
  - slot filters; drag-and-drop module swap;
  - power priorities;
  - engineering with roll presets (50/80/100%);
  - **"Experimental Mode"** for what-if builds that can't be fitted in game.
- **Stats panels:** totals (mass, fuel, cargo, power, rebuy); navigation (jump ranges incl. laden/unladen/max/total, boost); handling; **thermals (time to overheat)**; shields; armour; weapons (burst/sustained DPS, AX); price.
- **Analysis (retrofit planner):** turns a base build into a target build as **Buy/Sell/Convert/Engineer/Experimental steps** with materials and credits totals. Exports a TSV checklist, materials list, JSON, Inara outfit-search link and EDOMH link.
- **Other tools:** query tool for abbreviations; insurance and discount options; 12 languages; full backup and restore.
- **Interop:**
  - **imports:** EDSY JSON, SLEF, journal Loadout/EngineerCraft, CAPI profile, `#/L=` links, live CAPI login;
  - **exports:** `#/L=` hash, shortlinks, SLEF, Loadout, text, Inara, `edomh://edsy/?`;
  - **EDMC opens `edsy.org/#/I=<encoded Loadout>`**;
  - it explicitly rejects Coriolis imports.
- **Gaps:** no opponent or time-to-kill analysis; no module search (#37); no keyboard shortcuts (#36).
- **Pain points:** long-URL limits and dying shortlink services; merc module data was incomplete (#58, Sep 2026). Users say it is more accurate than Coriolis but has "cryptic abbreviations".

**Coriolis ("EDCD Edition"; coriolis.io; React 19; code MIT, data is Frontier IP)** has been revived since 2024 by Brighter Applications (v4.1.12, 2026-09-26). Several 2026 commits are co-authored by an AI assistant.

- **Analysis:** combat-focused.
  - **Offence:** time to wear down a chosen opponent.
  - **Defence:** shield longevity and module protection.
  - **Profiles:** graphs of speed vs mass, jump range vs cargo, DPS vs range.
- **Comparing and searching:** compare builds; fuzzy module search; favourite engineered modules.
- **Integrations:**
  - materials/requirements panel;
  - EDOMH and EDEngineer export;
  - SLEF import and export (export landed in 2026).
- **New: CMDR-Coriolis** (cmdr.coriolis.io, alpha, closed-source Django):
  - accounts import your fleet and materials via CAPI;
  - **journal ingestion APIs** (`POST /api/journal/` raw lines; `POST /api/sync/` from an EDMC plugin) driven by an API key;
  - builds show what you still need.
- **Gaps:**
  - **no heat model (#105, open since 2017)**;
  - power-distributor units off (#809);
  - **lags on new hulls**: Type-8 ~2.5 months after EDSY; Type-11 and Caspian only in Feb 2026.
- Added Google Analytics in May 2026.

**Interop formats Elite Nexus must speak:**
- journal `Loadout` JSON;
- **SLEF** (Inara-defined: header `appName`, `appVersion`, `appURL` plus Loadout data; jixxed opened a "slef-specification" stub in Aug 2026);
- EDSY `#/I=` import links (journal Loadout) and `#/L=` build hashes;
- Coriolis `/import?data=`;
- EDOMH `edomh://edsy|coriolis/?` wishlists (base64 of deflated JSON);
- the EDEngineer-style local REST shopping list;
- the CMDR-Coriolis journal API.

### 3.14 Specialist tools and web services (BGS, Powerplay, carriers, mining, trade, maps, utilities)

These are the tools players run alongside desktop apps. Elite Nexus must either absorb their features or integrate with them.

#### BGS
| Tool | What it offers | Status / notes |
|---|---|---|
| **Elite BGS** (elitebgs.app; API v5; BGSBot) | Faction presence (influence, happiness, active/pending/recovering states with trend, conflicts with days won and stake), system data, tick endpoint, Discord bot. Fed by its own EDDN listener | **Maintenance mode.** Snapshots still update. History query failed in testing. Repos idle since Nov 2024. **The tick detector has been stale since 2026-07-19** |
| **edservertick.de / EDCAS** | **Per-system tick prediction**: each system ticks at its own consistent time, so it gives a predicted window with a "tick imminent" state and history by time of day. Built from EDDN influence changes | Active. Key insight: a single "galaxy tick" no longer fits the data |
| **ETN, Elite Tactical Network** (elite-bgs.store, Mar 2026) | Multi-squadron web portal + companion app + Discord bot. Hex-card system grid; objectives (RAISE / LOWER / EXPAND…); per-CMDR activity since tick; leader orders; daily-cap-aware plans; rule alerts (jump spikes, rival surges, foreign carriers); PP2 tab; AI briefs | **Privacy red flag:** a "hash correlator" links anonymised EDDN uploader IDs to named CMDRs. Do not copy |
| **EDBGS** (codeberg nola/EDBGS, .NET) | Journal scan to a grouped Discord report via webhooks. Long reports are split. **Versioned report header** that bots can parse. Filters double-redeemed vouchers; CZ objectives; negative/secondary INF handling | 0.4.3 (Sep 2024) |
| BGS-Tally, ODET | See 3.3 and 3.8 | Active |
| Inara | Faction and squadron pages, nearest faction by state | Inara could not be fetched |

**Squadrons value:**
- exact per-system tick timing;
- influence and state history with margin alerts;
- conflict tracking (days won, stakes);
- per-CMDR tallies since the tick, in paste-ready form;
- objectives and orders;
- rival-surge warnings;
- daily-cap awareness;
- correct handling of failed or negative INF.

#### Powerplay 2.0
Mechanics (Ascendancy, 31 Oct 2024):
- The weekly cycle resolves **Thursday 07:00 UTC**.
- System states are Exploited, Fortified and Stronghold. An Exploited system needs a Fortified system within 20 ly or a Stronghold within 30 ly.
- Merit values were rebalanced through 2025.
- Operations (Jul 2026) award merits in Powerplay mode.

Tools:
- **EDPowerPlay** (edpowerplay.com, beta, live from EDDN):
  - cycle countdown;
  - trends for all 12 powers;
  - undermining targets;
  - a **"vulnerable" board** with a projected end-of-cycle mark and CP sparklines;
  - a **cascade / "blast radius" analysis** (what falls if this Fortified or Stronghold system drops);
  - a PP mining planner with a quote-age filter.
- **EliteMeritTracker** (EDMC plugin, v0.4.4, Sep 2026):
  - session, system and shiplocker views, with Powerplay data bags credited to their source system;
  - Discord and CSV export.
  - **Correctness lesson:** use the change in `TotalMerits`, not the sum of `MeritsGained`. Summing overcounted by 5.6%; the delta method matched the server at 363 of 363 checkpoints. Hold duplicate awards until the server total confirms them.
- **EDMC-PowerPlayProgress** (see 3.3); **ODET** (time-window attribution).
- **Other trackers:**
  - **PowerPlay Assistant** (elite.niceygy.net): guided task steps per activity, megaship finder, conflict finder, accessibility-minded design.
  - **Powerplay Rankings** (k5elite.com).
  - EDPPMT; EDPM (merit mining spots from a Spansh dump).

**Frontier-forum requests (late 2024 – mid 2025):**
- reports of systems close to being lost or upgraded;
- show merits and control points instead of % multipliers;
- Powerplay filters in the Ardent API (#4, #10).

#### Fleet carriers
- **EDCM** (Elite Dangerous Carrier Manager; Python; GPL-3; Windows and Linux; **v1.6.5, 2026-10-08**). The leading carrier tool.
  - Jumps tab with countdowns and position on PTN's numbered ladder.
  - Trade tab with Discord-ready **trade posts**.
  - **Finance tab showing "upkeep funded for N weeks".**
  - Services and misc tabs; decommission warning.
  - "Booze Cruise" assist.
  - Notifications by popup, sound or Discord.
  - **Crowd-sourced jump-timer statistics** (jump timers became variable).
  - Notes.
  - Pain points:
    - **"Ghost orders"**: carrier order fills by other players are not journaled.
    - The trade post fails when EDSM is down.
    - Balances refresh only when the carrier menu is opened.
    - Multi-PC users must sync journals.
- **CATS / CTS** (autojump plus tritium restock via Spansh routes). They **drive game inputs**, which CATS's own README says breaks Frontier's ToS. **Lesson: offer reminders and timers, never automation.**
- **Elsewhere:**
  - GalaxyGPS and NeutronDancer (CAPI carrier data, Spansh FC routes, countdowns).
  - Fleet Commander (Observatory).
  - BGS-Tally's FC window.
  - ODET's FC view.
- **Coordination and accounting:**
  - **Elite Carrier Job Board / CarrierBoard plugin**: shared loading jobs with a paste-from-Discord parser.
  - **PTN (Pilots Trade Network)**: 400+ carriers; guides; carrier calculator.
  - FCOC (Discord only).
  - fleetcarrier.space (via the FCMS plugin).
  - **Elite Cargo Log**: upload journals to compute deliveries, payouts and profit; shareable result links.
  - **EDAPITool**: CAPI carrier orders to CSV or Sheets, polled every 15 min.
  - EDG: tritium hotspots.
- **Squadron carriers (Vanguards, 19 Aug 2025):** Briefing Room, Treasury (shared credits) and Bank (shared materials and ships), with installable vendors added later.

#### Mining
- **EliteMining** (Viper-Dude; Python exe; GPL-3; **v5.4.2, 2026-09-27**). The de-facto standalone mining app.
  - Live stats with a cargo sidebar; sessions auto-start and prompt to end.
  - Material tracking by grade; graphs.
  - **Prospector announcements with presets.**
  - Text overlay.
  - Reports as HTML/CSV/PDF plus **shareable PNG "Mining Cards"**; Discord.
  - **Hotspot finder** (80k preloaded plus Spansh; overlaps and RES; auto-search on jump; "Find Best Mining System").
  - **Market tab via Ardent.**
  - Mining missions.
  - Carrier tab.
  - Optional VoiceAttack sequences.
  - Pain points: overlay offset above 150% scaling; AV false positives; **may hit Ardent/Spansh rate limits.**
- **EDMC-Mining-Analytics:** see 3.3. Issues: "plugin is laggy" #54; log grows without limit #79.
- **EDRockMaster Companion** (MS Store; Python + Svelte):
  - mining alerts and stats, combat counters, trade profit/hour, engineering lists;
  - **local-only by default**, with secrets stored in Windows Credential Manager.
- **Other mining tools:**
  - TritiumCalculator.
  - EDTools (edtools.cc): miner's tool, overlap lists, mission stations.
  - edmining.com: user-submitted spots, unverified.
  - EDMM: Stream Deck asteroid library.
  - QuarterMaster: paid, includes an AI copilot.
- **New in Sep 2026: Rhino SRV surface mining.**
  - **EDSMT:** overlay with compass, patch scope, nearest targets and telemetry; 2 km sweep coverage; **rig radar warning at 3.5/4.8 km before rigs are lost at 5 km**; earnings log.
  - **Rhino Surface Mapper:** deposit and rig maps, coverage, nearby buyers.
  - **This is an open niche.**

#### Trade & markets
- **Ardent Insight** (ardent-insight.com; API v2, v6.4.1; AGPL).
  - Scale: 197M systems, 695k stations, 20.6k carriers, **36.5M orders, about 2.2M updates/day**.
  - Endpoints: commodity reports; top 100 importers/exporters with filters (`maxDaysAgo`, carriers); nearby systems; **20 nearest stations offering a service** (by pad size); system markets.
  - Anonymous and **without rate limits**.
  - **Weekly SQLite dumps.**
  - Requests: Powerplay filters, Community Goals, engineers.
- **Spansh Trade Planner** and station search (commodity, ship and module prices). A redesigned trade router is in development.
- **Inara**: commodity search and A-to-B/loop routes with filters. Routes go stale or sell out fast. Inara blocks automated access.
- **Trade Dangerous v13 "Reforged"** (Jul–Sep 2026): rewritten **local multi-hop solver** with a native GUI, ship profiles and journal import. Issues: import failures (#268, 47 comments); GUI crash with 2,000+ routes (#299).
- **Players still miss EDDB** (shut down 2021) for multi-hop trade planning; "Inara is not as good".
- **Newer:**
  - **Gold Detector** (Discord bot that finds "Infrastructure Failure" gold arbitrage).
  - EDMC-MarketScout (local LAN dashboard).
  - Cargo Manifest (prices from EDDN daily averages).

#### Exploration and map web services
- **Spansh**
  - **Plotters:** neutron; galaxy (exact, with refuel and injections); Road to Riches; **Expressway to Exomastery**; tourist; trade; fleet carrier; colonisation.
  - **Searches:** bodies/POI, signals, systems (states, Powerplay), stations; neutron map.
  - **Saved bodies** can be excluded from R2R and exo routes.
  - Nightly dumps plus deltas.
  - **Planned:** a desktop version that records route progress automatically, a better trade router, an engineering router.
  - Scale: about 30M requests from about 20k users per month.
  - Pain points: slowness (author's post); **10,000-result cap**.
- **EDSM**
  - Still the default map/log backend for EDMC and EDD; now behind Cloudflare.
  - **33 hours of downtime in Mar 2026**; response times rising.
  - Logs API limited to 360 requests/hour per user.
  - Bodies dump covers only the last 7 days.
- **EDAstro**
  - Maps and charts; galactic records; **GEC** POI catalogue; EDDN real-time map; travel-history video creator.
  - 205M systems.
  - EDMC 6 has a built-in EDAstro sender, which deprecates the plugin.
- **Canonn**: codex/bio/geo research, signals site, Guardian maps, cloud API (nearest codex, prior scans).
- **EDGIS** (edgis.elitedangereuse.fr): coordinate lookup **including predicted coordinates for uncharted systems**, nearest neutron, system maps. Used by BGS-Tally and ETN.

#### Utilities
- **Bindings**
  - **EDRefCard** was **archived Sep 2024**; continued as edrefcard2 (Python 3.6 + ImageMagick 6).
  - **No bindings editor was updated in 2025–26.** This is an open gap.
- **EDHM-UI** (276 stars): HUD colour themes. Needs a fix after almost every game patch.
- **min-ed-launcher** (444 stars, v0.13.0 Jun 2026)
  - Launcher bypass, autorun/autoquit.
  - **Starts and stops companion tools with the game.**
  - Multi-account; Steam Deck Flatpak-EDMC fix; streaming aliases.
  - Open issues: run helpers inside the Proton prefix (#183); delay helpers until the game loads (#187).
- **EDU (Elite Dangerous Unified App)** (v0.5.0, Jul 2026)
  - Imports tools from EDCodex with update badges.
  - **Launch sequences exportable as .bat/.sh.**
  - Embedded browser tabs with dark mode and ad-block; **pin any tab as an always-on-top overlay.**
  - Chat commands; local webhook API.
- **Discord Rich Presence**: VeeLume tray app; the EDMC plugin is stale.
- **Streaming**: EDLD streamer overlay (X11). **No maintained OBS browser-source overlay that reads the journal was found.** This is an open gap.
- **VR**
  - EDVR Unofficial Patch (rendering fixes).
  - Elite VR Cockpit (virtual HOTAS).
  - ModernOverlay's SteamVR mode.
  - SrvSurvey OpenVR overlays; COVAS:NEXT OpenXR layer.
- **Second screens**
  - **LunaPanel** (Sep 2026): phone or tablet control panel whose **buttons read your real keybinds**, with state-aware macros and HUD-matched colours.
  - Matric; EDCoPTER; ED Colony Architect's LAN UI.

**What a native all-in-one does better here.** Run a local **EDDN subscriber** (Ardent shows about 25 messages/s, trivial for Rust) feeding a local store. That gives:
- BGS history and per-system ticks;
- Powerplay state with vulnerable and cascade analysis;
- fresh markets for trade, mining sell and colonisation sourcing;
- carrier market changes.

Seed it from Ardent's weekly SQLite and Spansh's populated/stations dumps, and use EDSM, Spansh and Ardent as circuit-broken fallbacks. Queries become instant, offline-capable and free of rate limits.

### 3.15 Direct "all-in-one" competitors and 2026 trends

Elite Nexus is not the only project trying to consolidate the toolchain. Watch these:

| Project | Stack / platform | Scope | Status | Why it matters |
|---|---|---|---|---|
| **EDNexus** (Signal-Thread-LLC/EDNexus) | .NET 10 + Avalonia 12; Windows and Linux/Steam Deck (Flatpak) | "Single, does-it-all commander console". Colonisation, trade (Spansh/EDSM), engineering, exobiology, materials, missions with a **War Board** for massacre stacks (Sep 2026), rank trackers, mining, radio, Discord Rich Presence, Twitch card, click-through overlay, opt-in spoken callouts; plugin SDK in progress | 2 stars, 311 commits, active | **Name collision with "Elite Nexus"**: check trademark, search and SEO before naming is final. Same thesis, different stack |
| **EDCAS** (arne-fuchs/edcas-client) | **Rust** TUI client + Rust/PostgreSQL/ZeroMQ server; WASM + xterm.js build | Explorer, Mining, Commander, Ship, Engineering tabs; galaxy and trade search; BGS; construction depots; Galnet. Uploads to EDDN and its own server (on by default). Server provides influence and population history, **per-system tick** (powers edservertick.de) and trade loops | Public repo "OUT OF DATE"; development moved private | The closest technical cousin (Rust + EDDN + local store) |
| **EDLD** (drworman/EDLD) | MIT; Windows, Linux, macOS; native window + **terminal view over SSH** + plain feed | Ledger from 27 event types, "At-Risk Holdings", session tabs including Powerplay, CAPI, uploads, plugins, streamer overlay (X11) | 2026 | Cross-platform, headless-friendly design |
| **ED Colony Architect** | Node SEA server + React; Windows; LAN UI | Colonisation super-tool (see 3.11) plus mining, BGS conflict finder, **EDDN threat radar**, commander log | v1.58, very fast cadence | Shows demand for a LAN/tablet UI and EDDN-powered awareness |
| **Elite Dangerous Warboard** | JavaFX; Windows + Linux | Massacre stacking, mining with prospector overlay, exobiology, colonisation, Spansh routes, EDDN upload | 2026 | Breadth on a non-Windows-only stack |
| **EDRockMaster Companion** | Python + Svelte via pywebview; MS Store | Mining, combat counters, trade/hour, engineering lists, on-foot inventory | 2026 | **Local-only by default**, which it uses as a selling point |
| **EDU (ED Unified)** | Windows + Linux | Launcher and catalogue for other tools, embedded web tabs pinnable as overlays | v0.5.0 | Shows the "launch everything" pain; Nexus should make it moot |
| **Elite Intel / COVAS:NEXT / Elite AI Bridge / EDGPT (MCP) / QuarterMaster** | Various | AI copilots | Very active | Expect users to want Nexus to be the **data source** for their AI assistant (MCP/WebSocket) |
| Homebrew 2026 companions (ED Outrider, E.D.E.N., ED Live Dashboard, ED Linux Dash, EDMC-MarketScout) | Mostly web/LAN, often AI-built | Exploration and trade dashboards, Linux- and tablet-first | Many posts removed by r/EliteDangerous moderators | **Proof of unmet demand**: Linux-native, tablet UI, local-first, one app |

**2026 trend summary**
1. **LLM copilots are the dominant new category.** COVAS:NEXT leads; Elite Intel has offline mode; there are MCP bridges.
2. **Linux and Steam Deck support is now expected.** Most new tools ship Linux builds, and Proton friction drives people to build their own tools.
3. **Second screens are moving to the LAN and web.** Examples: COVAS:NEXT QR pairing, ED Colony Architect, EDCoPTER, MarketScout, LunaPanel.
4. **Local EDDN subscribers power new web dashboards**: EDPowerPlay, edservertick, ETN, EDCAS.
5. **Activity niches are created by game updates**: colonisation in 2025, Operations and Rhino surface mining in 2026, squadron carriers in 2025.
6. **"Local-only, nothing uploaded" is now a marketing point** for some tools. Other tools de-anonymise EDDN uploaders (ETN). Privacy posture is a differentiator.
7. **Many new tools are openly AI-built.** Examples: ModernOverlay, Mining-Analytics, Trade Dangerous v13, ED Outrider. Expect more churn and more short-lived projects.

---

## 4. Merged feature matrix

This section de-duplicates every feature found across the audited tools into 167 rows grouped by area.

**Codes and marks**
- **Tool columns**: EDD, EDMC, OMH, OBS, EXB, ECP, ODT, SRV.
- **Marks**: `Y` built in · `P name` via that plugin · `~` partial · blank = absent.
- **Others** names additional tools or web services that offer the feature.
- **Pri** is the recommended Elite Nexus priority: **P0** = MVP, **P1** = next, **P2** = later.

**Cross-cutting principles.** These apply to every row and are not repeated in each note:
1. **One event-sourced local store.** Journals, companion files, CAPI snapshots and EDDN messages all go into it. Every module reads it; no module re-parses on its own.
2. **Local-first, web-second.** A local galaxy, station and market index (Spansh/Ardent dumps + live EDDN) answers queries instantly and offline. Web APIs are circuit-broken fallbacks.
3. **Every surface comes from one component model.** The same widget renders on the desktop, the overlay, the LAN tablet view, the voice channel and the local API.
4. **Hot-updatable, signed data packs.** Ships, modules, blueprints, bio rules, traders and colonisation facilities are data, not code.
5. **Mode-aware state.** Data is partitioned by commander FID, galaxy (Live/Legacy/Beta), game mode (Main/Operations/CQC) and vehicle.


### Commander / Dashboard

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Live commander summary (CMDR, credits, ship, system, station, docked state) | Y | Y | ~ | P Helm | ~ | Y | ~ | ~ | EDDI vars, COVAS | **P0** | One event-sourced state model feeds every widget, updating in under 100 ms from Journal + Status.json. Provider links (EDSM/Inara/Spansh) on every name, as EDMC does. |
| Ranks & progress (combat…exobiologist, navy, PP rank) with % to next | Y | ~ | ~ |  |  |  |  |  | Inara (web), EDMC InaraProgress | **P1** | Progress bars with an ETA estimated from recent gain rate, plus history graphs from the event store. |
| Reputation (superpowers, minor factions) | ~ |  |  |  |  |  |  |  | Inara (web) | **P2** | Derive from Reputation and mission events; show trends and the factions you work with most. |
| Live ship status (fuel, hull, cargo, current/max jump range, rebuy) | Y | ~ | Y | P Helm |  | Y |  |  | COVAS, EDDI | **P0** | Compute jump range from Loadout + Cargo + fuel using in-game-panel-accurate maths (EDOMH matches the panel). Warn when rebuy exceeds credits. |
| Typed live Status.json stream (flags, GUI focus, legal state, lat/long) | Y | P hooks |  | Y |  | Y |  | Y | EDDI Status monitor, COVAS | **P0** | Parse once and debounce into a shared bus. Today every tool re-reads it on its own. |
| Multi-commander (per-FID data, Live/Legacy split, rename migration, multibox) | Y | Y | Y | ~ |  | ~ | Y | ~ |  | **P0** | Key all data by FID + galaxy + game mode. Migrate renamed CMDRs in one click (ODET). Support simultaneous instances (EDMC cannot; SrvSurvey can). |
| Customisable workspace (dock, tabs, splitters, reorder, saved layouts) | Y | ~ |  | Y |  | Y | ~ |  |  | **P0** | EDD-level flexibility, including its profile auto-switch (docked, ship), but with sane defaults and discoverable UX. EDMC still cannot reorder plugin panels (#1792). |
| Pop-out windows, multi-monitor, always-on-top | Y | ~ |  | Y | Y | Y | Y | Y |  | **P0** | Any widget pops out and remembers monitor, position and scale. Never steals focus. |
| Themes, transparency, UI scale, HUD-colour matching, colourblind modes | Y | Y | Y | Y | Y | Y | Y | Y | COVAS reads GraphicsConfigurationOverride/EDHM | **P1** | Build the palette automatically from the player's HUD matrix. Offer colourblind presets and per-window scale. |
| Session summary (earnings/hour, jumps, scans, kills, P&L this session) | ~ | P HourlyIncome/StreamHUD |  |  | Y trip | Y | ~ mining |  | EDRockMaster | **P1** | Detect sessions automatically, show P&L across all activities, and produce a shareable session card (image/Discord). |
| Notification centre / alert history |  |  |  | P Aggregator/Archivist |  | Y voice+event log |  |  |  | **P1** | Unified notification bus with history and replay. Each alert type routes to popup, voice, overlay, webhook or LAN UI. |

### History & Stats

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Full journal backfill on first run (all CMDRs, all years) | Y |  | ~ | Y Read All | Y | Y History Refresh | Y | Y | BioScan/ExploData background import | **P0** | Parallel, resumable, incremental Rust ingest that indexes years of journals in seconds, with progress and cancel. Never block the UI. Read All and History Refresh are slow and confusing today. |
| Travel history log (jumps, docks, distances) with search/filter | Y |  |  |  | Y | Y |  | Y journeys |  | **P0** | Instant structured and full-text search across every event, with jump-to-map and jump-to-system actions. |
| History "time machine" (view any panel's state at any past event) | Y |  |  |  |  |  |  |  |  | **P2** | Cheap with an event-sourced store. Answers "what did I have when…" questions. |
| Raw journal event viewer (filters, JSON view) | Y | P LogEventMiner |  | P Archivist |  |  |  |  |  | **P1** | Power-user view over the same store; doubles as a debugging and support tool. |
| Statistics & charts over time (distance, jumps, per ship, per activity) | Y |  |  |  | ~ | Y |  |  |  | **P1** | Charts over any period from the event store; compare periods; per-ship and per-activity breakdowns. |
| Credits ledger / net worth over time | Y |  |  |  |  |  |  |  | EDLD | **P1** | Ledger with categories, plus assets at risk (unsold data, cargo, rebuy). |
| Combat log (kills, bounties, bonds by faction/ship type) | Y |  |  |  |  | Y |  |  |  | **P2** | Feeds BGS and Powerplay attribution. |
| Captain's log / notes | Y |  |  |  |  | Y |  | Y | COVAS logbook (AI memory) | **P1** | Notes attach to system, body or station and appear when you return. |
| Screenshot pipeline (convert, rename by system/body, metadata, gallery) | Y | P EDMC-Screenshot |  | P Photographer |  | Y |  | ~ albums |  | **P2** | Auto-tag with full context, embed metadata, and show a gallery tied to the travel history. |
| Export any table (CSV/XLSX/JSON) | Y |  | ~ | Y |  |  | ~ |  |  | **P1** | Generic export built into the table widget. |
| Personal first discoveries / footfalls / mapped (Codex Bingo-style) | ~ | P BioScan |  |  |  | Y |  | Y |  | **P1** | Rebuild from your own journals plus discovery flags (`WasDiscovered`, `WasFootfalled`). |

### Maps

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| 3D galaxy map (history, routes, bookmarks, regions) | Y |  |  |  |  |  |  |  | EDAstro (web), Raven Colonial 3D, Spansh | **P1** | wgpu renderer streaming the local galaxy index: millions of points at 60 fps, click to plot a route, colonisation and Powerplay overlays. |
| 2D galaxy charts / region overlays | Y |  |  |  |  |  |  |  | EDAstro | **P2** | Overlay EDAstro-style heatmaps from your own data and the local index. |
| System map / orrery (visual body layout with values) | Y |  |  |  | ~ |  |  | ~ | ED Colony Architect, Warboard, COVAS | **P1** | Orrery that shows value, bio and geo icons per body and colonisation slots, with a click-through to details. |
| Local map / nearby systems with filters | Y |  |  |  | Y |  |  |  | EDGIS | **P2** | Served from the local index; works offline. |
| Planet surface radar (samples, CCR rings, ship/SRV, POIs, heading) |  | P BioScan |  | ~ |  |  |  | Y | Canonn prior scans | **P1** | Live from Status.json lat/long + body radius. Rendered identically in the overlay, the desktop and the LAN tablet view. |

### Navigation & Routing

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| In-app Spansh plotting: neutron + galaxy (exact, ship-aware, Caspian SCO-aware) | Y | P NeutronDancer/SpanshTools |  |  | Y | Y | ~ CSV | ~ | COVAS, EDDI | **P0** | Auto-fill ship data (Loadout → SLEF), plot in the background, and cache results. Pass supercharge/injection flags correctly (NeutronDancer #51). |
| Other Spansh routes: R2R, Exomastery, tourist, trade, FC, colonisation | Y | P NeutronDancer |  |  |  | Y | ~ CSV | ~ | Spansh (web) | **P1** | One route model for every source. Exclude bodies you have already scanned using your own history. |
| Route follower (auto-copy next hop, persistence, catch-up after restart) | Y | P NeutronDancer |  |  | Y | Y | Y | Y | OBS Fleet Commander/Helm | **P0** | Works for NavRoute, Spansh, CSV and manual routes. Clipboard + a galaxy-map-only "next jump" overlay. Route lock (EDEB). |
| NavRoute hop table (star class, scoopable, neutron, value, discovered-by) | ~ | P NavRoute/Route Scoop |  |  | Y |  |  | Y | EDR route HUD | **P0** | Enrich each hop from the local index plus EDSM/Spansh: value, discoverer, fuel reach with the current tank. |
| Fuel / scoop / range warnings | ~ | P Route Scoop |  | P Helm | ~ | Y |  |  |  | **P1** | Predictive ("out of range with current fuel in 3 jumps") rather than reactive. |
| Bookmarks / POIs with groups, notes, sharing | Y |  |  |  |  | Y | ~ | Y | EDDI, Canonn POIs | **P1** | Bookmark groups double as routes (EDCoPilot pattern). Import/export packs. |
| Nearest service finder (IF, trader by type, broker, UC, Vista, shipyard) | Y Spansh Stations | P Canonn/EDR | Y |  |  | Y | ~ |  | EDDI route types, Ardent API, COVAS | **P0** | Local station index (Ardent/Spansh dump + EDDN) gives instant offline answers, ranked by jumps + Ls + pad + permits, with one recommendation and alternatives. |
| Surface navigation (lat/long target, compass, bookmarks) | Y | P Canonn/EDR/EDPlanetNavigator |  |  |  | Y | Y | Y |  | **P1** | Heading arrow plus glide-angle hint (SrvSurvey) in the overlay. |
| Docking pad locator (station diagram, assigned pad) | ~ v20-dev | P LandingPad/EDR |  |  |  | Y |  |  |  | **P1** | Overlay card on DockingGranted covering starports, outposts, carriers and Dodec layouts. One of the most-loved small features. |
| Distance / straight-line / boxel & sphere search helpers | Y | P DistanceCalc |  |  |  |  | Y | Y | EDGIS | **P2** | Built on the local galaxy index, so sphere and boxel queries run offline and instantly. Results feed routes and bookmarks. |
| Expedition / multi-waypoint planner | Y |  |  |  | ~ |  |  | ~ |  | **P2** | Built on the shared route model. |
| Galaxy-map automation (auto-plot by simulated keys) |  |  |  |  |  | Y |  |  | COVAS, CATS (ToS risk) | **P2** | Opt-in only, with no unattended automation. Prefer clipboard + overlay. |

### Exploration & Exobiology

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| System body list with values (type, Ls, landable, terraformable, atmo, signals) | Y | P Pioneer |  | P Evaluator | Y | Y |  | Y |  | **P0** | Main exploration view, filled live from FSS/DSS events plus pre-arrival data from EDSM/Spansh. |
| Exploration value (FD/FM bonuses, efficiency, honk, current vs max-if-mapped) | Y | P Pioneer |  | P Evaluator | Y | Y |  | Y |  | **P0** | Re-implement the published formula (credited to MattG). Show current, max and "worth mapping" with per-class distance thresholds. |
| Mapping priority / efficient visiting order in-system |  |  |  | P Evaluator |  |  |  | ~ WIP |  | **P1** | TSP over orbital positions with a supercruise time model. No tool does this well yet. |
| Built-in interesting-body alerts (rings, close orbits, high-g, ELW…) | Y Discoveries | P Canonn |  | Y Explorer | Y |  |  | ~ |  | **P0** | Ship Observatory's 23 criteria as defaults, each with its own voice and overlay toggles. |
| User-authored rules (scripts or forms) + shareable packs | Y actions |  |  | Y Lua | Y forms |  |  | ~ JSON |  | **P1** | One rule engine: an expression language for the simple cases, a scripting language (Rhai/Lua) for the complex ones, a form builder for non-coders, and signed shareable packs. |
| Exobiology species/variant predictions |  | P BioScan |  | P BioInsights | Y | ~ |  | Y | ed-exobiology crate | **P0** | Own versioned rule set, tested against Canonn/EDDN ScanOrganic statistics and hot-updated. Show confidence and value ranges that narrow after DSS and each sample. |
| Bio values + first-footfall / first-logged bonus detection | ~ | P BioScan |  | P BioInsights | Y | Y |  | Y |  | **P0** | Per body, answer "is this worth landing for?" with total, potential and bonus. |
| Sample tracker with colony range, live distance, "far enough" signal | ~ | P BioScan/Artemis/Canonn |  | Y Botanist | Y | Y |  | Y | EDDI event | **P0** | Radar plus voice plus overlay from one source. Auto-track Composition Scanner scans (SrvSurvey). |
| Codex: new-for-region / never-seen flags, regional checklist | ~ | P BioScan/Canonn |  | P BioInsights/GeoPredictor |  |  |  | Y Codex Bingo |  | **P1** | Region lookup via klightspeed's map (MIT, Rust port available). |
| Unsold data at risk (carto + exo) & death-loss tracking |  | P BioScan/Pioneer/ExplorerLite |  |  | Y | Y |  | ~ |  | **P0** | Prominent "credits at risk" meter, rebuy check, and a nudge to sell before risky activity. |
| Pre-arrival knowledge (EDSM/Spansh: discoverer, body counts, traffic) | Y | ~ core EDSM icon, P Canonn |  |  | Y | Y |  | Y |  | **P1** | Show on galaxy-map target (Canonn pattern) and on every route hop. |
| Prior samples by other CMDRs / nearest known bio guidance |  | P Canonn |  |  |  | Y |  | Y | Canonn API | **P2** | Show Canonn prior-scan coordinates as ghost markers in the surface radar, cached per body. |
| Geological signal prediction |  |  |  | P GeoPredictor |  |  |  |  |  | **P2** | Same rule engine as the bio predictions. Flag possible new geo codex entries per region. |
| Galactic records / personal bests |  |  |  | P Stat Scanner |  |  |  |  | EDAstro | **P2** | Compare scans against EDAstro record files, cached locally and refreshed weekly. Near-record alerts with rarity thresholds (Stat Scanner pattern). |
| Guardian / Thargoid site tools (ruin maps, Ram Tah) |  | P Canonn |  |  |  |  |  | Y |  | **P2** | Integrate rather than rebuild (SrvSurvey is GPL). |
| Notable phenomena / HGE / POI alerts | ~ | P Canonn |  | P AstroAnalytica |  | Y |  |  |  | **P2** | Rules over FSSSignalDiscovered plus GEC/Canonn POIs, delivered through the unified notification bus. |
| Carto / exobiology sales history | Y | P Pioneer/BioScan |  |  | Y |  |  |  |  | **P1** | Derived from Sell* events in the ledger. Show where and when data was sold and what is still unsold. |

### Trade & Markets

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Current market view (from Market.json) with known-price comparison | Y |  | ~ |  |  | ~ |  |  |  | **P1** | Highlight profitable sells against the local market index. |
| Commodity search: best buy/sell nearby (pad, freshness, Ls, carriers) | Y Spansh Stations |  | ~ |  |  | Y |  |  | Inara, Spansh, Ardent (web), COVAS | **P1** | Local market index (Ardent weekly SQLite + live EDDN): instant, offline, no rate limits, freshness shown. |
| Trade route planner (single/multi-hop, loops) | Y Spansh | P NeutronDancer/UpdateTD |  |  |  | Y |  |  | Spansh, Inara, Trade Dangerous v13, EDCAS | **P1** | Local EDDB-class multi-hop solver that knows your ship. Re-checks prices live en route (EDCoPilot idea). Players still miss EDDB. |
| Trade profit tracking per trip / session | ~ |  |  |  |  |  |  |  | EDRockMaster | **P1** | Match MarketBuy to MarketSell per commodity (FIFO) for true profit per trip, per hour and per route. |
| Cargo manifest with values, mission/stolen flags | ~ | P Cargo Manifest |  |  |  |  |  |  |  | **P1** | Value cargo against the local market index. Flag mission, stolen and corrosive cargo and the value at risk. |
| Rare goods finder |  | P PowerPlayProgress | Y |  |  |  |  |  |  | **P2** | Static rare-goods dataset plus local market freshness. Plan rare loops and Powerplay rare deliveries. |
| Market watchlist (carrier loading demand, fills by others) |  | P EDHauler |  |  |  |  | Y |  |  | **P2** | Diff successive Market.json/CAPI snapshots to reveal fills (ODET pattern). |
| Community Goals tracking + tier prediction |  |  | Y |  |  |  |  |  | Inara | **P2** | CAPI `/communitygoals` plus journal CG events. Projected tier with confidence bounds (EDOMH pattern). |

### Mining

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Mining session tracker (TPH, refined, limpets, duration, auto start/stop) | Y overlay | P Mining-Analytics |  | P Prospector |  | Y | Y |  | EliteMining | **P1** | Sessions start and stop from game state; P&L uses actual sell prices. |
| Prospector analysis & callouts (thresholds, motherlode, duplicates) | Y | P Mining-Analytics |  | P Prospector |  | Y | Y |  | EliteMining, EDDI | **P1** | Overlay card plus voice, with per-commodity thresholds. |
| Hotspot / overlap finder | ~ | P Mining-Analytics | ~ |  |  | ~ |  |  | EliteMining, Spansh, EDTools | **P1** | Local ring/signal index (Spansh dump + EDDN FSSBodySignals + own scans). |
| Best sell price for mined goods | ~ | P Mining-Analytics |  |  |  | Y |  |  | EliteMining (Ardent), Inara | **P1** | Same local market index, plus Powerplay merit context. |
| Yield histograms / ring comparison analytics | Y CFD chart | P Mining-Analytics |  |  |  |  |  |  | EliteMining | **P2** | From your local prospector history: CFD charts (EDD) and ring-vs-ring comparisons, exportable. |
| Rhino surface mining (rigs, deposits, coverage, rig-distance warnings) | ~ |  |  |  |  | ~ |  | Y | EDSMT, Rhino Surface Mapper, EDDI (develop) | **P1** | New niche (Sep 2026). Rig radar with 3.5/4.8 km warnings and deposit maps in the surface radar. |

### Missions

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Mission list with expiry countdown & hand-in reminders | Y |  |  |  |  | Y | Y |  | EDDI mission monitor | **P0** | "Speaking mission board" (d47 design): hand-in reminder on arrival, expiry warnings at 1 h and 10 min. |
| Massacre stack tracker (per-faction kills, ratio, diff, shareable value) |  | P Massacres |  |  |  |  | Y | Y | Warboard, Elite Intel, EDNexus | **P0** | ODET metrics (Diff, kills to next completion, wing value) plus an overlay. Handle Operations mode and redirects. |
| Cargo / delivery / collect missions (CargoDepot progress, carrier diff) | ~ |  |  |  |  |  | Y | ~ |  | **P1** | Delivery checklist ticked automatically by CargoDepot events. |
| Passenger missions (cabins, VIPs; Lynx Highliner) |  |  |  |  |  |  |  |  |  | **P1** | Gap: nothing tracks passenger mission stacks since the Lynx (Apr 2026) made them relevant. |
| Feasibility check on accept (cargo, cabins), redirect "plot it" |  |  |  |  |  |  |  |  | d47 (design) | **P2** | Check cargo space, cabins and jump range at MissionAccepted; one-click plot on redirect. |
| Operations mode (2026): squad ops, Merc Coins, kept out of main history | ~ |  | ~ |  |  | ~ | ~ |  | EDDI (develop) | **P1** | Partition state on GameModeChange; ops dashboard; never upload ops data where it's rejected. |

### BGS

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Current system factions (influence, states, conflicts) on jump | Y | P BGS-Tally |  |  |  | Y |  |  | Inara, Elite BGS | **P1** | From FSDJump/Location faction data, with trend arrows from the local EDDN history. |
| BGS activity tally per tick/system/faction (INF, BVs, CBs, trade, carto, CZs, murders) | ~ | P BGS-Tally/EDMFAT |  |  |  |  | Y |  | EDBGS | **P1** | One ledger shared with Powerplay and missions. Correct failed and negative INF. |
| Tick detection (per-system) |  | P BGS-Tally |  |  |  | Y | Y |  | edservertick/EDCAS | **P1** | Infer per-system ticks from the local EDDN subscriber. Galaxy tick feeds have gone stale (Elite BGS, Jul 2026). |
| Discord reports (webhooks, clipboard, preview) |  | P BGS-Tally |  |  |  |  | Y clipboard |  | EDBGS | **P1** | Preview, edit-on-repost, per-activity channels, and a bot-parseable header. |
| Influence history & alert rules (margins, rival surges, expansion/retreat) |  |  |  |  |  |  |  |  | Elite BGS, ETN, EDCAS | **P2** | Local EDDN-built history for watched factions. |
| Squadron objectives / coordination |  | P BGS-Tally |  |  |  |  |  |  | ETN | **P2** | Never de-anonymise EDDN uploaders (unlike ETN). |

### Powerplay

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Pledge, rank, merits (session / weekly cycle) | Y | P PowerPlayProgress/EliteMeritTracker | Y |  |  |  | Y |  | EDDI | **P1** | Use `TotalMerits` deltas (summing overcounts ~5.6%). Cycle boundary is Thu 07:00 UTC. |
| Merits per system & per activity attribution | ~ | P PowerPlayProgress/EliteMeritTracker |  |  |  |  | Y |  |  | **P1** | Time-window attribution with an explicit "Unknown" bucket. Account for data bags and Operations. |
| System Powerplay state (control progress, reinforcement, undermining) | ~ | P PowerPlayProgress |  |  |  | Y |  |  | Spansh API, EDPowerPlay, Inara | **P1** | From FSDJump Powerplay fields plus the local EDDN history, with progress bars and the Thu 07:00 UTC cycle countdown. |
| Vulnerable / cascade analysis, cycle countdown |  |  |  |  |  |  |  |  | EDPowerPlay | **P2** | Local EDDN-based state DB gives offline analysis and alerts. |
| Where to earn merits (rares, acquisition mining, tasks) |  |  |  |  |  |  |  |  | PowerPlay Assistant, EliteMining, EDPM | **P2** | Join activity rules with local market, mining and system state to suggest the best merit activity nearby. |
| Perk / unlock progress |  |  | Y |  |  |  |  |  |  | **P2** | Rank and perk ladder per power, with progress to the next unlock. |

### Colonisation

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Construction depot tracking (progress, remaining per commodity) | Y | P BGS-Tally/Architect/ConstructionTracker | Y |  |  | Y | Y | Y | Raven Colonial | **P0** | Hottest activity of 2025–26. One source of truth, so two trackers never fight. |
| Shopping list across projects incl. ship cargo + carrier stock (diff, trips) | ~ | P BGS-Tally/Architect | ~ |  |  | ~ | Y | Y | Raven Colonial | **P0** | Colour legend (BGS-Tally), finish-flag "one load completes it" hint (SrvSurvey), loads vs tonnes, scope by context. |
| Where to buy (nearest markets; multi-commodity optimisation) |  | P BGS-Tally/Architect |  |  |  |  | ~ | Y via RC | Raven Colonial, Warboard | **P1** | Solve "fewest stops for this list" against the local market index. |
| System architect planner (slots, points, prerequisites, economy links, build order) |  | P BGS-Tally | ~ |  |  |  | ~ |  | Raven Colonial, EDCPS (MILP), DaftMav sheet | **P1** | Versioned rules dataset plus an optimiser (EDCPS-style MILP compiled natively). |
| Team projects & per-CMDR contributions |  | P BGS-Tally |  |  |  | ~ |  | Y | Raven Colonial | **P1** | Integrate with Raven Colonial rather than rebuild it. |
| Colonisation target search (claimable, body scoring) |  |  |  |  |  |  |  |  | Spansh, ED Colony Architect, Warboard, Colonisation CLI | **P1** | Join system and body data locally to remove the tedious Spansh cross-referencing. |
| Claim-chain / route planner (hubs, built flags) |  |  |  |  |  |  | Y |  | Spansh colonisation plotter, ED Colony Architect, RC Nexus | **P2** | Plan claim chains (≤15 ly hops) over the local index, with hub/built flags (ODET) and a 3D map view. |
| Raven Colonial two-way sync |  | P BGS-Tally/ravencolonial_edmc |  |  |  |  |  | Y |  | **P1** | Per-plan sync toggle; avoid double counting. |

### Fleet Carrier

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Carrier status (tritium, balance, capacity, services, location) | Y | P GalaxyGPS/BGS-Tally | ~ | P Fleet Commander |  |  | Y |  | EDCM, EDDI | **P1** | One carrier cockpit for personal and squadron carriers, combining journal, CAPI and Market.json with freshness stamps. |
| Jump timers / cooldown notifications | ~ | P NeutronDancer/BGS-Tally |  | P Fleet Commander |  | ~ | Y |  | EDCM, COVAS | **P1** | Use the game's actual jump time. Show variable-timer statistics. |
| Carrier route plan & follow (Spansh FC router) | Y | P NeutronDancer/GalaxyGPS |  | P Fleet Commander |  | Y | ~ | Y | Spansh | **P1** | Plot via the Spansh FC router with tritium per jump. Auto-copy the next jump when the cooldown ends; restock warnings. |
| Carrier cargo & market orders (CAPI), ghost-order reconciliation | ~ | P GalaxyGPS/BGS-Tally | Y |  |  |  | Y |  | EDCM, EDAPITool | **P1** | Diff CAPI snapshots to detect fills by others (not journaled). |
| Finance projection ("upkeep funded N weeks", debt warnings) | ~ |  | ~ | P Fleet Commander |  |  |  |  | EDCM | **P1** | Project weekly upkeep from installed services and balance. Alert before debt or decommission (EDCM "funded N weeks"). |
| Squadron carriers (Vanguards) | Y |  | Y | P Fleet Commander |  |  | Y | ~ | EDDI | **P1** | First-class squadron carriers: bank, cargo and orders via the CAPI squadron endpoints. |
| Trade posts to Discord / job boards |  | P BGS-Tally/FCMS |  |  |  |  |  |  | EDCM, CarrierBoard, PTN | **P2** | Templated webhook posts that edit in place as orders change; optional job-board integration. |

### Engineering & Materials

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Materials inventory (raw/manufactured/encoded) with grades & caps | Y |  | Y |  |  | Y |  |  | EDDI, Inara | **P0** | Live caps with near-cap warnings; carrier and squadron-bank stock shown alongside. |
| Blueprint browser with craftability | Y |  | Y |  |  | Y |  |  | EDEngineer (dead) | **P0** | Driven by data packs, so new modules and Merc blueprints arrive without an app release. Filter to "craftable now". |
| Wishlist / shopping list (grades, quantities, experimentals) | Y |  | Y |  |  | ~ |  |  |  | **P0** | Import from EDSY, Coriolis and EDOMH links. "On your carrier" state. |
| Engineer unlock progress & pinned blueprints | Y |  | Y |  |  | ~ |  |  |  | **P1** | Persist unlocks from journal events (the game forgets them). Pins drive the route planner. |
| Engineer route planner (shortest path, unlock visits, remote workshop) |  |  | Y |  |  |  |  |  |  | **P1** | Shortest tour over pinned engineers, unlock visits and the remote workshop (EDOMH), plus return-to-start (EDOMH #440). |
| Material trader calculator / trade suggestions | Y |  | Y |  |  | Y |  |  |  | **P1** | Limit to traders you can reach given permits (EDOMH). |
| Farming locations (brain trees, crashed ships, HGE, data ports) | ~ | P EDR | Y |  |  |  |  |  |  | **P2** | Curated location packs plus your own discoveries; plot directly. |
| Material thresholds (min/desired) & near-cap alerts |  |  | ~ |  |  | Y |  |  | EDDI, EDEngineer | **P1** | Min/desired per material (EDDI) drives alerts and trader suggestions. |
| Synthesis recipes | Y |  | Y |  |  |  |  |  |  | **P1** | Synthesis uses the same wishlist and priority-allocation model as engineering (EDD pattern). |
| Tech broker / Guardian unlock planning | Y |  | Y |  |  |  |  |  |  | **P2** | Same wishlist pipeline, covering Guardian and human tech brokers and their material sources. |
| Pickup relevance notifications |  |  | Y |  |  |  |  |  |  | **P1** | Toast and voice on pickup (relevant, irrelevant, on wishlist), routed through the notification bus (EDOMH pattern). |

### Ship builds

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Current ship loadout with engineering | Y |  | Y |  |  | Y |  |  | EDDI | **P0** | Render from Loadout with engineering modifiers; stats match the in-game panel. |
| Fleet & stored modules (location, transfer ETA/cost) | Y | ~ Status window, P EDR |  |  |  | Y |  |  | COVAS, BGS-Tally FC shipyard | **P1** | Where every ship and module is, with transfer time and cost from your current location. |
| Open/export current ship to EDSY / Coriolis / SLEF / Inara | Y | Y | Y SLEF→Spansh |  |  |  | Y links |  | EDDI | **P0** | Speak every interop format (Loadout, SLEF, `#/I=`, `/import?data=`, `edomh://`). |
| Full ship build planner (engineering, stats, power, thermals, DPS) | ~ |  | Y |  |  |  |  |  | EDSY, Coriolis | **P2** | Own stats engine, since EDSY data is non-commercial and Coriolis lags. Add build comparison (EDOMH #983). |
| Retrofit planner (current → target checklist, materials, credits) | ~ EDSY import |  | ~ |  |  |  |  |  | EDSY | **P2** | Diff the current loadout against a target build into buy/engineer steps with materials and credits (EDSY Analysis pattern). |
| Combat analysis vs opponent (TTK, shield longevity) |  |  |  |  |  |  |  |  | Coriolis | **P2** | Only once the stats engine is mature; until then, link to Coriolis. |
| Where to buy a ship / module | Y |  |  |  |  | Y |  |  | Spansh, Inara | **P1** | Your own docking history ("last seen at", EDD) plus the local station index (Spansh/Ardent dumps). |

### Odyssey suits & weapons

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Odyssey materials (backpack, locker, carrier, squadron) with relevance | Y |  | Y |  |  | Y |  |  |  | **P0** | One inventory model for backpack, locker, carrier and squadron bank, with relevance colouring (EDOMH). |
| Suit/weapon upgrade & modification planner + wishlist | Y |  | Y |  |  | ~ |  |  |  | **P1** | Shares the engineering wishlist and route planner with ship engineering. |
| Odyssey engineer unlock tracking | ~ |  | Y |  |  |  |  |  |  | **P1** | Prerequisites turn green as they are met (EDOMH pattern). |
| Bartender simulator / barter values |  |  | Y |  |  |  |  |  |  | **P2** | Barter calculator over the wishlist, including carrier bartender stock (FCMaterials.json). |
| Settlement types / data-port spawn chances / where to find |  | P EDR | Y |  |  |  |  | Y maps |  | **P2** | Location packs, plus SrvSurvey-style settlement maps in the LAN view. |
| Suit loadouts (current & saved) | Y | ~ | Y |  |  | Y |  |  |  | **P1** | Current and saved loadouts from SuitLoadout events, linked to the planner. |

### Voice / TTS & alerts

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| TTS event announcements (configurable) | Y |  |  | Y | Y | Y |  |  | EDDI, COVAS | **P1** | Bundled local neural TTS (Piper/Kokoro), with cloud voices optional. The free Edge endpoint broke in Oct 2026. |
| One speech arbiter (priority queue, ducking, speak/silent/hide per event) |  |  |  |  |  | ~ |  |  | COVAS reaction matrix | **P1** | Users run 3–4 talking tools that talk over each other. Expose a local speech API so others route through Nexus. |
| Scriptable phrases / personalities / packs | Y |  |  |  |  | Y |  |  | EDDI (Cottle) | **P2** | Template packs with tokens and filters (EDCoPilot) on top of the speech arbiter; shareable. |
| Speech recognition commands | Y |  |  |  |  | ~ needs VoiceAttack |  |  | VoiceAttack/HCS, COVAS, Elite Intel | **P2** | Local STT (Whisper/Parakeet) with a grammar fast-path. Typed fallback in in-game chat (Elite Intel "@Vega"). |
| LLM conversational copilot |  |  |  |  |  | ~ via COVAS |  |  | COVAS:NEXT, Elite Intel | **P2** | Optional. Better: expose Nexus state through MCP/WebSocket so any copilot uses Nexus as its data source. |
| Popups / sounds / chimes | ~ | P EDR | Y | Y |  |  | Y |  |  | **P0** | Observatory's NotificationArgs model (update in place, GUID, grouping). |
| Context suppression (game mode, combat, mining, on foot; per-ship) | ~ | P BioScan whitelist |  |  |  | Y |  |  | COVAS | **P1** | Rules keyed on game mode, GuiFocus, vehicle, ship and activity, shared by voice, overlay and popups. |
| Keypress automation (bindings-aware) | Y |  |  |  |  | Y |  |  | COVAS, VoiceAttack | **P2** | Opt-in per action, with a stuck-key watchdog. |

### Overlay / second screen

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| In-game overlay (transparent, click-through, anchored widgets) | Y T/Tc/Tf pop-outs | P ModernOverlay | ~ AR | ~ popups | Y HUD | ~ | Y | Y |  | **P1** | One overlay host for all modules: per-widget anchor, opacity, hover-to-opaque, profiles. The MVP gets a basic click-through window. |
| Context-triggered overlays (FSS, approach, surface, galaxy map, docking) | ~ | P NeutronDancer/BioScan |  |  | ~ |  |  | Y |  | **P1** | Driven by GuiFocus/Status flags. Invisible until relevant (SrvSurvey). |
| VR overlay (OpenVR/OpenXR) |  | P ModernOverlay exp./EDR |  |  |  | ~ 3rd-party |  | Y OpenVR | COVAS OpenXR layer | **P2** | OpenXR API layer (COVAS approach) with per-ship layouts (SrvSurvey). |
| LAN web / tablet / phone second screen (QR pairing) | Y web server |  |  |  |  | ~ Windows client |  |  | COVAS QR UI, EDCoPTER, ED Colony Architect | **P1** | Built-in PWA with QR pairing and token auth, reusing desktop widgets. Answers the Linux/VR/second-PC demand. |
| OBS / stream overlay (browser source) | ~ OBS text files | P StreamHUD | ~ |  |  |  |  |  | EDLD | **P2** | Gap: no maintained journal-fed OBS source. |
| Never steal focus / auto-refocus game | ~ |  |  |  |  | Y |  |  | COVAS | **P0** | Design rule for every window. |

### Website sync & data

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| EDDN upload (all schemas, queue, privacy rules, delay-until-docked) | Y | Y | Y |  |  |  |  | ~ disabled | EDDI, Warboard, EDCAS | **P0** | Follow EDDN rules exactly; never upload Operations, Legacy or Beta data to live schemas. |
| EDDN live listener feeding local data |  |  |  |  |  |  |  |  | Ardent, EDPowerPlay, edservertick, EDCAS, ED Colony Architect | **P1** | Embedded ZeroMQ subscriber (~25 msg/s). The key enabler for trade, BGS, PP, colonisation and carriers. |
| EDSM journal upload & lookups | Y | Y |  |  |  |  |  |  | EDDI | **P0** | Circuit-break and fall back to Spansh/local data (33 h outage in Mar 2026). |
| Inara upload | Y | Y |  |  |  |  |  | Y | EDDI | **P0** | Batched at 35 s or more; per-CMDR key. |
| Frontier CAPI (profile, market, shipyard, carrier, squadron, journal) | Y | Y | Y |  |  |  | Y |  | EDDI, EDSY, GalaxyGPS, EDCM | **P1** | Per-endpoint health and cooldown UI; token-expiry warning; customer_id check; `/journal` for cloud players. |
| Spansh (lookups, plotters, dumps) | Y | ~ links, P plugins | ~ |  |  | Y | Y | Y |  | **P0** | Typed Spansh client with job polling, caching and a circuit breaker. Delta dumps feed the local index. |
| EDAstro / Canonn uploads | Y | Y EDAstro, P Canonn |  |  |  |  |  | ~ |  | **P2** | Opt-in senders that follow each service's event whitelist (EDAstro tells clients which events it accepts). |
| Discord webhooks / Rich Presence |  | P BGS-Tally/Discord Presence |  |  |  | ~ | ~ |  | EDNexus | **P2** | Per-activity webhooks with preview; optional Rich Presence with privacy controls. |
| Upload transparency & privacy (what was sent where, local-only mode) |  | ~ | ~ queue |  |  |  |  |  | EDRockMaster (local-only) | **P0** | "Sent log" per service, streamer privacy toggles, local-only by default for anything not on EDDN. |

### Bindings

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Read .binds (know keys, detect missing binds for features) | Y |  |  |  |  | Y |  |  | COVAS (collision check), EDDI+bindED | **P1** | Parse all bind presets and warn when a feature needs an unbound key. Feeds the reference card and editor. |
| Bindings reference card / visual viewer | ~ |  |  |  |  |  |  |  | EDRefCard (archived → edrefcard2) | **P2** | Gap: nothing maintained since 2024. |
| Bindings backup / profile management | ~ v20-dev |  |  |  |  | Y auto-backup |  |  |  | **P1** | Cheap and much loved. |
| Bindings editor | ~ v20-dev |  |  |  |  |  |  |  |  | **P2** | Gap: no released editor (EDD v20-dev panel is the first). |

### Misc / platform

| Feature | EDD | EDMC | OMH | OBS | EXB | ECP | ODT | SRV | Others | Pri | How Elite Nexus does it better |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|:-:|---|
| Plugin / extension system | Y DLL+actions | Y Python |  | Y .eop |  |  |  |  | COVAS Python plugins | **P1** | Sandboxed WASM with a versioned, stable API and crash isolation. Signed registry with compatibility badges. Plugins must never stall the core (EDMC Tk-thread lesson). |
| Local API (REST/WebSocket) + MCP server | Y web server/WebSocket |  |  |  |  |  |  |  | EDEngineer REST, EDDI VA bridge | **P1** | Read-only by default. Lets bots, AI copilots and Stream Deck/LunaPanel use Nexus data (EDOMH #967). |
| Auto-update with channels & rollback | ~ release check | Y | Y | Y |  |  | Y | Y |  | **P0** | Signed and atomic; never deletes the install (SrvSurvey #917). |
| Hot-updatable data packs (ships, modules, blueprints, bio rules, traders) |  | Y | Y |  |  |  |  | Y |  | **P0** | Signed packs; unknown IDs shown gracefully until a pack arrives. |
| Opt-in reporting of unknown journal events/fields | ~ |  | Y |  |  |  |  |  |  | **P1** | Adapt within hours of a game patch. |
| Remote killswitches for uploaders |  | Y |  |  |  |  |  |  |  | **P1** | Signed remote manifest that can disable an uploader or redact a field per version (EDMC pattern). |
| Native Linux / Steam Deck / macOS | ~ | Y | Y | ~ | ~ | ~ | ~ | ~ | COVAS, Warboard, EDLD | **P0** | Single static Rust binary; auto-detects Proton, Flatpak and CrossOver paths. |
| Localisation | Y | Y | Y |  |  |  |  | ~ |  | **P1** | ICU/Fluent strings from day one; community translation via Crowdin. |
| Launch orchestration (start/stop with game; launcher integration) |  |  |  |  |  |  |  |  | min-ed-launcher, EDU | **P1** | Detect the game and start/stop helpers; never require starting before the game. |
| In-game chat command channel (type commands in local chat) | Y chat pack | P EDR/Canonn/NeutronDancer |  |  |  |  |  | Y | Elite Intel "@Vega" | **P2** | Zero alt-tab input that works in VR too. Parse SendText events; answer via overlay or voice. |
| Galnet / news |  |  |  |  |  | Y |  |  | EDDI, COVAS | **P2** | Galnet feed in the notification centre, with optional read-aloud. |
| Backups, config recovery, support bundle | Y Safe Mode | Y | Y |  |  |  |  |  |  | **P0** | Atomic writes and versioned backups (EDMC 6.1.1 lesson). |
| Time trials / racing |  |  |  |  |  | Y |  |  | SRVTracker | **P2** | Out of core scope; a candidate for a plugin. |


<!-- rows=167 P0=40 P1=81 P2=46 -->

### P0 (MVP) scope at a glance

The 40 P0 rows above define a daily-driver MVP:

- **Commander / Dashboard**:
  - Live commander summary (CMDR, credits, ship, system, station, docked state)
  - Live ship status (fuel, hull, cargo, current/max jump range, rebuy)
  - Typed live Status.json stream (flags, GUI focus, legal state, lat/long)
  - Multi-commander (per-FID data, Live/Legacy split, rename migration, multibox)
  - Customisable workspace (dock, tabs, splitters, reorder, saved layouts)
  - Pop-out windows, multi-monitor, always-on-top
- **History & Stats**:
  - Full journal backfill on first run (all CMDRs, all years)
  - Travel history log (jumps, docks, distances) with search/filter
- **Navigation & Routing**:
  - In-app Spansh plotting: neutron + galaxy (exact, ship-aware, Caspian SCO-aware)
  - Route follower (auto-copy next hop, persistence, catch-up after restart)
  - NavRoute hop table (star class, scoopable, neutron, value, discovered-by)
  - Nearest service finder (IF, trader by type, broker, UC, Vista, shipyard)
- **Exploration & Exobiology**:
  - System body list with values (type, Ls, landable, terraformable, atmo, signals)
  - Exploration value (FD/FM bonuses, efficiency, honk, current vs max-if-mapped)
  - Built-in interesting-body alerts (rings, close orbits, high-g, ELW…)
  - Exobiology species/variant predictions
  - Bio values + first-footfall / first-logged bonus detection
  - Sample tracker with colony range, live distance, "far enough" signal
  - Unsold data at risk (carto + exo) & death-loss tracking
- **Missions**:
  - Mission list with expiry countdown & hand-in reminders
  - Massacre stack tracker (per-faction kills, ratio, diff, shareable value)
- **Colonisation**:
  - Construction depot tracking (progress, remaining per commodity)
  - Shopping list across projects incl. ship cargo + carrier stock (diff, trips)
- **Engineering & Materials**:
  - Materials inventory (raw/manufactured/encoded) with grades & caps
  - Blueprint browser with craftability
  - Wishlist / shopping list (grades, quantities, experimentals)
- **Ship builds**:
  - Current ship loadout with engineering
  - Open/export current ship to EDSY / Coriolis / SLEF / Inara
- **Odyssey suits & weapons**:
  - Odyssey materials (backpack, locker, carrier, squadron) with relevance
- **Voice / TTS & alerts**:
  - Popups / sounds / chimes
- **Overlay / second screen**:
  - Never steal focus / auto-refocus game
- **Website sync & data**:
  - EDDN upload (all schemas, queue, privacy rules, delay-until-docked)
  - EDSM journal upload & lookups
  - Inara upload
  - Spansh (lookups, plotters, dumps)
  - Upload transparency & privacy (what was sent where, local-only mode)
- **Misc / platform**:
  - Auto-update with channels & rollback
  - Hot-updatable data packs (ships, modules, blueprints, bio rules, traders)
  - Native Linux / Steam Deck / macOS
  - Backups, config recovery, support bundle


---

## 5. Top 15 pain points Elite Nexus must explicitly solve

The list is ranked by how often and how strongly each pain shows up across GitHub issues, Reddit, Steam threads and tool docs. Each entry has the evidence, then the Elite Nexus answer, with acceptance targets where they can be measured.

### 1. Tool sprawl, launch order and duplicated work
- **Evidence**
  - Players run 4–8 apps.
  - "Post the third party apps you use" (r/EliteDangerous, Mar 2025, 72 comments) and "EDCoPilot vs the ecosystem" (Oct 2026) both ask for consolidation.
  - EDMC must be started *before* the game or startup data is missed (EDMC wiki). EDMC-Massacres needs a relog if started late. EDCoPilot crashes if launched after the game under Proton.
  - min-ed-launcher and EDU exist mainly to launch everything.
  - Each tool re-parses the same journals.
- **Elite Nexus answer**
  - One app with modules. **Start order doesn't matter**: catch up from the journals at any time.
  - Detect the game process; optional launcher integration.
  - One parser and one store, shared by every feature.

### 2. Performance at scale: startup, RAM, CPU, long imports
- **Evidence**
  - **EDD**: loads all history into memory. #3629: ~1.75 M entries took minutes per stage, then hangs. Official advice is to load less.
  - **EDOMH**: #923 "uses nearly as much memory as the base game"; #939 CPU above 50%.
  - **Observatory**: Read All takes minutes.
  - **Mining-Analytics**: "plugin is laggy" (#54).
  - **BGS-Tally**: window slow with many systems (#35).
  - **EDCoPilot**: FPS dips under Proton.
- **Elite Nexus answer.** Rust plus an incremental, indexed event store, lazily paged, with no full-history load.
  - **Targets**: cold start under 2 s, under ~150 MB RAM idle, a 10-year backfill in under 60 s on SSD, and near-zero CPU while idle in game.

### 3. Breakage after game updates and hard-coded data
- **Evidence**
  - EDD's "unknown module/actor" report threads are its most-commented issues: #3544 (115 comments), #3635 (101), #3849 (Rhino/Nomad, 2026).
  - Operations broke ODET massacre tracking (#50, #51) and SrvSurvey overlays (#1041).
  - Coriolis lagged months on new hulls; EDHM-UI breaks every patch.
  - Frontier's journal manual jumped from v37 (May 2023) to v38 (Apr 2026).
- **Elite Nexus answer**
  - **Parser**: tolerant serde models that keep unknown fields and the raw line.
  - **Data**: **signed, hot-updatable data packs** shipped without an app release.
  - **Feedback**: opt-in **unknown-event and unknown-ID reporting** (EDOMH pattern).
  - **Isolation**: `GameModeChange`-aware state partitioning.
  - **Testing**: a regression corpus of real journals in CI.

### 4. Fragile extension ecosystems
- **Evidence**
  - Every EDMC major version leaves plugins "broken": Pioneer #5, BioScan #28, PowerPlayProgress #61, InaraProgress.
  - Plugins share EDMC's single Tk thread and can stall it (#2590, "Awaiting CMDR Login").
  - Installs are manual zips (#513). The Plugin Browser can't install anything.
  - Observatory updates broke BioInsights (#193). ExploData forces DB-version lockstep.
- **Elite Nexus answer**
  - **First-party modules** cover the top ~15 plugin use cases, so most users need no plugins.
  - For the rest: **sandboxed WASM plugins** with a semver'd stable API, crash/time isolation, and a signed registry with compatibility badges and one-click install, update and rollback.

### 5. Linux, Steam Deck and macOS are second-class
- **Evidence**
  - Windows-only, or Proton/Wine with workarounds: EDD (Mono without speech, web or 3D), Observatory (Wine 10+, chimes only), SrvSurvey (Proton prefix only), ODET (Windows), EDCoPilot (community script), EDEB (Proton TTS fix).
  - Clipboard backends on Wayland/X11/Flatpak dominate NeutronDancer issues.
  - Emoji glyphs are missing in Linux overlays (BioScan #27, 47 comments).
  - Flatpak sandboxing problems; EDMC dropped macOS in 5.11.
- **Elite Nexus answer**
  - A single native binary for Windows/Linux (and macOS later).
  - Built-in clipboard and font handling (bundled icon font instead of emoji).
  - Auto-detected Proton, Flatpak and CrossOver journal paths.
  - Steam Deck-friendly LAN UI.

### 6. Overlays are unreliable, and VR is barely served
- **Evidence**
  - EDMCModernOverlay: controller won't launch (#29, 74 comments), overlay not showing (#28, 52), mouse capture (#121), borders, OBS capture (#108), MinEDLauncher (#211); VR is its top-voted request.
  - Observatory popups fail in exclusive fullscreen; the EDEB HUD needs borderless and draws on the flat screen only.
  - ODET overlay flicker at 4K (#36).
  - Only SrvSurvey (OpenVR) and COVAS:NEXT (OpenXR layer) do native VR.
- **Elite Nexus answer**
  - One overlay host shared by all modules: click-through by default, per-widget anchors and profiles, and context triggers from GuiFocus/Status.
  - A **VR path**: OpenXR API layer / OpenVR overlay with per-ship layouts.
  - The LAN UI as a universal fallback.

### 7. Focus stealing and the missing second screen / remote view
- **Evidence**
  - Alt-tab focus loss stops joystick input (Reddit 2026; Frontier VR thread).
  - Users want tablets, phones and second PCs: ODET #28 web server, EDCoPilot's Windows-only remote client, EDCoPTER and ED Colony Architect LAN UIs, COVAS:NEXT QR pairing.
  - GeForce NOW and console players are locked out.
- **Elite Nexus answer**
  - **Never take focus.** Re-focus the game after interactions.
  - **Built-in LAN PWA with QR pairing**, rendering the same widgets as the desktop.
  - CAPI `/journal` catch-up for cloud players and second PCs.

### 8. Exploration and exobiology split across five tools
- **Evidence**
  - The usual exo stack is Observatory+BioInsights **plus** SrvSurvey **plus** EDEB **plus** BioScan/Pioneer **plus** EDCoPilot callouts.
  - Wishes: a built-in exobio radar, TTS in EDEB, bio info not hidden behind tiny hover icons; BioScan unscrollable on the Deck.
  - Accuracy disputes between predictors (SrvSurvey #605).
- **Elite Nexus answer.** One pipeline, shown consistently across desktop, overlay, voice and LAN:
  - pre-arrival intel;
  - FSS/DSS value;
  - predictions with confidence;
  - "worth landing?";
  - surface radar with colony ranges;
  - sample progress;
  - credits at risk.
  - The rules dataset is validated against Canonn/EDDN data.

### 9. Colonisation and carrier logistics: fragmented and stale
- **Evidence**
  - Raven Colonial plus SrvSurvey plus EDMC plugins conflict; you must disable colonisation in one of them.
  - Carrier cargo drifts: ODET #13 (21 comments), SrvSurvey #525/#560/#222, EDOMH #643/#695.
  - Build type and slot counts are guessed.
  - Target search means tedious Spansh cross-referencing.
  - Carrier fills by others aren't journaled ("ghost orders").
  - Squadron-carrier transfers are invisible.
- **Elite Nexus answer**
  - **One tracker** with Raven Colonial two-way sync.
  - Carrier stock reconciled from journal + Market.json + CAPI diffs.
  - A shared, versioned facility-rules dataset.
  - Local target search.
  - An optimiser-backed planner.
  - "Upkeep funded for N weeks" carrier finance.

### 10. Dependence on fragile web services
- **Evidence**
  - EDSM had a **33 h outage (Mar 2026)** and sits behind Cloudflare. EDD needed fixes; EDCM's trade post fails when EDSM is down.
  - Spansh is slow at times and caps results at 10k.
  - Inara blocks automation.
  - Elite BGS ticks have been stale since Jul 2026.
  - EliteMining warns about Ardent/Spansh rate limits.
  - EDD's star-DB downloads time out (#3744).
- **Elite Nexus answer**
  - **Local-first data**: an embedded EDDN subscriber plus seeded dumps (Ardent SQLite, Spansh populated/stations, deltas).
  - Circuit breakers, caches and per-service health indicators.
  - Graceful degradation everywhere.

### 11. CAPI authentication friction and weak multi-account support
- **Evidence**
  - Credential and auth errors dominate EDMC history (#242, 64 comments; #432).
  - About 25-day re-auth.
  - `customer_id` mismatches; Steam/Epic linking.
  - Login loops (ODET #15/#22); EDOMH 406 errors (#925).
  - Multi-accounting needs separate Windows users (EDMC). Requests: EDMC #2701, ODET #34, EDDI #505.
  - EDOMH can't delete commanders (#686).
- **Elite Nexus answer**
  - Per-endpoint CAPI health and cooldown UI.
  - **Pre-expiry re-auth reminders.**
  - A FID check.
  - First-class multi-account and multi-instance support.
  - Commander management (rename migration, delete, merge).

### 12. Voice tools talk over each other; the speech stack costs money and breaks
- **Evidence**
  - EDCoPilot, EDDI, HCS and COVAS overlap ("several voice tools talking at once", Frontier thread, May 2025).
  - Voice input requires paid VoiceAttack.
  - The **free Edge TTS endpoint started returning 403 around 2026-10-10**: EDCoPilot shipped three hotfixes.
  - COVAS: API costs, latency, tool-call failures, a stuck-key bug (#206).
- **Elite Nexus answer**
  - **One speech arbiter**: priority queue, ducking, speak/silent/hide per event, context suppression.
  - **Bundled local TTS/STT** (Piper/Kokoro; Whisper/Parakeet) with a grammar fast-path.
  - An optional LLM.
  - A local speech API so other tools route through Nexus.

### 13. Steep learning curves and hidden configuration
- **Evidence**
  - **EDD**: ~60 panels plus splitters, grids, profiles and packs. Users get lost (#3085). Wiki pages are stale. EDD ships EDDLite as its "too heavy" escape hatch.
  - **EDCoPilot**: INI-only settings; a mandatory History Refresh that confuses newcomers.
  - **EDSY**: cryptic abbreviations.
  - **EDOMH**: dense lists (#612, #739).
- **Elite Nexus answer**
  - A first-run wizard that auto-detects journals, commanders and accounts.
  - **Activity presets** that auto-switch.
  - Progressive disclosure.
  - Searchable settings and a command palette.
  - In-context help.

### 14. Data loss on updates and storage corruption
- **Evidence**
  - **EDD**: #3870 user settings deleted after update; "database locked" (#3464); corruption on exit (#3766, #3799); duplicate jumps via junction paths (#3833).
  - **EDEB**: upgrades wiped routes, history and speech settings.
  - **ODET**: #16 settings lost.
  - **SrvSurvey**: #917 auto-update deleted the install folder.
  - **EDMC**: needed config backup and auto-recovery in 6.1.1.
- **Elite Nexus answer**
  - Settings kept separate from data.
  - Atomic writes.
  - Versioned migrations with automatic pre-migration backups.
  - Canonicalised journal paths (de-duplicated by file identity).
  - Update rollback.

### 15. Trust: unsigned binaries, antivirus false positives, privacy and licensing
- **Evidence**
  - **Code signing and antivirus**
    - EDD binaries are unsigned and blocked by Smart App Control (#3871); its maintainers decline to sign.
    - Antivirus false positives for EDMC (#2663), SrvSurvey (#137, #235), COVAS (#95, #106), EDOMH (Defender), EDCoPilot (Norton), EDEB (SmartScreen) and EliteMining.
  - **Privacy**
    - ETN tries to de-anonymise EDDN uploaders.
    - EDOMH support bundles include journals.
  - **Licensing**
    - ODET has no license and private dependencies.
    - EDCoPilot is closed source.
- **Elite Nexus answer**
  - **Code-signed** builds and reproducible releases.
  - Privacy defaults: a **"what was sent where" log**, local-only by default for anything beyond EDDN/EDSM/Inara, streamer privacy toggles, and redaction in support bundles.
  - A clear open-source license.

**Honourable mentions (just outside the top 15)**
- **Trade**: no EDDB-class multi-hop trade planner since 2021.
- **Missions & Powerplay**:
  - passenger-mission stacks untracked (Lynx, 2026);
  - Powerplay merit attribution accuracy (sum vs `TotalMerits` delta, 5.6% overcount).
- **Bindings**: tools unmaintained (EDRefCard archived; no editor released).
- **Data sharing**: the "please upload your data" culture problem. Nexus should make contributing to EDDN/EDSM the frictionless default.


## Appendix A. Data sources and integration rules (cheat-sheet)

### A.1 Local game files
- **Location**
  - Windows: `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous\`. Resolve the Saved Games Known Folder rather than hard-coding the path, because users move it.
  - Proton: `steamapps/compatdata/359320/pfx/...`
  - Also support Flatpak and CrossOver paths, plus a manual override.
- **Journal files**
  - Live 4.x names look like `Journal.2026-10-12T091500.01.log`. Legacy and Horizons-era names look like `Journal.YYMMDDHHMMSS.01.log`. Beta and alpha builds add `JournalBeta`/`JournalAlpha` prefixes.
  - **Pick the newest file by timestamp, not by name.**
  - Each file has one JSON object per line, starting with a Fileheader. A `Continued` event marks part rollover.
  - `SystemAddress` and `MarketID` need u64.
  - `*_Localised` keys sit beside `$symbol;` values.
- **Watching**
  - File-change notifications are unreliable on Proton, Flatpak and network drives. Poll as a fallback: about 1 s while the game runs, about 10 s otherwise.
  - Observatory's trick also helps: touching (opening and closing) the newest journal every 250 ms forces Windows to flush change notifications. Without it, popups can be delayed by up to 30–40 s.
- **Companion files.** Each is rewritten whole and announced by a journal event of the same name. Re-read it when the event arrives, then check its timestamp against the event's and retry if they don't match.

  | File | Written when |
  |---|---|
  | Status.json | Every few seconds while state changes |
  | NavRoute.json | A route is plotted (cleared by NavRouteClear) |
  | Market.json | The commodity market screen is opened |
  | Outfitting.json | The outfitting screen is opened |
  | Shipyard.json | The shipyard screen is opened |
  | Cargo.json | Cargo changes, for the ship or the SRV |
  | ShipLocker.json | Odyssey ship locker changes |
  | Backpack.json | Odyssey backpack changes |
  | ModulesInfo.json | The modules panel is opened |
  | FCMaterials.json | The fleet carrier bartender is opened |

  Related non-journal files:
  - `*.binds` (key bindings)
  - `GraphicsConfigurationOverride.xml` (HUD colours)
  - `VisitedStarsCache.dat` (also available from CAPI `/visitedstars`)
- **Partition state** by:
  - commander FID
  - galaxy: Live vs Legacy (gameversion below 4.0); Live vs Beta
  - **game mode**: `GameModeChange` reports MainGame, Operation or ProvingGrounds
  - multicrew
  - vehicle: ship, SRV (including Rhino), fighter, Nomad (`LaunchVessel`), on foot, taxi
  
  Never upload Operations, CQC, Beta or Legacy data to services that reject it.

### A.2 EDDN (upload and listen)
- **Upload.** `POST https://eddn.edcd.io:4430/upload/`
  - The trailing slash is required.
  - Body is JSON, optionally gzip-compressed, at most 1 MiB.
  - Header fields:
    - `softwareName`: unique and consistent.
    - `softwareVersion`: semver. **Bump it whenever message content changes.**
    - `uploaderID`: required.
    - `gameversion` and `gamebuild`: required; send `""` if unknown. For journal data, take them from Fileheader/LoadGame. For CAPI data, use `CAPI-Live-<endpoint>` or `CAPI-Legacy-<endpoint>`.
  - What to strip or check before sending:
    - Strip `_Localised` keys and all commander-specific data.
    - Never send `null`.
    - Cross-check augmented StarSystem/SystemAddress/StarPos against the last Location/FSDJump/CarrierJump event.
    - Check you are still docked at the matching station before sending CAPI market data.
  - Use `/test` schemas while developing.
  - Retries: back off at least 60 s. Never retry a 400 or 426 response.
  - "No data is better than bad data."
- **Live schemas (Oct 2026):** approachsettlement, blackmarket, codexentry, commodity/3, dockingdenied, dockinggranted, fcmaterials_capi, fcmaterials_journal, fssallbodiesfound, fssbodysignals, fssdiscoveryscan, fsssignaldiscovered, journal/1, navbeaconscan, navroute, outfitting/2, scanbarycentre, shipyard/2. An **outfitting v3 schema** is coming; EDMC targets it for 6.2.
- **Listen.** ZeroMQ SUB on `tcp://eddn.edcd.io:9500` (plain TCP, no TLS).
  - Subscribe to the empty topic and filter client-side.
  - Messages are zlib-compressed; inflate them, then dispatch on `$schemaRef`.
  - De-duplicate yourself.
  - Reconnect around the Thursday 07:06 UTC restart.
  - Expect about 25 messages/s, based on Ardent's 2.17M updates per day.
  - Check that the chosen Rust ZeroMQ crate interoperates with the relay's old libzmq.

### A.3 Frontier CAPI
- **Registration.** Apply in the Frontier Developer Zone (user.frontierstore.net). Approval is not instant. Each app gets its own Client ID; publishing a public Client ID in open-source code is normal practice.
- **OAuth**
  - OAuth2 with PKCE at `auth.frontierstore.net`, scope `auth capi`. Use `audience=frontier,steam,epic` so Steam and Epic accounts can log in.
  - Send a custom User-Agent: `EDCD-<App>-<version>` style.
- **Tokens**
  - Access tokens last about 4 h. Refresh tokens last about 25 days; then a full re-login is needed. **Warn users before expiry.**
  - Compare the auth `customer_id` with the journal FID to avoid multi-account mix-ups.
- **Hosts**
  - Live: `companion.orerve.net`
  - Legacy: `legacy-companion.orerve.net`
  - PTS: `pts-companion.orerve.net`
- **Endpoints**
  - `/profile`, `/market`, `/shipyard`, `/communitygoals`
  - `/fleetcarrier`: returns 204 if the commander has no carrier
  - `/journal[/yyyy/mm/dd]`: 206 means partial. This is the catch-up path for cloud and second-PC players.
  - `/visitedstars`: 102 means still generating
  - 418 means maintenance.
- **Rate limits**
  - Aim for at most about 1 request/min. Frontier has said throttling may start above 2/s.
  - EDMC uses a 60 s cooldown generally and 15 min for `/fleetcarrier`.
- **Known gaps**
  - Carrier order fills by other players are **not journaled**. CAPI snapshots are the only way to detect them.
  - Squadron-carrier transfers are not journaled.

### A.4 Web services
| Service | Use | Rules / limits |
|---|---|---|
| **EDSM** | Journal upload (`api-journal-v1`, per-CMDR API key, honour the `/discard` list), system and body lookups, logs API | Live galaxy only. Respect the `X-Rate-Limit-*` headers. Logs API 360 requests/hour. Bodies dump covers 7 days only. Behind Cloudflare; 33 h outage in Mar 2026. Treat as optional and circuit-broken |
| **Inara** | Commander profile sync (about 40 event types, batched) | Batch at least every 31–35 s and stay under about 2 requests/min. User supplies an API key; the app name must be known to Inara (confirm the current rules with Inara). Live only |
| **Spansh** | All plotters (job API: POST `/api/route` → poll `/api/results/<job>`); body, system and station searches; dumps (galaxy ≈110 GiB gzipped; populated and stations 4.2 GiB; 1-day, 7-day and 1-month deltas) | No published terms. Cache, credit, use a descriptive User-Agent, and ask before heavy automated use. 10k-result cap. Do not re-host the dumps |
| **Ardent Insight** | Markets, commodities, nearest services, nearby systems; weekly SQLite dumps | AGPL code. Anonymous with no rate limit ("respectful use"). Fine to use at runtime; don't vendor its code |
| **EDAstro** | Upload (EDMC sends CarrierStats, CarrierJumpRequest, ScanOrganic), records, GEC POIs | All rights reserved. API 100 requests per 15 min |
| **Canonn** | Codex reference, codex per body, nearest codex, prior bio scans, POIs | No data licence. Attribute; ask before bulk redistribution |
| **Raven Colonial** | Colonisation projects, plans, carrier cargo (REST, per-user API key) | Coordinate with the author. Avoid double-counting (use `/contribute` vs `/supply` correctly) |
| **Elite BGS / tick feeds** | Faction snapshots, ticks | Maintenance mode. Use per-system tick inference instead |
| **EDGIS** | Coordinates, including predicted coordinates for uncharted systems; nearest neutron | Used by BGS-Tally and ETN |
| **Discord** | Webhooks for reports | Include a parseable version header (EDBGS pattern); edit-on-repost (BGS-Tally pattern) |

## Appendix B. Reusable data, code and licensing (for an MIT/Apache-2.0 Elite Nexus)

This is not legal advice. It assumes Elite Nexus ships under MIT or Apache-2.0. Items marked "unverified" could not be fetched (Inara, EDSM, Frontier pages returned 403/503).

| Item | Content | Licence / terms | Recency | Verdict |
|---|---|---|---|---|
| **EliteDangerousCore** (EDDiscovery) | Journal parsing; `FrontierData/Items`: ships, modules, engineering and synthesis recipes, engineers, suits, hand items | **Apache-2.0** | Commits to 2026-10-10; Rhino and Nomad included | **Best permissive source to port** (keep the NOTICE and mark changes) |
| **EDDI data definitions** | Modules, materials, microresources, blueprints, engineers, regions, commodities | Apache-2.0 | Active | Port with notice. Its organic species data is marked obsolete; don't use it |
| **EDOMH** (jixxed) | Horizons and Odyssey blueprints, materials, engineers (in Java source) | **MIT since 2026-04-24** (was GPL-3.0 from Dec 2024). Official binaries contain proprietary components | 3.15.6 (2026-09-16) | Port only from **post-2026-04-24** commits; record the commit hash |
| **ObservatoryCore + Botanist** | Typed journal model; genus names; colony ranges | MIT | v1.5.2 (2026-10-01) | Bundle with notice |
| **ed-journals / ed-state / ed-exobiology** (Rust) | Journal and companion-file models, reader, state, species prediction | MIT | 0.13.0 / 0.1.0 (2026); commits Sep 2026 | Use as dependencies, but validate them (beta) |
| **klightspeed/EliteDangerousRegionMap** | Galactic region lookup (includes a **Rust** implementation) | MIT | Stable | Bundle. Needed for exobiology region rules |
| **EDDN schemas** | JSON schemas | BSD-3-Clause | Live | Bundle; follow the uploader rules |
| **EDCD/FDevIDs** | ID/symbol/name CSVs (commodity, outfitting, shipyard, materials, microresources…) | **No LICENSE**; maintainer: "free for all, like public domain" | Updated 2026-09-05 (Rhino) | Bundle de facto, credit EDCD; consider asking for CC0 |
| **coriolis-data** | Ships, modules, blueprints, experimentals JSON | MIT covers code only. **JSON is declared Frontier IP** | 2026-04-24; **missing the Lynx** | Grey area. Reference only, or fetch at runtime |
| **EDSY** | Most complete and accurate ship/module/blueprint database | **CC BY-NC 4.0** | db 2026-09-23 | **Do not bundle or port.** Link/import/export interop only |
| **BioScan** | Species rules (Python) | GPL-2.0-or-later | 2.12.2 (Jan 2026) | Re-implement; don't copy the tables |
| **SrvSurvey** | `bio-criteria/*.json` (21 genera), predictor, Guardian data | GPL-3.0 | 2026-09 | Re-implement |
| **EDMC** | Uploader and CAPI client logic, ships/modules JSON | GPL-2.0-or-later | 6.1.x | Use as a **behaviour reference only** |
| **EDMC-Canonn / Canonn-GCloud** | Plugin and cloud functions | GPL-3.0 | 2026 | Re-implement; use the Canonn API at runtime with attribution |
| **Ardent** | API, collector | AGPL-3.0 | Active | Use the API and dumps at runtime; don't vendor code |
| **EDEngineer** | Blueprints JSON | MIT | **Archived 2025-07-04**; data last changed 2024-03 | Reusable but stale |
| **Spansh / EDSM / EDAstro / Inara / CAPI** | Services and dumps | No open data licences / proprietary terms (several unverified) | Live | **Runtime fetch only.** Never re-host dumps |
| **Frontier assets** | Game art, fonts, logos | Frontier IP | n/a | Ship none without permission. Carry the standard fan-tool disclaimer (non-commercial use, not endorsed by Frontier) |

**Takeaways**
1. **Engineering and item tables**: port them from EliteDangerousCore (Apache-2.0) or post-relicence EDOMH (MIT). Cross-check against FDevIDs.
2. **Exobiology rules**: derive your own from public facts (Canonn research, codex statistics, your own journals) and credit Canonn and the Deep Space Network. Optionally contribute to `ed-exobiology`.
3. **Ship-build maths**: implement your own engine. EDSY's data is non-commercial-only. Coriolis data is a grey area and lags new hulls.
4. **Shipping**: keep a `THIRD_PARTY_NOTICES` file and a per-dataset provenance file. Stay non-commercial if you use any Frontier-derived data.
5. **Alternative licence**: making Elite Nexus GPL-3.0 would allow absorbing GPL-2.0+/GPL-3.0 code (EDMC, BioScan, SrvSurvey, Canonn). AGPL and CC BY-NC would still be off-limits. This is a strategic decision to make **before** any code is written.

## Appendix C. Rust ecosystem notes

| Crate / repo | Use | Licence | Latest | Notes |
|---|---|---|---|---|
| `ed-journals` (rster2002) | Serde models and reader for journals and companion files; sync and async; directory watching; platform journal detection | MIT | 0.13.0 (2026) | Beta with breaking changes, but the best foundation. Contribute upstream |
| `ed-state`, `ed-exobiology` | State projection; species prediction | MIT | 0.1.0 (2026-07) | Very new; work in progress |
| `elite_journal`, `eddn` (nixpulvis) | Journal structs; EDDN subscriber (with optional egui GUI) | Beerware | 0.7.0 / 0.6.0 (2026-09) | Reference implementations |
| `nixpulvis/galos` | Postgres galaxy DB, Spansh dump reader, router, map | MIT | Active | Useful reference for a local galaxy index |
| `edsm-dumps-model` | EDSM dump models | MIT OR Apache-2.0 | 0.9.4 (2025) | |
| `zeromq` (zmq.rs, pure Rust) / `omq-tokio` | ZeroMQ SUB for the EDDN listener | MIT / ISC | 0.6.0 (2026-05) / 0.25.0 (2026-10) | Avoids a C build. **Test against the old EDDN relay** |
| `zmq` (libzmq bindings) | ZeroMQ | MIT/Apache (libzmq is MPL-2.0) | 0.10.0 (2022) | Stale; needs a C toolchain |
| `flate2` | zlib inflate for EDDN | MIT OR Apache-2.0 | 1.1.x | |
| `oauth2` | CAPI PKCE flow | MIT OR Apache-2.0 | 5.0.0 | |
| EDCAS (arne-fuchs) | Rust TUI client + server (EDDN, Postgres) | see repo | Public repo stale | Prior art for a Rust ED stack |

No Elite-specific Rust crates for CAPI, Spansh or Inara were found. These clients will be written in-house.

## Appendix D. Primary sources

GitHub issue numbers cited in the text refer to each tool's own repository.

**Core tools**
- EDDiscovery
  - https://github.com/EDDiscovery/EDDiscovery (+ /wiki, /releases)
  - https://github.com/EDDiscovery/EliteDangerousCore
  - https://github.com/EDDiscovery/EDDLite
  - https://github.com/EDDiscovery/EDDCanonn
- ED Market Connector
  - https://github.com/EDCD/EDMarketConnector (+ /wiki/Plugins, PLUGINS.md, ChangeLog.md, docs/Killswitches.md)
  - https://github.com/EDCD/EDMC-Plugin-Registry
- EDMC plugins
  - https://github.com/aussig/BGS-Tally
  - https://github.com/canonn-science/EDMC-Canonn
  - https://github.com/Silarn/EDMC-BioScan
  - https://github.com/Silarn/EDMC-Pioneer
  - https://github.com/Silarn/EDMC-ExploData
  - https://github.com/CMDR-WDX/EDMC-Massacres
  - https://github.com/CMDR-Kiel42/EDMC_SpanshRouter
  - https://github.com/dwomble/EDMC-NeutronDancer
  - https://github.com/wuuthradd/EDMC-SpanshTools
  - https://github.com/Fenris159/EDMC_GalaxyGPS
  - https://github.com/inorton/EDMCOverlay
  - https://github.com/SweetJonnySauce/EDMCModernOverlay
  - https://github.com/lekeno/edr
  - https://github.com/alby666/EDMC-PowerPlayProgress
  - https://github.com/Fumlop/EliteMeritTracker
  - https://github.com/SweetJonnySauce/EDMC-Mining-Analytics
  - https://github.com/Balvald/ArtemisScannerTracker
  - https://github.com/dwomble/EDMC-ExplorerLite
  - https://github.com/bgol/LandingPad
  - https://github.com/kfpopeye/EliteDangerous (Architect Tracker)
  - https://github.com/Greybaer/EDMC-ConstructionTracker
  - https://github.com/Fenris159/ravencolonial_edmc
  - https://github.com/SweetJonnySauce/EDMCHotkeys
- Odyssey Materials Helper
  - https://github.com/jixxed/ed-odyssey-materials-helper
  - https://edomh.nl, plus companion sites schemas.edomh.nl, math.edomh.nl, pinplanner.edomh.nl
- Observatory and plugins
  - https://github.com/Xjph/ObservatoryCore
  - https://observatory.xjph.net
  - https://edjp.colacube.net/observatory (MattG: BioInsights, Evaluator…)
  - https://github.com/fredjk-gh/ObservatoryPlugins
  - https://github.com/mcmuttons/GeoPredictor
- Exploration Buddy: https://www.panostrede.de/EDEB/ (+ /changelog.html), EDCodex entry 585
- EDCoPilot
  - https://www.razzafrag.com (/featurelist, /voice-commands, /blog, /eula)
  - https://github.com/Razzafrag/EDCoPilot-Installer
  - https://edcopter.net
  - https://github.com/ScronicDeEggdog/EDCoLauncher
- OD Elite Tracker
  - https://github.com/WarmedxMints/ODEliteTracker
  - https://github.com/WarmedxMints/ODMissionStacker
- SrvSurvey and Raven Colonial
  - https://github.com/njthomson/SrvSurvey (+ /wiki)
  - https://ravencolonial.com
  - https://github.com/njthomson/RavenColonialWeb
  - https://github.com/Fenris159/SrvSurvey (XP fork)
- EDEngineer: https://github.com/msarilar/EDEngineer (archived)

**Voice and AI assistants**
- https://github.com/EDCD/EDDI
- https://github.com/RatherRude/Elite-Dangerous-AI-Integration (COVAS:NEXT)
- https://github.com/SudoKrondor/EliteIntel
- https://github.com/dseelinger/d47 (issue #11 mission-UX analysis)
- https://github.com/Somfic/EliteAPI
- https://store.steampowered.com/app/3046550/VoiceAttack_v2/
- https://www.hcsvoicepacks.com

**Colonisation**
- https://github.com/gaborauth/ed-colonisation-planner (EDCPS)
- https://github.com/ChITty98/EliteDanagerousColonyTracker (ED Colony Architect)
- https://github.com/Kailoren/EDCT
- https://github.com/anthonylangsworth/Colonisation
- https://aetherwave.co/guides/colonisation-reference/
- https://spansh.co.uk/colonisation

**Ship builders**
- https://edsy.org and https://github.com/taleden/EDSY
- https://coriolis.io, https://github.com/EDCD/coriolis, https://github.com/EDCD/coriolis-data
- https://cmdr.coriolis.io

**BGS, Powerplay, carriers, mining, trade**
- BGS
  - https://elitebgs.app
  - https://edservertick.de
  - https://edcas.de
  - https://github.com/arne-fuchs/edcas-client
  - https://elite-bgs.store (ETN)
  - https://codeberg.org/nola/EDBGS
- Powerplay
  - https://edpowerplay.com
  - https://elite.niceygy.net
  - https://k5elite.com
- Carriers
  - https://github.com/skywalker-elite/Elite-Dangerous-Carrier-Manager (EDCM)
  - https://github.com/mck-9061/CATS
  - https://pilotstradenetwork.com
  - https://elitecargolog.maxdeals.net
  - https://github.com/djdarcy/EDAPITool
- Mining
  - https://github.com/Viper-Dude/EliteMining
  - https://github.com/EDRockMaster/edrockmaster-plugin
  - https://edtools.cc
  - https://www.radioraxxla.com/EDSMT/
  - https://github.com/pbgaspar/rhino-surface-mapper
- Trade
  - https://ardent-insight.com, https://github.com/iaincollins/ardent-api
  - https://github.com/eyeonus/Trade-Dangerous
- Utilities
  - https://github.com/rfvgyhn/min-ed-launcher
  - https://github.com/RedUnix/ed-unified
  - https://github.com/BlueMystical/EDHM_UI
  - https://github.com/brammmers/edrefcard2
  - https://github.com/drworman/EDLD
  - https://github.com/Signal-Thread-LLC/EDNexus

**Data services and specs**
- EDDN
  - https://github.com/EDCD/EDDN (live branch: README.md, docs/Developers.md)
  - https://eddn.edcd.io
- CAPI
  - https://github.com/Athanasius/fd-api (CAPI docs)
  - https://github.com/EDCD/FDevIDs
- Journal schemas and manual
  - https://github.com/jixxed/ed-journal-schemas
  - https://elite-journal.readthedocs.io
  - https://hosting.zaonce.net/community/journal/v38/Journal_Manual_v38.pdf
- Galaxy data and research
  - https://spansh.co.uk (+ Patreon /about)
  - https://status.edsm.net
  - https://edastro.com
  - https://canonn.science
  - https://github.com/canonn-science
  - https://github.com/klightspeed/EliteDangerousRegionMap
- Rust crates
  - https://github.com/rster2002/ed-journals
  - https://docs.rs/ed-journals
  - https://github.com/nixpulvis/galos

**Community threads** (titles and paraphrases only)
- r/EliteDangerous
  - "Post the third party apps/tools you use most often" (2025-03-17): https://www.reddit.com/r/EliteDangerous/comments/1jd8l8j/
  - "Ed copilot Vs the eco system" (2026-10-01): https://www.reddit.com/r/EliteDangerous/comments/1wuws95/
  - Newcomer tools thread (2026-09-08): https://www.reddit.com/r/EliteDangerous/comments/1wad5fg/
  - "Please Upload Your Data When You Explore" (2026-06-21): https://www.reddit.com/r/EliteDangerous/comments/1ubt7g8/
  - Linux/Proton (2026-08-02, 2026-02-19): https://www.reddit.com/r/EliteDangerous/comments/1vdwo9y/ · https://www.reddit.com/r/EliteDangerous/comments/1r95c68/
  - VR workflows (2024-04-10; 2025-03-22; 2025-11-21): https://www.reddit.com/r/EliteDangerous/comments/1c0ortl/ · https://www.reddit.com/r/EliteDangerous/comments/1jh4bil/ · https://www.reddit.com/r/EliteDangerous/comments/1p33skw/
  - Colonisation tooling (2025-03-01; 2025-10-13): https://www.reddit.com/r/EliteDangerous/comments/1j1atrc/ · https://www.reddit.com/r/EliteDangerous/comments/1o55bb2/
- Frontier forums (snippets only, pages returned 403): EDCoPilot threads 633380 / 637766 / 644356; ModernOverlay 644427; NeutronDancer 644411; BGS-Tally 644424; OD Elite Tracker 637488; Raven Colonial 639577; EDEB 614947 / 615445.
- Steam: EDEB HUD/borderless (2024-10-30); EDCoPilot AV and VoiceAttack (2024-11, 2025-02); ship builders (2025-03-21).

