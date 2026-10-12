//! Local galaxy / market database for Elite Nexus.
//!
//! A SQLite (WAL) database of systems, stations, markets, factions and ring hotspots, fed by EDDN
//! ([`Ingest::from_eddn`]), the player's own journal ([`Ingest::from_journal_event`]) and Spansh
//! dumps ([`GalaxyDb::import_spansh_dump`]).
//!
//! * **Writes** go through one dedicated writer thread: [`GalaxyDb::ingest`] only sends the item over
//!   a channel (never blocks on I/O); the writer commits batches in one transaction at most every
//!   ~250 ms and then notifies [`GalaxyDb::subscribe_updates`] receivers.
//! * **Reads** run on the caller's thread on pooled `query_only` connections; with WAL they never wait
//!   for the writer. Commodity queries scan the commodity-clustered partial covering indexes
//!   (`market_sellers`, `market_buyers`) and filter them against an in-memory station index
//!   (position, pads, flags, services), which the writer refreshes after every commit; station
//!   searches scan that index; `systems_near` uses an R*Tree.
//!
//! ```no_run
//! # fn main() -> nexus_galaxy::Result<()> {
//! use nexus_galaxy::*;
//! let db = GalaxyDb::open("galaxy.db")?;
//! let hits = db.search_commodity(&CommodityQuery {
//!     commodity: "tritium".into(),
//!     mode: TradeMode::SellToStation,
//!     origin: Some([0.0, 0.0, 0.0]),
//!     max_distance_ly: Some(50.0),
//!     min_pad: Some('L'),
//!     ..Default::default()
//! })?;
//! # Ok(()) }
//! ```

mod commodities;
mod convert;
mod error;
mod index;
mod model;
mod names;
mod query;
mod schema;
mod services;
mod spansh;
mod station;
mod trade;
mod util;
mod writer;

pub use commodities::normalize_symbol;
pub use convert::eddn_schema_name;
pub use error::{GalaxyError, Result};
pub use model::*;
pub use names::clean_station_name;
pub use services::{MaterialTrader, Services, TechBroker};
pub use spansh::{ImportProgress, ImportSummary};
pub use station::{Pad, StationKind};

/// Progress reported by [`GalaxyDb::import_spansh_dump`].
pub type Progress = ImportProgress;

use crossbeam_channel::{Receiver, Sender};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use writer::{Shared, WriterMsg};

/// Tuning knobs for [`GalaxyDb::open_with`].
#[derive(Debug, Clone)]
pub struct GalaxyOptions {
    /// Minimum time between two commits of live data.
    pub commit_interval: Duration,
    /// Record price changes of live market snapshots in `price_history`.
    pub price_history: bool,
    /// Minimum relative change (0.01 = 1 %) of the buy or sell price to record a history point;
    /// 0 records every change.
    pub price_history_min_change: f64,
    /// History older than this is pruned (hourly).
    pub price_history_days: i64,
    /// Idle read connections kept open.
    pub read_pool_size: usize,
    /// Capacity of each [`GalaxyDb::subscribe_updates`] channel (updates are dropped when full).
    pub update_channel_capacity: usize,
}

impl Default for GalaxyOptions {
    fn default() -> Self {
        GalaxyOptions {
            commit_interval: Duration::from_millis(250),
            price_history: true,
            price_history_min_change: 0.0,
            price_history_days: 30,
            read_pool_size: 8,
            update_channel_capacity: 4096,
        }
    }
}

/// Handle to the galaxy database. Cheap to clone; `Send + Sync`. The writer thread stops (after
/// committing everything queued) when the last handle is dropped.
#[derive(Clone)]
pub struct GalaxyDb {
    inner: Arc<Inner>,
}

struct Inner {
    path: PathBuf,
    tx: Option<Sender<WriterMsg>>,
    shared: Arc<Shared>,
    pool: query::ReadPool,
    opts: GalaxyOptions,
    writer: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Closing the channel makes the writer commit what it has and exit.
        drop(self.tx.take());
        if let Some(h) = self.writer.lock().take()
            && h.join().is_err()
        {
            tracing::error!("galaxy writer thread panicked");
        }
    }
}

impl std::fmt::Debug for GalaxyDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GalaxyDb")
            .field("path", &self.inner.path)
            .finish()
    }
}

impl GalaxyDb {
    /// Opens (creating if needed) the database at `path` and starts the writer thread.
    pub fn open(path: impl AsRef<Path>) -> Result<GalaxyDb> {
        GalaxyDb::open_with(path, GalaxyOptions::default())
    }

    pub fn open_with(path: impl AsRef<Path>, opts: GalaxyOptions) -> Result<GalaxyDb> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        let conn = schema::open_writer(&path)?;
        let shared = Arc::new(Shared::default());
        shared.load_commodities(&conn)?;
        *shared.stations.write() = index::StationIndex::load(&conn)?;
        let (tx, rx) = crossbeam_channel::unbounded();
        let w = writer::Writer::new(conn, rx, shared.clone(), opts.clone());
        let handle = std::thread::Builder::new()
            .name("galaxy-writer".into())
            .spawn(move || w.run())?;
        tracing::info!(path = %path.display(), "galaxy database open");
        Ok(GalaxyDb {
            inner: Arc::new(Inner {
                pool: query::ReadPool::new(path.clone(), opts.read_pool_size),
                path,
                tx: Some(tx),
                shared,
                opts,
                writer: parking_lot::Mutex::new(Some(handle)),
            }),
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    fn send(&self, msg: WriterMsg) -> Result<()> {
        self.inner
            .tx
            .as_ref()
            .ok_or(GalaxyError::WriterGone)?
            .send(msg)
            .map_err(|_| GalaxyError::WriterGone)
    }

    // -----------------------------------------------------------------------------------------
    // Writes
    // -----------------------------------------------------------------------------------------

    /// Queues one item for the writer thread (non-blocking).
    pub fn ingest(&self, item: Ingest) {
        self.ingest_many(vec![item]);
    }

    /// Queues several items for the writer thread (non-blocking).
    pub fn ingest_many(&self, items: Vec<Ingest>) {
        if items.is_empty() {
            return;
        }
        if self.send(WriterMsg::Ingest(items)).is_err() {
            tracing::warn!("galaxy writer stopped; dropping ingest");
        }
    }

    /// Converts and queues an EDDN envelope; returns the number of items queued.
    pub fn ingest_eddn(&self, envelope: &serde_json::Value) -> usize {
        let items = Ingest::from_eddn(envelope);
        let n = items.len();
        self.ingest_many(items);
        n
    }

    /// Converts and queues one of the player's journal events (or Market.json); returns the number
    /// of items queued.
    pub fn ingest_journal(&self, event: &str, json: &serde_json::Value) -> usize {
        let items = Ingest::from_journal_event(event, json);
        let n = items.len();
        self.ingest_many(items);
        n
    }

    /// Blocks until everything queued before this call is committed.
    pub fn flush(&self) -> Result<()> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.send(WriterMsg::Flush(tx))?;
        rx.recv().map_err(|_| GalaxyError::WriterGone)
    }

    /// Items waiting for the writer thread.
    pub fn pending(&self) -> usize {
        self.inner.tx.as_ref().map_or(0, Sender::len)
    }

    /// Streams a Spansh dump (gzip or plain JSON, one record per line) into the database with
    /// bounded memory. `progress` is called after every chunk (~24 MB of JSON); return `false` to
    /// cancel (what was read so far stays imported). Blocks the calling thread; run it off the UI
    /// thread. Live ingestion continues in parallel through the same writer.
    pub fn import_spansh_dump<R: Read>(
        &self,
        reader: R,
        progress: impl FnMut(&ImportProgress) -> bool,
    ) -> Result<ImportSummary> {
        let tx = self.inner.tx.as_ref().ok_or(GalaxyError::WriterGone)?;
        spansh::import(tx, &self.inner.shared, reader, progress)
    }

    /// Receives a [`MarketUpdate`] after each committed live (EDDN / journal) market snapshot.
    /// Updates are dropped for a receiver whose channel is full; drop the receiver to unsubscribe.
    pub fn subscribe_updates(&self) -> Receiver<MarketUpdate> {
        let (tx, rx) = crossbeam_channel::bounded(self.inner.opts.update_channel_capacity.max(1));
        self.inner.shared.subscribers.lock().push(tx);
        rx
    }

    // -----------------------------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------------------------

    /// Best stations to buy from / sell to for one commodity.
    pub fn search_commodity(&self, q: &CommodityQuery) -> Result<Vec<CommodityHit>> {
        let cid = self.inner.shared.commodities.read().resolve(&q.commodity);
        let Some(cid) = cid else {
            return Ok(Vec::new());
        };
        query::search_commodity(
            &*self.inner.pool.get()?,
            &self.inner.shared.stations,
            cid,
            q,
        )
    }

    /// Station details + system + every market row with commodity names.
    pub fn station_market(&self, market_id: u64) -> Result<Option<StationMarket>> {
        let conn = self.inner.pool.get()?;
        let cache = self.inner.shared.commodities.read();
        let mut m = query::station_market(&conn, &cache, market_id)?;
        if let Some(m) = &mut m {
            // prohibited lists hold symbols (EDDN) or display names (Spansh): show names
            for p in &mut m.prohibited {
                if let Some(meta) = cache.resolve(p).and_then(|id| cache.meta(id)) {
                    p.clone_from(&meta.name);
                }
            }
        }
        Ok(m)
    }

    pub fn station(&self, market_id: u64) -> Result<Option<StationInfo>> {
        query::station(&*self.inner.pool.get()?, market_id)
    }

    pub fn stations_in_system(&self, system_id64: u64) -> Result<Vec<StationInfo>> {
        query::stations_in_system(&*self.inner.pool.get()?, system_id64)
    }

    /// Stations whose name starts with `prefix` (case-insensitive), most recently updated first.
    pub fn find_stations(&self, prefix: &str, limit: usize) -> Result<Vec<StationInfo>> {
        query::find_stations(&*self.inner.pool.get()?, prefix, limit)
    }

    /// Most recently updated markets (for a "latest updates" feed).
    pub fn recent_markets(&self, limit: usize) -> Result<Vec<StationInfo>> {
        query::recent_markets(&*self.inner.pool.get()?, limit)
    }

    /// Nearest stations with the requested services / trader types / pad size.
    pub fn stations_near(&self, q: &StationQuery) -> Result<Vec<StationHit>> {
        query::stations_near(&*self.inner.pool.get()?, &self.inner.shared.stations, q)
    }

    /// Case-insensitive exact name lookup (prefers a system with known coordinates).
    pub fn system_by_name(&self, name: &str) -> Result<Option<SystemInfo>> {
        query::system_by_name(&*self.inner.pool.get()?, name)
    }

    pub fn system_by_id(&self, id64: u64) -> Result<Option<SystemInfo>> {
        query::system_by_id(&*self.inner.pool.get()?, id64)
    }

    /// Systems whose name starts with `prefix`, for autocompletion.
    pub fn find_systems(&self, prefix: &str, limit: usize) -> Result<Vec<SystemInfo>> {
        query::find_systems(&*self.inner.pool.get()?, prefix, limit)
    }

    /// Systems within `radius_ly` of `pos`, nearest first.
    pub fn systems_near(
        &self,
        pos: [f64; 3],
        radius_ly: f64,
        limit: usize,
    ) -> Result<Vec<SystemHit>> {
        query::systems_near(&*self.inner.pool.get()?, pos, radius_ly, limit)
    }

    /// Factions present in a system, highest influence first.
    pub fn system_factions(&self, system_id64: u64) -> Result<Vec<FactionInfo>> {
        query::system_factions(&*self.inner.pool.get()?, system_id64)
    }

    /// Ring hotspots of one commodity near a position.
    pub fn hotspots_near(&self, q: &HotspotQuery) -> Result<Vec<HotspotHit>> {
        let signal = self
            .commodity_symbol(&q.signal)
            .unwrap_or_else(|| normalize_symbol(&q.signal));
        query::hotspots_near(&*self.inner.pool.get()?, &signal, q)
    }

    /// Best single-hop trade routes near an origin.
    pub fn trade_routes(&self, q: &TradeQuery) -> Result<Vec<TradeRoute>> {
        trade::trade_routes(&self.inner.pool, &self.inner.shared, q)
    }

    /// Recorded price changes of one commodity at one station, oldest first.
    pub fn price_history(&self, market_id: u64, commodity: &str) -> Result<Vec<PricePoint>> {
        let Some(cid) = self.inner.shared.commodities.read().resolve(commodity) else {
            return Ok(Vec::new());
        };
        query::price_history(&*self.inner.pool.get()?, market_id, cid)
    }

    /// All known commodities, sorted by category and name.
    pub fn commodities(&self) -> Vec<CommodityInfo> {
        let cache = self.inner.shared.commodities.read();
        let mut v: Vec<CommodityInfo> = cache
            .iter()
            .map(|(_, m)| CommodityInfo {
                symbol: m.symbol.clone(),
                name: m.name.clone(),
                category: m.category.clone(),
                is_rare: m.rare,
            })
            .collect();
        v.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then_with(|| a.name.cmp(&b.name))
        });
        v
    }

    /// Resolves a commodity symbol / journal name / display name to its canonical symbol.
    pub fn commodity_symbol(&self, input: &str) -> Option<String> {
        let cache = self.inner.shared.commodities.read();
        cache
            .resolve(input)
            .and_then(|id| cache.meta(id))
            .map(|m| m.symbol.clone())
    }

    /// Row counts, newest market update, file size and writer counters.
    pub fn stats(&self) -> Result<DbStats> {
        let mut s = query::stats(&*self.inner.pool.get()?, &self.inner.shared.stations)?;
        let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        let mut wal = self.inner.path.clone().into_os_string();
        wal.push("-wal");
        s.db_bytes = size(&self.inner.path) + size(Path::new(&wal));
        let sh = &self.inner.shared;
        s.pending_writes = self.pending() as u64;
        s.ingested = sh.ingested.load(Ordering::Relaxed);
        s.market_snapshots = sh.market_snapshots.load(Ordering::Relaxed);
        s.stale_markets = sh.stale_markets.load(Ordering::Relaxed);
        s.commits = sh.commits.load(Ordering::Relaxed);
        Ok(s)
    }
}
