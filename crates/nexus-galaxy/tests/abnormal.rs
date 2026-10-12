//! Abnormal prices: the indexed whole-database scan (prices ≥ 1.5× off the galactic average, and
//! fleet-carrier orders compared with the stations' average).

use nexus_galaxy::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct TempDb {
    dir: PathBuf,
    db: Option<GalaxyDb>,
}

impl TempDb {
    fn new(name: &str) -> TempDb {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("abnormal-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        let db = GalaxyDb::open(dir.join("galaxy.db")).expect("open db");
        TempDb { dir, db: Some(db) }
    }
    fn db(&self) -> &GalaxyDb {
        self.db.as_ref().expect("db")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        self.db = None;
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn system(id: u64, name: &str, pos: [f64; 3]) -> Ingest {
    Ingest::System(SystemUpdate {
        id64: id,
        name: name.into(),
        pos: Some(pos),
        timestamp: now() - 60,
        details: Some(SystemDetails { population: Some(1_000_000), ..Default::default() }),
    })
}

fn station(mid: u64, name: &str, sys: u64, sys_name: &str, kind: &str) -> Ingest {
    Ingest::Station(StationUpdate {
        market_id: mid,
        name: name.into(),
        system_id64: Some(sys),
        system_name: Some(sys_name.into()),
        station_type: Some(kind.into()),
        dist_ls: Some(500.0),
        services: Some(Services::from_names(["dock", "commodities"])),
        timestamp: now() - 60,
        ..Default::default()
    })
}

/// (symbol, buy, sell, mean, stock, demand)
fn priced(mid: u64, sys_name: &str, name: &str, kind: &str, items: &[(&str, i64, i64, i64, i64, i64)], age_s: i64) -> Ingest {
    Ingest::Market(MarketSnapshot {
        market_id: mid,
        system_name: sys_name.into(),
        station_name: name.into(),
        station_type: Some(kind.into()),
        timestamp: now() - age_s,
        items: items
            .iter()
            .map(|&(s, buy, sell, mean, stock, demand)| MarketItem { symbol: s.into(), buy, sell, mean, stock, stock_bracket: Some(2), demand, demand_bracket: Some(2) })
            .collect(),
        ..Default::default()
    })
}

#[test]
fn abnormal_prices_find_carrier_orders_and_far_off_station_prices() {
    let t = TempDb::new("scan");
    let db = t.db();
    let (c1, c2) = (3_700_000_001u64, 3_700_000_002u64);
    db.ingest_many(vec![
        system(1001, "Alpha", [0.0, 0.0, 0.0]),
        system(1002, "Beta", [30.0, 0.0, 0.0]),
        system(1003, "Gamma", [900.0, 0.0, 0.0]),
        station(11, "Alpha Port", 1001, "Alpha", "Coriolis"),
        station(12, "Beta Hub", 1002, "Beta", "Coriolis"),
        station(13, "Far Out", 1003, "Gamma", "Coriolis"),
        station(c1, "X9X-99X", 1002, "Beta", "FleetCarrier"),
        station(c2, "Q1Q-11Q", 1003, "Gamma", "FleetCarrier"),
        station(14, "Tritium Depot", 1001, "Alpha", "Coriolis"),
        // Normal prices that set the averages, plus one station paying 3× for painite.
        priced(11, "Alpha", "Alpha Port", "Coriolis", &[("gold", 48_000, 52_000, 50_000, 100, 1000), ("painite", 0, 1_200_000, 400_000, 0, 50)], 30),
        // 1.3× is not abnormal; silver sold at a tenth of the average is.
        priced(12, "Beta", "Beta Hub", "Coriolis", &[("gold", 0, 65_000, 50_000, 0, 1000), ("silver", 500, 0, 5_000, 2_000, 0)], 30),
        // Far away: gold bought at 2× average, but older than the age limit.
        priced(13, "Gamma", "Far Out", "Coriolis", &[("gold", 0, 100_000, 50_000, 0, 1000)], 10 * 24 * 3600),
        // Carriers report no average: a 100× gold buy order and a cheap tritium sale.
        priced(c1, "Beta", "X9X-99X", "FleetCarrier", &[("gold", 0, 5_000_000, 0, 0, 20), ("tritium", 1_000, 0, 0, 5_000, 0)], 30),
        // A carrier price close to average stays out.
        priced(c2, "Gamma", "Q1Q-11Q", "FleetCarrier", &[("gold", 0, 60_000, 0, 0, 500)], 30),
        priced(14, "Alpha", "Tritium Depot", "Coriolis", &[("tritium", 48_000, 52_000, 50_000, 100, 100)], 30),
    ]);
    db.flush().expect("flush");

    let q = DealQuery { mode: TradeMode::SellToStation, origin: Some([0.0; 3]), max_distance_ly: None, min_ratio: 2.0, max_age_hours: Some(72.0), cargo_capacity: 500, limit: 50, ..Default::default() };
    let sell = db.price_deals(&q).expect("scan");
    let found: Vec<(&str, &str)> = sell.iter().map(|d| (d.commodity.as_str(), d.station.as_str())).collect();
    assert_eq!(found, vec![("gold", "X9X-99X"), ("painite", "Alpha Port")], "carrier order first (most extra value), stale and 1.3× rows out");
    assert_eq!(sell[0].average, 50_000, "the carrier is measured against the stations' average");
    assert!((sell[0].ratio - 100.0).abs() < 1e-9);
    assert!(sell[0].is_carrier);

    let near = db.price_deals(&DealQuery { max_distance_ly: Some(10.0), ..q.clone() }).expect("scan");
    assert_eq!(near.iter().map(|d| d.station.as_str()).collect::<Vec<_>>(), vec!["Alpha Port"], "the radius applies");

    let no_carriers = db.price_deals(&DealQuery { include_carriers: false, ..q.clone() }).expect("scan");
    assert!(no_carriers.iter().all(|d| !d.is_carrier));

    let buy = db.price_deals(&DealQuery { mode: TradeMode::BuyFromStation, ..q.clone() }).expect("scan");
    let found: Vec<(&str, &str)> = buy.iter().map(|d| (d.commodity.as_str(), d.station.as_str())).collect();
    assert_eq!(found, vec![("tritium", "X9X-99X"), ("silver", "Beta Hub")], "cheap carrier tritium and silver at a tenth");

    let strict = db.price_deals(&DealQuery { min_ratio: 5.0, ..q }).expect("scan");
    assert_eq!(strict.len(), 1, "only the 100× carrier order is at least 5×");
}
