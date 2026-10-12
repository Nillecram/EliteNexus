//! Single-record lookups, autocomplete, nearest system, global search, system dumps and the
//! search field catalogue.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::dump;
use crate::records::{
    BodyRecord, FieldValues, SearchableField, SearchableFields, SmartSearchResponse, StationRecord,
    SystemNames, SystemPoint, SystemRecord,
};
use crate::search::{SearchKind, SearchRequest, filter};
use crate::util::Coords;
use crate::{Error, Result, Spansh};

/// `{"record": {…}}` (`/api/system|body|station/{id}`) or `{"system": {…}}` (`/api/dump`,
/// `/api/nearest`).
#[derive(Deserialize)]
#[serde(default, bound(deserialize = "T: DeserializeOwned"))]
struct Envelope<T> {
    record: Option<T>,
    system: Option<T>,
}

impl<T> Default for Envelope<T> {
    fn default() -> Self {
        Self {
            record: None,
            system: None,
        }
    }
}

impl<T> Envelope<T> {
    fn into_inner(self) -> Option<T> {
        self.record.or(self.system)
    }
}

impl Spansh {
    /// GET an enveloped record; a missing record is a decode error.
    async fn get_record<T: DeserializeOwned>(&self, path: &str, what: &str) -> Result<T> {
        let env: Envelope<T> = self.get_json(path, &[], what).await?;
        env.into_inner().ok_or_else(|| Error::Decode {
            url: self.url(path),
            detail: "response carries no record".to_owned(),
        })
    }

    /// System-name autocomplete (prefix match) with id64 and coordinates:
    /// `GET /api/systems/field_values/system_names?q=`. Debounce calls (≥ 200 ms) in the UI.
    pub async fn autocomplete(&self, prefix: &str) -> Result<Vec<SystemPoint>> {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return Ok(Vec::new());
        }
        let names: SystemNames = self
            .get_json(
                "systems/field_values/system_names",
                &[("q", prefix.to_owned())],
                "system names",
            )
            .await?;
        let mut out = names.min_max;
        // Names without coordinates (if Spansh ever sends only `values`).
        for name in names.values {
            if !out.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                out.push(SystemPoint {
                    name,
                    ..SystemPoint::default()
                });
            }
        }
        Ok(out)
    }

    /// `GET /api/system/{id64}`. Unknown systems give [`Error::NotFound`].
    pub async fn system(&self, id64: u64) -> Result<SystemRecord> {
        self.get_record(&format!("system/{id64}"), &format!("system {id64}"))
            .await
    }

    /// Look a system up by exact name (case-insensitive): a systems search on `name`, falling
    /// back to autocomplete + [`Spansh::system`]. `Ok(None)` when Spansh does not know it.
    pub async fn system_by_name(&self, name: &str) -> Result<Option<SystemRecord>> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(None);
        }
        let req = SearchRequest::new()
            .filter("name", filter::eq(name))
            .size(10);
        match self.search_systems(&req).await {
            Ok(hits) => {
                if let Some(found) = hits
                    .results
                    .into_iter()
                    .find(|s| s.name.eq_ignore_ascii_case(name))
                {
                    return Ok(Some(found));
                }
            }
            // Network trouble: don't hammer; anything else (rejected body): try autocomplete.
            Err(e) if e.is_transient() || matches!(e, Error::Cancelled) => return Err(e),
            Err(e) => {
                tracing::debug!(error = %e, "systems search by name failed; trying autocomplete")
            }
        }
        let suggestion = self
            .autocomplete(name)
            .await?
            .into_iter()
            .find(|p| p.name.eq_ignore_ascii_case(name));
        match suggestion.and_then(|p| p.id64) {
            Some(id64) => match self.system(id64).await {
                Ok(record) => Ok(Some(record)),
                Err(Error::NotFound { .. }) => Ok(None),
                Err(e) => Err(e),
            },
            None => Ok(None),
        }
    }

    /// `GET /api/body/{id64}`.
    pub async fn body(&self, id64: u64) -> Result<BodyRecord> {
        self.get_record(&format!("body/{id64}"), &format!("body {id64}"))
            .await
    }

    /// `GET /api/station/{market_id}` (stations, carriers, settlements).
    pub async fn station(&self, market_id: u64) -> Result<StationRecord> {
        self.get_record(
            &format!("station/{market_id}"),
            &format!("station {market_id}"),
        )
        .await
    }

    /// Full system dump: `GET /api/dump/{id64}` — every body (rings, signals, landmarks) and
    /// station (market, outfitting, shipyard). Large (Sol ≈ 2 MB); cache it.
    pub async fn dump(&self, id64: u64) -> Result<dump::System> {
        self.get_record::<dump::System>(&format!("dump/{id64}"), &format!("system {id64}"))
            .await
    }

    /// Nearest known system to a position: `GET /api/nearest?x=&y=&z=`.
    pub async fn nearest(&self, pos: Coords) -> Result<SystemPoint> {
        let query = [
            ("x", pos.x.to_string()),
            ("y", pos.y.to_string()),
            ("z", pos.z.to_string()),
        ];
        let env: Envelope<SystemPoint> = self.get_json("nearest", &query, "nearest system").await?;
        env.into_inner().ok_or_else(|| Error::NotFound {
            what: format!("system near {}, {}, {}", pos.x, pos.y, pos.z),
        })
    }

    /// Global search over systems, bodies and stations: `GET /api/search?q=` (10 hits).
    pub async fn smart_search(&self, query: &str) -> Result<SmartSearchResponse> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(SmartSearchResponse::default());
        }
        self.get_json("search", &[("q", query.to_owned())], "search")
            .await
    }

    /// Field catalogue of a search kind: `GET /api/{kind}/searchable_fields`.
    pub async fn searchable_fields(&self, kind: SearchKind) -> Result<Vec<SearchableField>> {
        let fields: SearchableFields = self
            .get_json(
                &format!("{kind}/searchable_fields"),
                &[],
                &format!("{kind} searchable fields"),
            )
            .await?;
        Ok(fields.fields)
    }

    /// Values of a group field (e.g. `primary_economy`), optionally filtered:
    /// `GET /api/{kind}/field_values/{field}?q=`.
    pub async fn field_values(
        &self,
        kind: SearchKind,
        field: &str,
        query: Option<&str>,
    ) -> Result<FieldValues> {
        if field.is_empty() || !field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(Error::InvalidRequest(format!(
                "invalid field name {field:?}"
            )));
        }
        let q: Vec<(&str, String)> = query.map(|q| vec![("q", q.to_owned())]).unwrap_or_default();
        self.get_json(
            &format!("{kind}/field_values/{field}"),
            &q,
            &format!("{kind} field {field}"),
        )
        .await
    }
}
