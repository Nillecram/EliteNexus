//! The `bodies` table: planets and moons with their footfall / discovery flags, signals and physical
//! facts. Admission ([`admits`]) and merging ([`merge`]) are pure functions over [`BodyInfo`]; the
//! writer loads the row, merges one [`BodyUpdate`] into it and saves it when something changed, so
//! replaying the same observation (the journal history at every start) writes nothing.

use crate::error::Result;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, Row, params};

const BODY_COLS: &str = "system_id64, body_id, name, planet_class, atmosphere, atmosphere_type, gravity_g, \
     temperature_k, pressure_atm, volcanism, terraform_state, distance_ls, landable, scan_updated, was_discovered, \
     discovered_seen, was_mapped, mapped_seen, was_footfalled, footfall_seen, footfall_source, bio_signals, \
     geo_signals, genera, signals_updated, updated, source, checked_online";

/// Whether an observation may create a row; rows that exist always merge. The commander's own journal
/// keeps every landable body and every body with signals; shared sources (EDDN: every commander's
/// scans, mostly bare rock) only biology bodies, unless `all_eddn_bodies`. Earth-like / water /
/// ammonia worlds, terraformables, on-foot evidence and online lookups (made on demand, for biology
/// bodies) are always kept.
pub(crate) fn admits(u: &BodyUpdate, all_eddn_bodies: bool) -> bool {
    let strict = u.source != Source::Journal && !all_eddn_bodies;
    let bio = u.signals.as_ref().is_some_and(|s| s.bio > 0);
    let geo = u.signals.as_ref().is_some_and(|s| s.geo > 0);
    // Disembark / ScanOrganic: somebody was on foot there (a footfall without a scan).
    let on_foot = u.scan.is_none() && u.was_footfalled == Some(true);
    let scan = u
        .scan
        .as_ref()
        .is_some_and(|s| s.is_notable() || (s.landable && !strict));
    bio || (geo && !strict) || on_foot || scan || u.checked_online.is_some()
}

/// Folds one observation into a stored body (rules on [`BodyUpdate`]).
pub(crate) fn merge(b: &mut BodyInfo, u: &BodyUpdate) {
    let ts = u.timestamp;
    if let Some(name) = u.name.as_deref().map(str::trim).filter(|n| !n.is_empty())
        && (b.name.is_none() || ts >= b.updated_unix)
    {
        b.name = Some(name.to_string());
    }
    // A scan is complete: the newest one replaces every physical fact (`None` atmosphere = airless);
    // an older one adds nothing.
    if let Some(s) = &u.scan
        && b.scan_unix.is_none_or(|t| ts >= t)
    {
        b.planet_class = Some(s.planet_class.clone()).filter(|c| !c.is_empty());
        b.atmosphere.clone_from(&s.atmosphere);
        b.atmosphere_type.clone_from(&s.atmosphere_type);
        b.gravity_g = s.gravity_g;
        b.temperature_k = s.temperature_k;
        b.pressure_atm = s.pressure_atm;
        b.volcanism.clone_from(&s.volcanism);
        b.terraform_state.clone_from(&s.terraform_state);
        b.distance_ls = s.distance_ls;
        b.landable = Some(s.landable);
        b.scan_unix = Some(ts);
    }
    // On equal times the commander's own journal is credited (an EDDN echo of their own scan).
    let ours = u.source == Source::Journal;
    merge_flag(&mut b.was_discovered, u.was_discovered, ts, ours);
    merge_flag(&mut b.was_mapped, u.was_mapped, ts, ours);
    if let Some(value) = u.was_footfalled {
        merge_footfall(b, value, ts, u.source);
    }
    if let Some(t) = u.checked_online {
        b.checked_online_unix = Some(b.checked_online_unix.map_or(t, |c| c.max(t)));
    }
    if let Some(s) = &u.signals {
        let newer = b.signals_unix.is_none_or(|t| ts >= t);
        if newer {
            b.bio_signals = Some(s.bio);
            b.geo_signals = Some(s.geo);
            b.signals_unix = Some(ts);
        }
        // Genera never go stale; an empty list says nothing.
        if !s.genera.is_empty() && (newer || b.genera.is_empty()) {
            b.genera.clone_from(&s.genera);
        }
    }
    if ts >= b.updated_unix {
        b.updated_unix = ts;
        b.source = u.source;
    }
}

/// Footfall: monotonic like the other flags, except that the commander's own journal is authoritative.
/// The game never un-footfalls a body, so a journal `false` proves an older `true` from elsewhere
/// wrong (it replaces it), and such a `true` arriving after the journal's `false` is ignored.
fn merge_footfall(b: &mut BodyInfo, value: bool, ts: i64, source: Source) {
    let ours = source == Source::Journal;
    let stored_ours = b.footfall_source == Some(Source::Journal);
    match b.was_footfalled {
        Some(Seen { value: true, at_unix }) if !value => {
            if ours && !stored_ours && ts > at_unix {
                b.was_footfalled = Some(Seen { value: false, at_unix: ts });
                b.footfall_source = Some(source);
            }
        }
        Some(Seen { value: false, at_unix }) if value && !ours && stored_ours && ts < at_unix => {}
        _ => {
            if merge_flag(&mut b.was_footfalled, Some(value), ts, ours) {
                b.footfall_source = Some(source);
            }
        }
    }
}

/// Monotonic merge of a `Was*` flag: `true` sticks (dated by its first report); `false` is dated by
/// its newest report. Returns whether this observation now dates the flag.
fn merge_flag(slot: &mut Option<Seen>, new: Option<bool>, ts: i64, wins_ties: bool) -> bool {
    let Some(value) = new else { return false };
    let later = |at: i64| ts > at || (ts == at && wins_ties);
    let earlier = |at: i64| ts < at || (ts == at && wins_ties);
    match slot {
        None => {
            *slot = Some(Seen { value, at_unix: ts });
            true
        }
        Some(seen) if seen.value => {
            let take = value && earlier(seen.at_unix);
            if take {
                seen.at_unix = ts;
            }
            take
        }
        Some(seen) if value => {
            *seen = Seen { value: true, at_unix: ts };
            true
        }
        Some(seen) => {
            let take = later(seen.at_unix);
            if take {
                seen.at_unix = ts;
            }
            take
        }
    }
}

// ---------------------------------------------------------------------------------------------
// SQL
// ---------------------------------------------------------------------------------------------

fn small(v: Option<i64>) -> Option<u32> {
    v.map(|v| v.clamp(0, i64::from(u32::MAX)) as u32)
}

fn seen(value: Option<i64>, at: Option<i64>) -> Option<Seen> {
    value.map(|v| Seen {
        value: v != 0,
        at_unix: at.unwrap_or(0),
    })
}

fn body_info(r: &Row) -> rusqlite::Result<BodyInfo> {
    Ok(BodyInfo {
        system_id64: r.get::<_, i64>(0)? as u64,
        body_id: small(r.get(1)?).unwrap_or(0),
        name: r.get(2)?,
        planet_class: r.get(3)?,
        atmosphere: r.get(4)?,
        atmosphere_type: r.get(5)?,
        gravity_g: r.get(6)?,
        temperature_k: r.get(7)?,
        pressure_atm: r.get(8)?,
        volcanism: r.get(9)?,
        terraform_state: r.get(10)?,
        distance_ls: r.get(11)?,
        landable: r.get::<_, Option<i64>>(12)?.map(|v| v != 0),
        scan_unix: r.get(13)?,
        was_discovered: seen(r.get(14)?, r.get(15)?),
        was_mapped: seen(r.get(16)?, r.get(17)?),
        was_footfalled: seen(r.get(18)?, r.get(19)?),
        footfall_source: r.get::<_, Option<i64>>(20)?.map(Source::from_code),
        bio_signals: small(r.get(21)?),
        geo_signals: small(r.get(22)?),
        genera: r
            .get::<_, Option<String>>(23)?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        signals_unix: r.get(24)?,
        updated_unix: r.get(25)?,
        source: Source::from_code(r.get(26)?),
        checked_online_unix: r.get(27)?,
    })
}

pub(crate) fn load(conn: &Connection, system_id64: u64, body_id: u32) -> Result<Option<BodyInfo>> {
    let sql = format!("SELECT {BODY_COLS} FROM bodies WHERE system_id64 = ?1 AND body_id = ?2");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row(params![system_id64 as i64, i64::from(body_id)], body_info)
        .optional()?)
}

pub(crate) fn save(conn: &Connection, b: &BodyInfo) -> Result<()> {
    let sql = format!(
        "INSERT OR REPLACE INTO bodies ({BODY_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
         ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)"
    );
    let flag = |s: &Option<Seen>| (s.map(|s| i64::from(s.value)), s.map(|s| s.at_unix));
    let (discovered, discovered_seen) = flag(&b.was_discovered);
    let (mapped, mapped_seen) = flag(&b.was_mapped);
    let (footfalled, footfall_seen) = flag(&b.was_footfalled);
    let genera = (!b.genera.is_empty()).then(|| serde_json::to_string(&b.genera).unwrap_or_default());
    conn.prepare_cached(&sql)?.execute(params![
        b.system_id64 as i64,
        i64::from(b.body_id),
        b.name,
        b.planet_class,
        b.atmosphere,
        b.atmosphere_type,
        b.gravity_g,
        b.temperature_k,
        b.pressure_atm,
        b.volcanism,
        b.terraform_state,
        b.distance_ls,
        b.landable.map(i64::from),
        b.scan_unix,
        discovered,
        discovered_seen,
        mapped,
        mapped_seen,
        footfalled,
        footfall_seen,
        b.footfall_source.map(Source::code),
        b.bio_signals.map(i64::from),
        b.geo_signals.map(i64::from),
        genera,
        b.signals_unix,
        b.updated_unix,
        b.source.code(),
        b.checked_online_unix,
    ])?;
    Ok(())
}

pub(crate) fn in_system(conn: &Connection, system_id64: u64) -> Result<Vec<BodyInfo>> {
    let sql = format!("SELECT {BODY_COLS} FROM bodies WHERE system_id64 = ?1 ORDER BY body_id");
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map([system_id64 as i64], body_info)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// `name` is `COLLATE NOCASE`, so `=` is case-insensitive and uses `bodies_name`.
pub(crate) fn by_name(conn: &Connection, name: &str) -> Result<Option<BodyInfo>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let sql = format!("SELECT {BODY_COLS} FROM bodies WHERE name = ?1 ORDER BY updated DESC LIMIT 1");
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([name], body_info)
        .optional()?)
}

pub(crate) fn find(conn: &Connection, prefix: &str, limit: usize) -> Result<Vec<BodyInfo>> {
    let lo = prefix.trim_start();
    if lo.trim().is_empty() {
        return Ok(Vec::new());
    }
    let hi = format!("{lo}\u{10FFFF}");
    let sql = format!(
        "SELECT {BODY_COLS} FROM bodies WHERE name >= ?1 AND name < ?2 ORDER BY name LIMIT ?3"
    );
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![lo, hi, limit.clamp(1, 1000) as i64], body_info)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn landable(ts: i64, footfalled: Option<bool>, source: Source) -> BodyUpdate {
        BodyUpdate {
            system_id64: 908552967066,
            body_id: 6,
            name: Some("Swoilz MH-W c17-3 A 4".into()),
            scan: Some(BodyScan {
                planet_class: "High metal content body".into(),
                atmosphere: Some("thin carbon dioxide atmosphere".into()),
                atmosphere_type: Some("CarbonDioxide".into()),
                gravity_g: Some(0.089),
                temperature_k: Some(165.2),
                pressure_atm: Some(0.011),
                landable: true,
                distance_ls: Some(1200.0),
                ..Default::default()
            }),
            was_discovered: Some(true),
            was_mapped: Some(false),
            was_footfalled: footfalled,
            timestamp: ts,
            source,
            ..Default::default()
        }
    }

    fn signals(ts: i64, bio: u32, genera: &[&str], source: Source) -> BodyUpdate {
        BodyUpdate {
            system_id64: 908552967066,
            body_id: 6,
            signals: Some(BodySignals {
                bio,
                geo: 0,
                genera: genera.iter().map(|g| g.to_string()).collect(),
            }),
            timestamp: ts,
            source,
            ..Default::default()
        }
    }

    fn fold(updates: &[BodyUpdate]) -> BodyInfo {
        let mut b = BodyInfo::default();
        for u in updates {
            merge(&mut b, u);
        }
        b
    }

    #[test]
    fn admission() {
        let all = false;
        assert!(admits(&landable(1, None, Source::Journal), all), "own landables are kept");
        assert!(!admits(&landable(1, None, Source::Eddn), all), "EDDN bare rock is not");
        assert!(admits(&landable(1, None, Source::Eddn), true), "unless asked for");
        assert!(admits(&signals(1, 3, &[], Source::Eddn), all), "EDDN biology");
        let mut geo = signals(1, 0, &[], Source::Eddn);
        geo.signals.as_mut().unwrap().geo = 2;
        assert!(!admits(&geo, all));
        geo.source = Source::Journal;
        assert!(admits(&geo, all));
        assert!(!admits(&signals(1, 0, &[], Source::Journal), all), "no signals at all");
        let mut elw = landable(1, Some(false), Source::Eddn);
        elw.scan = Some(BodyScan {
            planet_class: "Earthlike body".into(),
            landable: false,
            ..Default::default()
        });
        assert!(admits(&elw, all));
        let mut tf = landable(1, None, Source::Eddn);
        tf.scan.as_mut().unwrap().landable = false;
        tf.scan.as_mut().unwrap().terraform_state = Some("Terraformable".into());
        assert!(admits(&tf, all));
        let on_foot = BodyUpdate {
            system_id64: 5,
            body_id: 1,
            was_footfalled: Some(true),
            source: Source::Eddn,
            ..Default::default()
        };
        assert!(admits(&on_foot, all), "somebody sampled organics there");
    }

    #[test]
    fn flags_are_monotonic_and_dated() {
        // Not footfalled at 100 (our scan) and at 300 (EDDN): "not footfalled as of 300".
        let b = fold(&[
            landable(100, Some(false), Source::Journal),
            landable(300, Some(false), Source::Eddn),
        ]);
        assert_eq!(b.was_footfalled, Some(Seen { value: false, at_unix: 300 }));
        assert_eq!(b.footfall_source, Some(Source::Eddn));
        // Footfalled at 400; a later "false" (another scan made before it, delivered late) can't undo it.
        let b = fold(&[
            landable(100, Some(false), Source::Journal),
            landable(400, Some(true), Source::Eddn),
            landable(500, Some(false), Source::Eddn),
            landable(200, Some(false), Source::Eddn),
        ]);
        assert_eq!(b.was_footfalled, Some(Seen { value: true, at_unix: 400 }));
        // An earlier report of `true` dates the footfall back.
        let mut b2 = b.clone();
        merge(&mut b2, &landable(350, Some(true), Source::Journal));
        assert_eq!(b2.was_footfalled, Some(Seen { value: true, at_unix: 350 }));
        assert_eq!(b2.footfall_source, Some(Source::Journal));
        // `None` says nothing.
        merge(&mut b2, &landable(900, None, Source::Eddn));
        assert_eq!(b2.was_footfalled, Some(Seen { value: true, at_unix: 350 }));
        // Discovered / mapped follow the same rule.
        let mut m = landable(600, Some(true), Source::Eddn);
        m.was_mapped = Some(true);
        merge(&mut b2, &m);
        assert_eq!(b2.was_mapped, Some(Seen { value: true, at_unix: 600 }));
        assert_eq!(b2.was_discovered, Some(Seen { value: true, at_unix: 100 }));
        // Same time from EDDN and our journal (EDDN echo of our own scan): our journal is credited.
        let b = fold(&[
            landable(700, Some(false), Source::Eddn),
            landable(700, Some(false), Source::Journal),
            landable(700, Some(false), Source::Eddn),
        ]);
        assert_eq!(b.footfall_source, Some(Source::Journal));
    }

    #[test]
    fn own_journal_is_authoritative_for_footfall() {
        let foot = |ts: i64, value: bool, source: Source| BodyUpdate {
            system_id64: 908552967066,
            body_id: 6,
            was_footfalled: Some(value),
            timestamp: ts,
            source,
            ..Default::default()
        };
        // Somebody's "footfalled" at 100, then our own scan at 200 says no: ours stands.
        let mut b = fold(&[foot(100, true, Source::Eddn), foot(200, false, Source::Journal)]);
        assert_eq!(b.was_footfalled, Some(Seen { value: false, at_unix: 200 }));
        assert_eq!(b.footfall_source, Some(Source::Journal));
        // An older "footfalled" delivered late can't undo our scan either.
        merge(&mut b, &foot(150, true, Source::EddnArchive));
        assert_eq!(b.was_footfalled, Some(Seen { value: false, at_unix: 200 }));
        // A newer one can: somebody landed after we scanned it.
        merge(&mut b, &foot(300, true, Source::Eddn));
        assert_eq!(b.was_footfalled, Some(Seen { value: true, at_unix: 300 }));
        assert_eq!(b.footfall_source, Some(Source::Eddn));
        // A shared "not footfalled" never undoes a footfall; our own earlier scan doesn't either.
        merge(&mut b, &foot(400, false, Source::Eddn));
        merge(&mut b, &foot(250, false, Source::Journal));
        assert_eq!(b.was_footfalled, Some(Seen { value: true, at_unix: 300 }));
    }

    #[test]
    fn online_lookups_are_remembered() {
        let nothing = BodyUpdate {
            system_id64: 908552967066,
            body_id: 6,
            name: Some("Swoilz MH-W c17-3 A 4".into()),
            source: Source::EddnArchive,
            checked_online: Some(1_000),
            ..Default::default()
        };
        assert!(admits(&nothing, false), "a lookup is cached even when it found nothing");
        let mut b = fold(&[landable(500, None, Source::Journal), nothing.clone()]);
        assert_eq!(b.checked_online_unix, Some(1_000));
        assert_eq!((b.updated_unix, b.source), (500, Source::Journal), "finding nothing is not an observation");
        assert_eq!(b.was_footfalled, None);
        merge(&mut b, &BodyUpdate { checked_online: Some(900), ..nothing });
        assert_eq!(b.checked_online_unix, Some(1_000));
    }

    #[test]
    fn newer_scan_wins_and_older_adds_nothing() {
        let mut newer = landable(200, None, Source::Eddn);
        newer.scan.as_mut().unwrap().atmosphere = None; // e.g. the game changed the body
        newer.scan.as_mut().unwrap().atmosphere_type = None;
        let b = fold(&[landable(100, None, Source::Journal), newer.clone()]);
        assert_eq!(b.atmosphere, None);
        assert_eq!(b.scan_unix, Some(200));
        // delivered in the other order: the older scan changes nothing
        let b2 = fold(&[newer, landable(100, None, Source::Journal)]);
        assert_eq!(b2.atmosphere, None);
        assert_eq!(b2.planet_class.as_deref(), Some("High metal content body"));
        assert_eq!(b2.updated_unix, 200);
        assert_eq!(b2.source, Source::Eddn);
    }

    #[test]
    fn signals_and_genera_merge() {
        let dss = ["$Codex_Ent_Bacterial_Genus_Name;", "$Codex_Ent_Stratum_Genus_Name;"];
        // FSS count, then DSS genera, then a newer FSS count without genera: genera stay.
        let b = fold(&[
            signals(100, 2, &[], Source::Eddn),
            signals(200, 2, &dss, Source::Journal),
            signals(300, 2, &[], Source::Eddn),
        ]);
        assert_eq!(b.bio_signals, Some(2));
        assert_eq!(b.genera.len(), 2);
        assert_eq!(b.signals_unix, Some(300));
        // An older DSS delivered late still fills in unknown genera, but not the counts.
        let b = fold(&[signals(300, 3, &[], Source::Eddn), signals(200, 2, &dss, Source::Eddn)]);
        assert_eq!(b.bio_signals, Some(3));
        assert_eq!(b.genera.len(), 2);
        // Merging is idempotent (history replay at every start writes nothing).
        let mut again = b.clone();
        merge(&mut again, &signals(200, 2, &dss, Source::Eddn));
        merge(&mut again, &signals(300, 3, &[], Source::Eddn));
        assert_eq!(again, b);
    }
}
