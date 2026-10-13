//! Run planner: fills the orders the commander pinned on the Market page. For every pinned place
//! that buys something it finds where to buy it (a pinned seller of the same good, cargo already in
//! the hold, or the database's best seller near that buyer); for every pinned seller without a
//! pinned buyer it finds the best buyer. It then fits the purchases into the free hold space and the
//! credits, most profit per tonne first, and orders the stops so each good is bought before it's
//! sold and the trip is as short as it can be (exactly for up to eight stops). Runs in a background
//! job: it queries the galaxy database.

use crate::finder::Line;
use nexus_galaxy::{CommodityHit, CommodityQuery, GalaxyDb, Sort, StationMarket, TradeMode};
use rustc_hash::FxHashMap;

/// A pinned market entry, as the planner needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub market_id: u64,
    pub symbol: String,
    pub name: String,
    /// true: the station buys it (you sell there); false: the station sells it (you buy there).
    pub sell_here: bool,
}

#[derive(Debug, Clone)]
pub struct PlanInput {
    pub orders: Vec<Order>,
    pub origin: [f64; 3],
    pub cargo_capacity: i64,
    /// Tonnes in the hold now (all of it: what isn't sold on this run keeps taking space).
    pub cargo_used: i64,
    pub credits: i64,
    /// Sellable cargo (symbol, name, tonnes), sold before anything is bought for the same order.
    pub hold: Vec<Line>,
    pub use_hold: bool,
    pub min_pad: Option<char>,
    /// Fleet carriers may be picked as sellers or buyers the planner finds.
    pub carriers: bool,
    /// How far from the other end of an order the planner looks for a seller or buyer, ly.
    pub radius: f64,
    /// Pick sellers by price instead of by the shortest detour.
    pub cheapest: bool,
    pub max_age_h: f64,
}

/// A station on the run.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub market_id: u64,
    pub station: String,
    pub system: String,
    pub pos: Option<[f64; 3]>,
    pub is_carrier: bool,
    pub max_pad: Option<char>,
    /// When its market was last reported (unix seconds).
    pub updated_unix: i64,
}

/// One good taken from a seller (or the hold) to a buyer.
#[derive(Debug, Clone, PartialEq)]
pub struct Leg {
    pub symbol: String,
    pub name: String,
    pub tonnes: i64,
    /// `None`: already in the hold.
    pub from: Option<(Place, i64)>,
    pub to: (Place, i64),
    /// The planner found this end (it wasn't pinned).
    pub found_from: bool,
    pub found_to: bool,
}

/// (good, tonnes, price).
pub type Act = (String, i64, i64);

/// A station on the run and what to do there.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    pub place: Place,
    pub buys: Vec<Act>,
    pub sells: Vec<Act>,
    /// Straight-line distance from the previous stop (or from where you are), when both positions are known.
    pub leg_ly: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunPlan {
    pub legs: Vec<Leg>,
    pub stops: Vec<Stop>,
    pub cost: i64,
    pub revenue: i64,
    pub distance_ly: f64,
    /// What was left out or couldn't be found, in plain words.
    pub notes: Vec<String>,
}

impl RunPlan {
    pub fn profit(&self) -> i64 {
        self.revenue - self.cost
    }
}

fn dist(a: Option<[f64; 3]>, b: Option<[f64; 3]>) -> Option<f64> {
    let (a, b) = (a?, b?);
    Some(((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt())
}

fn place_of(m: &StationMarket) -> Place {
    let s = &m.station;
    Place { market_id: s.market_id, station: s.name.clone(), system: s.system.clone(), pos: s.pos, is_carrier: s.is_carrier, max_pad: s.max_pad, updated_unix: s.market_updated_unix }
}

fn place_of_hit(h: &CommodityHit) -> Place {
    Place { market_id: h.market_id, station: h.station.clone(), system: h.system.clone(), pos: h.pos, is_carrier: h.is_carrier, max_pad: h.max_pad, updated_unix: h.updated_unix }
}

/// A pinned order with today's price and quantity.
struct Live {
    order: Order,
    place: Place,
    price: i64,
    qty: i64,
}

/// A purchase the planner could make, before the hold and credits are shared out.
struct Candidate {
    symbol: String,
    name: String,
    from: (Place, i64),
    to: (Place, i64),
    max_t: i64,
    found_from: bool,
    found_to: bool,
}

pub fn plan(db: &GalaxyDb, input: &PlanInput) -> Result<RunPlan, String> {
    let mut notes = Vec::new();
    // 1. Today's prices of the pinned orders.
    let mut markets: FxHashMap<u64, Option<StationMarket>> = FxHashMap::default();
    let mut live: Vec<Live> = Vec::new();
    for o in &input.orders {
        if let std::collections::hash_map::Entry::Vacant(e) = markets.entry(o.market_id) {
            e.insert(db.station_market(o.market_id).map_err(|e| e.to_string())?);
        }
        let Some(m) = markets.get(&o.market_id).and_then(Option::as_ref) else {
            notes.push(format!("{}: the market is no longer in the database.", o.name));
            continue;
        };
        let row = m.items.iter().find(|r| r.symbol == o.symbol);
        let (price, qty) = match row {
            Some(r) if o.sell_here => (r.sell, r.demand),
            Some(r) => (r.buy, r.stock),
            None => (0, 0),
        };
        if price <= 0 || qty <= 0 {
            let what = if o.sell_here { "no longer buys" } else { "no longer sells" };
            notes.push(format!("{} {what} {} (as of its latest market).", m.station.name, o.name));
            continue;
        }
        live.push(Live { order: o.clone(), place: place_of(m), price, qty });
    }

    let mut hold_left: FxHashMap<String, i64> = if input.use_hold { input.hold.iter().map(|(s, _, t)| (s.clone(), *t)).collect() } else { FxHashMap::default() };
    let mut stock_left: Vec<i64> = live.iter().map(|l| if l.order.sell_here { 0 } else { l.qty }).collect();
    let mut legs: Vec<Leg> = Vec::new();
    let mut cands: Vec<Candidate> = Vec::new();
    let query = |symbol: &str, mode, origin: [f64; 3]| CommodityQuery {
        commodity: symbol.to_string(),
        mode,
        origin: Some(origin),
        max_distance_ly: Some(input.radius),
        min_pad: input.min_pad,
        max_age_hours: Some(input.max_age_h),
        min_quantity: 1,
        include_carriers: input.carriers,
        include_planetary: true,
        limit: 40,
        sort: Sort::BestPrice,
    };
    // Detour of buying at `via` on the way from here to `to`.
    let detour = |via: Option<[f64; 3]>, to: Option<[f64; 3]>| dist(Some(input.origin), via).unwrap_or(f64::INFINITY) + dist(via, to).unwrap_or(0.0);

    // 2. Every pinned buyer: from the hold first, then pinned sellers, then a seller near it.
    for t in live.iter().filter(|l| l.order.sell_here) {
        let mut need = t.qty;
        if let Some(h) = hold_left.get_mut(&t.order.symbol).filter(|h| **h > 0) {
            let take = need.min(*h);
            *h -= take;
            need -= take;
            legs.push(Leg { symbol: t.order.symbol.clone(), name: t.order.name.clone(), tonnes: take, from: None, to: (t.place.clone(), t.price), found_from: false, found_to: false });
        }
        if need <= 0 {
            continue;
        }
        let mut pinned_source = false;
        for (si, s) in live.iter().enumerate().filter(|(_, l)| !l.order.sell_here && l.order.symbol == t.order.symbol) {
            pinned_source = true;
            let take = need.min(stock_left[si]);
            if take > 0 && s.place.market_id != t.place.market_id {
                stock_left[si] -= take;
                need -= take;
                cands.push(Candidate { symbol: t.order.symbol.clone(), name: t.order.name.clone(), from: (s.place.clone(), s.price), to: (t.place.clone(), t.price), max_t: take, found_from: false, found_to: false });
            }
        }
        if need <= 0 || pinned_source {
            continue;
        }
        let around = t.place.pos.unwrap_or(input.origin);
        let hits = db.search_commodity(&query(&t.order.symbol, TradeMode::BuyFromStation, around)).map_err(|e| e.to_string())?;
        // Sellers that leave a profit, best first; one with enough stock for a hold if there is
        // one, otherwise up to three that together come closest.
        let mut usable: Vec<&CommodityHit> = hits.iter().filter(|h| h.price < t.price && h.market_id != t.place.market_id).collect();
        if input.cheapest {
            usable.sort_by(|a, b| a.price.cmp(&b.price).then(detour(a.pos, t.place.pos).total_cmp(&detour(b.pos, t.place.pos))));
        } else {
            usable.sort_by(|a, b| detour(a.pos, t.place.pos).total_cmp(&detour(b.pos, t.place.pos)).then(a.price.cmp(&b.price)));
        }
        if usable.is_empty() {
            notes.push(format!(
                "Nobody within {:.0} ly of {} sells {} for less than it pays ({} t not covered).",
                input.radius,
                t.place.station,
                t.order.name,
                nexus_data::names::group_digits(need),
            ));
            continue;
        }
        let want = need.min(input.cargo_capacity.max(1));
        let picks: Vec<&CommodityHit> = match usable.iter().find(|h| h.quantity >= want) {
            Some(h) => vec![*h],
            None => usable.into_iter().take(3).collect(),
        };
        for h in picks {
            let take = need.min(h.quantity);
            if take <= 0 {
                break;
            }
            need -= take;
            cands.push(Candidate { symbol: t.order.symbol.clone(), name: t.order.name.clone(), from: (place_of_hit(h), h.price), to: (t.place.clone(), t.price), max_t: take, found_from: true, found_to: false });
        }
    }

    // 3. Pinned sellers nobody pinned a buyer for: the best buyer near them.
    for (si, s) in live.iter().enumerate().filter(|(_, l)| !l.order.sell_here) {
        if live.iter().any(|l| l.order.sell_here && l.order.symbol == s.order.symbol) || stock_left[si] <= 0 {
            continue;
        }
        let around = s.place.pos.unwrap_or(input.origin);
        let hits = db.search_commodity(&query(&s.order.symbol, TradeMode::SellToStation, around)).map_err(|e| e.to_string())?;
        let best = hits
            .iter()
            .filter(|h| h.price > s.price && h.market_id != s.place.market_id)
            .max_by(|a, b| a.price.cmp(&b.price).then(dist(s.place.pos, b.pos).unwrap_or(f64::INFINITY).total_cmp(&dist(s.place.pos, a.pos).unwrap_or(f64::INFINITY))));
        match best {
            Some(h) => cands.push(Candidate {
                symbol: s.order.symbol.clone(),
                name: s.order.name.clone(),
                from: (s.place.clone(), s.price),
                to: (place_of_hit(h), h.price),
                max_t: stock_left[si].min(h.quantity),
                found_from: false,
                found_to: true,
            }),
            None => notes.push(format!("Nobody within {:.0} ly of {} pays more than {} for {}.", input.radius, s.place.station, crate::widgets::credits(s.price), s.order.name)),
        }
    }

    // 4. Share out the free hold space and the credits, most profit per tonne first.
    let mut free = (input.cargo_capacity - input.cargo_used).max(0);
    let mut credits = input.credits.max(0);
    cands.sort_by_key(|c| std::cmp::Reverse(c.to.1 - c.from.1));
    for c in cands {
        let affordable = credits / c.from.1.max(1);
        let t = c.max_t.min(free).min(affordable);
        if t < c.max_t {
            let why = if affordable < c.max_t.min(free) { "credits" } else { "hold space" };
            notes.push(format!("{}: {} of {} t fit ({why}).", c.name, nexus_data::names::group_digits(t), nexus_data::names::group_digits(c.max_t)));
        }
        if t <= 0 {
            continue;
        }
        free -= t;
        credits -= t * c.from.1;
        legs.push(Leg { symbol: c.symbol, name: c.name, tonnes: t, from: Some(c.from), to: c.to, found_from: c.found_from, found_to: c.found_to });
    }

    // 5. Stops, in an order that buys before it sells.
    let (stops, distance_ly, cyclic) = order_stops(input.origin, &legs);
    if cyclic {
        notes.push("Some goods are bought where others are sold and the other way round: one of those stations needs a second visit.".into());
    }
    let cost = legs.iter().filter_map(|l| l.from.as_ref().map(|f| f.1 * l.tonnes)).sum();
    let revenue = legs.iter().map(|l| l.to.1 * l.tonnes).sum();
    Ok(RunPlan { legs, stops, cost, revenue, distance_ly, notes })
}

/// Orders the stations of a run: each good bought before it's sold, shortest straight-line trip
/// from `origin` (every order is tried for up to eight stations, the nearest next one beyond).
/// Returns the stops, the trip length, and whether the goods' order could not be met everywhere.
pub fn order_stops(origin: [f64; 3], legs: &[Leg]) -> (Vec<Stop>, f64, bool) {
    let mut places: Vec<Place> = Vec::new();
    let idx = |p: &Place, places: &mut Vec<Place>| match places.iter().position(|q| q.market_id == p.market_id) {
        Some(i) => i,
        None => {
            places.push(p.clone());
            places.len() - 1
        }
    };
    // before[j]: stops that must come before stop j.
    let mut before: Vec<Vec<usize>> = Vec::new();
    let mut acts: Vec<(Vec<Act>, Vec<Act>)> = Vec::new();
    for l in legs {
        let to = idx(&l.to.0, &mut places);
        let from = l.from.as_ref().map(|f| idx(&f.0, &mut places));
        before.resize(places.len(), Vec::new());
        acts.resize(places.len(), (Vec::new(), Vec::new()));
        acts[to].1.push((l.name.clone(), l.tonnes, l.to.1));
        if let Some(f) = from {
            acts[f].0.push((l.name.clone(), l.tonnes, l.from.as_ref().map_or(0, |x| x.1)));
            if f != to && !before[to].contains(&f) {
                before[to].push(f);
            }
        }
    }
    let n = places.len();
    let leg = |a: Option<[f64; 3]>, b: Option<[f64; 3]>| dist(a, b).unwrap_or(0.0);
    let mut order: Vec<usize> = Vec::new();
    let mut cyclic = false;
    if n > 0 && n <= 8 {
        // Depth-first over the orders the precedence allows, keeping the shortest.
        fn search(
            at: Option<[f64; 3]>,
            done: &mut Vec<usize>,
            len: f64,
            best: &mut (f64, Vec<usize>),
            places: &[Place],
            before: &[Vec<usize>],
            leg: &dyn Fn(Option<[f64; 3]>, Option<[f64; 3]>) -> f64,
        ) {
            if len >= best.0 {
                return;
            }
            if done.len() == places.len() {
                *best = (len, done.clone());
                return;
            }
            for j in 0..places.len() {
                if done.contains(&j) || !before[j].iter().all(|b| done.contains(b)) {
                    continue;
                }
                done.push(j);
                search(places[j].pos, done, len + leg(at, places[j].pos), best, places, before, leg);
                done.pop();
            }
        }
        let mut best = (f64::INFINITY, Vec::new());
        search(Some(origin), &mut Vec::new(), 0.0, &mut best, &places, &before, &leg);
        order = best.1;
    }
    if order.len() != n {
        // Greedy: the nearest station whose goods are aboard; when none qualifies (a cycle), the nearest.
        order.clear();
        let mut at = Some(origin);
        while order.len() < n {
            let open = |j: &usize| !order.contains(j);
            let ready: Vec<usize> = (0..n).filter(open).filter(|j| before[*j].iter().all(|b| order.contains(b))).collect();
            let pool: Vec<usize> = if ready.is_empty() {
                cyclic = true;
                (0..n).filter(open).collect()
            } else {
                ready
            };
            let Some(next) = pool.into_iter().min_by(|a, b| leg(at, places[*a].pos).total_cmp(&leg(at, places[*b].pos))) else { break };
            at = places[next].pos;
            order.push(next);
        }
    }
    let mut at = Some(origin);
    let mut total = 0.0;
    let stops = order
        .into_iter()
        .map(|j| {
            let d = dist(at, places[j].pos);
            total += d.unwrap_or(0.0);
            at = places[j].pos.or(at);
            let (buys, sells) = acts[j].clone();
            Stop { place: places[j].clone(), buys, sells, leg_ly: d }
        })
        .collect();
    (stops, total, cyclic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{TempDb, good, station};
    use serde_json::json;

    fn place(id: u64, x: f64) -> Place {
        Place { market_id: id, station: format!("S{id}"), system: format!("Sys{id}"), pos: Some([x, 0.0, 0.0]), is_carrier: false, max_pad: Some('L'), updated_unix: 0 }
    }

    fn leg(from: Option<(u64, f64)>, to: (u64, f64)) -> Leg {
        Leg { symbol: "gold".into(), name: "Gold".into(), tonnes: 10, from: from.map(|(id, x)| (place(id, x), 100)), to: (place(to.0, to.1), 200), found_from: false, found_to: false }
    }

    #[test]
    fn stops_buy_before_selling_on_the_shortest_trip() {
        // Seller A at 50, its buyer at 10; seller B at 20, its buyer at 60. From 0.
        let legs = [leg(Some((1, 50.0)), (2, 10.0)), leg(Some((3, 20.0)), (4, 60.0))];
        let (stops, total, cyclic) = order_stops([0.0; 3], &legs);
        let ids: Vec<u64> = stops.iter().map(|s| s.place.market_id).collect();
        // 0 → 20 (buy B) → 50 (buy A) → 60 (sell B) → 10 (sell A) = 20+30+10+50 = 110;
        // 0 → 20 → 50 → 10 → 60 = 20+30+40+50 = 140.
        assert_eq!(ids, vec![3, 1, 4, 2]);
        assert!((total - 110.0).abs() < 1e-9);
        assert!(!cyclic);
        assert_eq!(stops[0].buys.len(), 1);
        assert_eq!(stops[3].sells, vec![("Gold".to_string(), 10, 200)]);
        assert_eq!(stops[0].leg_ly, Some(20.0));
    }

    #[test]
    fn cargo_in_the_hold_has_no_seller_to_visit() {
        let legs = [leg(None, (2, 30.0)), leg(Some((1, 10.0)), (2, 30.0))];
        let (stops, total, _) = order_stops([0.0; 3], &legs);
        assert_eq!(stops.len(), 2, "one seller, one buyer");
        assert_eq!(stops[1].sells.len(), 2, "both sales at the buyer");
        assert!((total - 30.0).abs() < 1e-9);
    }

    #[test]
    fn plans_sellers_for_pinned_buy_orders() {
        let t = TempDb::new("runplan");
        let db = t.db();
        // A carrier-style buy order for gold at x=100; sellers at x=90 (cheap, little stock) and x=60.
        station(db, ("Target", 9001, 100.0), "Buyer Port", 7001, json!([good("gold", 0, 0, 20_000, 300)]));
        station(db, ("Near", 9002, 90.0), "Near Mart", 7002, json!([good("gold", 9_000, 50, 0, 0)]));
        station(db, ("Mid", 9003, 60.0), "Mid Mart", 7003, json!([good("gold", 9_500, 5_000, 0, 0)]));
        // A pinned cheap silver source, with a buyer the planner should find.
        station(db, ("Silverton", 9004, 20.0), "Silver Source", 7004, json!([good("silver", 1_000, 400, 0, 0)]));
        station(db, ("Silverton B", 9005, 25.0), "Silver Buyer", 7005, json!([good("silver", 0, 0, 4_000, 1_000)]));
        db.flush().unwrap();
        let input = PlanInput {
            orders: vec![
                Order { market_id: 7001, symbol: "gold".into(), name: "Gold".into(), sell_here: true },
                Order { market_id: 7004, symbol: "silver".into(), name: "Silver".into(), sell_here: false },
            ],
            origin: [0.0; 3],
            cargo_capacity: 400,
            cargo_used: 6,
            credits: 100_000_000,
            hold: vec![("gold".into(), "Gold".into(), 6)],
            use_hold: true,
            min_pad: Some('L'),
            carriers: true,
            radius: 200.0,
            cheapest: false,
            max_age_h: 72.0,
        };
        let p = plan(db, &input).expect("plan");
        let gold: Vec<&Leg> = p.legs.iter().filter(|l| l.symbol == "gold").collect();
        assert_eq!(gold[0].from, None, "the 6 t aboard are sold first");
        assert_eq!(gold[0].tonnes, 6);
        let bought = gold.iter().find(|l| l.from.is_some()).expect("a seller for the rest");
        assert_eq!(bought.from.as_ref().unwrap().0.station, "Mid Mart", "Near Mart has too little stock for a hold");
        let silver = p.legs.iter().find(|l| l.symbol == "silver").expect("silver leg");
        assert_eq!(silver.to.0.station, "Silver Buyer");
        assert!(silver.found_to);
        // 394 t of free space shared: gold first (10,500 Cr/t profit, the 294 t still wanted), then silver (3,000 Cr/t).
        assert_eq!(bought.tonnes, 294);
        assert_eq!(silver.tonnes, 100);
        assert!(p.notes.iter().any(|n| n.starts_with("Silver: 100 of 400 t fit (hold space)")), "{:?}", p.notes);
        assert_eq!(p.revenue, 300 * 20_000 + 100 * 4_000);
        assert_eq!(p.cost, 294 * 9_500 + 100 * 1_000);
        // Every stop on the trip; the gold seller comes before the gold buyer.
        let at = |name: &str| p.stops.iter().position(|s| s.place.station == name).unwrap();
        assert!(at("Mid Mart") < at("Buyer Port"));
    }

    #[test]
    fn small_sellers_are_combined() {
        let t = TempDb::new("runplan-small");
        let db = t.db();
        station(db, ("Hub", 9101, 50.0), "Hub Port", 7101, json!([good("palladium", 0, 0, 60_000, 100)]));
        station(db, ("A", 9102, 40.0), "A Mart", 7102, json!([good("palladium", 45_000, 30, 0, 0)]));
        station(db, ("B", 9103, 30.0), "B Mart", 7103, json!([good("palladium", 46_000, 40, 0, 0)]));
        db.flush().unwrap();
        let input = PlanInput {
            orders: vec![Order { market_id: 7101, symbol: "palladium".into(), name: "Palladium".into(), sell_here: true }],
            origin: [0.0; 3],
            cargo_capacity: 400,
            cargo_used: 0,
            credits: 100_000_000,
            hold: Vec::new(),
            use_hold: true,
            min_pad: None,
            carriers: true,
            radius: 100.0,
            cheapest: false,
            max_age_h: 72.0,
        };
        let p = plan(db, &input).expect("plan");
        let mut from: Vec<(String, i64)> = p.legs.iter().map(|l| (l.from.as_ref().unwrap().0.station.clone(), l.tonnes)).collect();
        from.sort();
        assert_eq!(from, vec![("A Mart".to_string(), 30), ("B Mart".to_string(), 40)], "neither has 100 t: both are used");
        let names: Vec<&str> = p.stops.iter().map(|s| s.place.station.as_str()).collect();
        assert_eq!(names, vec!["B Mart", "A Mart", "Hub Port"], "on the way out");
        assert!((p.distance_ly - 50.0).abs() < 1e-9);
    }
}
