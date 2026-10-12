//! Untouched biology: planets where a valuable species should grow and that show no trace of a
//! data-sharing commander having mapped or sampled them, so the first-logged bonus (×5) is likely
//! still there.
//!
//! Only the opposite can be known for sure: Spansh lists a body's genera once someone DSS-maps it
//! with a tool that uploads to EDDN (EDMC, EDDiscovery, …), and biology landmarks once a sample
//! adds a codex entry; a footfall flag says someone landed. A body with biology signals but none
//! of that has never been mapped or sampled by a commander who shares data. Two blind spots stay:
//! players without such tools, and samples of a species the commander already had in their codex
//! for that region (the game writes no codex entry then). So this is a strong lead, not a
//! guarantee.

use crate::ai_tools::{STANDARD_ATM, STANDARD_G, journal_planet_class};
use nexus_data::bio_predict::{self as bp, BodyFacts};
use nexus_data::exobio::{self, Species};
use nexus_spansh::{BodyRecord, Coords, Reference, SearchRequest, SortDir, Spansh, filter, sort};
use serde_json::json;

/// What the evidence says about a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// Never DSS-mapped or sampled by anyone sharing data; the species is predicted from conditions.
    NoTrace,
    /// DSS-mapped (so the genus is confirmed there) but no samples on record.
    GenusConfirmed,
}

impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Evidence::NoTrace => "No record",
            Evidence::GenusConfirmed => "Mapped, no samples",
        }
    }

    pub fn explain(self) -> &'static str {
        match self {
            Evidence::NoTrace => "No commander who shares data has mapped or sampled this planet. The species is predicted from its conditions.",
            Evidence::GenusConfirmed => {
                "Someone mapped it with the DSS, so the genus is confirmed there, but no samples are on record. They may have sampled without leaving a record, so the bonus may be gone."
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub system: String,
    pub system_pos: Option<[f64; 3]>,
    pub body: String,
    pub distance_ly: f64,
    pub distance_ls: Option<f64>,
    pub atmosphere: String,
    pub temperature_k: Option<f64>,
    pub gravity_g: Option<f64>,
    pub bio_signals: u32,
    pub evidence: Evidence,
    /// Chance the target species is there (genus odds × species share).
    pub chance: f32,
    /// Payout for the target species with the first-logged bonus (5×).
    pub value_first_logged: i64,
    /// Newest data Spansh has about the body (unix seconds).
    pub last_data: Option<i64>,
}

/// How a search went: what was looked at and why bodies were left out.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// Bodies with the right conditions and biology signals within range (Spansh's count, capped).
    pub matched: u64,
    pub examined: usize,
    /// Someone logged samples there (they landed).
    pub sampled: usize,
    /// DSS-mapped, and the target genus isn't there.
    pub mapped_without: usize,
    pub populated: usize,
    /// Your journal or live EDDN data say someone set foot there.
    pub footfalled: usize,
    /// The precise rules rule the species out.
    pub ruled_out: usize,
    /// Distance of the farthest body examined (Spansh returns the nearest first).
    pub reach_ly: f64,
}

pub struct Query {
    pub species: &'static Species,
    pub origin: [f64; 3],
    pub radius_ly: f64,
    /// Candidates to collect before the search stops reading further out.
    pub limit: usize,
}

/// Species worth a trip, most valuable first (10 M Cr or more, never Thargoid samples).
pub fn targets() -> Vec<&'static Species> {
    let mut v: Vec<&'static Species> = exobio::all_species().iter().filter(|s| s.value >= 10_000_000 && !s.thargoid).collect();
    v.sort_by(|a, b| b.value.cmp(&a.value).then(a.name.cmp(b.name)));
    v
}

/// Journal `AtmosphereType` → every spelling Spansh uses for it ("Thin Carbon dioxide", "Hot thin …").
fn spansh_atmospheres(journal: &str) -> Vec<String> {
    if journal.eq_ignore_ascii_case("None") {
        return vec!["No atmosphere".into()];
    }
    let (base, rich) = match journal.strip_suffix("Rich") {
        Some(b) => (b, true),
        None => (journal, false),
    };
    // CamelCase → "Carbon dioxide"; "AmmoniaOxygen" → "Ammonia and Oxygen".
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in base.chars() {
        if c.is_ascii_uppercase() && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    let name = if words.len() == 2 && words[0] == "Ammonia" && words[1] == "Oxygen" {
        "Ammonia and Oxygen".to_string()
    } else {
        let mut s = String::new();
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                s.push(' ');
                s.push_str(&w.to_ascii_lowercase());
            } else {
                s.push_str(w);
            }
        }
        s
    };
    let name = if rich { format!("{name}-rich") } else { name };
    ["", "Thin ", "Hot thin ", "Hot "].iter().map(|p| format!("{p}{name}")).collect()
}

/// Journal `PlanetClass` → Spansh sub-type.
fn spansh_subtype(journal: &str) -> &str {
    match journal {
        "High metal content body" => "High metal content world",
        "Metal rich body" => "Metal-rich body",
        "Rocky ice body" => "Rocky Ice world",
        "Earthlike body" => "Earth-like world",
        other => other,
    }
}

/// The Spansh body search for bodies within the species' conditions envelope with biology signals.
pub fn request(q: &Query, page: u32) -> SearchRequest {
    let env = &q.species.envelope;
    let mut req = SearchRequest::new()
        .near(Reference::Coords(Coords { x: q.origin[0], y: q.origin[1], z: q.origin[2] }))
        .within(q.radius_ly)
        .filter("is_landable", filter::eq(true))
        .filter("signals", json!([filter::named_count("Biological", 1, 99)]))
        .sort(sort::field("distance", SortDir::Asc))
        .size(nexus_spansh::search::MAX_PAGE_SIZE)
        .page(page);
    if let Some(types) = &env.body_types {
        req = req.filter("subtype", filter::any_of(types.iter().map(|t| spansh_subtype(t).to_string())));
    }
    if let Some(atmos) = &env.atmospheres {
        req = req.filter("atmosphere", filter::any_of(atmos.iter().flat_map(|a| spansh_atmospheres(a))));
    }
    if env.min_gravity_g.is_some() || env.max_gravity_g.is_some() {
        req = req.filter("gravity", filter::between(env.min_gravity_g.unwrap_or(0.0), env.max_gravity_g.unwrap_or(100.0)));
    }
    if env.min_temp_k.is_some() || env.max_temp_k.is_some() {
        req = req.filter("surface_temperature", filter::between(env.min_temp_k.unwrap_or(0.0), env.max_temp_k.unwrap_or(10_000.0)));
    }
    req
}

/// Prediction inputs from a Spansh search record.
fn facts(b: &BodyRecord) -> BodyFacts {
    let volcanism = b
        .volcanism_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("No volcanism"))
        .map(|v| format!("{} volcanism", v.to_ascii_lowercase()));
    let pos = match (b.system_x, b.system_y, b.system_z) {
        (Some(x), Some(y), Some(z)) => Some([x, y, z]),
        _ => None,
    };
    BodyFacts {
        planet_class: b.subtype.as_deref().map(journal_planet_class).unwrap_or_default(),
        atmosphere: b.atmosphere.clone(),
        atmosphere_composition: b.atmosphere_composition.iter().map(|s| (s.name.replace(' ', ""), s.share)).collect(),
        gravity_g: b.gravity.map(|g| g * STANDARD_G / exobio::JOURNAL_G_MS2),
        temperature_k: b.surface_temperature,
        pressure_atm: b.surface_pressure.map(|p| p * STANDARD_ATM / exobio::JOURNAL_ATM_PA),
        volcanism,
        landable: true,
        materials: b.materials.iter().map(|m| (m.name.to_ascii_lowercase(), m.share)).collect(),
        distance_from_arrival_ls: b.distance_to_arrival.unwrap_or(0.0),
        system_name: b.system_name.clone(),
        star_pos: pos,
        region: b.system_region.clone(),
        bio_signal_count: Some(b.signal("Biological")).filter(|n| *n > 0),
        ..BodyFacts::default()
    }
}

/// Why a body is left out, or its evidence and the chance of the species.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    Sampled,
    Mapped,
    RuledOut,
    Candidate(Evidence, f32),
}

/// Judges one Spansh body for `species`.
pub fn classify(b: &BodyRecord, species: &Species) -> Verdict {
    // A biology landmark means someone logged samples there, on foot.
    let sampled = b.landmarks.iter().any(|l| l.value > 0 || exobio::genus(&l.kind).is_some());
    if sampled {
        return Verdict::Sampled;
    }
    let genera = &b.genuses;
    let has_genus = genera.iter().any(|g| exobio::genus(g).is_some_and(|x| x.name == species.genus));
    if !genera.is_empty() && !has_genus {
        return Verdict::Mapped;
    }
    let chance = bp::predict(&facts(b), genera).iter().find(|p| p.species == species.name).map_or(0.0, |p| p.likelihood);
    if chance <= 0.0 {
        return Verdict::RuledOut;
    }
    Verdict::Candidate(if genera.is_empty() { Evidence::NoTrace } else { Evidence::GenusConfirmed }, chance)
}

/// Pages of 500 bodies read at most (nearest first).
const MAX_PAGES: u32 = 4;

/// Searches Spansh nearest-first until `limit` candidates turn up (at most 2,000 bodies), drops
/// populated systems and bodies your own data says were footfalled, and ranks every candidate
/// found by expected first-logged value against distance (all of them, so they match `Stats`).
pub async fn find(spansh: &Spansh, q: &Query, footfalled: impl Fn(&str) -> bool) -> Result<(Vec<Candidate>, Stats), String> {
    let mut stats = Stats::default();
    let mut out: Vec<Candidate> = Vec::new();
    for page in 0..MAX_PAGES {
        let res = spansh.search_bodies(&request(q, page)).await.map_err(|e| e.to_string())?;
        if page == 0 {
            stats.matched = res.count;
        }
        let n = res.results.len();
        stats.examined += n;
        if let Some(d) = res.results.last().and_then(|b| b.distance) {
            stats.reach_ly = d;
        }
        let mut found: Vec<(BodyRecord, Evidence, f32)> = Vec::new();
        for b in res.results {
            match classify(&b, q.species) {
                Verdict::Sampled => stats.sampled += 1,
                Verdict::Mapped => stats.mapped_without += 1,
                Verdict::RuledOut => stats.ruled_out += 1,
                Verdict::Candidate(e, c) => found.push((b, e, c)),
            }
        }
        let populated = populated_systems(spansh, &found).await;
        for (b, evidence, chance) in found {
            if b.system_id64.is_some_and(|id| populated.contains(&id)) {
                stats.populated += 1;
            } else if footfalled(&b.name) {
                stats.footfalled += 1;
            } else {
                out.push(candidate(b, evidence, chance, q.species));
            }
        }
        if out.len() >= q.limit || n < nexus_spansh::search::MAX_PAGE_SIZE as usize {
            break;
        }
    }
    // Best first: expected bonus value, a little less for planets someone has at least mapped, and
    // a gentle distance penalty.
    let score = |c: &Candidate| {
        let trust = if c.evidence == Evidence::NoTrace { 1.0 } else { 0.85 };
        f64::from(c.chance) * c.value_first_logged as f64 * trust / (1.0 + c.distance_ly / 250.0)
    };
    out.sort_by(|a, b| score(b).total_cmp(&score(a)));
    Ok((out, stats))
}

/// Populated systems never pay the bonus: one systems search answers for a whole page.
async fn populated_systems(spansh: &Spansh, found: &[(BodyRecord, Evidence, f32)]) -> std::collections::HashSet<u64> {
    let mut ids: Vec<u64> = found.iter().filter_map(|(b, _, _)| b.system_id64).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut populated = std::collections::HashSet::new();
    if !ids.is_empty() {
        let req = SearchRequest::new().filter("id64", filter::any_of(ids.iter().map(u64::to_string))).size(nexus_spansh::search::MAX_PAGE_SIZE);
        if let Ok(res) = spansh.search_systems(&req).await {
            populated.extend(res.results.iter().filter(|s| s.population.unwrap_or(0) > 0).map(|s| s.id64));
        }
    }
    populated
}

fn candidate(b: BodyRecord, evidence: Evidence, chance: f32, species: &Species) -> Candidate {
    let last = [b.updated_at.as_deref(), b.signals_updated_at.as_deref()].into_iter().flatten().filter_map(nexus_spansh::parse_timestamp).max();
    let system_pos = match (b.system_x, b.system_y, b.system_z) {
        (Some(x), Some(y), Some(z)) => Some([x, y, z]),
        _ => None,
    };
    Candidate {
        bio_signals: b.signal("Biological"),
        system_pos,
        system: b.system_name,
        body: b.name,
        distance_ly: b.distance.unwrap_or(0.0),
        distance_ls: b.distance_to_arrival,
        atmosphere: b.atmosphere.unwrap_or_default(),
        temperature_k: b.surface_temperature,
        gravity_g: b.gravity,
        evidence,
        chance,
        value_first_logged: bp::payout(species.value, true),
        last_data: last,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(json: serde_json::Value) -> BodyRecord {
        serde_json::from_value(json).expect("body")
    }

    fn stratum() -> &'static Species {
        exobio::species("Stratum Tectonicas").expect("species")
    }

    fn hmc(extra: serde_json::Value) -> BodyRecord {
        let mut v = json!({
            "id64": 1, "name": "Test Sector AB-C d1-2 3", "type": "Planet", "subtype": "High metal content world",
            "is_landable": true, "gravity": 0.3, "surface_temperature": 190.0, "surface_pressure": 0.01,
            "atmosphere": "Thin Carbon dioxide", "atmosphere_composition": [{"name": "Carbon dioxide", "share": 99.0}],
            "signals": [{"name": "Biological", "count": 2}], "system_name": "Test Sector AB-C d1-2", "system_id64": 42,
            "system_x": 1000.0, "system_y": 0.0, "system_z": 3000.0, "system_region": "Inner Orion Spur", "distance": 120.0,
            "distance_to_arrival": 800.0, "updated_at": "2024-04-23T10:00:00Z"
        });
        if let (Some(base), Some(add)) = (v.as_object_mut(), extra.as_object()) {
            for (k, x) in add {
                base.insert(k.clone(), x.clone());
            }
        }
        body(v)
    }

    #[test]
    fn spansh_spellings_cover_every_thickness() {
        assert_eq!(spansh_atmospheres("CarbonDioxide"), vec!["Carbon dioxide", "Thin Carbon dioxide", "Hot thin Carbon dioxide", "Hot Carbon dioxide"]);
        assert_eq!(spansh_atmospheres("CarbonDioxideRich")[1], "Thin Carbon dioxide-rich");
        assert_eq!(spansh_atmospheres("SulphurDioxide")[1], "Thin Sulphur dioxide");
        assert_eq!(spansh_atmospheres("ArgonRich")[1], "Thin Argon-rich");
        assert_eq!(spansh_atmospheres("AmmoniaOxygen")[1], "Thin Ammonia and Oxygen");
        assert_eq!(spansh_atmospheres("None"), vec!["No atmosphere"]);
    }

    #[test]
    fn the_request_follows_the_species_envelope() {
        let q = Query { species: stratum(), origin: [0.0, 0.0, 0.0], radius_ly: 500.0, limit: 10 };
        let j = request(&q, 0).to_json();
        let f = &j["filters"];
        assert_eq!(f["subtype"]["value"], json!(["High metal content world"]));
        assert!(f["atmosphere"]["value"].as_array().is_some_and(|a| a.contains(&json!("Thin Carbon dioxide"))));
        assert_eq!(f["signals"][0]["name"], "Biological");
        assert_eq!(f["distance"]["max"], "500");
    }

    #[test]
    fn evidence_of_samples_or_another_genus_excludes_a_body() {
        let sp = stratum();
        assert!(matches!(classify(&hmc(json!({})), sp), Verdict::Candidate(Evidence::NoTrace, c) if c > 0.0));
        // someone logged samples (a biology landmark) → they landed
        let sampled = hmc(json!({"landmarks": [{"type": "Bacterium", "subtype": "Bacterium Aurasus", "value": 1000000}]}));
        assert_eq!(classify(&sampled, sp), Verdict::Sampled);
        // a settlement is not evidence of biology sampling
        let settlement = hmc(json!({"landmarks": [{"type": "Surface Station", "subtype": "Settlement", "value": 0}]}));
        assert!(matches!(classify(&settlement, sp), Verdict::Candidate(..)));
        // mapped: genera known
        let with_stratum = hmc(json!({"genuses": [{"name": "Stratum"}, {"name": "Bacterial"}]}));
        assert!(matches!(classify(&with_stratum, sp), Verdict::Candidate(Evidence::GenusConfirmed, _)));
        let without = hmc(json!({"genuses": [{"name": "Bacterial"}]}));
        assert_eq!(classify(&without, sp), Verdict::Mapped, "mapped and no Stratum there");
        // too cold for Stratum
        let cold = hmc(json!({"surface_temperature": 140.0}));
        assert_eq!(classify(&cold, sp), Verdict::RuledOut);
        // candidates carry their system's position for tours
        let c = candidate(hmc(json!({})), Evidence::NoTrace, 0.9, sp);
        assert_eq!(c.system_pos, Some([1000.0, 0.0, 3000.0]));
    }

    /// Live: `cargo test -p nexus-app untouched -- --ignored --nocapture` (needs the internet).
    #[test]
    #[ignore]
    fn live_search_three_thousand_ly_out() {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
        let q = Query { species: stratum(), origin: [1200.0, -40.0, 2800.0], radius_ly: 400.0, limit: 10 };
        let started = std::time::Instant::now();
        let (list, st) = rt.block_on(find(&Spansh::new(), &q, |_| false)).expect("search");
        println!("{:?} in {:.1} s", st, started.elapsed().as_secs_f64());
        for c in &list {
            println!("{:>5.0} ly  {:<40} {:?} {:>3.0}%  {}", c.distance_ly, c.body, c.evidence, c.chance * 100.0, c.atmosphere);
        }
        assert!(!list.is_empty());
        assert!(list.iter().all(|c| c.chance > 0.0));
    }

    #[test]
    fn targets_are_the_valuable_species() {
        let t = targets();
        assert!(t.first().is_some_and(|s| s.value >= 19_000_000));
        assert!(t.iter().any(|s| s.name == "Stratum Tectonicas"));
        assert!(t.iter().all(|s| s.value >= 10_000_000));
    }
}
