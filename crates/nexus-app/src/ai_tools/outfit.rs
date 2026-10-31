//! The assistant's Spansh catalogue tools: which stations list a ship or a module, and which landable bodies hold a
//! raw material. Each splits "ask Spansh" (`NexusTools`) from "shape the records into JSON" (plain functions), so the
//! shaping, and the size rule that keeps a reply under what the model is given of a tool result, are tested offline.

use super::{NexusTools, SPANSH_TIMEOUT, arg_f64, arg_str, r1};
use nexus_data::materials::{self, Material, MaterialKind};
use nexus_spansh::{BodyRecord, Coords, SearchKind, SearchRequest, SortDir, StationRecord, filter};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::future::Future;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// What the model is given of a result, less room for the notes added after the size check.
const BUDGET: usize = nexus_ai::agent::MAX_RESULT_CHARS - 1_000;

/// Spansh's body search answers 100 rows a page here; five pages are 500 bodies. The cap keeps the answer quick: each
/// page is one request to Spansh.
const BODY_PAGE: u32 = 100;
const MAX_BODY_PAGES: u32 = 5;
/// Bodies listed, and the number of hits after which the search stops reading further pages.
const BODY_HITS: usize = 10;

/// Matches of a module listed per station, from the most to the fewest that fit.
const MATCH_LEVELS: [usize; 3] = [6, 3, 1];
/// Spellings offered when a name is not recognised.
const SUGGESTIONS: usize = 8;

const SHIP_NOTE: &str = "The station's shipyard catalogue from EDDN reports, not its stock: a listed ship can be sold out (EDDN also carries ships shown as unavailable). Only a visit to the shipyard confirms it. Never call it in stock.";
const MODULE_NOTE: &str = "The station's outfitting catalogue from EDDN reports, not its stock: only a visit to the outfitting screen confirms what is for sale.";

/// How long the lists of names Spansh knows (ships, modules) are kept, and how long a failed fetch is remembered (an
/// empty list) so the next question doesn't wait for another timeout.
const CATALOGUE_TTL: Duration = Duration::from_secs(6 * 3600);
const CATALOGUE_FAILED_TTL: Duration = Duration::from_secs(60);
/// What `BodySearch::interrupted` holds when the search ran out of time.
const TIME_LIMIT: &str = "time limit";
/// The whole body search, all pages together, stops after this.
const BODY_SEARCH_LIMIT: Duration = Duration::from_secs(60);
type Catalogue = (&'static str, Instant, Vec<String>);
static CATALOGUES: LazyLock<Mutex<Vec<Catalogue>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// `"krait mk2"`, `"Krait MkII"`, `"krait mark 2"` and `"Krait Mk II"` all give `kraitmkii`.
fn ship_key(name: &str) -> String {
    let lower = name.to_lowercase().replace("mark", "mk");
    let chars: Vec<char> = lower.chars().collect();
    let roman = |c: char| match c {
        '1' => Some("i"),
        '2' => Some("ii"),
        '3' => Some("iii"),
        '4' => Some("iv"),
        '5' => Some("v"),
        _ => None,
    };
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == 'm' && chars.get(i + 1) == Some(&'k') {
            out.push_str("mk");
            let mut j = i + 2;
            if chars.get(j) == Some(&'.') {
                j += 1;
            }
            while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                j += 1;
            }
            if let Some(r) = chars.get(j).and_then(|c| roman(*c))
                && !chars.get(j + 1).is_some_and(|c| c.is_alphanumeric())
            {
                out.push_str(r);
                i = j + 1;
            } else {
                i += 2;
            }
            continue;
        }
        if chars[i].is_ascii_alphanumeric() {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

/// Lower-case letters and digits only: `"Frame Shift Drive (SCO)"` and `"frame shift drive sco"` match.
fn plain_key(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// The name in `names` that `input` stands for (spelling, case and spacing aside), or the names it may mean: those
/// containing it, else those sharing a word of three letters or more with it.
fn pick(input: &str, names: &[&str], key: fn(&str) -> String) -> Result<String, Vec<String>> {
    let want = key(input);
    if let Some(n) = names.iter().find(|n| key(n) == want) {
        return Ok((*n).to_string());
    }
    let containing: Vec<&str> = names.iter().copied().filter(|n| !want.is_empty() && key(n).contains(&want)).collect();
    if let [only] = containing[..] {
        return Ok(only.to_string());
    }
    let mut maybe = containing;
    if maybe.is_empty() {
        let words: Vec<String> = input.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 3).map(str::to_lowercase).collect();
        maybe = names.iter().copied().filter(|n| words.iter().any(|w| key(n).contains(&key(w)))).collect();
    }
    Err(maybe.into_iter().take(SUGGESTIONS).map(str::to_string).collect())
}

/// The name Spansh and the game use for a ship, with the spelling the game files give it (`Krait Mk II`), or what the
/// input may have meant.
pub(super) fn resolve_ship(input: &str) -> Result<String, Vec<String>> {
    let names: Vec<&str> = nexus_data::ships::all().iter().map(|s| s.name.as_str()).collect();
    pick(input, &names, ship_key)
}

/// How long the next Spansh page may take, or `None` when the search has used up its time.
fn page_budget(elapsed: Duration) -> Option<Duration> {
    let left = BODY_SEARCH_LIMIT.checked_sub(elapsed).filter(|d| !d.is_zero())?;
    Some(left.min(SPANSH_TIMEOUT))
}

/// Spansh's own spelling of a ship the game files name `resolved`, and whether it could be checked against Spansh's
/// list of ship names (`false`: the list is missing, or doesn't hold that ship).
fn spansh_ship_name(resolved: String, catalogue: &[String]) -> (String, bool) {
    let names: Vec<&str> = catalogue.iter().map(String::as_str).collect();
    match pick(&resolved, &names, ship_key) {
        Ok(name) => (name, true),
        Err(_) => (resolved, false),
    }
}

/// The module name to search for and whether it was checked against Spansh's list of module names. A name that list
/// doesn't hold is an error with suggestions.
fn spansh_module_name(asked: &str, catalogue: &[String]) -> Result<(String, bool), String> {
    if catalogue.is_empty() {
        return Ok((asked.to_string(), false));
    }
    let names: Vec<&str> = catalogue.iter().map(String::as_str).collect();
    pick(asked, &names, plain_key).map(|n| (n, true)).map_err(|maybe| unknown("module", asked, "English names, e.g. Fuel Scoop, Frame Shift Drive", &maybe))
}

/// Said when the name sent to Spansh could not be checked against its spelling: an empty answer then proves nothing.
fn note_unchecked(out: &mut Value, what: &str) {
    add_note(out, &format!("Spansh's list of {what} names could not be used to check this name, and Spansh matches names exactly: if no station is listed, that may be a spelling mismatch, not a real absence."));
}

fn names_of(v: &nexus_spansh::records::FieldValues) -> Vec<String> {
    let list = v.values.get("name").unwrap_or(&v.values);
    list.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

fn add_note(out: &mut Value, text: &str) {
    let note = out["note"].as_str().map(|n| format!("{n} {text}")).unwrap_or_else(|| text.to_string());
    out["note"] = json!(note);
}

/// Drops the farthest stations until `out` fits the budget, and says so.
fn fit_stations(out: &mut Value) {
    let total = out["stations"].as_array().map_or(0, Vec::len);
    while out.to_string().len() > BUDGET {
        match out["stations"].as_array_mut() {
            Some(list) if list.len() > 1 => {
                list.pop();
            }
            _ => break,
        }
    }
    let kept = out["stations"].as_array().map_or(0, Vec::len);
    if kept < total {
        add_note(out, &format!("Only the nearest {kept} of {total} stations fit in one reply: ask with a smaller limit."));
    }
}

fn round_ls(ls: Option<f64>) -> Option<i64> {
    ls.map(|v| v.round() as i64)
}

/// The largest landing pad the station has.
fn pad(s: &StationRecord) -> Option<&'static str> {
    if s.has_large_pad == Some(true) || s.large_pads.is_some_and(|n| n > 0) {
        Some("L")
    } else if s.medium_pads.is_some_and(|n| n > 0) {
        Some("M")
    } else if s.small_pads.is_some_and(|n| n > 0) {
        Some("S")
    } else {
        None
    }
}

fn station_row(s: &StationRecord) -> Value {
    json!({
        "station": s.name, "system": s.system_name, "type": s.station_type,
        "distance_ly": s.distance.map(r1), "distance_ls": round_ls(s.distance_to_arrival), "largest_pad": pad(s),
    })
}

/// Fleet carriers fill these results near any busy system, and they move.
fn note_carriers(out: &mut Value, stations: &[StationRecord]) {
    if stations.iter().any(|s| s.station_type.as_deref().is_some_and(|t| t.ends_with("Carrier"))) {
        add_note(out, "Fleet carriers (type ...-Class Carrier) can jump away: a carrier may no longer be where Spansh last saw it.");
    }
}

fn strip_nulls_in(rows: &mut [Value]) {
    for r in rows {
        super::strip_nulls(r);
    }
}

/// The reply of `find_ship`: the stations Spansh listed, nearest first, with the ship's price and the date of the
/// shipyard report.
fn shape_ships(near: &str, ship: &str, asked: &str, checked: bool, stations: &[StationRecord]) -> Value {
    let mut rows: Vec<Value> = stations
        .iter()
        .map(|s| {
            let mut row = station_row(s);
            row["price_cr"] = json!(s.ships.iter().find(|x| x.name.eq_ignore_ascii_case(ship)).and_then(|x| x.price));
            row["shipyard_reported"] = json!(s.shipyard_updated_at);
            row
        })
        .collect();
    strip_nulls_in(&mut rows);
    let mut out = json!({ "near": near, "ship": ship, "stations": rows, "note": SHIP_NOTE });
    if !ship.eq_ignore_ascii_case(asked) {
        out["asked_as"] = json!(asked);
    }
    if stations.is_empty() {
        add_note(&mut out, "No station near enough lists it in Spansh.");
    }
    if !checked {
        note_unchecked(&mut out, "ship");
    }
    note_carriers(&mut out, stations);
    fit_stations(&mut out);
    out
}

/// The modules of a station that are `module` of the class and rating asked for, best first.
fn module_matches<'a>(s: &'a StationRecord, module: &str, class: Option<u32>, rating: Option<&str>) -> Vec<&'a nexus_spansh::records::OutfittingEntry> {
    let mut found: Vec<_> = s
        .modules
        .iter()
        .filter(|m| m.name.eq_ignore_ascii_case(module))
        .filter(|m| class.is_none_or(|c| m.class == Some(c)))
        .filter(|m| rating.is_none_or(|r| m.rating.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
        .collect();
    found.sort_by(|a, b| b.class.cmp(&a.class).then_with(|| a.rating.cmp(&b.rating)).then_with(|| a.ship.cmp(&b.ship)));
    found
}

/// The reply of `find_module`. A station lists dozens of matches when no class or rating is asked for, so the list per
/// station is cut (6, then 3, then 1) before stations are dropped, and the reply says what was cut.
fn shape_modules(near: &str, module: &str, class: Option<u32>, rating: Option<&str>, checked: bool, stations: &[StationRecord]) -> Value {
    let found: Vec<(&StationRecord, Vec<_>)> = stations.iter().map(|s| (s, module_matches(s, module, class, rating))).filter(|(_, m)| !m.is_empty()).collect();
    let skipped = stations.len() - found.len();
    let mut out = Value::Null;
    for (n, per) in MATCH_LEVELS.iter().copied().enumerate() {
        let mut cut = false;
        let mut rows: Vec<Value> = found
            .iter()
            .map(|(s, matches)| {
                let mut row = station_row(s);
                row["matches"] = matches
                    .iter()
                    .take(per)
                    .map(|m| {
                        let mut v = json!({ "class": m.class, "rating": m.rating, "price_cr": m.price, "for_ship": m.ship });
                        super::strip_nulls(&mut v);
                        v
                    })
                    .collect();
                if matches.len() > per {
                    row["more_matches"] = json!(matches.len() - per);
                    cut = true;
                }
                row["outfitting_reported"] = json!(s.outfitting_updated_at);
                row
            })
            .collect();
        strip_nulls_in(&mut rows);
        out = json!({ "near": near, "module": module, "stations": rows, "note": MODULE_NOTE });
        if let Some(c) = class {
            out["class"] = json!(c);
        }
        if let Some(r) = rating {
            out["rating"] = json!(r);
        }
        if cut {
            add_note(&mut out, &format!("Only the best {per} matches of each station are listed (more_matches counts the rest): ask for a class or a rating to narrow them."));
        }
        if out.to_string().len() <= BUDGET || n + 1 == MATCH_LEVELS.len() {
            break;
        }
    }
    if skipped > 0 {
        add_note(&mut out, &format!("{skipped} station(s) Spansh returned were left out: they list the module, but not with that class and rating together."));
    }
    if found.is_empty() {
        add_note(&mut out, "No station near enough lists it in Spansh.");
    }
    if !checked {
        note_unchecked(&mut out, "module");
    }
    note_carriers(&mut out, stations);
    fit_stations(&mut out);
    out
}

/// The bodies of one Spansh page that hold `material` (any share), light enough and near enough their star, with
/// their share.
fn bodies_with(records: Vec<BodyRecord>, material: &str, max_gravity_g: f64, max_ls: Option<f64>) -> Vec<(f64, BodyRecord)> {
    records
        .into_iter()
        .filter(|b| b.gravity.is_none_or(|g| g <= max_gravity_g))
        .filter(|b| max_ls.is_none_or(|m| b.distance_to_arrival.is_none_or(|d| d <= m)))
        .filter_map(|b| {
            let share = b.materials.iter().find(|m| m.name.eq_ignore_ascii_case(material)).map(|m| m.share)?;
            (share > 0.0).then_some((share, b))
        })
        .collect()
}

/// What `find_material_body` read, for `shape_bodies`.
struct BodySearch<'a> {
    near: &'a str,
    material: &'a Material,
    max_gravity_g: f64,
    max_ls: Option<f64>,
    /// Bodies read from Spansh.
    searched: usize,
    /// The search stopped before the cap because it had enough hits.
    enough: bool,
    /// Why it stopped early, when Spansh stopped answering.
    interrupted: Option<String>,
}

/// The reply of `find_material_body`: the richest bodies first, and how far the search looked.
fn shape_bodies(s: &BodySearch, mut hits: Vec<(f64, BodyRecord)>) -> Value {
    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal).then_with(|| a.1.distance.partial_cmp(&b.1.distance).unwrap_or(Ordering::Equal)));
    let found = hits.len();
    let mut rows: Vec<Value> = hits
        .iter()
        .take(BODY_HITS)
        .map(|(share, b)| {
            let geo = b.signal("Geological");
            json!({
                "body": b.name, "system": b.system_name, "share_pct": (share * 100.0).round() / 100.0,
                "distance_ly": b.distance.map(r1), "distance_ls": round_ls(b.distance_to_arrival),
                "gravity_g": b.gravity.map(|g| (g * 100.0).round() / 100.0), "geological_signals": (geo > 0).then_some(geo),
            })
        })
        .collect();
    strip_nulls_in(&mut rows);
    let cap = (BODY_PAGE * MAX_BODY_PAGES) as usize;
    let mut out = json!({
        "near": s.near, "material": s.material.name, "rarity_grade": s.material.grade, "bodies": rows,
        "searched_bodies": s.searched, "search_cap": cap, "max_gravity_g": s.max_gravity_g,
        "note": format!("Looked at the {} nearest landable bodies with gravity up to {} g, nearest first, never more than {cap} (a cap that keeps the answer quick): a richer body may lie farther out. share_pct is the material's share of the surface deposits as Spansh holds it, from scans players sent to EDDN; a body nobody scanned is missing.", s.searched, s.max_gravity_g),
    });
    if let Some(m) = s.max_ls {
        out["max_distance_ls"] = json!(m);
    }
    if found > BODY_HITS {
        out["found"] = json!(found);
        add_note(&mut out, &format!("{found} bodies hold it in the part searched: the richest {BODY_HITS} are listed."));
    }
    if s.enough {
        add_note(&mut out, "The search stopped as soon as it had enough bodies, so the nearer ones are listed, not the richest in the whole cap.");
    }
    match s.interrupted.as_deref() {
        Some(TIME_LIMIT) => add_note(&mut out, &format!("The search stopped early: it reached its {} second time limit.", BODY_SEARCH_LIMIT.as_secs())),
        Some(e) => add_note(&mut out, &format!("The search stopped early: {e}.")),
        None => {}
    }
    if found == 0 {
        add_note(&mut out, "None of them holds it: try a larger max_gravity_g or another near_system.");
    }
    out
}

impl NexusTools {
    /// One Spansh call, under the usual timeout.
    fn spansh_call<T>(&self, call: impl Future<Output = nexus_spansh::Result<T>>) -> Result<T, String> {
        self.spansh_call_within(SPANSH_TIMEOUT, call)
    }

    fn spansh_call_within<T>(&self, timeout: Duration, call: impl Future<Output = nexus_spansh::Result<T>>) -> Result<T, String> {
        match self.rt.block_on(async { tokio::time::timeout(timeout, call).await }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(format!("Spansh: {e}")),
            Err(_) => Err("Spansh: no answer in time".to_string()),
        }
    }

    /// Every name Spansh knows in a station field (`ships` or `modules`), kept for a few hours. Empty when Spansh's list
    /// couldn't be loaded (that is remembered for a minute); the reply then says the name was not checked.
    fn catalogue(&self, field: &'static str) -> Vec<String> {
        {
            let mut cache = CATALOGUES.lock();
            cache.retain(|(_, t, names)| t.elapsed() < if names.is_empty() { CATALOGUE_FAILED_TTL } else { CATALOGUE_TTL });
            if let Some((_, _, names)) = cache.iter().find(|(f, _, _)| *f == field) {
                return names.clone();
            }
        }
        let names = self.spansh_call(self.spansh.field_values(SearchKind::Stations, field, None)).map(|v| names_of(&v)).unwrap_or_default();
        CATALOGUES.lock().push((field, Instant::now(), names.clone()));
        names
    }

    fn nearest_stations(&self, origin: [f64; 3], field: &str, item: Value, limit: usize) -> Result<Vec<StationRecord>, String> {
        let req = SearchRequest::new()
            .near(Coords { x: origin[0], y: origin[1], z: origin[2] })
            .filter(field, json!([item]))
            .sort_by("distance", SortDir::Asc)
            .size(limit as u32);
        Ok(self.spansh_call(self.spansh.search_stations(&req))?.results)
    }

    pub(super) fn find_ship(&self, a: &Value) -> Result<Value, String> {
        let asked = arg_str(a, "ship").ok_or("ship is required")?;
        let ship = resolve_ship(&asked).map_err(|maybe| unknown("ship", &asked, "e.g. Asp Explorer, Krait Mk II, Mandalay", &maybe))?;
        let limit = arg_f64(a, "limit").map(|v| v as usize).unwrap_or(10).clamp(1, 25);
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        // Spansh matches the name exactly, and spells it its own way ("Krait MkII", but "Kestrel Mk II").
        let (spansh_name, checked) = spansh_ship_name(ship, &self.catalogue("ships"));
        let stations = self.nearest_stations(origin, "ships", json!({ "name": [spansh_name] }), limit)?;
        Ok(shape_ships(&near, &spansh_name, &asked, checked, &stations))
    }

    pub(super) fn find_module(&self, a: &Value) -> Result<Value, String> {
        let asked = arg_str(a, "module").ok_or("module is required")?;
        let class = match arg_f64(a, "class") {
            None => None,
            Some(c) if c.fract() == 0.0 && (1.0..=8.0).contains(&c) => Some(c as u32),
            Some(_) => return Err("class is a whole number from 1 to 8.".to_string()),
        };
        let rating = match arg_str(a, "rating") {
            None => None,
            Some(r) if r.len() == 1 && matches!(r.to_ascii_uppercase().as_bytes()[0], b'A'..=b'I') => Some(r.to_ascii_uppercase()),
            Some(_) => return Err("rating is one letter, A to I.".to_string()),
        };
        let limit = arg_f64(a, "limit").map(|v| v as usize).unwrap_or(10).clamp(1, 25);
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let (module, checked) = spansh_module_name(&asked, &self.catalogue("modules"))?;
        let mut item = json!({ "name": [module] });
        if let Some(c) = class {
            item["class"] = json!([c.to_string()]);
        }
        if let Some(r) = &rating {
            item["rating"] = json!([r]);
        }
        let stations = self.nearest_stations(origin, "modules", item, limit)?;
        Ok(shape_modules(&near, &module, class, rating.as_deref(), checked, &stations))
    }

    pub(super) fn find_material_body(&self, a: &Value) -> Result<Value, String> {
        let asked = arg_str(a, "material").ok_or("material is required")?;
        let material = materials::material(&asked).ok_or_else(|| format!("Unknown material \"{asked}\". Give a raw material, e.g. Polonium, Tellurium, Niobium or Arsenic."))?;
        if material.kind != MaterialKind::Raw {
            return Err(format!(
                "{} is a {} material, not a raw one: this tool finds raw materials, the ones lying on planetary surfaces (e.g. Polonium).",
                material.name,
                material.kind.label().to_lowercase()
            ));
        }
        let max_gravity_g = arg_f64(a, "max_gravity_g").unwrap_or(1.0);
        if max_gravity_g <= 0.0 {
            return Err("max_gravity_g must be above 0.".to_string());
        }
        let max_ls = arg_f64(a, "max_distance_ls").filter(|d| *d > 0.0);
        let me = self.me()?;
        let (origin, near) = self.origin(a, &me)?;
        let mut hits: Vec<(f64, BodyRecord)> = Vec::new();
        let (mut searched, mut enough, mut interrupted) = (0usize, false, None);
        let started = Instant::now();
        for page in 0..MAX_BODY_PAGES {
            // Every page gets what is left of the search's time; none starts once it is used up.
            let Some(budget) = page_budget(started.elapsed()) else {
                interrupted = Some(TIME_LIMIT.to_string());
                break;
            };
            let req = SearchRequest::new()
                .near(Coords { x: origin[0], y: origin[1], z: origin[2] })
                .filter("is_landable", filter::eq(true))
                .filter("gravity", filter::between(0.0, max_gravity_g))
                .sort_by("distance", SortDir::Asc)
                .size(BODY_PAGE)
                .page(page);
            let rows = match self.spansh_call_within(budget, self.spansh.search_bodies(&req)) {
                Ok(res) => res.results,
                Err(e) if page == 0 => return Err(e),
                Err(e) => {
                    interrupted = Some(e);
                    break;
                }
            };
            let read = rows.len();
            searched += read;
            hits.extend(bodies_with(rows, &material.name, max_gravity_g, max_ls));
            if hits.len() >= BODY_HITS {
                enough = page + 1 < MAX_BODY_PAGES && read == BODY_PAGE as usize;
                break;
            }
            if read < BODY_PAGE as usize {
                break;
            }
        }
        Ok(shape_bodies(&BodySearch { near: &near, material, max_gravity_g, max_ls, searched, enough, interrupted }, hits))
    }
}

/// The error for a name that isn't recognised.
fn unknown(what: &str, asked: &str, examples: &str, maybe: &[String]) -> String {
    let mut e = format!("Unknown {what} \"{asked}\" ({examples}).");
    if !maybe.is_empty() {
        e.push_str(&format!(" Did you mean: {}?", maybe.join(", ")));
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_spansh::records::{OutfittingEntry, ShipyardEntry};

    fn station(i: usize) -> StationRecord {
        StationRecord {
            name: format!("Hutton Orbital Station Number {i}"),
            system_name: format!("A Rather Long System Name {i}"),
            station_type: Some("Coriolis Starport".into()),
            distance: Some(i as f64 * 1.37),
            distance_to_arrival: Some(1234.5 + i as f64),
            has_large_pad: Some(true),
            ..StationRecord::default()
        }
    }

    fn module(class: u32, rating: &str) -> OutfittingEntry {
        OutfittingEntry { name: "Fuel Scoop".into(), category: Some("internal".into()), class: Some(class), rating: Some(rating.into()), price: Some(1_000_000 + u64::from(class) * 1000), ..OutfittingEntry::default() }
    }

    fn body(name: &str, gravity: f64, ls: f64, shares: &[(&str, f64)]) -> BodyRecord {
        BodyRecord {
            name: name.into(),
            system_name: "Deciat".into(),
            gravity: Some(gravity),
            distance: Some(3.0),
            distance_to_arrival: Some(ls),
            materials: shares.iter().map(|(n, s)| nexus_spansh::records::NameShare { name: (*n).into(), share: *s }).collect(),
            ..BodyRecord::default()
        }
    }

    #[test]
    fn ship_names_ignore_spelling() {
        for s in ["krait mk2", "Krait MkII", "krait mk ii", "Krait Mk II", "KRAIT MARK 2"] {
            assert_eq!(resolve_ship(s), Ok("Krait Mk II".to_string()), "{s}");
        }
        assert_eq!(resolve_ship("cobra mk5"), Ok("Cobra Mk V".to_string()));
        assert_eq!(resolve_ship("python mk 2"), Ok("Python Mk II".to_string()));
        assert_eq!(resolve_ship("Python"), Ok("Python".to_string()), "an exact name wins over longer ones");
        assert_eq!(resolve_ship("type-9"), Ok("Type-9 Heavy".to_string()), "a part that fits one ship only");
        let ambiguous = resolve_ship("asp x").unwrap_err();
        assert!(ambiguous.iter().any(|n| n == "Asp Explorer") && ambiguous.iter().any(|n| n == "Asp Scout"), "{ambiguous:?}");
        assert_eq!(resolve_ship("zzz"), Err(vec![]));
    }

    #[test]
    fn spansh_spells_ships_its_own_way() {
        // The names `field_values/ships` gives: no space in "MkII", but one in "Kestrel Mk II".
        let names = ["Krait MkII", "Kestrel Mk II", "Cobra MkV", "Asp Explorer"];
        assert_eq!(pick("Krait Mk II", &names, ship_key), Ok("Krait MkII".to_string()));
        assert_eq!(pick("kestrel mk2", &names, ship_key), Ok("Kestrel Mk II".to_string()));
        assert_eq!(pick("Cobra Mk V", &names, ship_key), Ok("Cobra MkV".to_string()));
        let modules = ["Fuel Scoop", "Frame Shift Drive", "Frame Shift Drive (SCO)", "Frame Shift Drive Interdictor"];
        assert_eq!(pick("fuel scoop", &modules, plain_key), Ok("Fuel Scoop".to_string()));
        assert_eq!(pick("frame shift drive sco", &modules, plain_key), Ok("Frame Shift Drive (SCO)".to_string()));
        assert_eq!(pick("scoop", &modules, plain_key), Ok("Fuel Scoop".to_string()));
        assert_eq!(pick("frame shift", &modules, plain_key).unwrap_err().len(), 3);
    }

    #[test]
    fn ships_list_price_and_report_date_and_say_catalogue() {
        let mut s = station(1);
        s.ships = vec![ShipyardEntry { name: "Krait MkII".into(), price: Some(42_000_000), symbol: None }];
        s.shipyard_updated_at = Some("2026-10-30 12:00:00+00".into());
        let v = shape_ships("Deciat", "Krait MkII", "krait mk2", true, &[s]);
        assert_eq!(v["stations"][0]["price_cr"], 42_000_000);
        assert_eq!(v["stations"][0]["largest_pad"], "L");
        assert_eq!(v["stations"][0]["shipyard_reported"], "2026-10-30 12:00:00+00");
        assert_eq!(v["asked_as"], "krait mk2");
        assert!(v["note"].as_str().unwrap().contains("catalogue"));
        assert!(shape_ships("Deciat", "Krait MkII", "Krait MkII", true, &[])["note"].as_str().unwrap().contains("No station"));
        assert!(!v["note"].as_str().unwrap().contains("carrier"), "no carrier among them");
        let mut carrier = station(2);
        carrier.station_type = Some("Drake-Class Carrier".into());
        let v = shape_ships("Deciat", "Krait MkII", "Krait MkII", true, &[carrier]);
        assert!(v["note"].as_str().unwrap().contains("jump away"), "{v}");
    }

    #[test]
    fn modules_keep_the_class_and_rating_asked_and_trim_to_fit() {
        let mut s = station(1);
        s.modules = vec![module(5, "A"), module(5, "B"), module(6, "A")];
        let v = shape_modules("Deciat", "Fuel Scoop", Some(5), Some("A"), true, &[s.clone()]);
        let m = v["stations"][0]["matches"].as_array().unwrap();
        assert_eq!(m.len(), 1, "{v}");
        assert_eq!((m[0]["class"].as_u64(), m[0]["rating"].as_str()), (Some(5), Some("A")));
        // A station that lists the module, but not as 6A, is not an answer for 6A.
        let mut other = station(2);
        other.modules = vec![module(5, "A")];
        let v = shape_modules("Deciat", "Fuel Scoop", Some(6), Some("A"), true, &[s.clone(), other]);
        assert_eq!(v["stations"].as_array().unwrap().len(), 1);
        assert!(v["note"].as_str().unwrap().contains("left out"), "{v}");
        // Without a class, every size and rating matches: the lists are cut, and the reply says so.
        let mut all = station(3);
        all.modules = (1..=8).flat_map(|c| ["A", "B", "C", "D", "E"].map(|r| module(c, r))).collect();
        let v = shape_modules("Deciat", "Fuel Scoop", None, None, true, &[all]);
        assert_eq!(v["stations"][0]["matches"].as_array().unwrap().len(), 6);
        assert_eq!(v["stations"][0]["more_matches"], 34);
        assert_eq!(v["stations"][0]["matches"][0]["class"], 8, "the biggest first");
        assert!(v["note"].as_str().unwrap().contains("more_matches"));
    }

    /// The Ask loop cuts a result at `MAX_RESULT_CHARS` in the middle of the JSON: 25 stations with every
    /// size and rating of a module each must still come out whole.
    #[test]
    fn replies_fit_the_assistants_result_size() {
        let limit = nexus_ai::agent::MAX_RESULT_CHARS;
        let stations: Vec<StationRecord> = (0..25)
            .map(|i| {
                let mut s = station(i);
                s.outfitting_updated_at = Some("2026-10-30 12:00:00+00".into());
                s.modules = (1..=8).flat_map(|c| ["A", "B", "C", "D", "E"].map(|r| module(c, r))).collect();
                s.ships = vec![ShipyardEntry { name: "Krait MkII".into(), price: Some(42_000_000), symbol: None }];
                s.shipyard_updated_at = Some("2026-10-30 12:00:00+00".into());
                s
            })
            .collect();
        let v = shape_modules("Deciat", "Fuel Scoop", None, None, true, &stations);
        let n = v["stations"].as_array().unwrap().len();
        assert!(v.to_string().len() < limit, "{} chars", v.to_string().len());
        assert!(n >= 15, "the nearest are kept, not just a few: {n}");
        assert!(n == 25 || v["note"].as_str().unwrap().contains(&format!("nearest {n} of 25")), "{}", v["note"]);
        let v = shape_ships("Deciat", "Krait MkII", "Krait MkII", true, &stations);
        assert_eq!(v["stations"].as_array().unwrap().len(), 25);
        assert!(v.to_string().len() < limit, "{} chars", v.to_string().len());
        // Long station names can't push a reply over either: the farthest stations go first.
        let mut wide = stations.clone();
        for s in &mut wide {
            s.name = "N".repeat(300);
        }
        let v = shape_ships("Deciat", "Krait MkII", "Krait MkII", true, &wide);
        assert!(v.to_string().len() < limit, "{} chars", v.to_string().len());
        assert!(v["note"].as_str().unwrap().contains("of 25 stations"), "{}", v["note"]);
        assert_eq!(v["stations"][0]["distance_ly"], 0.0, "the nearest is the one kept");
    }

    #[test]
    fn bodies_are_ranked_by_share_within_gravity_and_distance() {
        let polonium = materials::material("Polonium").unwrap();
        let records = vec![
            body("Deciat 1", 0.2, 100.0, &[("Polonium", 1.5), ("Iron", 20.0)]),
            body("Deciat 2", 0.3, 100.0, &[("Polonium", 2.75)]),
            body("Deciat 3", 0.3, 100.0, &[("Iron", 20.0)]),
            body("Deciat 4", 1.7, 100.0, &[("Polonium", 9.0)]),
            body("Deciat 5", 0.3, 90_000.0, &[("polonium", 8.0)]),
        ];
        let hits = bodies_with(records, "Polonium", 1.0, Some(50_000.0));
        assert_eq!(hits.len(), 2, "no polonium, too heavy and too far are left out");
        let s = BodySearch { near: "Deciat", material: polonium, max_gravity_g: 1.0, max_ls: Some(50_000.0), searched: 5, enough: false, interrupted: None };
        let v = shape_bodies(&s, hits);
        assert_eq!(v["bodies"][0]["body"], "Deciat 2");
        assert_eq!(v["bodies"][0]["share_pct"], 2.75);
        assert_eq!(v["bodies"][1]["body"], "Deciat 1");
        assert_eq!((v["searched_bodies"].as_u64(), v["search_cap"].as_u64()), (Some(5), Some(500)));
        assert_eq!(v["max_distance_ls"], 50_000.0);
        let none = shape_bodies(&s, Vec::new());
        assert!(none["note"].as_str().unwrap().contains("None of them"), "{none}");
    }

    /// When Spansh's list of names could not be loaded (or doesn't hold the name), "none found" proves nothing.
    #[test]
    fn unchecked_names_say_an_empty_answer_proves_nothing() {
        let list = |names: &[&str]| names.iter().map(|n| (*n).to_string()).collect::<Vec<_>>();
        // The list is there: Spansh's spelling is used and checked.
        assert_eq!(spansh_ship_name("Krait Mk II".into(), &list(&["Krait MkII", "Python"])), ("Krait MkII".to_string(), true));
        // No list (the fetch failed): the game's spelling goes out unchecked, and the reply says so.
        let (name, checked) = spansh_ship_name("Krait Mk II".into(), &[]);
        assert_eq!((name.as_str(), checked), ("Krait Mk II", false));
        // A list that lacks the ship is as good as none.
        assert!(!spansh_ship_name("Krait Mk II".into(), &list(&["Python"])).1);
        let v = shape_ships("Deciat", &name, "krait mk2", checked, &[]);
        let note = v["note"].as_str().unwrap();
        assert!(note.contains("could not be used to check") && note.contains("spelling mismatch"), "{note}");
        let v = shape_ships("Deciat", "Krait MkII", "krait mk2", true, &[]);
        assert!(!v["note"].as_str().unwrap().contains("spelling mismatch"), "checked names add nothing");
        // Modules: no list means unchecked; a list that lacks the name is an error with suggestions.
        assert_eq!(spansh_module_name("fuel scoop", &[]), Ok(("fuel scoop".to_string(), false)));
        assert_eq!(spansh_module_name("fuel scoop", &list(&["Fuel Scoop"])), Ok(("Fuel Scoop".to_string(), true)));
        assert!(spansh_module_name("Warp Core", &list(&["Fuel Scoop"])).unwrap_err().contains("Unknown module"));
        let v = shape_modules("Deciat", "fuel scoop", None, None, false, &[]);
        assert!(v["note"].as_str().unwrap().contains("spelling mismatch"), "{v}");
        assert!(!shape_modules("Deciat", "Fuel Scoop", None, None, true, &[])["note"].as_str().unwrap().contains("spelling mismatch"));
    }

    #[test]
    fn the_body_search_has_a_time_limit() {
        let s = Duration::from_secs;
        assert_eq!(page_budget(s(0)), Some(SPANSH_TIMEOUT), "a page never gets more than the usual timeout");
        assert_eq!(page_budget(s(40)), Some(s(20)), "later pages get what is left");
        assert_eq!(page_budget(s(59)), Some(s(1)));
        assert_eq!(page_budget(BODY_SEARCH_LIMIT), None);
        assert_eq!(page_budget(s(90)), None);
        // So five pages can never take longer than the limit, whatever Spansh does.
        let polonium = materials::material("Polonium").unwrap();
        let search = BodySearch { near: "Deciat", material: polonium, max_gravity_g: 1.0, max_ls: None, searched: 200, enough: false, interrupted: Some(TIME_LIMIT.into()) };
        let note = shape_bodies(&search, Vec::new())["note"].as_str().unwrap().to_string();
        assert!(note.contains("60 second time limit"), "{note}");
    }

    #[test]
    fn many_bodies_are_cut_to_the_richest_ten() {
        let polonium = materials::material("Polonium").unwrap();
        let hits: Vec<(f64, BodyRecord)> = (0..40).map(|i| (f64::from(i), body(&format!("Deciat {i}"), 0.2, 100.0, &[("Polonium", f64::from(i))]))).collect();
        let s = BodySearch { near: "Deciat", material: polonium, max_gravity_g: 1.0, max_ls: None, searched: 300, enough: true, interrupted: Some("Spansh: no answer in time".into()) };
        let v = shape_bodies(&s, hits);
        assert_eq!(v["bodies"].as_array().unwrap().len(), 10);
        assert_eq!(v["bodies"][0]["share_pct"], 39.0);
        assert_eq!(v["found"], 40);
        let note = v["note"].as_str().unwrap();
        assert!(note.contains("stopped as soon") && note.contains("stopped early"), "{note}");
        assert!(v.to_string().len() < nexus_ai::agent::MAX_RESULT_CHARS);
    }
}
