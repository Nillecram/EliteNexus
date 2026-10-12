# Community websites — replicate/link spec for Elite Nexus

Research date: **2026-10-12**. Markers: **[VERIFIED]** means checked against the live site or source code during this research. **UNVERIFIED** means inferred or not directly checked.

**Bottom line**:

- INARA and the EDSM *website* both block scripted access, and INARA's terms forbid scraping and republishing. Elite Nexus should **re-implement** their search features on its own data and only **deep-link** to the sites:
  - **EDDN** for live data
  - **Spansh dumps and API** for bulk data and search
  - the **local journal** for everything personal
- The open, documented APIs that are fine to use are EDSM's API, EDAstro's API, Spansh, and Frontier's CG feed.

## 1. INARA (https://inara.cz)

### 1.0 Access and terms [VERIFIED 2026-10-12]

- **Bot check**: non-browser clients get HTTP **503** with an "INARA Access check required … make sure you're a real visitor and not a bot" page and `Retry-After: 3600`. Only `https://inara.cz/robots.txt` returned 200. The page details below come from **Wayback Machine snapshots** (`https://web.archive.org/web/<timestamp>/<url>`, Sept–Oct 2026 captures) and were cross-checked in the saved HTML.
- **Terms** (https://inara.cz/elite/policies/):
  - Use is personal and **non-commercial** only.
  - Content must not be copied or redistributed.
  - It is forbidden to use "automated means (such as bots, scrapers, or crawlers) to access or collect content … in a way that violates these Terms".
- **robots.txt** disallows:
  - all `/elite/cmdr-*` pages and `/elite/market-traderoutes-search/`
  - several `minorfaction-*`/`starsystem-*` subpages and `ajax*.php`
  - any URL containing `?formbrief=` or `?setlanguage=`

  It fully blocks several crawlers (PerplexityBot, SemrushBot, AhrefsBot, Yandex, …).
- **Consequence**: Elite Nexus must not fetch INARA pages. It may open them in the user's browser with prefilled query strings, i.e. deep links.

### 1.1 Search URL conventions

- All searches are `GET` forms. `ps1` is the reference ("near") system name, `piN` are integer selects, `paN[]` are multi-selects, `psN` are strings, and `formbrief=1` is added by the forms.
- **IDs are INARA-internal**, not Frontier IDs. For example commodity `61` = Advanced Catalysers, ship `xship70` = Anaconda, module `129773` = "1D Abrasion Blaster [Fixed]".
- Shared enums:
  - **Powers**: 1 Patreus, 2 A. Duval, 3 Mahon, 4 A. Lavigny-Duval, 5 Winters, 7 Li Yong-Rui, 8 Torval, 9 Antal, 10 Delaine, 11 Grom, 12 Archer, 13 Kaine. `100+N` means "controlled by N".
  - **Powerplay state**: 0 Unoccupied, 1 Expansion, 2 Contested, 3 Exploited, 4 Fortified, 5 Stronghold.
  - **Pad**: 1 S, 2 M, 3 L.
  - **Max station distance**: 0 or 100–100000 Ls.
  - **Surface stations**: 0 yes (incl. Odyssey settlements), 2 yes (excluding Odyssey), 1 no.
- **Official deep links** (dev guide):
  - `https://inara.cz/elite/starsystem/?search=<NAME or SystemAddress>`
  - `https://inara.cz/elite/station/?search=<STATION [SYSTEM]>` or `?search=<CARRIER CALLSIGN>`
  - `https://inara.cz/elite/minorfaction/?search=<NAME>`

  Name lookups: `/elite/starsystems/?search=`, `/elite/stations/?search=`, `/elite/minorfactions/?search=`.

### 1.2 Features to replicate, and where Elite Nexus gets the data

The menu at the time of the check had these sections:

- **Data**: nearest, trade routes, commodities, components trading, outfitting
- **Engineering**: engineers, blueprints, components, experimental effects, synthesis, material traders
- **Ships**: ships, outfitting, tech broker
- **Galactic data**: star systems, stations, powers, minor factions, commanders, squadrons
- **Galaxy**: galaxy, community goals, bulletin board, security reports, powerplay, Thargoid war, rankings, statistics

| INARA feature | URL and key parameters [VERIFIED forms] | What it shows | Elite Nexus data source | Notes |
|---|---|---|---|---|
| **Commodity search (nearest buy/sell)** | `https://inara.cz/elite/commodities/?formbrief=1&pi1=<1 buy\|2 sell>&pa1[]=<commodityId>&ps1=<system>&pi10=<order 1 price\|2 supply\|3 distance\|4 updated>&pi11=<max Ly 0/50…5000>&pi3=<pad>&pi9=<max Ls>&pi4=<surface>&pi8=<carriers 0/2/1>&pi13=<stronghold carriers>&pi5=<max age h 0…4320, default 720>&pi12=<% better than average \| -1 anarchy black markets>&pi7=<min supply/demand>&pi14=<power>&pa4[]=<pp state>&ps3=<faction>` | Stations buying or selling the commodity with price, supply/demand, distance and age. The results table was not captured (UNVERIFIED columns). | **EDDN `commodity/3`** (live; carries `meanPrice` for "better than average"), bootstrapped from **Spansh `galaxy_stations`** (`market.commodities[]`). Alternative: Spansh `/api/stations/search` with a `market` filter (spansh.md §4.7 A). | Keep price history per (market_id, commodity) for averages and trends. |
| **Trade routes** | `https://inara.cz/elite/market-traderoutes/?ps1=&pi10=<cargo, default 720>&pi2=<max route Ly 10…80, 500, 1000>&pi5=<max age h 8…72>&pi3&pi9&pi4&pi7=<min supply>&pi12=<min demand>&pi8=<round trips>&pi14/pa4[] (source power/state)&pi15/pa5[] (target power/state)&pi1=<order 0 profit\|4 profit/h\|1 updated\|2 route dist\|3 dist>` | From → To station and system, per-leg commodity, buy price and supply, sell price, profit and demand, route distance, distance from reference, updated, profit per unit, per trip and per hour | Local market DB plus coordinates, or **Spansh `/api/trade/route`** | Single-hop and round-trip search over the market DB within N Ly is straightforward locally. |
| **Components trading** (Odyssey goods, assets and data at carrier bars) | `https://inara.cz/elite/market-materials/?pi1=<buy\|sell>&pa1[]=<cat\|id>&ps1=&pi2=<min amount>&pi3=<max price>` | Carrier (name and callsign), system, selling/buying, item, amount, price, distance, updated | **EDDN `fcmaterials_journal/1` and `fcmaterials_capi/1`**. Probably not in the Spansh dumps (UNVERIFIED). The player's own data comes from the journal `ShipLocker`/`BackPack`. | |
| **Outfitting and ship search** | `https://inara.cz/elite/nearest-outfitting/?pa3[]=<xshipN \| moduleId \| equipN>&ps1=&pi18=<pad>&pi19=<Ls>&pi17=<surface>&pi14=<ignore stronghold carriers>&pi2=<ignore fleet carriers>&pi21=<discounted / higher equip chance>` | Nearest stations selling a ship, module or personal equipment | **EDDN `shipyard/2` and `outfitting/2`** + Spansh `galaxy_stations` (`shipyard.ships[]`, `outfitting.modules[]`) or the Spansh `/api/stations/search` `ships`/`modules` filters | Personal equipment (suits, weapons) and "discount" data have no EDDN schema, so they are effectively INARA-only (UNVERIFIED). |
| **Nearest stations by service**: **material traders, technology brokers, Interstellar Factors**, UC, Vista Genomics, Pioneer Supplies, … | `https://inara.cz/elite/nearest-stations/?ps1=&pa1[]=<service>&pi18=<pad>&pi19=<Ls>&pi17=<surface>&pi2=<ignore carriers>&pi16=<station type>` and many faction/system filters (`pi13,pi14,pa5[],ps2,pi25,pi8,pi9,pi10,pi11,pa2[],pi26,pi3,pi4,pi5,pi7,pi23,pi6,pi20,pa4[],ps3,pi24`). **Service IDs**: 4 market, 3 black market, 9 outfitting, 14 shipyard, 18 Interstellar Factors, 25 material trader (25-10 raw, 25-11 manufactured, 25-12 encoded), 26 technology broker (26-10 human, 26-20 **Sirius**, 26-30 guardian), 6 Universal Cartographics, 38 Vista Genomics, 39 Pioneer Supplies, 41 Frontline Solutions, 37 Bartender, 33 Redemption Office, 23 Power Contact, 42 System Colonisation, 43 Construction Services, 44 Refinery Contact, 32/29/28 carrier admin/services/vendor, 15 Tuning, 16 Workshop. **INARA presets**: `?pi17=1&pi18=3&pi19=5000&pa1[]=25` (material traders), `…&pa1[]=26` (tech brokers), `…&pa1[]=18` (IF). | Nearest stations with that service | **Spansh dumps** `stations[].services[]` (`"Material Trader"`, `"Technology Broker"`, `"Interstellar Factors Contact"`, …) + **EDDN `journal/1` Docked** (`StationServices`, `StationEconomies`, `StationType`, `LandingPads`, `DistFromStarLS`). **Trader and broker *type***: the Spansh API station record (`material_trader`: Raw/Manufactured/Encoded; `technology_broker`: Human/Guardian; spansh.md §5), or the community rule, which is UNVERIFIED (Raw ⇐ Extraction/Refinery, Manufactured ⇐ Industrial, Encoded ⇐ High Tech/Military; Guardian broker ⇐ High Tech, Human ⇐ Industrial). The player's own visits record `MaterialTrade.TraderType` / `TechnologyBroker.BrokerType` in the journal. | Spansh's enum has no "Sirius" broker. Store a `broker_type` override when the user's journal shows one. |
| **Nearest signals** (RES, nav beacons, tourist beacons, Titans) | `https://inara.cz/elite/nearest-signals/?ps1=&pi30=<20 beacon\|21 tourist beacon\|1 compromised nav beacon\|5 low RES\|6 RES\|7 high RES\|8 haz RES\|26 Titan>` plus system filters | Systems with that signal | **EDDN `fsssignaldiscovered/1`** (`SignalName`, `SignalType`, `USSType`, `SpawningState`, `SpawningFaction`, `ThreatLevel`, …) + local journal `FSSSignalDiscovered`. Curated `edtools res.json` exists (see §4). | Needed for massacre and RES farming tools. |
| **Nearest bodies** (materials, rings, landables) | `https://inara.cz/elite/nearest-bodies/?ps1=&pi40=<ring type>&pi41=<reserve 10…50>&pa4[]=<surface materials>&pi30=<order>&pi31=<star type>&pi32=<planet type>&pi33=<volcanism>&pi34=<atmosphere>&pi35=<terraforming>&pi36=<tidally locked>&pi37=<landable>` | Bodies matching filters, ordered by distance or material amount | **Spansh dump bodies** (`materials`, `rings`, `reserveLevel`, `isLandable`, `subType`, …) or the Spansh `/api/bodies/search`. EDDN `journal/1` Scan for freshness. | INARA has **no ring-hotspot filter**; Elite Nexus can add one (`SAASignalsFound` ring signals). |
| **Nearest misc**: wars, restore, mining and massacre missions, multi-state factions | `https://inara.cz/elite/nearest-misc/?ps1=&pi20=<1 wars/civil wars\|6 restore missions\|7 mining missions\|9 massacre missions\|10 multistate factions>` | Wars: system, surface conflicts, faction, opponent, distance | **EDDN `journal/1` FSDJump/Location** `Conflicts[]` (`WarType`, `Status`, `Faction1/2{Name,Stake,WonDays}`) and `Factions[]` (states). The mission lists are heuristics; INARA's rules are UNVERIFIED. The edtools massacre logic is in §4. | |
| **Engineers** | `https://inara.cz/elite/engineers/` (tabs: ship modules, personal equipment, unlock requirements), `/elite/engineer/{id}/`. Also `/elite/blueprints/`, `/elite/components/`, `/elite/experimentaleffects/`, `/elite/synthesis/`. | Engineer base (station and system), max grade per blueprint, unlock requirements | **Static reference data**: blueprints and grades come from coriolis-data `modifications`, and the vocabularies from Spansh `/api/engineers`, `/api/blueprints`, `/api/effects`. Engineer bases and unlock requirements are a small static table (on the order of 20–30 ship engineers plus about a dozen Odyssey suit/weapon engineers; exact counts UNVERIFIED). Maintain it in-app rather than copying INARA. **Player state** comes from the journal `EngineerProgress` (`Engineers[]{Engineer, EngineerID, Progress Known/Invited/Acquainted/Unlocked/Barred, Rank, RankProgress}`), `Materials`, `EngineerCraft`, `ShipLocker`. | Material counts are personal, so do the "can I craft this?" check locally. |
| **Community goals** | `https://inara.cz/elite/communitygoals/` | Title, station and system, objective, tier x/y, contributors, progress %, time left, INARA's estimated completion, a tier and reward table (top-10 / top-N % bands with contribution ranges), description | **Frontier feed** `GET https://api.orerve.net/2.0/website/initiatives/list?lang=en` [VERIFIED: 200, CORS `*`; `activeInitiatives[]{id, title, bulletin, news, objective, activityType, starsystem_name, market_name, target_commodity_list, qty, target_qty, expiry, images}`, all values strings] + **journal `CommunityGoal`** (the player's contribution, tier reached, percentile band). | The percentile contribution ranges are crowd-sourced through INARA's API (`setCommanderCommunityGoalProgress`), so they are INARA-only. |
| **Powerplay** | `https://inara.cz/elite/powers/`, `/elite/power/{id}/`, `/elite/powerplay/` | Powers, stronghold counts, cycle countdown, systems | **EDDN `journal/1` FSDJump/Location/CarrierJump** (`ControllingPower`, `Powers[]`, `PowerplayState`, `PowerplayStateControlProgress`, `PowerplayStateReinforcement`, `PowerplayStateUndermining`, `PowerplayConflictProgress[]`) + Spansh dumps (`controllingPower`, `powers`, `powerState*`, `powerConflictProgress`) + EDSM `powerPlay.json.gz`. The player's own data is in the journal `Powerplay`, `PowerplayMerits`, `PowerplayRank`. | Aggregates such as counts per power and state are computed locally from the system table. |
| **BGS / minor factions** | `https://inara.cz/elite/minorfactions/?search=`, `/elite/minorfaction/?search=`, `/elite/nearest-minorfactions/`, `/elite/nearest-starsystems/` (fields UNVERIFIED; captures were challenge pages) | Faction presence, influence, states, controlled assets | **EDDN `journal/1`** `Factions[]` (Influence, FactionState, Active/Pending/RecoveringStates, Happiness, Allegiance, Government; personal fields are stripped by the EDDN schema) + `SystemFaction`; Spansh `galaxy_populated` `factions[]`; Spansh `factions.json.gz` (presence index). | Influence **history** needs our own time series built from EDDN, because dumps are snapshots. |
| Commanders, squadrons, logbooks, rankings, gallery, forum | various | – | INARA-only (crowd and user content). | Link out only. |

### 1.3 INARA API (optional "sync my commander to INARA") [VERIFIED dev guide and docs snapshots]

**Request**:

- `POST https://inara.cz/inapi/v1/` with uncompressed JSON: `{"header":{…},"events":[…]}`.
- Header fields:
  - `appName`, `appVersion` (semantic, e.g. `1.2.4`)
  - `isBeingDeveloped` (true skips global events like `setCommunityGoal`)
  - `APIkey`: the user's personal key from `https://inara.cz/elite/cmdr-settings-api/`, or an app-wide generic key for read-only events
  - `commanderName` (take it from the journal or cAPI)
  - `commanderFrontierID` (`F123456`)
- Each event has `eventName`, `eventTimestamp` (ISO 8601, ≤30 days old), optional `eventCustomID`, and `eventData`.

**Response**: `eventStatus` per event: 200 OK, 202 warning, 204 no results, 400 error. A failed authorisation fails the whole batch.

**Events**: about 50, nearly all writes:

- Credits, permits, statistics, ranks (pilot, engineer, power), reputation (major and minor)
- Inventory (general, cargo, materials), storage modules, ships and loadouts, transfers, suit loadouts
- Travel: `addCommanderTravelFSDJump`, `…CarrierJump`, `…Dock`, `…Land`, `setCommanderTravelLocation`
- Missions, combat log, friends, `setCommanderCommunityGoalProgress`, `setCommunityGoal`

The only reads are **`getCommanderProfile`** and **`getCommunityGoalsRecent`** (both work with the generic key). The docs say the API offers no markets, faction influence, systems or stations.

**Registration**: the app must be **registered/whitelisted**. The dev guide asks developers to tell the author the exact app name/ID they use, with a purpose, description and homepage. New apps start in development mode.

**Limits**: no number is published. The dev guide asks clients to:

- batch events and send them on a timer or at session milestones
- never send one event per request ("excessive amount of requests … may lead to the API key invalidation")
- never send Legacy 3.8 or beta data
- never mass-scrape commander profiles

**Recommendation**: build INARA sync as an opt-in exporter. Translate journal events into INARA events in batches every few minutes, and on Docked or Shutdown. This is the same pattern EDMC's `plugins/inara.py` and EDDiscovery's `InaraSync.cs` use; read them for the event mapping.

## 2. EDSY and Coriolis: loadout links and data licensing

### 2.1 How EDMC builds the links [VERIFIED: EDMC `plugins/common_coreutils.py`, `plugins/edsy.py`, `plugins/coriolis.py`, `monitor.py`]

1. **Payload**: EDMC calls `monitor.ship()`, which returns a *reduced* journal `Loadout` object:
   - `timestamp` (now, `%Y-%m-%dT%H:%M:%SZ`), `event:"Loadout"`, `Ship`, `ShipID`, `ShipName?`, `ShipIdent?`.
   - `Modules[]`, sorted with hardpoints first, then standard slots in the order `ShipCockpit, CargoHatch, Armour, PowerPlant, MainEngines, FrameShiftDrive, LifeSupport, PowerDistributor, Radar, FuelTank`, then other slots.
   - Each module has `Health` and `Value` **removed**. Fields such as `UnladenMass`, `FuelCapacity`, `HullValue` and `Rebuy` are *not* included.

   Sending the full journal `Loadout` event should also work, because both import paths read `Ship` and `Modules` (see 2.2). This is inferred from the source and was not tested in a browser.
2. **Encoding**:
   ```python
   string = json.dumps(loadout, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8')
   gzip it (gzip.GzipFile, i.e. RFC 1952 gzip, not raw zlib)
   encoded = base64.urlsafe_b64encode(gz).decode().replace('=', '%3D')
   ```
3. **URLs**:
   - EDSY: `https://edsy.org/#/I=<encoded>`, or beta `https://edsy.org/beta/#/I=<encoded>`.
   - Coriolis: `https://coriolis.io/import?data=<encoded>`, or beta `https://beta.coriolis.io/import?data=<encoded>`. EDMC lets users override the URL, which must end with `/import?data=`.

   The same encoded string works for both sites.
4. **Length limit**: very long URLs can break in browsers. EDMC's option `use_alt_shipyard_open` writes `last_shipyard.html`, a local redirect page containing the link, and opens that file instead. Coriolis warns when `location.href.length >= 2083`. EDSY detects MSIE truncation at 2083 characters.

Example (synthetic one-module Loadout, generated locally). The gzip magic bytes always make the payload start with `H4sI`:
```
https://edsy.org/#/I=H4sIABarzGoC_yXLwQqCQBSF4Xc56wnGWbSYtQRCUWCrIkSaG15QR2auhonv3lhwVufnW3Dybmwpwt4XFEIdLLiXqpkHCi7wRFXkD5nq2dYxGiice1gJIylcAvvAMsNqhbL1kuwh1B2VDb8k3zDWR0oNDylFdvTm3lHA_yty2EyBJuo3evS186OkKNxRlLrblNFmv8v0LjNXre1vN6xfxamCobgAAAA%3D
https://coriolis.io/import?data=H4sIABarzGoC_yXLwQqCQBSF4Xc56wnGWbSYtQRCUWCrIkSaG15QR2auhonv3lhwVufnW3Dybmwpwt4XFEIdLLiXqpkHCi7wRFXkD5nq2dYxGiice1gJIylcAvvAMsNqhbL1kuwh1B2VDb8k3zDWR0oNDylFdvTm3lHA_yty2EyBJuo3evS186OkKNxRlLrblNFmv8v0LjNXre1vN6xfxamCobgAAAA%3D
```

Rust equivalent:
```rust
let json = serde_json::to_vec(&reduced_loadout_sorted)?;          // sort keys: build from a BTreeMap
let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
gz.write_all(&json)?;
let enc = base64::engine::general_purpose::URL_SAFE.encode(gz.finish()?).replace('=', "%3D");
let edsy = format!("https://edsy.org/#/I={enc}");
let coriolis = format!("https://coriolis.io/import?data={enc}");
```

### 2.2 What the sites accept [VERIFIED in their source]

**EDSY** (`edsy.js`):

- `processURLHash` handles `#/L=<native build hash>` and `#/I=<import>`.
- `importData` first URI-decodes the text, base64-decodes it (URL-safe alphabet, padding optional), and runs `pako.inflate` (auto-detects gzip or zlib). If that fails it treats the input as plain text.
- It then accepts:
  - **SLEF** arrays (`[{header:{appName…}, data:{Ship…}}]`)
  - single journal events or arrays of them (`Loadout`, plus `EngineerCraft`)
  - Frontier CAPI `/profile` JSON
  - a pasted list of EDSY/ED Shipyard URLs
  - `FDAPI:<cmdr>` lines, which trigger EDSY's own CAPI import
- EDSY also emits `edomh://edsy/?<b64 deflate>` deep links for ED Odyssey Materials Helper.

**Coriolis** (EDCD/coriolis `src/app/Coriolis.jsx`):

- Routes `/import?` and `/import/:data` decode with `pako.inflate(Buffer.from(data,'base64'))`.
- They accept:
  - SLEF: one build imports directly; several builds open the import modal.
  - CAPI ship JSON (`modules`)
  - journal Loadout (`Modules`)
- Coriolis now also has a "CMDR link" flow from `cmdr.coriolis.io`, which passes a key via postMessage or `#/cmdr-linked/<b64>`. It is new and out of scope (UNVERIFIED purpose).

**SLEF** (Ship Loadout Exchange Format, EDCD) is `[{"header":{"appName":"…","appVersion":"…","appURL":"…"},"data":{<journal Loadout>}}]`. EDDiscovery emits it in `Ship.JSONLoadout(slef:true)`. Spansh's galaxy plotter also requires SLEF or Coriolis JSON in `ship_build`.

### 2.3 Licensing: can Elite Nexus embed the data?

**EDCD/coriolis-data** (https://github.com/EDCD/coriolis-data, used for ship and module stats) [VERIFIED `LICENSE.md` on `master` and `develop`; `package.json` says `"license":"MIT"`]:

- `LICENSE.md` states that **all data and JSON files are the intellectual property of Frontier Developments plc and subject to Frontier's terms**.
- Only the *code* (JS, CSS, HTML and SVG) is MIT, copyright 2015 Coriolis.io / Colin McLeod.
- **Precedent**: EDMC (GPL-2.0+) vendors it as a git submodule, and its `coriolis-update-files.py` generates `modules.json`/`ships.json`. The Spansh front-end bundles `coriolis-data/dist`. EDDiscovery ships its own tables.
- **Recommendation**: embedding is common community practice under Frontier's fan-content tolerance. It is not covered by an open licence, so:
  1. keep the data in a separate, replaceable data package
  2. credit "Ship/module data: EDCD coriolis-data, © Frontier Developments plc"
  3. no commercial use
  4. be ready to remove it on request

  This is not legal advice.
- Hosted, compiled copies are at https://coriolis.io/data/ and https://beta.coriolis.io/data/ (README). Prefer pinning a git tag (latest observed `develop` `package.json` version 4.1.8) over runtime fetching.

**EDSY** (https://edsy.org) [VERIFIED header of `edsy.js` and `eddb.js`]:

- Code and design are © taleden under **CC BY-NC 4.0**.
- Game data (`eddb.js`, `version 424019902`, `edsy_lastmodified_db 20260923`) is Frontier property, used under the Frontier media-usage rules.
- It may be reused only non-commercially, with attribution. Prefer coriolis-data or our own tables, and link to EDSY rather than copy it.

**EDCD/FDevIDs** (https://github.com/EDCD/FDevIDs) has CSV id/symbol/name tables (commodities, outfitting, shipyard, materials, …). No licence file was found (`LICENSE`/`LICENSE.md` return 404). Treat it the same way as coriolis-data.

## 3. EDAstro (https://edastro.com)

### 3.1 Journal upload API (what "send to EDAstro" does) [VERIFIED]

**Who sends**: EDDiscovery (`EliteDangerousCore/EliteDangerous/3rdPartyInterfaces/EDAstro/EDAstroClass.cs`, `EDAstroSync.cs`) and EDMC.

- Since EDMC **6.0.0** the uploader is a core plugin, `plugins/edastro_core.py`.
- The third-party "EDAstro" EDMC plugin (https://github.com/Silarn/EDAstro, GPL-2.0) now only says "no longer needed as of EDMC 6.0.0"; its functionality was disabled on 2025-12-15.
- The older ATEL-EDMC plugin (https://github.com/Intergalactic-Astronomical-Union/ATEL-EDMC) is deprecated in favour of EDDN.

**Step 1: discover accepted events.** `GET https://edastro.com/api/accepting` returns a JSON array of journal event names. Today it returned [VERIFIED 2026-10-12]:

```json
["CarrierJumpRequest","CarrierStats","Docked","ScanOrganic"]
```

The server decides; EDD and the Silarn plugin re-read this list (Silarn every hour). EDMC core hard-codes `CarrierStats`, `CarrierJumpRequest` and `ScanOrganic`.

**Step 2: upload.** `POST https://edastro.com/api/journal` with `Content-Type: application/json`. The body is a JSON **array** whose first element is a header and whose remaining elements are journal events:

```json
[{"appName":"EliteNexus","appVersion":"0.1.0","odyssey":true,"system":"Col 285 Sector …"},
 {"timestamp":"2026-10-12T09:00:00Z","event":"ScanOrganic","ScanType":"Analyse","Genus":"$Codex_Ent_Stratum_Genus_Name;","Genus_Localised":"Stratum","Species":"…","Species_Localised":"…","Variant":"…","Variant_Localised":"…","SystemAddress":123,"Body":12}]
```

Header fields by client:

- EDD sends `appName`, `appVersion` and `odyssey`, and batches up to **200 events per POST**, waiting 10 s between batches. It strips EDD-private `EDD*` fields.
- EDMC core sends `appName:"EDAstro"`, `odyssey` and `system`, one event per POST, with a 20 s timeout.
- Silarn sent `appName`, `appVersion`, `odyssey` and `system`.

**Response** is HTTP 200 with JSON `{"status": <code>, "message": "…"}`:

- `status` 200 means at least one event was accepted.
- `status` 401 means none were accepted but there was no error (EDMC treats both as success).
- Anything else is an error; log it.

A test endpoint `/api/journal.test` appears in EDAstro's own `send-test-to-api.sh` (https://github.com/Orvidius/edastro/blob/main/api/send-test-to-api.sh).

**Privacy filtering** (copy EDMC's whitelist). EDAstro says it collects nothing personally identifying; `CarrierStats` is the most sensitive event. EDMC whitelists these keys:

| Event | Keys sent |
|---|---|
| `CarrierStats` | `timestamp, event, Callsign, Name, CarrierID, AllowNotorious, PendingDecommission, DockingAccess, FuelLevel, Crew` |
| `CarrierJumpRequest` | `timestamp, event, SystemName, SystemAddress, CarrierID, Body, BodyID, DepartureTime` |
| `ScanOrganic` | `timestamp, ScanType, Genus, Genus_Localised, Species, Species_Localised, Variant, Variant_Localised, event, Body, SystemAddress` |

EDMC's source has a missing comma, `"BodyID" "DepartureTime"`, so it actually sends neither `BodyID` nor `DepartureTime`. Do not copy that bug.

The Silarn plugin made the carrier events opt-in. **Recommendation**: make EDAstro upload opt-in, with carrier events behind a separate toggle.

There is also a manual journal upload page at https://edastro.com/api/eddn-upload (web form, for back-filling).

### 3.2 Public data endpoints [VERIFIED https://edastro.com/api-details.html]

All endpoints are rate-limited to **100 requests per 15 minutes**. The counters arrive in the headers `RateLimit-Limit: 100`, `RateLimit-Remaining`, `RateLimit-Reset: <s>` [VERIFIED]. The site is behind Cloudflare.

| Endpoint | Returns |
|---|---|
| `GET https://edastro.com/api/starsystem?q={name\|id64}` | JSON **array** with one system object. Up to 10 comma-separated names/ids per call. |
| `GET https://edastro.com/gec/json/all` | All GEC (Galactic Exploration Catalog) POIs, in EDSM's GMP-JSON format |
| `GET https://edastro.com/gec/json/combined` | GEC plus Galactic Mapping Project POIs. `id` is unique only together with `source`. |
| `GET https://edastro.com/gec/json/rare` | "Rare" POIs |
| `GET https://edastro.com/gec/json/single/{POI-ID}` | One POI |
| `GET https://edastro.com/gec/json/id64/{id64}` | POI by system |
| `GET https://edastro.com/gec/json/nearest/{x}/{y}/{z}[/{minrating}]` | Nearest POI, optionally with a minimum explorer rating |
| `GET https://edastro.com/gec/json/favorites/{userID\|userName}` | A user's favourites |
| `GET https://edastro.com/gec/json/categories` | `[{"id":3,"name":"Stellar Features","curationOnly":false}, …]`. 18 categories: Undefined, Stellar Features, Planetary Features, Planetary Circumnavigation, Sights and Scenery, Green Gas Giants, Deep Space Outpost, Organic, Mystery and Xenology, Historical, Tourist Beacons, Memorials, Legacy, Community, Notable Stellar Phenomena, Nebulae, Glitches (curation only), Inhabited System [VERIFIED]. |
| `GET https://edastro.com/gec/json/stats` | Totals per region and category |
| `https://edastro.com/gec/new/{systemName}[/{POIname}]` | Web form to create a POI (requires login; just link to it) |

**`/api/starsystem` object** [VERIFIED for `q=Sol`, 13 KB]:

- System: `name`, `id64`, `coord_x/y/z`, `coordinates[]`, `id64sectorID`, `id64boxelID`, `id64boxelnum`, `id64mass`, `masscode`, `mainStarType`, `mainStarID`, `bodyCount`, `nonbodyCount`, `numELW`, `numWW`, `numAW`, `numTerra`, `FSSprogress`, `FSSdate`, `complete`, `date_added`, `edsm_id`, `edsm_date`.
- Bodies: `stars[]`, `planets[]` (`name`, `bodyId`, `bodyId64`, `distanceToArrivalLS`, `earthMasses`, `gravity`, `isLandable`, `materials[{Name,Percent}]`, `atmoComposition[{Name,Percent}]`, orbital elements, `meanAnomalyDate`, `eddn_date`, …), `barycenters[]`.
- Other: `codex[{codexname, codexname_local, category_local, subcategory_local, reportedOn}]`, `carriers[]`.

**Other public assets**: maps, charts and spreadsheets are linked from https://edastro.com/mapcharts/. The site also has the sector viewer https://edastro.com/sector, the galaxy map https://edastro.com/galmap, records https://edastro.com/records, exploration ship lists https://edastro.com/exploration/, and the ship-name generator https://edastro.com/shipnames/. Elite Nexus should *link* to these rather than replicate them.

### 3.3 What to replicate locally

- **Codex and exobiology tracker**: built from the local journal (`ScanOrganic`, `CodexEntry`, `SellOrganicData`, `FSSBodySignals`/`SAASignalsFound` genuses).
- **GEC POIs**: fetch `gec/json/all` at most daily, cache it, and show POIs near the current position. That respects the 100/15 min rate limit.
- **Carrier tracking**: from the journal (`CarrierJumpRequest`, `CarrierStats`, `CarrierJump`).
- **EDAstro source code**: https://github.com/Orvidius/edastro (GPL-3.0, Perl). The README calls it a backup, "never meant for human eyes"; it holds the map, spreadsheet and EDDN-tracker scripts. GEC is a separate repo, https://github.com/Orvidius/gec.

## 4. ED Tools (https://edtools.cc)

**General** [VERIFIED page sources fetched 2026-10-12]:

- A single-author PHP site; footers read "© CMDR VicTic" with a per-tool version and date (for example PvE v0.06, 2026-02-04).
- No robots.txt, terms, licence or public source repository were found. Contact details are on each tool's `?a=about` page.
- **No public API.** A few undocumented XHR/JSON endpoints exist, and some datasets are offered "for your own use" (below).
- Every tool takes a GET query string, so **deep-linking** from Elite Nexus is trivial and the polite option. Do not scrape it.
- There are no wing-mining, colonisation or trade tools: `/wing`, `/colonisation`, `/trade` and `/api` all return 404.
- `/x` and `/x.php` are equivalent.

| Tool | URL and parameters | Output columns | How it works and how Elite Nexus can replicate it |
|---|---|---|---|
| **PvE / massacre finder** | `https://edtools.cc/pve?s=<ref>&md=<radius 30–250 LY>&sc=<''\|2\|3>`, plus `&lo=on` for large pads only. Drill-down: `pve?s=<target>&a=rt`. | Distance, Source System, L-Pad/M-Pad/P-Pad (LS to the nearest station of each pad class, inferred), Fed/Imp/All/Ind (faction counts by allegiance), Target/Sources (`[N]` sources), RES/rings. The drill-down lists the unique non-anarchy mission-giving factions (Name, Allegiance, Government, State, Systems) and all sources. | Help text: "systems that give pirate massacre missions in only one 'Target' system". **Algorithm (inferred)**: for each populated *source* system, collect the systems within massacre-mission range (≈10 LY, UNVERIFIED) that host a pirate (Anarchy-government) faction. Keep sources with exactly one such *target*, then group by target so stacking potential (sources per target, factions per source) is visible. **Data**: populated systems, factions with government and allegiance, station pads and distances, and target rings and RES. All of this is in the Spansh `galaxy_populated` dump and EDDN `journal/1` FSDJump/Docked. RES availability comes from journal `FSSSignalDiscovered` (EDDN `fsssignaldiscovered/1`) or curated lists. |
| **Hotspots** | `https://edtools.cc/hotspot?s=<ref>&m=<mineral>&ms=<min hotspots>`, plus `&pop=on`. `m` is one of Alexandrite, Benitoite, Bromellite, Grandidierite, LowTemperatureDiamond, Monazite, Musgravite, Opal, Painite, Platinum, Rhodplumsite, Serendibite, Tritium. | Distance, System (`*` = populated), Ring (tooltip lists all hotspots), Type, Hotspots, LS. Only pristine rings; the 100 nearest. | Ring `SAASignalsFound` data. **Replicate** with the dump's `bodies[].rings[].signals` + `reserveLevel=="Pristine"`, or the Spansh bodies search `ring_signals` + `reserve_level` (see spansh.md §4.7 B). |
| **Miner's Tool** | `https://edtools.cc/miner?c=<cid>&s=<ref>&so=Distance\|Price\|Demand`. XHR: `miner?a=p&cid=46` (buyers JSON: system, station, price, demand, timestamp, market_id, pad, ago, coords, type) and `miner?a=r&n=Platinum` (curated hotspot overlaps). | "Where to mine": ref distance, hotspot system, ring, closest sell, port, price, when, LY. "Where to sell": ref distance, system, port, price, demand, when, closest hotspot. Also a "shortest trip" from ref → hotspot → buyer. | Commodity IDs are EDDB-era: 46 Platinum, 83 Painite, 97 Osmium, 276 LTD, 343 Rhodplumsite, 344 Serendibite, 345 Monazite, 346 Musgravite, 347 Benitoite, 348 Grandidierite, 349 Alexandrite, 350 Void Opals. **Replicate** with the local market DB (EDDN `commodity/3` + dump) joined to the hotspot DB, minimising `d(ref,hotspot)+d(hotspot,buyer)`. |
| **Multiple prices** | `https://edtools.cc/multi?s=<ref>&c_<cid>=on…` (one checkbox per mineral) | Distance, System, Station (pad), count of selected minerals bought, then price/demand/age per mineral | Page says prices come from EDDN every 30 minutes. **Replicate**: market DB query for stations with demand > 0 for several commodities. |
| **Mining Missions** | `https://edtools.cc/minmis?s=<ref>`, plus `&lp=on` | Distance, System, Station, LS, Pad, Economies, Type, State (with age). The 100 closest. | Stations *without* an Extraction or Refinery economy, which are the ones likely to offer mining missions (inferred). **Data**: station economies and state (dump, EDDN Docked). |
| **Rares** | `https://edtools.cc/rares?s=<ref>`, plus `&ico=on` and `&eng=on` (only rares needed for engineer unlocks) | #/Distance, System, Port, Dst LS, Commodity, Alloc, Cost, Illegal, Permit (141 rows) | Data is from EDDN, updated daily. Dataset: **https://edtools.cc/rares.json** (includes `market_id`, `port_state`, `updated_at`). **Replicate**: the market DB `is_rare` flag (Spansh station record `market[].is_rare`/`is_home_station`) or a static rare list. |
| **Distances** | `https://edtools.cc/dist`. XHR: `dist.php?a=mc&s=Sol` returns `{name,x,y,z}` (coordinates from EDSM). | Distance matrix, route length, round trip, all computed client-side | Trivial locally. |
| **Sphere** | `https://edtools.cc/sphere`. XHR: `sphere.php?a=s&s=<sys>&d=<≤100>[&ico=on]` returns an HTML table. | Distance, Name, Faction, Population, Government, Allegiance, State, Security, Economy (populated systems) | Spatial query over the populated-system DB. |
| **List** | `https://edtools.cc/list?s=<ref>&ord=<1..25>`. JSON: `list?a=r&ord=N` returns `{name,url,url_title,list:[{name,coords,comment}]}`. Custom lines in `System\|comment` form are resolved via `sys_coord.php?s=` (proxies EDSM). | Dist, Name, Comment, plus a nearest-neighbour route (Jump, Name, Comment) and a 3D map (`map3d.php`, ED3D library / three.js r75) | 24 curated POI lists (engineer bases, Guardian beacons, Thargoid sites, brain trees, hotspot overlaps, RES combos). **Replicate**: user POI lists with a TSP route. Seed from EDSM GMP and EDAstro GEC (§3.2). |
| **Road to Riches (legacy)** | `https://edtools.cc/expl?s=<start>&d=<dist>&n=<count>&ap=on[&x=on]`. Straight line: `expl?a=v&f=<from>&t=<to>&r=30`. | #, Jump, System, Planets (ELW/TWW/HMC with ls), Visited (cookie) | Stale eddb.io data (`expl_pop.json` dates from 2020). Use Spansh R2R instead. |
| **Selenium** (obsolete) | `https://edtools.cc/selenium?s=<ref>` | Distance, System, Planet, LS, Gravity, Se share, Common/Rare/Very-rare | Planet `materials` share. The Spansh bodies search `materials` value_struct does the same. |
| **RES** (unmaintained) | `https://edtools.cc/res?s=<ref>`, with filters `f_cb`, `f_hz`, `f_hi`, `f_rg`, `f_lo`, `ico` | Distance, System, Beacon, Haz, High, Reg, Low, Notes (550 hand-entered rows) | Dataset at **https://edtools.cc/res.json** (2023-09-05). **Replicate** from EDDN `fsssignaldiscovered/1` / local journal `FSSSignalDiscovered`, keeping signals whose `SignalType`/`SignalName` denotes a resource extraction site. The exact identifier strings are UNVERIFIED. |

**Recommendation**:

- Link out to edtools with prefilled query strings: the current system for `s`, and the commodity or mineral.
- Re-implement the massacre finder, hotspot finder and miner's trip natively on the local DB, which is fed by Spansh dumps plus EDDN.
- If you want to use `rares.json`/`res.json`, credit the author and ask first.

## 5. EDRefCard (https://edrefcard.info)

**Code** [VERIFIED]:

- The live site runs **https://github.com/brammmers/edrefcard2**: MIT, "Copyright (c) 2017 Richard Buckle". It was created 2023-12-31 and its last commit is `f2e91f8` on 2025-05-03. https://edrefcard.info/devices shows "Version 1.4.20250503", which matches `www/scripts/bindings.py` `__version__`.
- The original **https://github.com/richardbuckle/EDRefCard** (MIT, archived, default branch `dev`) says it has been superseded by edrefcard2. It credits code derived "with permission" from CMDR jgm.
- Stack: a Python 3 CGI script (`www/scripts/bindings.py`) using `lxml` (XML), Wand/ImageMagick 6 (drawing) and `pickle` (saved metadata). There is a Dockerfile, and Apache rewrites live in `conf/apache/edrefcard.conf`.

**Input**: the user uploads a game `.binds` file. These live in `%LOCALAPPDATA%\Frontier Developments\Elite Dangerous\Options\Bindings\`, for example `Custom.4.x.binds`.

- The form POSTs multipart to `scripts/bindings.py` with field `bindings` (the file).
- Checkboxes choose which groups appear: `showship`, `showsrv`, `showscanners`, `showfighter`, `showonfoot`, `showmulticrew`, `showheadlook`, `showui`, `showgalaxymap`, `showcamera`, `showcommandercreator`, `showmisc`.
- `styling` is `modifier`, `group` or `none`. The code also supports `category`.
- `description` is optional, up to 190 characters, and *publishes* the card.

**Parsing (`parseBindings`)**:

- Reads every control element's `<Binding>`, `<Primary>` and `<Secondary>`, using the attributes `Device`, `DeviceIndex` and `Key`. `{NoDevice}` is skipped and `Neg_`/`Pos_` prefixes are stripped.
- `<Modifier>` children become numbered modifiers: joystick modifiers from 1, keyboard modifiers from 101. `<Hold>` becomes a pseudo-modifier.
- Control metadata comes from `www/scripts/controlsData.py`: `controls` has 423 entries with Group, Category, Order, Name, Type (Digital/Analogue), HasAnalogue and HideIfSameAs.
- Device keys are either Frontier names from the game's `DeviceMappings.xml` (`SaitekX52Pro`, `ThrustMasterHOTAS4`, …) or 8-hex vendor+product IDs (`231D0200` VKB, `044F0400` Cougar).
- Special cases: T16000M stick plus throttle becomes `T16000MFCS`, and the VPC MongoosT-50CM3 has 32-button modes.
- Ignored devices: Mouse, ArduinoLeonardo, vJoy and `16D00AEA` (EDTracker).

**Data** (`www/scripts/bindingsData.py`) [VERIFIED by parsing the AST, not executing it]:

- `supportedDevices` has **68** entries mapping a template to its handled device IDs, e.g. `HotasCougar → {Template:'hotascougar', HandledDevices:['044F0400']}`.
- `hotasDetails` has **115** device keys and about 3.8k boxes, e.g. `'Joy_1': {'Type':'Digital','x':2003,'y':557,'width':670}`; height defaults to 54. Positions are pixels on 3840×2160 JPEG templates in `www/res/`.
- `keymap` has **48** key-name prettifications.

**Output**:

- One JPEG per detected device (and per `DeviceIndex`), saved as `configs/<xx>/<id>-<template>[-1].jpg`.
- Text in each box auto-shrinks from 40 px. Colours are by modifier (14 colours cycled), by group (Ship Crimson, SRV MediumPurple, OnFoot CornflowerBlue, Galaxy map ForestGreen, UI DarkOrange, …) or by category.
- A **keyboard card** is a grouped list of key caps and actions, not a picture of a keyboard.
- Warnings cover unknown devices and digital binds placed on analogue axes.

**URLs**:

- `https://edrefcard.info/binds/<6 lowercase letters>`: a saved card.
- `/configs/<id>.binds`: the raw binds file.
- `/list` (supports `?deviceFilter=`): about 3,958 published cards.
- `/devices`: supported devices.
- `/device/<Key>`: a debug image with labelled boxes.
- There is no JSON API. robots.txt allows only `/index.html`, `/list` and `/devices`.

**Replicating it natively** (recommended rather than linking):

1. Watch the user's `Options/Bindings` folder (the `notify` crate). The active preset is probably named in `StartPreset*.start` (UNVERIFIED).
2. Parse `.binds` with `quick-xml`/`roxmltree`.
3. Normalise device keys, using the user's own `DeviceMappings.xml` for VID/PID ↔ name.
4. Group by control group and category, using a converted copy of `controlsData.py`.
5. Render an interactive overlay in the UI (egui/wgpu) and export PNG with the `image` crate plus `ab_glyph`/`cosmic-text`.
6. Convert the Python tables to JSON or RON once with an AST-based converter. Never `exec` them.

**Licensing**:

- The code and tables (including the coordinates) are MIT. Keep the copyright and permission notice and credit Buckle, jgm and brammmers.
- The **template JPEGs have no stated licence and visibly contain manufacturer product photos**, so do not ship them without permission (UNVERIFIED rights). The coordinates are only meaningful with those exact images. With our own artwork, we would re-measure the boxes but can reuse the per-device control-name lists.
- The default `.binds` and `DeviceMappings.xml` in the repo are Frontier files; do not redistribute them.

## 6. EDSM (https://www.edsm.net): website features beyond the API

### 6.0 Access status [VERIFIED 2026-10-12]

- **Website pages** (`/en/*`, `/robots.txt`, `/en/galactic-mapping/json`) return a **Cloudflare challenge** (403, `Cf-Mitigated: challenge`) to scripts. Page details below come from Wayback snapshots, Feb–Mar 2026.
- **The API and dumps are open**:
  - `GET https://www.edsm.net/api-status-v1/elite-server` returns `{"lastUpdate":"2026-10-12 09:36:38","type":"success","message":"Good","status":1}`.
  - `GET https://www.edsm.net/api-v1/system?systemName=Sol&showCoordinates=1` returns 200.
  - `HEAD /dump/*` returns 200.
- **Maintenance signals**:
  - The galactic map is labelled "Galactic Map (Archived)".
  - The Journal API page was last revised 2022-11-30 and the Logs API page 2019-11-14.
  - Legacy/console support was dropped at Update 14.
  - No shutdown notice was seen in the snapshots. The live notices could not be checked (UNVERIFIED).
  - Treat EDSM as a **secondary** source behind EDDN and Spansh.

### 6.1 Website features (menu, Mar 2026 snapshot)

| Feature | URL | Replicate in Elite Nexus? |
|---|---|---|
| Galactic Mapping POIs (GMP) | `/en/galactic-mapping` ("Archived"); data is in `/en/galactic-mapping/json` and `/en/galactic-mapping/json-description` (now behind the challenge) | Yes, as a POI overlay. Get the GMP POIs via **EDAstro `gec/json/combined`** (GEC + GMP), §3.2. |
| Expeditions, Routes, POI visitors | `/en/expeditions`, `/en/galactic-routes`, `/en/poi-visitors-list` | Expedition planner: user route plus progress, from the journal |
| **Commanders Map** (everyone's positions) | `/en/map/users` | Not personal; needs crowd data. Skip, or build traffic from EDDN. |
| **Traffic reports** | `/en/map/traffic` | Count EDDN `journal/1` FSDJump arrivals per system (anonymous) |
| Route planner | `/en/map/planner` | Spansh plotters |
| Codex | `/en/codex` | Local journal `CodexEntry` + EDSM `codex.json.gz` dump |
| Records and statistics | `/en/statistics/systems`, `/en/statistics/bodies`, `/en/statistics/bodies/planets` | Optional, computed from dumps |
| Top 100 lists | `/en/top/logs`, `/en/top/discovered`, `/en/top/visited-systems`, `/en/top/distances`, `/en/top/search-and-rescue` | No (crowd) |
| Guilds, Engineers, CG timeline | `/en/guilds`, `/en/engineers`, `/en/timeline/community-goals` | Engineers and CGs: see §1.2 |
| Tools | `/en/systems/pairs`, `/en/systems/duplicates`, `/en/systems/unresolved`, `/en/systems/stations` (oldest stations), `/en/tools/embed/examples`, `/en/tools/streaming` (streaming widget) | A streaming overlay widget is a nice-to-have: a local web overlay served by Elite Nexus |
| Faction, system and station pages | e.g. `/en/faction/id/{id}/name/{name}` | Our own detail panels (Spansh dump) |
| **Personal (login)**: commander profile, **flight logs**, **travel map**, badges and achievements, rank/credit graphs, ships and fleet | URLs UNVERIFIED (gated) | **Yes, all locally from the journal** (§6.4) |

### 6.2 API (one line each)

Where `[VERIFIED]` is absent, the detail comes from prior knowledge or the menu labels (UNVERIFIED).

| API | Doc URL | What it gives |
|---|---|---|
| Status v1 | https://www.edsm.net/en/api-status-v1 | `GET /api-status-v1/elite-server`: Frontier server status [VERIFIED live] |
| Systems v1 | https://www.edsm.net/en/api-v1 (the menu links here) | `GET /api-v1/system`, `/systems`, `/sphere-systems`, `/cube-systems`: names, coordinates, ids, permit, primary star [`/api-v1/system` VERIFIED live] |
| System v1 | https://www.edsm.net/en/api-system-v1 | Per system: bodies, estimated scan value, stations, market, shipyard, outfitting, factions (+history), traffic, deaths |
| Commander v1 | https://www.edsm.net/en/api-commander-v1 | Ranks, credits, materials/data/cargo for the key's commander |
| Logs v1 | https://www.edsm.net/en/api-logs-v1 | `get-logs` (commanderName + apiKey; `systemName`, `startDateTime`/`endDateTime`, max one-week window, `showId`), `get-position`, `set-comment`, `get-comment`, `get-comments`. **360 requests/hour/user** [VERIFIED doc snapshot]. |
| Journal v1 | https://www.edsm.net/en/api-journal-v1 | Upload raw journal events (§6.4) [VERIFIED doc snapshot] |

### 6.3 Nightly dumps (`https://www.edsm.net/dump/<file>`)

The page says the files are "generated every day, except the full bodies dump", but no full bodies file is listed.

| File | Content | Size per page (Mar 2026) | HEAD 2026-10-12 |
|---|---|---|---|
| `systemsWithCoordinates.json.gz` | All systems with coordinates | 3.28 GB | 2,353,770,496 B, modified 05:10 GMT |
| `systemsWithCoordinates7days.json.gz` | Systems with coordinates updated in the last 7 days | 8.84 MB | – |
| `systemsWithoutCoordinates.json.gz` | Systems known by name only | 16.95 MB | – |
| `systemsPopulated.json.gz` | Populated systems with factions and stations | 408.61 MB | 519,139,591 B, modified 04:40 |
| `powerPlay.json.gz` | Powerplay systems | 2.89 MB | – |
| `stations.json.gz` | Stations with market, shipyard and outfitting | 2.24 GB | 800,321,536 B, modified 02:28 |
| `codex.json.gz` | Codex entries | 336.78 MB | – |
| `bodies7days.json.gz` | Bodies updated in the last 7 days | 401.77 MB | 174,731,934 B, modified 07:00 |

The files are still regenerated daily. **Caution**: two of today's files are exact multiples of 4096 bytes and much smaller than in March. They may have been caught mid-write or truncated, so verify gzip integrity (CRC) before importing (UNVERIFIED). Spansh's dumps are larger, fresher and better documented (spansh.md §7); use EDSM dumps only as a fallback.

### 6.4 Travel map and flight log: how EDSM gets the data, and how to do it locally

**EDSM ingestion** [VERIFIED Journal API doc snapshot]:

- Clients `POST https://www.edsm.net/api-journal-v1`. Form-encoded and JSON bodies are both accepted; `GET` is refused.
- Required fields: `commanderName`, `apiKey` (the user's key from EDSM settings), `fromSoftware`, `fromSoftwareVersion`, `fromGameVersion`, `fromGameBuild` (live game only), and `message` (one journal line, or an array batch).
- Optional state fields carry context: `_systemAddress`, `_systemName`, `_systemCoordinates`, `_marketId`, `_stationName`, `_shipId`.
- Fetch the ignore list at startup: `GET https://www.edsm.net/api-journal-v1/discard`.
- Result codes:
  - 100 OK, 101 already stored, 102 older than stored, 103 duplicate (cached about 300 s), 104 crew session
  - 201–208 auth, software or version errors (205 blacklisted software, 208 legacy game)
  - 301–304 message errors
- EDSM's own description of the events:
  - `FSDJump` sets the commander position in the flight log, updates the ship's current system and removes fuel.
  - `CarrierJump` sets the commander position.
  - `Location` updates the system and station, and inserts an FSDJump on a location mismatch.
  - `Docked` sets the station.

**Elite Nexus local travel map** (no EDSM needed):

1. Tail `Journal.*.log` in `%USERPROFILE%\Saved Games\Frontier Developments\Elite Dangerous\`.
2. For every `FSDJump` and `CarrierJump`, and every `Location` whose system differs from the last recorded one, store `timestamp`, `StarSystem`, `SystemAddress`, `StarPos[x,y,z]` (Ly, Sol at the origin), `JumpDist` and `FuelUsed`.
3. Drop consecutive duplicates. Skip multicrew sessions where the player is not the captain.
4. Render a 3D polyline with time colouring, plus heatmap and statistics: first discoveries (`Scan` `WasDiscovered`), visited count and total distance.
5. **History import** for pre-install travel, as an option: page through the EDSM Logs API `get-logs` with the user's EDSM key in one-week windows (360 requests/hour, so about 5 years takes under an hour). Fill coordinates from the systems dumps; whether `get-logs` returns coordinates itself is UNVERIFIED.

INARA has no read-back of travel history (its API is upload-only).

**Optional "send to EDSM"**: implement the Journal v1 uploader as EDMC's `plugins/edsm.py` does. Keep it opt-in and respect the `discard` list.

## 7. Cross-cutting data-source matrix for Elite Nexus

| Data | Bootstrap | Live updates | Personal |
|---|---|---|---|
| Systems and coordinates | Spansh `systems.json.gz` / `galaxy_populated` | EDDN `journal/1` FSDJump/Location/CarrierJump, `navroute/1` | Journal |
| Stations and services | Spansh `galaxy_stations` (hourly) | EDDN `journal/1` Docked, `dockinggranted/1`/`dockingdenied/1`, `approachsettlement/1` | Journal Docked |
| Market | Spansh `galaxy_stations` | EDDN `commodity/3` | Journal Market (`Market.json`) |
| Outfitting and shipyard | Spansh `galaxy_stations` | EDDN `outfitting/2`, `shipyard/2` | `Outfitting.json`, `Shipyard.json` |
| Carrier bar materials | – | EDDN `fcmaterials_journal/1`, `fcmaterials_capi/1` | `ShipLocker` |
| Bodies, rings, hotspots, bio signals | Spansh `galaxy*.json.gz` | EDDN `journal/1` Scan/SAASignalsFound, `fssbodysignals/1`, `fssallbodiesfound/1`, `fssdiscoveryscan/1`, `scanbarycentre/1` | Journal |
| Signals (RES, CZ, beacons, megaships) | edtools `res.json` (curated, stale) | EDDN `fsssignaldiscovered/1`, `navbeaconscan/1` | Journal `FSSSignalDiscovered` |
| BGS and Powerplay | Spansh `galaxy_populated`, `factions.json.gz` | EDDN `journal/1` (Factions, Conflicts, Powerplay fields) | Journal `Powerplay*` |
| Community goals | Frontier `initiatives/list` | Same (poll every 15–30 min) | Journal `CommunityGoal` |
| POIs | EDAstro `gec/json/combined` (GEC + GMP) | Daily refresh | – |
| Ship and module stats | EDCD coriolis-data (Frontier IP, §2.3) | Pin a release | Journal Loadout |
| Engineers and blueprints | coriolis-data `modifications`, Spansh `/api/{engineers,blueprints,effects}` | – | Journal `EngineerProgress`, `EngineerCraft` |

The EDDN schema list is from https://github.com/EDCD/EDDN/tree/live/schemas [VERIFIED]: `approachsettlement-v1.0`, `blackmarket-v1.0`, `codexentry-v1.0`, `commodity-v3.0`, `dockingdenied-v1.0`, `dockinggranted-v1.0`, `fcmaterials_capi-v1.0`, `fcmaterials_journal-v1.0`, `fssallbodiesfound-v1.0`, `fssbodysignals-v1.0`, `fssdiscoveryscan-v1.0`, `fsssignaldiscovered-v1.0`, `journal-v1.0`, `navbeaconscan-v1.0`, `navroute-v1.0`, `outfitting-v2.0`, `scanbarycentre-v1.0`, `shipyard-v2.0`.

`journal/1` accepts `Docked`, `FSDJump`, `Scan`, `Location`, `SAASignalsFound`, `CarrierJump` and `CodexEntry`. It strips `*_Localised` keys and the personal faction fields (`HappiestSystem`, `HomeSystem`, `MyReputation`, `SquadronFaction`).

Journal event and field names mentioned in this document (EngineerProgress, MaterialTrade.TraderType, TechnologyBroker.BrokerType, CommunityGoal, Powerplay fields, Conflicts) come from the Frontier Player Journal documentation and community knowledge. They were not re-verified during this research (UNVERIFIED).

## 8. Sources

**INARA** (live site bot-gated; content from Wayback snapshots, Sept–Oct 2026 captures, `https://web.archive.org/web/<ts>/<url>`):

- Pages: https://inara.cz/elite/commodities/, https://inara.cz/elite/market-traderoutes/, https://inara.cz/elite/market-materials/, https://inara.cz/elite/nearest-outfitting/, https://inara.cz/elite/nearest/ (→ /elite/nearest-stations/), https://inara.cz/elite/nearest-signals/, https://inara.cz/elite/nearest-bodies/, https://inara.cz/elite/nearest-misc/, https://inara.cz/elite/engineers/, https://inara.cz/elite/communitygoals/, https://inara.cz/elite/powers/, https://inara.cz/elite/minorfactions/, https://inara.cz/elite/starsystems/, https://inara.cz/elite/stations/
- API: https://inara.cz/elite/inara-api/, https://inara.cz/elite/inara-api-devguide/, https://inara.cz/elite/inara-api-docs/
- Policies and robots: https://inara.cz/elite/policies/, https://inara.cz/robots.txt (live)

**EDSY and Coriolis**:

- EDSY: https://edsy.org/ (`edsy.js` `processURLHash`/`importData`; licence header in `edsy.js` and `eddb.js`)
- Coriolis: https://github.com/EDCD/coriolis (`src/app/Coriolis.jsx` routes `/import?`, `/import/:data`; `src/app/components/ModalImport.jsx`; `LICENSE.md`)
- coriolis-data: https://github.com/EDCD/coriolis-data (`LICENSE.md`, `package.json`, README)
- EDMC: https://github.com/EDCD/EDMarketConnector (`plugins/common_coreutils.py` `shipyard_url_common`, `plugins/edsy.py`, `plugins/coriolis.py`, `monitor.py` `ship()`, `EDMarketConnector.py` `shipyard_url`; `coriolis-data` git submodule)
- EDDiscovery SLEF: https://github.com/EDDiscovery/EliteDangerousCore/blob/master/EliteDangerous/History/Ships/Ship.cs
- FDevIDs: https://github.com/EDCD/FDevIDs

**EDAstro**:

- Pages: https://edastro.com/api-details.html, https://edastro.com/api-info.html, https://edastro.com/datasources.html, https://edastro.com/api/accepting (live), https://edastro.com/gec/json/categories (live), https://edastro.com/api/starsystem?q=Sol (live)
- EDMC core uploader: https://github.com/EDCD/EDMarketConnector/blob/main/plugins/edastro_core.py
- EDDiscovery: https://github.com/EDDiscovery/EliteDangerousCore/tree/master/EliteDangerous/3rdPartyInterfaces/EDAstro
- Former plugins: https://github.com/Silarn/EDAstro, https://github.com/Intergalactic-Astronomical-Union/ATEL-EDMC
- EDAstro code: https://github.com/Orvidius/edastro, https://github.com/Orvidius/gec

**ED Tools**: https://edtools.cc/ plus `/pve`, `/hotspot`, `/miner`, `/multi`, `/minmis`, `/rares`, `/dist`, `/sphere`, `/list`, `/expl`, `/selenium`, `/res`, their `?a=about` pages, `/rares.json` and `/res.json`. All fetched live; no robots.txt.

**EDRefCard**:

- Site: https://edrefcard.info/, https://edrefcard.info/devices, https://edrefcard.info/list
- Code: https://github.com/brammmers/edrefcard2 (live code), https://github.com/richardbuckle/EDRefCard (archived original)

**EDSM**:

- Live: https://www.edsm.net/api-status-v1/elite-server, https://www.edsm.net/api-v1/system, https://www.edsm.net/dump/*
- Wayback: https://www.edsm.net/en/nightly-dumps, https://www.edsm.net/en/api-journal-v1, https://www.edsm.net/en/api-logs-v1, https://www.edsm.net/en/galactic-mapping

**Frontier**: https://api.orerve.net/2.0/website/initiatives/list?lang=en (live)

**EDDN**: https://github.com/EDCD/EDDN/tree/live/schemas (`journal-v1.0.json`, `fsssignaldiscovered-v1.0.json`, `fcmaterials_journal-v1.0.json` read)
