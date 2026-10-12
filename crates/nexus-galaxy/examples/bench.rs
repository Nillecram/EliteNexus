//! Query latency benchmark on a realistic-size synthetic galaxy.
//!
//! ```text
//! cargo run -p nexus-galaxy --release --example bench -- <db path> [stations=80000] [--rebuild]
//! ```
//!
//! Generates (once) a bubble-like galaxy through the public ingest API: ~70 % of the stations in a
//! dense 250 ly sphere around Sol, a Colonia cluster, fleet carriers spread over the bubble and the
//! galaxy, Odyssey settlements; each market has the realistic mix of ~30 produced and ~60 consumed
//! commodities (carriers: a handful). Then times every query type.

use nexus_galaxy::*;
use std::time::{Duration, Instant};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f()
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        self.f() < p
    }
    /// Point in a sphere, denser towards the centre.
    fn in_sphere(&mut self, c: [f64; 3], r: f64, power: f64) -> [f64; 3] {
        loop {
            let p = [
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            ];
            let d2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
            if d2 <= 1.0 && d2 > 0.0 {
                let s = r * d2.sqrt().powf(power) / d2.sqrt();
                return [c[0] + p[0] * s, c[1] + p[1] * s * 0.4, c[2] + p[2] * s];
            }
        }
    }
}

const COLONIA: [f64; 3] = [-9530.5, -910.28125, 19808.125];
const ECONOMIES: [&str; 9] = [
    "Industrial",
    "High Tech",
    "Extraction",
    "Refinery",
    "Agriculture",
    "Military",
    "Tourism",
    "Service",
    "Colony",
];
const TYPES: [(&str, f64); 7] = [
    ("Coriolis", 0.22),
    ("Orbis", 0.08),
    ("Ocellus", 0.06),
    ("Outpost", 0.30),
    ("CraterPort", 0.12),
    ("CraterOutpost", 0.12),
    ("AsteroidBase", 0.10),
];

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct Gen {
    systems: Vec<(u64, String, [f64; 3])>,
}

fn generate(db: &GalaxyDb, n_stations: usize) -> Gen {
    let mut rng = Rng(0x5EED_CAFE);
    let commodities: Vec<CommodityInfo> = db
        .commodities()
        .into_iter()
        .filter(|c| !c.is_rare && c.category != "NonMarketable" && c.category != "Salvage")
        .collect();
    let mean: Vec<f64> = commodities
        .iter()
        .map(|_| 10f64.powf(rng.range(2.0, 4.9)))
        .collect();
    let t0 = now();
    let started = Instant::now();

    // systems: ~1 per 2 stations in the bubble and Colonia, plus empty systems around them
    let n_bubble_sys = n_stations * 35 / 100;
    let n_colonia_sys = n_stations / 100;
    let mut systems = Vec::new();
    let mut id = 10_000u64;
    let mut name_n = 0usize;
    let mut push_sys =
        |systems: &mut Vec<(u64, String, [f64; 3])>, pos: [f64; 3], rng: &mut Rng| {
            id += 1 + rng.below(1000) as u64;
            name_n += 1;
            let name = format!(
                "Synth {} {}-{} a{}",
                ["Sector", "Region", "Reach", "Expanse"][name_n % 4],
                name_n / 26,
                (b'A' + (name_n % 26) as u8) as char,
                name_n % 97
            );
            systems.push((id, name, pos));
        };
    push_sys(&mut systems, [0.0, 0.0, 0.0], &mut rng);
    systems[0].1 = "Sol".into();
    for _ in 0..n_bubble_sys {
        let p = rng.in_sphere([0.0, 0.0, 0.0], 250.0, 0.8);
        push_sys(&mut systems, p, &mut rng);
    }
    for _ in 0..n_colonia_sys {
        let p = rng.in_sphere(COLONIA, 60.0, 1.0);
        push_sys(&mut systems, p, &mut rng);
    }
    let n_populated = systems.len();
    // unpopulated systems that EDDN FSDJumps add over time
    for _ in 0..n_stations * 2 {
        let p = if rng.chance(0.6) {
            rng.in_sphere([0.0, 0.0, 0.0], 1500.0, 1.0)
        } else {
            [
                rng.range(-40000.0, 40000.0),
                rng.range(-2000.0, 2000.0),
                rng.range(-20000.0, 60000.0),
            ]
        };
        push_sys(&mut systems, p, &mut rng);
    }
    let mut batch: Vec<Ingest> = Vec::with_capacity(4096);
    for (i, (id, name, pos)) in systems.iter().enumerate() {
        let populated = i < n_populated;
        batch.push(Ingest::System(SystemUpdate {
            id64: *id,
            name: name.clone(),
            pos: Some(*pos),
            timestamp: t0 - rng.below(30 * 86_400) as i64,
            details: populated.then(|| SystemDetails {
                population: Some(rng.below(1_000_000_000) as u64),
                economy: Some(ECONOMIES[rng.below(ECONOMIES.len())].into()),
                security: Some(["High", "Medium", "Low", "Anarchy"][rng.below(4)].into()),
                factions: Some(
                    (0..rng.below(6) + 2)
                        .map(|k| FactionUpdate {
                            name: format!("Faction {}", (id + k as u64) % 9000),
                            influence: Some(rng.f()),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
        }));
        if batch.len() >= 4000 {
            db.ingest_many(std::mem::take(&mut batch));
        }
    }

    // stations + markets
    let mut rows = 0usize;
    for s in 0..n_stations {
        let market_id = 3_000_000_000 + s as u64 * 7;
        let roll = rng.f();
        let (kind, sys_idx, n_sell, n_buy) = if roll < 0.12 {
            // fleet carriers: mostly in the bubble, some anywhere
            let sys = if rng.chance(0.8) {
                rng.below(n_bubble_sys)
            } else {
                rng.below(systems.len())
            };
            ("FleetCarrier", sys, rng.below(4), rng.below(6))
        } else if roll < 0.30 {
            (
                "OnFootSettlement",
                rng.below(n_populated),
                8 + rng.below(10),
                15 + rng.below(15),
            )
        } else {
            let mut t = rng.f();
            let mut kind = "Outpost";
            for (k, p) in TYPES {
                if t < p {
                    kind = k;
                    break;
                }
                t -= p;
            }
            (
                kind,
                rng.below(n_populated),
                20 + rng.below(20),
                45 + rng.below(30),
            )
        };
        let (sys_id, sys_name, _) = &systems[sys_idx];
        let primary = ECONOMIES[rng.below(ECONOMIES.len())];
        let secondary = ECONOMIES[rng.below(ECONOMIES.len())];
        let mut services = vec!["dock", "commodities", "refuel"];
        for (svc, p) in [
            ("repair", 0.9),
            ("rearm", 0.8),
            ("outfitting", 0.6),
            ("shipyard", 0.4),
            ("exploration", 0.7),
            ("blackmarket", 0.2),
            ("facilitator", 0.15),
            ("materialtrader", 0.05),
            ("techBroker", 0.04),
            ("vistagenomics", 0.3),
            ("searchrescue", 0.3),
        ] {
            if kind != "FleetCarrier" && rng.chance(p) {
                services.push(svc);
            }
        }
        let ts = t0 - (rng.f().powi(3) * 60.0 * 86_400.0) as i64;
        batch.push(Ingest::Station(StationUpdate {
            market_id,
            name: format!("Station {s}"),
            system_id64: Some(*sys_id),
            system_name: Some(sys_name.clone()),
            station_type: Some(kind.into()),
            dist_ls: Some(rng.range(5.0, 5000.0).powf(1.2)),
            services: Some(Services::from_names(&services)),
            primary_economy: Some(primary.into()),
            economies: Some(vec![(primary.into(), 0.7), (secondary.into(), 0.3)]),
            timestamp: ts,
            ..Default::default()
        }));
        let mut items = Vec::with_capacity(n_sell + n_buy);
        let mut used = std::collections::HashSet::new();
        for k in 0..n_sell + n_buy {
            let c = rng.below(commodities.len());
            if !used.insert(c) {
                continue;
            }
            let m = mean[c];
            let item = if k < n_sell {
                let buy = (m * rng.range(0.7, 1.0)) as i64;
                MarketItem {
                    symbol: commodities[c].symbol.clone(),
                    buy,
                    sell: buy * 97 / 100,
                    mean: m as i64,
                    stock: rng.below(50_000) as i64 + 1,
                    stock_bracket: Some(2),
                    demand: 0,
                    demand_bracket: Some(0),
                }
            } else {
                MarketItem {
                    symbol: commodities[c].symbol.clone(),
                    buy: 0,
                    sell: (m * rng.range(1.0, 1.4)) as i64,
                    mean: m as i64,
                    stock: 0,
                    stock_bracket: Some(0),
                    demand: rng.below(200_000) as i64 + 1,
                    demand_bracket: Some(2),
                }
            };
            items.push(item);
        }
        rows += items.len();
        batch.push(Ingest::Market(MarketSnapshot {
            market_id,
            system_name: sys_name.clone(),
            station_name: format!("Station {s}"),
            station_type: Some(kind.into()),
            timestamp: ts,
            items,
            source: Source::Spansh,
            ..Default::default()
        }));
        if batch.len() >= 2000 {
            db.ingest_many(std::mem::take(&mut batch));
            while db.pending() > 50 {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    db.ingest_many(batch);
    db.flush().expect("flush");
    let secs = started.elapsed().as_secs_f64();
    println!(
        "generated {} systems, {n_stations} stations, {rows} market rows in {secs:.1} s ({:.0} market rows/s)",
        systems.len(),
        rows as f64 / secs
    );
    Gen { systems }
}

fn time<T>(
    label: &str,
    runs: usize,
    mut f: impl FnMut(usize) -> T,
    describe: impl Fn(&T) -> String,
) {
    let mut times = Vec::with_capacity(runs);
    let mut last = None;
    for i in 0..runs {
        let t = Instant::now();
        let r = f(i);
        times.push(t.elapsed());
        last = Some(r);
    }
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{label:<62} median {:>7.2} ms   p90 {:>7.2} ms   max {:>7.2} ms   {}",
        ms(times[times.len() / 2]),
        ms(times[(times.len() * 9 / 10).min(times.len() - 1)]),
        ms(times[times.len() - 1]),
        last.as_ref().map(describe).unwrap_or_default()
    );
}

fn main() {
    let _ = tracing_subscriber::fmt().with_env_filter("warn").try_init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: bench <db path> [stations=80000] [--rebuild]");
        std::process::exit(2);
    };
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(80_000);
    if args.iter().any(|a| a == "--rebuild") {
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path}{ext}"));
        }
    }
    let db = GalaxyDb::open(path).expect("open");
    let stats = db.stats().expect("stats");
    let gen_systems = if stats.stations_with_market < 1000 {
        generate(&db, n).systems
    } else {
        println!("using existing database ({} stations)", stats.stations);
        Vec::new()
    };
    let stats = db.stats().expect("stats");
    println!(
        "db: {} systems, {} stations ({} with market), {} market rows, {} faction rows, {:.0} MB on disk\n",
        stats.systems,
        stats.stations,
        stats.stations_with_market,
        stats.market_rows,
        stats.faction_rows,
        stats.db_bytes as f64 / 1e6
    );

    let sol = [0.0, 0.0, 0.0];
    let goods = [
        "gold",
        "tritium",
        "painite",
        "agronomictreatment",
        "progenitorcells",
        "advancedcatalysers",
    ];
    let mut rng = Rng(42);
    let origins: Vec<[f64; 3]> = (0..20).map(|_| rng.in_sphere(sol, 150.0, 0.8)).collect();
    let n_hits = |v: &Vec<CommodityHit>| format!("{} hits", v.len());

    time(
        "search_commodity buy, global best price (limit 50)",
        30,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    time(
        "search_commodity sell, global best price, pad L, no carriers",
        30,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                mode: TradeMode::SellToStation,
                min_pad: Some('L'),
                include_carriers: false,
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    time(
        "search_commodity buy, origin + 50 ly",
        30,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                origin: Some(origins[i % origins.len()]),
                max_distance_ly: Some(50.0),
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    time(
        "search_commodity sell, origin + 200 ly, pad L, 48 h",
        30,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                mode: TradeMode::SellToStation,
                origin: Some(origins[i % origins.len()]),
                max_distance_ly: Some(200.0),
                min_pad: Some('L'),
                max_age_hours: Some(48.0),
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    time(
        "search_commodity buy, origin, sort nearest (no max distance)",
        30,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                origin: Some(origins[i % origins.len()]),
                sort: Sort::Nearest,
                limit: 20,
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    time(
        "search_commodity buy, freshest (global)",
        20,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                sort: Sort::Freshest,
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    let recent = db.recent_markets(200).expect("recent");
    time(
        "station_market (random station)",
        50,
        |i| {
            db.station_market(recent[i % recent.len()].market_id)
                .expect("q")
        },
        |m| format!("{} rows", m.as_ref().map_or(0, |m| m.items.len())),
    );
    let names: Vec<String> = if gen_systems.is_empty() {
        recent.iter().map(|s| s.system.clone()).collect()
    } else {
        (0..50)
            .map(|i| gen_systems[(i * 7919) % gen_systems.len()].1.clone())
            .collect()
    };
    time(
        "system_by_name",
        50,
        |i| db.system_by_name(&names[i % names.len()]).expect("q"),
        |s| format!("found: {}", s.is_some()),
    );
    time(
        "find_systems prefix (autocomplete, limit 10)",
        50,
        |_| db.find_systems("Synth Re", 10).expect("q"),
        |v| format!("{} hits", v.len()),
    );
    time(
        "systems_near 30 ly (limit 100)",
        30,
        |i| {
            db.systems_near(origins[i % origins.len()], 30.0, 100)
                .expect("q")
        },
        |v| format!("{} hits", v.len()),
    );
    time(
        "stations_near: encoded material trader",
        30,
        |i| {
            db.stations_near(&StationQuery {
                origin: origins[i % origins.len()],
                material_trader: Some(MaterialTrader::Encoded),
                limit: 10,
                ..Default::default()
            })
            .expect("q")
        },
        |v| {
            format!(
                "{} hits, nearest {:.1} ly",
                v.len(),
                v.first().map_or(0.0, |h| h.distance_ly)
            )
        },
    );
    time(
        "stations_near: interstellar factors, pad L",
        30,
        |i| {
            db.stations_near(&StationQuery {
                origin: origins[i % origins.len()],
                services: Services::INTERSTELLAR_FACTORS,
                min_pad: Some('L'),
                limit: 10,
                ..Default::default()
            })
            .expect("q")
        },
        |v| format!("{} hits", v.len()),
    );
    time(
        "trade_routes radius 30 ly, hop 20 ly, 720 t",
        10,
        |i| {
            db.trade_routes(&TradeQuery {
                origin: origins[i % origins.len()],
                radius_ly: 30.0,
                max_hop_ly: 20.0,
                cargo_capacity: 720,
                max_age_hours: None,
                ..Default::default()
            })
            .expect("q")
        },
        |v| {
            format!(
                "{} routes, best {} cr",
                v.len(),
                v.first().map_or(0, |r| r.total_profit)
            )
        },
    );
    time(
        "trade_routes radius 60 ly, hop 30 ly, 720 t",
        10,
        |i| {
            db.trade_routes(&TradeQuery {
                origin: origins[i % origins.len()],
                radius_ly: 60.0,
                max_hop_ly: 30.0,
                cargo_capacity: 720,
                max_age_hours: None,
                ..Default::default()
            })
            .expect("q")
        },
        |v| {
            format!(
                "{} routes, best {} cr",
                v.len(),
                v.first().map_or(0, |r| r.total_profit)
            )
        },
    );
    time(
        "stats()",
        10,
        |_| db.stats().expect("q"),
        |s| format!("{} rows", s.market_rows),
    );

    // write while reading: live EDDN-like snapshots during queries
    let w = db.clone();
    let writer = std::thread::spawn(move || {
        let t = Instant::now();
        for i in 0..2000u64 {
            w.ingest(Ingest::Market(MarketSnapshot {
                market_id: 3_000_000_000 + (i * 7919 % 80_000) * 7,
                system_name: "Sol".into(),
                station_name: format!("Live {i}"),
                timestamp: now() + 1 + i as i64,
                items: (0..100)
                    .map(|k| MarketItem {
                        symbol: goods[k % goods.len()].to_string(),
                        buy: 1000 + k as i64,
                        stock: 10,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }));
            std::thread::sleep(Duration::from_millis(2));
        }
        w.flush().expect("flush");
        t.elapsed()
    });
    time(
        "search_commodity origin + 50 ly while ~200 snapshots/s are written",
        50,
        |i| {
            db.search_commodity(&CommodityQuery {
                commodity: goods[i % goods.len()].into(),
                origin: Some(origins[i % origins.len()]),
                max_distance_ly: Some(50.0),
                ..Default::default()
            })
            .expect("q")
        },
        n_hits,
    );
    let took = writer.join().expect("writer");
    println!(
        "\n2000 live snapshots written in {:.1} s",
        took.as_secs_f64()
    );
}
