//! Compact in-memory index of every station (position, pads, flags, services, market age).
//!
//! Commodity searches scan the commodity-clustered covering indexes (`market_sellers`,
//! `market_buyers`: contiguous pages, fast even with a cold cache) and filter the rows against this
//! index instead of doing one random station/market page lookup per candidate. `stations_near` and
//! the trade-route candidate search are plain scans of it. ~64 bytes per station.
//!
//! The writer thread refreshes the touched entries from the database after every commit, so the
//! index only ever reflects committed data.

use crate::error::Result;
use rusqlite::Connection;
use rustc_hash::FxHashMap;

pub(crate) const F_CARRIER: u8 = 1;
pub(crate) const F_PLANETARY: u8 = 2;
pub(crate) const F_SETTLEMENT: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StationLite {
    pub pos: Option<[f64; 3]>,
    pub services: u64,
    pub market_updated: i64,
    /// NaN when unknown.
    pub dist_ls: f32,
    pub n_commodities: u32,
    pub max_pad: u8,
    pub flags: u8,
    pub mat_trader: u8,
    pub tech_broker: u8,
}

impl StationLite {
    pub fn is_carrier(&self) -> bool {
        self.flags & F_CARRIER != 0
    }
    pub fn is_planetary(&self) -> bool {
        self.flags & F_PLANETARY != 0
    }
}

/// Station filters shared by the queries.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StationFilter {
    /// 0 = any.
    pub min_pad: u8,
    pub carriers: bool,
    pub planetary: bool,
    /// Negative = no limit.
    pub max_dist_ls: f64,
}

impl StationFilter {
    pub fn passes(&self, s: &StationLite) -> bool {
        if self.min_pad > 0 && s.max_pad < self.min_pad {
            return false;
        }
        if !self.carriers && s.is_carrier() {
            return false;
        }
        if !self.planetary && s.is_planetary() {
            return false;
        }
        // An unknown (NaN) arrival distance fails a distance limit.
        if self.max_dist_ls >= 0.0
            && (s.dist_ls.is_nan() || f64::from(s.dist_ls) > self.max_dist_ls)
        {
            return false;
        }
        true
    }
}

const COLS: &str = "market_id, x, y, z, services, market_updated, dist_ls, n_commodities, max_pad, is_carrier, \
     is_planetary, is_odyssey_settlement, mat_trader, tech_broker";

fn from_row(r: &rusqlite::Row) -> rusqlite::Result<(i64, StationLite)> {
    let pos = crate::query::opt_pos(r.get(1)?, r.get(2)?, r.get(3)?);
    let flag = |i: usize, f: u8| -> rusqlite::Result<u8> {
        Ok(if r.get::<_, i64>(i)? != 0 { f } else { 0 })
    };
    Ok((
        r.get(0)?,
        StationLite {
            pos,
            services: r.get::<_, i64>(4)? as u64,
            market_updated: r.get(5)?,
            dist_ls: r.get::<_, Option<f64>>(6)?.map_or(f32::NAN, |d| d as f32),
            n_commodities: r.get::<_, i64>(7)?.clamp(0, i64::from(u32::MAX)) as u32,
            max_pad: r.get::<_, i64>(8)?.clamp(0, 3) as u8,
            flags: flag(9, F_CARRIER)? | flag(10, F_PLANETARY)? | flag(11, F_SETTLEMENT)?,
            mat_trader: r.get::<_, i64>(12)?.clamp(0, 255) as u8,
            tech_broker: r.get::<_, i64>(13)?.clamp(0, 255) as u8,
        },
    ))
}

#[derive(Debug, Default)]
pub(crate) struct StationIndex {
    map: FxHashMap<i64, StationLite>,
}

impl StationIndex {
    pub fn load(conn: &Connection) -> Result<StationIndex> {
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM stations"))?;
        let rows = stmt.query_map([], from_row)?;
        let mut map = FxHashMap::default();
        for r in rows {
            let (id, s) = r?;
            map.insert(id, s);
        }
        Ok(StationIndex { map })
    }

    /// Re-reads `ids` from the database (removing the ones that no longer exist).
    pub fn refresh(&mut self, conn: &Connection, ids: &[i64]) -> Result<()> {
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {COLS} FROM stations WHERE market_id = ?1"))?;
        for &id in ids {
            let mut rows = stmt.query([id])?;
            match rows.next()? {
                Some(r) => {
                    let (_, s) = from_row(r)?;
                    self.map.insert(id, s);
                }
                None => {
                    self.map.remove(&id);
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, market_id: i64) -> Option<&StationLite> {
        self.map.get(&market_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (i64, &StationLite)> {
        self.map.iter().map(|(k, v)| (*k, v))
    }
}
