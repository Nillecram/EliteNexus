# nexus-galaxy

Local galaxy / market database for Elite Nexus: systems, stations, markets, factions and ring
hotspots in SQLite (WAL), fed by EDDN, the player's own journal and Spansh dumps, with queries that
answer in about a millisecond.

## Architecture

* **One writer thread.** `GalaxyDb::ingest*` only push [`Ingest`] items onto a channel. The writer
  commits batches in one `BEGIN IMMEDIATE` transaction, at most once every 250 ms (a lone item after a
  quiet period is committed at once), then notifies `subscribe_updates()` receivers and refreshes the
  in-memory station index.
* **Reads on the caller's thread.** They use a small pool of `query_only` connections. With WAL,
  readers never wait for the writer.
* **In-memory station index** (~64 bytes per station). It holds position, pads, flags, services,
  trader/broker type and market age. Commodity searches scan the commodity-clustered covering
  partial indexes (`market_sellers` for "buy here", `market_buyers` for "sell here"), best price
  first, and filter each row against this index. That avoids one random page read per station, so
  queries stay fast with a cold cache. `stations_near` and the trade-route candidate search are
  in-memory scans; the database is touched only for the final rows.
* **Staleness rules.** A market snapshot older than the stored one is ignored, which is what happens
  when EDDN delivers late or a dump is older than live data. Station and system details are replaced
  only by newer information.
* `systems_rtree` (R*Tree) serves `systems_near`. Station positions are copied from their system.
  Markets that arrive before their system is known (EDDN `commodity/3` carries only the system
  *name*) are linked automatically once a `FSDJump` / `Location` / `Docked` / dump record supplies
  the system.

## Schema (v2)

| table | key | notes |
|---|---|---|
| `systems` | `id64` | `name_lower` indexed; x/y/z; population, allegiance, government, economies, security, controlling faction + state, power, powerplay state, powers (JSON), `updated` = time of the last *state* update (0 = position only) |
| `systems_rtree` | `id` | R*Tree over system positions |
| `stations` | `market_id` | system link (`system_id64`, name, copied x/y/z), normalised `type`, `dist_ls`, `max_pad` (1 S, 2 M, 3 L, 0 unknown), `is_planetary`, `is_carrier`, `is_odyssey_settlement`, `services` (bitmask, see `Services`), `mat_trader`, `tech_broker`, economies (JSON), faction, government, allegiance, carrier docking access, `market_updated`, `n_commodities`, prohibited (JSON) |
| `commodities` | `id` | small internal ids; `symbol` (lower-case, unique), name, category, FDev id, rare flag; seeded from FDevIDs `commodity.csv` + `rare_commodity.csv`, extended on the fly |
| `market` | `(market_id, commodity_id)` WITHOUT ROWID | buy, sell, mean, stock, stock/demand bracket, demand, updated; partial covering indexes `market_sellers (commodity_id, buy, stock, updated) WHERE buy > 0 AND stock > 0` and `market_buyers (commodity_id, sell, demand, updated) WHERE sell > 0 AND demand > 0` |
| `system_factions` | `(system_id64, faction)` | influence, state, government, allegiance, happiness, active/pending/recovering states (JSON) |
| `ring_signals` | `(system_id64, body_name, signal)` | hotspot count, ring type; `signal` is a commodity symbol (`painite`, `lowtemperaturediamond`, `opal`) |
| `price_history` | `(market_id, commodity_id, ts)` | buy and sell when a live snapshot changes a price (see below); pruned after 30 days |
| `bodies` | `(system_id64, body_id)` WITHOUT ROWID | planets and moons (see below): name (`COLLATE NOCASE`, indexed), planet class, atmosphere + type, gravity (g), temperature (K), pressure (atm), volcanism, terraform state, `distance_ls`, landable, `was_discovered` / `was_mapped` / `was_footfalled` each with its `*_seen` time, `footfall_source`, bio / geo signal counts, genera (JSON codex symbols), `scan_updated`, `signals_updated`, `updated`, `source`, `checked_online` |

Material trader type comes from the station economies, using FDevIDs' *How to determine MatTrader
and Broker type*:

* High Tech or Military gives Encoded.
* Extraction or Refinery gives Raw.
* Industrial gives Manufactured.

The primary economy decides; the secondary is used only when the primary doesn't match. Technology
brokers are Guardian at High Tech (primary, else secondary) and Human otherwise.

## Bodies (footfall, biology)

No public site exposes first footfall, so the `bodies` table collects the game's `WasFootfalled`
(and `WasDiscovered` / `WasMapped`) from journal `Scan` events, plus bio / geo signal counts and DSS
genera from `FSSBodySignals` / `SAASignalsFound`, and on-foot evidence (`Disembark` in the own
journal, `ScanOrganic` from EDDN). Every source produces a `BodyUpdate`; the writer merges them:

* physical facts: the newest scan replaces them (a scan is complete); an older one adds nothing;
* `was_*` flags are monotonic (once `true`, always `true`), each with a time: when `true` was first
  seen, or while `false` the newest observation that said so ("not footfalled as of …"). For
  footfall the commander's own journal is authoritative: its `false` overrides an older `true`
  from elsewhere, and such a `true` arriving later is ignored;
* signal counts come from the newest signals event; genera are never replaced by an empty list;
* `checked_online` remembers an on-demand online lookup (the app's EDDN-archive footfall lookup),
  so it is not repeated within a day.

What is kept: from the own journal every landable body, every body with signals, Earth-like / water /
ammonia worlds and terraformables. From EDDN (≈200 scans a minute, mostly bare rock) only biology
bodies, Earth-like / water / ammonia worlds, terraformables and on-foot evidence, which bounds
growth to explorer biology activity (`GalaxyOptions::all_eddn_bodies` keeps every landable body too:
roughly ten times more rows). Rows that exist are always updated. Stars, belt clusters and
non-landable planets are never stored, and body events add no `systems` rows. Queries:
`bodies_in_system(id64)`, `body_by_name(name)` (case-insensitive), `find_bodies(prefix, limit)`.

## Feeding it

```rust
let db = GalaxyDb::open(data_dir.join("galaxy.db"))?;
db.ingest_eddn(&envelope);                       // EDDN: commodity/3, journal/1, fssbodysignals/1, scanorganic/1, approachsettlement/1
db.ingest_journal("FSDJump", &event_json);       // own journal (FSDJump/Location/CarrierJump/Docked/SAASignalsFound/ApproachSettlement/Scan/FSSBodySignals/Disembark)
db.ingest_journal("Market", &market_json_file);  // our Market.json -> full market snapshot
db.ingest(Ingest::Raw(RawIngest::Eddn(v)));      // or let the writer thread convert
let rx = db.subscribe_updates();                 // MarketUpdate after each committed live snapshot
```

## Spansh dumps

`GalaxyDb::import_spansh_dump(reader, |progress| keep_going)` streams any of the dumps from
`https://downloads.spansh.co.uk/`:

* Input is gzip (auto-detected) or plain JSON: one record per line, `[` / `]` framing, trailing commas.
* It reads chunks of about 24 MB of JSON and parses them in parallel (rayon).
* At most 2 chunks are in flight to the writer, so memory stays bounded.
* Live EDDN ingestion keeps running through the same writer.

Record types are detected per line:

* galaxy records: system, factions, stations, markets, ring hotspots;
* `systems*` records: coordinates only;
* `factions` records: faction presence and controlling faction.

Which file to use:

| file | size (2026-10-12) | use |
|---|---:|---|
| **`galaxy_stations.json.gz`** | 4.2 GiB | **Best bootstrap for a market database.** It has every system that has a station, fleet carriers included, with services, pads, economies, markets, factions, powers and the ring hotspots of those systems. It is refreshed **at least hourly**, so a re-download is never more than an hour behind. Use `If-None-Match` / `ETag`. |
| `galaxy_1day.json.gz` | 1.4 GiB | Daily top-up: full records of systems that changed in the last 24 h. The importer upserts whole records, and older markets are skipped automatically. |
| `galaxy_7days.json.gz` / `galaxy_1month.json.gz` | 2.8 / 5.1 GiB | Catch up after missing days or weeks. |
| `galaxy_populated.json.gz` | 4.2 GiB | Populated systems only. It misses carriers and stations in unpopulated systems, so prefer `galaxy_stations`. |
| `systems_1day` / `_1week` / `_1month` / `_6months.json.gz` | 3.8 MB / 23 MB / 89 MB / 609 MB | Coordinates for `system_by_name` / `systems_near`. Tested: `systems_1day` streamed live, 120,786 systems in 4.1 s. |
| `factions.json.gz` | 16 MB | Faction presence and controlling faction only (no influence). Use it when you don't import galaxy records. |
| `systems_neutron.json.gz` | 173 MB | Not useful here; it is meant for a local neutron plotter. |
| `galaxy.json.gz`, `systems.json.gz` | 110 GiB, 5.9 GiB | Don't: far beyond what a companion needs. |

Suggested strategy:

1. Bootstrap once from `galaxy_stations`.
2. Keep live with EDDN (`nexus-eddn`).
3. After an offline period, apply `galaxy_1day` or `galaxy_7days`.

`If-None-Match` works on all files, and so does `Range` (HTTP 206), so interrupted downloads can be
resumed. Import cost is dominated by gzip inflate (single thread) and the writer: 75,000 to 83,000
market rows/s were measured through the public ingest path. That puts a full `galaxy_stations`
import at a few minutes.

## Measured query latency

Measured with `cargo run -p nexus-galaxy --release --example bench -- <db> 80000 --rebuild`. The
database was synthetic and bubble-like: 188,801 systems, 80,000 stations, 4.38 M market rows,
129,300 faction rows, 496 MB. The run was on the dev box, shared with other builds. Generating it
through the public ingest path ran at 83,000 market rows/s.

| query | median | p90 |
|---|---:|---:|
| `search_commodity` buy, global best price | 0.16 ms | 0.35 ms |
| `search_commodity` sell, global, pad L, no carriers | 0.18 ms | 0.38 ms |
| `search_commodity` buy, origin + 50 ly | 1.15 ms | 1.49 ms |
| `search_commodity` sell, origin + 200 ly, pad L, 48 h | 0.26 ms | 0.35 ms |
| `search_commodity` nearest (no max distance) | 3.06 ms | 3.54 ms |
| `search_commodity` freshest (global) | 3.01 ms | 3.18 ms |
| `station_market` | 0.06 ms | 0.08 ms |
| `system_by_name` | 0.01 ms | 0.02 ms |
| `find_systems` prefix | 0.05 ms | 0.05 ms |
| `systems_near` 30 ly | 0.39 ms | 0.52 ms |
| `stations_near` encoded material trader | 0.26 ms | 0.30 ms |
| `stations_near` interstellar factors + pad L | 0.42 ms | 0.50 ms |
| `trade_routes` 30 ly radius, 20 ly hop | 7.5 ms | 8.8 ms |
| `trade_routes` 60 ly radius, 30 ly hop | 21.5 ms | 28.0 ms |
| `stats()` | 0.65 ms | 3.6 ms |
| `search_commodity` 50 ly while ~260 snapshots/s are written | 1.19 ms | 1.63 ms |

Before the in-memory station index, an R*Tree over stations with one market lookup per station
measured 63 ms median and 480 ms max for "origin + 50 ly". With a cold cache it reached 1.9 s,
because every candidate station costs one random page read.

## Price history

A point is recorded when a live snapshot (EDDN or journal, never a dump) changes the buy or sell
price of a row. Both the previous value and the new value are stored. Two `GalaxyOptions` settings
apply:

* `price_history_min_change` (relative, default 0 = every change) suppresses noise.
* `price_history_days` (default 30) controls hourly pruning.

Measured on the live EDDN feed: 277 market snapshots in 10 minutes produced 75 history rows. Most
revisits change only a few prices. Even if every snapshot were a revisit, that is in the order of
a million rows (tens of MB) per 30-day window.