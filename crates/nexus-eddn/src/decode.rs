//! Frame -> JSON -> dedupe -> stats/headline -> [`Ingest`] items.

use crate::dedupe::{Dedupe, message_key};
use crate::stats::{EddnStats, now_ms};
use nexus_galaxy::{Ingest, clean_station_name, eddn_schema_name};
use serde_json::Value;
use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Inflated messages larger than this are rejected (EDDN messages are a few KB; the gateway limit
/// was 1 MiB compressed).
const MAX_INFLATED: u64 = 32 << 20;

pub(crate) fn inflate(frame: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(frame.len() * 6);
    flate2::read::ZlibDecoder::new(frame)
        .take(MAX_INFLATED)
        .read_to_end(&mut out)?;
    Ok(out)
}

pub(crate) struct Decoder {
    stats: Arc<EddnStats>,
    dedupe: Dedupe,
}

impl Decoder {
    pub fn new(stats: Arc<EddnStats>, window: Duration) -> Decoder {
        Decoder {
            stats,
            dedupe: Dedupe::new(window),
        }
    }

    /// Handles one zlib-compressed relay frame; returns the database items to ingest.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Vec<Ingest> {
        let json = match inflate(frame) {
            Ok(j) => j,
            Err(e) => {
                self.stats.decode_error();
                tracing::debug!(error = %e, "EDDN frame does not inflate");
                return Vec::new();
            }
        };
        match serde_json::from_slice::<Value>(&json) {
            Ok(v) => self.handle_message(&v),
            Err(e) => {
                self.stats.decode_error();
                tracing::debug!(error = %e, "EDDN frame is not JSON");
                Vec::new()
            }
        }
    }

    pub fn handle_message(&mut self, v: &Value) -> Vec<Ingest> {
        let Some(schema_ref) = v.get("$schemaRef").and_then(Value::as_str) else {
            self.stats.decode_error();
            return Vec::new();
        };
        if schema_ref.trim_end_matches('/').ends_with("/test") {
            self.stats.test_dropped();
            return Vec::new();
        }
        if self.dedupe.is_duplicate(message_key(v), Instant::now()) {
            self.stats.duplicate();
            return Vec::new();
        }
        let schema = eddn_schema_name(schema_ref).unwrap_or("unknown");
        self.stats.message(schema, now_ms(), headline(schema, v));
        let items = Ingest::from_eddn(v);
        self.stats.ingested(items.len());
        items
    }
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn n(v: &Value, key: &str) -> Option<i64> {
    let x = v.get(key)?;
    x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))
}

fn len(v: &Value, key: &str) -> usize {
    v.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

/// `$Codex_Ent_Stratum_07_F_Name;` -> `Stratum 07 F`, `$SAA_SignalType_Biological;` -> `Biological`.
fn symbol_text(raw: &str) -> String {
    let t = raw.trim().trim_start_matches('$').trim_end_matches(';');
    let t = t
        .strip_suffix("_Name")
        .or_else(|| t.strip_suffix("_name"))
        .unwrap_or(t);
    let t = t
        .strip_prefix("Codex_Ent_")
        .or_else(|| t.strip_prefix("SAA_SignalType_"))
        .or_else(|| t.strip_prefix("Codex_"))
        .unwrap_or(t);
    t.replace('_', " ")
}

fn place(station: Option<&str>, system: Option<&str>) -> String {
    match (station, system) {
        (Some(st), Some(sy)) => format!("{} · {sy}", clean_station_name(st)),
        (Some(x), None) | (None, Some(x)) => clean_station_name(x),
        (None, None) => "?".to_string(),
    }
}

/// A short human-readable line for the ticker. Uses only message content (never the header, so
/// never the uploader).
pub(crate) fn headline(schema: &str, v: &Value) -> Option<String> {
    let m = v.get("message")?;
    let text = match schema {
        "commodity/3" => format!(
            "Market {} ({} commodities)",
            place(s(m, "stationName"), s(m, "systemName")),
            len(m, "commodities")
        ),
        "journal/1" => {
            let system = s(m, "StarSystem");
            match s(m, "event").unwrap_or("?") {
                "FSDJump" => match n(m, "Population").filter(|p| *p > 0) {
                    Some(p) => format!("Jump to {} (pop {})", system?, compact(p)),
                    None => format!("Jump to {}", system?),
                },
                "Location" => format!("Location {}", place(s(m, "StationName"), system)),
                "CarrierJump" => format!(
                    "Carrier {} jumped to {}",
                    s(m, "StationName").unwrap_or("?"),
                    system?
                ),
                "Docked" => format!("Docked at {}", place(s(m, "StationName"), system)),
                "Scan" => format!("Scan {}", s(m, "BodyName").or(system)?),
                "SAASignalsFound" => {
                    let body = s(m, "BodyName")?;
                    let sig: Vec<String> = m
                        .get("Signals")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| {
                                    Some(format!(
                                        "{} ×{}",
                                        symbol_text(s(x, "Type")?),
                                        n(x, "Count").unwrap_or(0)
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if sig.is_empty() {
                        format!("Signals on {body}")
                    } else {
                        format!("Signals on {body}: {}", sig.join(", "))
                    }
                }
                other => format!("{other} in {}", system.unwrap_or("?")),
            }
        }
        "outfitting/2" | "outfitting/3" => format!(
            "Outfitting {} ({} modules)",
            place(s(m, "stationName"), s(m, "systemName")),
            len(m, "modules")
        ),
        "shipyard/2" => {
            format!(
                "Shipyard {} ({} ships)",
                place(s(m, "stationName"), s(m, "systemName")),
                len(m, "ships")
            )
        }
        "navroute/1" => {
            let route = m.get("Route").and_then(Value::as_array)?;
            let first = route.first().and_then(|r| s(r, "StarSystem"))?;
            let last = route.last().and_then(|r| s(r, "StarSystem"))?;
            format!(
                "Route plotted {first} → {last} ({} jumps)",
                route.len().saturating_sub(1)
            )
        }
        "fssdiscoveryscan/1" => {
            format!(
                "Discovery scan {} ({} bodies)",
                s(m, "SystemName")?,
                n(m, "BodyCount").unwrap_or(0)
            )
        }
        "fssallbodiesfound/1" => format!(
            "All {} bodies found in {}",
            n(m, "Count").unwrap_or(0),
            s(m, "SystemName")?
        ),
        "fssbodysignals/1" => format!("Body signals {}", s(m, "BodyName").or(s(m, "StarSystem"))?),
        "fsssignaldiscovered/1" => {
            format!("{} signals in {}", len(m, "signals"), s(m, "StarSystem")?)
        }
        "codexentry/1" => format!(
            "Codex {} in {}",
            symbol_text(s(m, "Name")?),
            s(m, "System")?
        ),
        "approachsettlement/1" => {
            format!("Approaching {}", place(s(m, "Name"), s(m, "StarSystem")))
        }
        "dockinggranted/1" => format!("Docking granted at {}", s(m, "StationName")?),
        "dockingdenied/1" => format!(
            "Docking denied at {} ({})",
            s(m, "StationName")?,
            s(m, "Reason").unwrap_or("?")
        ),
        "scanbarycentre/1" => format!("Barycentre scanned in {}", s(m, "StarSystem")?),
        "navbeaconscan/1" => format!(
            "Nav beacon {} ({} bodies)",
            s(m, "StarSystem")?,
            n(m, "NumBodies").unwrap_or(0)
        ),
        "fcmaterials_journal/1" | "fcmaterials_capi/1" => {
            format!("Carrier bar {}", s(m, "CarrierName").or(s(m, "CarrierID"))?)
        }
        "scanorganic/1" => format!(
            "Organic {} in {}",
            symbol_text(s(m, "Species")?),
            s(m, "StarSystem")?
        ),
        other => other.to_string(),
    };
    Some(text)
}

fn compact(p: i64) -> String {
    let f = p as f64;
    if f >= 1e9 {
        format!("{:.1} B", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1} M", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.0} k", f / 1e3)
    } else {
        p.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn zlib(v: &Value) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(v.to_string().as_bytes()).expect("zlib");
        e.finish().expect("zlib")
    }

    fn commodity() -> Value {
        json!({"$schemaRef": "https://eddn.edcd.io/schemas/commodity/3",
               "header": {"uploaderID": "deadbeefcafe", "softwareName": "E:D Market Connector [Windows]", "softwareVersion": "6.1.2",
                          "gatewayTimestamp": "2026-10-12T09:23:12.361367Z"},
               "message": {"systemName": "Sol", "stationName": "Daedalus", "marketId": 128016384u64, "timestamp": "2026-10-12T09:23:10Z",
                           "commodities": [{"name": "gold", "meanPrice": 1, "buyPrice": 2, "stock": 3, "stockBracket": 1,
                                            "sellPrice": 1, "demand": 0, "demandBracket": ""}]}})
    }

    #[test]
    fn frames_decode_dedupe_and_count() {
        let stats = Arc::new(EddnStats::new(10));
        let mut d = Decoder::new(stats.clone(), Duration::from_secs(600));
        let items = d.handle_frame(&zlib(&commodity()));
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], Ingest::Market(m) if m.market_id == 128016384));
        // same data from another uploader a moment later: duplicate
        let mut again = commodity();
        again["header"]["uploaderID"] = json!("another");
        again["message"]["timestamp"] = json!("2026-10-12T09:23:15Z");
        assert!(d.handle_frame(&zlib(&again)).is_empty());
        // test schema, garbage, not zlib
        let mut test = commodity();
        test["$schemaRef"] = json!("https://eddn.edcd.io/schemas/commodity/3/test");
        assert!(d.handle_frame(&zlib(&test)).is_empty());
        assert!(d.handle_frame(b"not zlib at all").is_empty());
        assert!(d.handle_frame(&zlib(&json!([1, 2]))).is_empty());
        let snap = stats.snapshot();
        assert_eq!(snap.total_messages, 1);
        assert_eq!(snap.duplicates, 1);
        assert_eq!(snap.test_dropped, 1);
        assert_eq!(snap.decode_errors, 2);
        assert_eq!(snap.ingested, 1);
        assert_eq!(snap.per_schema, vec![("commodity/3".to_string(), 1)]);
        assert_eq!(
            snap.recent_headlines[0].text,
            "Market Daedalus · Sol (1 commodities)"
        );
        assert!(snap.last_message_unix_ms.is_some());
    }

    #[test]
    fn headlines_never_mention_the_uploader() {
        let samples = [
            commodity(),
            json!({"$schemaRef": "https://eddn.edcd.io/schemas/journal/1", "header": {"uploaderID": "CMDR Secret"},
                   "message": {"event": "FSDJump", "StarSystem": "Sol", "Population": 18320926115u64}}),
            json!({"$schemaRef": "https://eddn.edcd.io/schemas/journal/1", "header": {"uploaderID": "CMDR Secret"},
                   "message": {"event": "SAASignalsFound", "BodyName": "X 1 A Ring",
                               "Signals": [{"Type": "Painite", "Count": 2}, {"Type": "$SAA_SignalType_Geological;", "Count": 1}]}}),
            json!({"$schemaRef": "https://eddn.edcd.io/schemas/navroute/1", "header": {"uploaderID": "CMDR Secret"},
                   "message": {"Route": [{"StarSystem": "A"}, {"StarSystem": "B"}, {"StarSystem": "C"}]}}),
            json!({"$schemaRef": "https://eddn.edcd.io/schemas/codexentry/1", "header": {"uploaderID": "CMDR Secret"},
                   "message": {"Name": "$Codex_Ent_Stratum_07_F_Name;", "System": "Y"}}),
        ];
        let texts: Vec<String> = samples
            .iter()
            .map(|v| {
                let schema = eddn_schema_name(v["$schemaRef"].as_str().unwrap_or_default())
                    .unwrap_or_default();
                headline(schema, v).expect("headline")
            })
            .collect();
        for t in &texts {
            assert!(!t.contains("Secret") && !t.contains("deadbeef"), "{t}");
        }
        assert_eq!(texts[1], "Jump to Sol (pop 18.3 B)");
        assert_eq!(texts[2], "Signals on X 1 A Ring: Painite ×2, Geological ×1");
        assert_eq!(texts[3], "Route plotted A → C (2 jumps)");
        assert_eq!(texts[4], "Codex Stratum 07 F in Y");
    }
}
