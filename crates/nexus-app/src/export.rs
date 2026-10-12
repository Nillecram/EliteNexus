//! Ship build export links (EDSY, Coriolis): the journal `Loadout` event, compact JSON, gzipped,
//! URL-safe base64 with `=` escaped as `%3D` — the format both sites (and EDMC) use.

use base64::Engine;
use std::io::Write;

fn encode_loadout(loadout_line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(loadout_line).ok()?;
    let compact = serde_json::to_string(&value).ok()?;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz.write_all(compact.as_bytes()).ok()?;
    let bytes = gz.finish().ok()?;
    Some(base64::engine::general_purpose::URL_SAFE.encode(bytes).replace('=', "%3D"))
}

pub fn edsy_url(loadout_line: &str) -> Option<String> {
    encode_loadout(loadout_line).map(|d| format!("https://edsy.org/#/I={d}"))
}

pub fn coriolis_url(loadout_line: &str) -> Option<String> {
    encode_loadout(loadout_line).map(|d| format!("https://coriolis.io/import?data={d}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn roundtrip() {
        let line = r#"{ "timestamp":"2026-09-05T00:13:39Z", "event":"Loadout", "Ship":"empire_courier", "ShipID":17 }"#;
        let url = edsy_url(line).unwrap();
        let data = url.strip_prefix("https://edsy.org/#/I=").unwrap().replace("%3D", "=");
        let gz = base64::engine::general_purpose::URL_SAFE.decode(data).unwrap();
        let mut s = String::new();
        flate2::read::GzDecoder::new(&gz[..]).read_to_string(&mut s).unwrap();
        assert!(s.contains("\"Ship\":\"empire_courier\""));
        assert!(coriolis_url(line).unwrap().starts_with("https://coriolis.io/import?data="));
    }
}
