//! SQLite schema, pragmas and seeding.

use crate::error::Result;
use rusqlite::Connection;
use std::time::Duration;

/// Bump when the schema changes incompatibly. The database is a cache that can be rebuilt from
/// EDDN and dumps, so an older schema is dropped and recreated.
pub(crate) const SCHEMA_VERSION: i64 = 2;

const TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS systems (
    id64                      INTEGER PRIMARY KEY,
    name                      TEXT NOT NULL,
    name_lower                TEXT NOT NULL,
    x                         REAL,
    y                         REAL,
    z                         REAL,
    population                INTEGER,
    allegiance                TEXT,
    government                TEXT,
    economy                   TEXT,
    second_economy            TEXT,
    security                  TEXT,
    controlling_faction       TEXT,
    controlling_faction_state TEXT,
    controlling_power         TEXT,
    powerplay_state           TEXT,
    powers                    TEXT,
    updated                   INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS systems_name_lower ON systems(name_lower);
CREATE VIRTUAL TABLE IF NOT EXISTS systems_rtree USING rtree(id, min_x, max_x, min_y, max_y, min_z, max_z);

CREATE TABLE IF NOT EXISTS stations (
    market_id             INTEGER PRIMARY KEY,
    system_id64           INTEGER,
    system_name           TEXT NOT NULL,
    system_name_lower     TEXT NOT NULL,
    name                  TEXT NOT NULL,
    type                  TEXT,
    dist_ls               REAL,
    max_pad               INTEGER NOT NULL DEFAULT 0,
    is_planetary          INTEGER NOT NULL DEFAULT 0,
    is_carrier            INTEGER NOT NULL DEFAULT 0,
    is_odyssey_settlement INTEGER NOT NULL DEFAULT 0,
    services              INTEGER NOT NULL DEFAULT 0,
    mat_trader            INTEGER NOT NULL DEFAULT 0,
    tech_broker           INTEGER NOT NULL DEFAULT 0,
    economy               TEXT,
    second_economy        TEXT,
    economies             TEXT,
    faction               TEXT,
    faction_state         TEXT,
    government            TEXT,
    allegiance            TEXT,
    carrier_access        TEXT,
    x                     REAL,
    y                     REAL,
    z                     REAL,
    updated               INTEGER NOT NULL DEFAULT 0,
    market_updated        INTEGER NOT NULL DEFAULT 0,
    n_commodities         INTEGER NOT NULL DEFAULT 0,
    prohibited            TEXT
);
CREATE INDEX IF NOT EXISTS stations_system ON stations(system_id64);
CREATE INDEX IF NOT EXISTS stations_unlinked ON stations(system_name_lower) WHERE system_id64 IS NULL;
CREATE INDEX IF NOT EXISTS stations_name ON stations(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS stations_market_updated ON stations(market_updated);

CREATE TABLE IF NOT EXISTS commodities (
    id       INTEGER PRIMARY KEY,
    symbol   TEXT NOT NULL UNIQUE,
    name     TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    fdev_id  INTEGER,
    is_rare  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS market (
    market_id      INTEGER NOT NULL,
    commodity_id   INTEGER NOT NULL,
    buy            INTEGER NOT NULL,
    sell           INTEGER NOT NULL,
    mean           INTEGER NOT NULL,
    stock          INTEGER NOT NULL,
    stock_bracket  INTEGER,
    demand         INTEGER NOT NULL,
    demand_bracket INTEGER,
    updated        INTEGER NOT NULL,
    PRIMARY KEY (market_id, commodity_id)
) WITHOUT ROWID;
-- "where can I buy X": stations selling (player buys), cheapest first
CREATE INDEX IF NOT EXISTS market_sellers ON market(commodity_id, buy, stock, updated) WHERE buy > 0 AND stock > 0;
-- "where can I sell X": stations buying (player sells), best price first (scanned backwards)
CREATE INDEX IF NOT EXISTS market_buyers ON market(commodity_id, sell, demand, updated) WHERE sell > 0 AND demand > 0;

CREATE TABLE IF NOT EXISTS system_factions (
    system_id64       INTEGER NOT NULL,
    faction           TEXT NOT NULL,
    influence         REAL,
    state             TEXT,
    government        TEXT,
    allegiance        TEXT,
    happiness         TEXT,
    active_states     TEXT,
    pending_states    TEXT,
    recovering_states TEXT,
    updated           INTEGER NOT NULL,
    PRIMARY KEY (system_id64, faction)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS system_factions_faction ON system_factions(faction);

CREATE TABLE IF NOT EXISTS ring_signals (
    system_id64 INTEGER NOT NULL,
    body_name   TEXT NOT NULL,
    signal      TEXT NOT NULL,
    count       INTEGER NOT NULL,
    ring_type   TEXT,
    updated     INTEGER NOT NULL,
    PRIMARY KEY (system_id64, body_name, signal)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS ring_signals_signal ON ring_signals(signal, count);

CREATE TABLE IF NOT EXISTS price_history (
    market_id    INTEGER NOT NULL,
    commodity_id INTEGER NOT NULL,
    ts           INTEGER NOT NULL,
    buy          INTEGER NOT NULL,
    sell         INTEGER NOT NULL,
    PRIMARY KEY (market_id, commodity_id, ts)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS price_history_ts ON price_history(ts);
"#;

const DROP_ALL: &str = r#"
DROP TABLE IF EXISTS systems;
DROP TABLE IF EXISTS systems_rtree;
DROP TABLE IF EXISTS stations;
DROP TABLE IF EXISTS station_rtree;
DROP TABLE IF EXISTS commodities;
DROP TABLE IF EXISTS market;
DROP TABLE IF EXISTS system_factions;
DROP TABLE IF EXISTS ring_signals;
DROP TABLE IF EXISTS price_history;
"#;

/// Pragmas shared by every connection.
fn common_pragmas(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute_batch(
        "PRAGMA temp_store = MEMORY;
         PRAGMA foreign_keys = OFF;
         PRAGMA mmap_size = 268435456;",
    )?;
    Ok(())
}

/// Opens the writer connection, enables WAL and creates/migrates the schema.
pub(crate) fn open_writer(path: &std::path::Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    common_pragmas(&conn)?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        tracing::warn!(mode, "could not enable WAL journal mode");
    }
    conn.execute_batch(
        "PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -65536;
         PRAGMA wal_autocheckpoint = 4000;
         PRAGMA journal_size_limit = 67108864;",
    )?;
    conn.set_prepared_statement_cache_capacity(64);

    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version != 0 && version != SCHEMA_VERSION {
        tracing::warn!(
            found = version,
            expected = SCHEMA_VERSION,
            "galaxy database schema changed; rebuilding it"
        );
        conn.execute_batch(DROP_ALL)?;
    }
    conn.execute_batch(TABLES)?;
    conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    seed_commodities(&conn)?;
    Ok(conn)
}

/// Opens a read connection (`query_only`).
pub(crate) fn open_reader(path: &std::path::Path) -> Result<Connection> {
    use rusqlite::OpenFlags;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    common_pragmas(&conn)?;
    conn.execute_batch(
        "PRAGMA query_only = ON;
         PRAGMA cache_size = -32768;",
    )?;
    conn.set_prepared_statement_cache_capacity(64);
    Ok(conn)
}

fn seed_commodities(conn: &Connection) -> Result<()> {
    let have: i64 = conn.query_row("SELECT COUNT(*) FROM commodities", [], |r| r.get(0))?;
    if have > 0 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO commodities (symbol, name, category, fdev_id, is_rare) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for c in crate::commodities::seed() {
            stmt.execute(rusqlite::params![
                c.symbol, c.name, c.category, c.fdev_id, c.rare
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtree_is_compiled_in() {
        let conn = Connection::open_in_memory().expect("open");
        let used: i64 = conn
            .query_row(
                "SELECT sqlite_compileoption_used('ENABLE_RTREE')",
                [],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(used, 1);
        conn.execute_batch(TABLES).expect("schema");
    }
}
