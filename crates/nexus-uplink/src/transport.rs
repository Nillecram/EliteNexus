//! HTTP transport. Every request the uplink makes goes through [`Transport`], so tests (and dry
//! runs) can capture requests instead of sending them.

use crate::json::{form_urldecode, gunzip};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One HTTP request, fully materialised (headers and the exact body bytes).
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout: Duration,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>, timeout: Duration) -> Self {
        HttpRequest { method: Method::Get, url: url.into(), headers: Vec::new(), body: Vec::new(), timeout }
    }

    pub fn post(url: impl Into<String>, body: Vec<u8>, timeout: Duration) -> Self {
        HttpRequest { method: Method::Post, url: url.into(), headers: Vec::new(), body, timeout }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Case-insensitive header lookup.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The body as text, transparently gunzipping `Content-Encoding: gzip` bodies.
    pub fn body_text(&self) -> String {
        if self.header_value("Content-Encoding").is_some_and(|v| v.eq_ignore_ascii_case("gzip")) {
            match gunzip(&self.body) {
                Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                Err(_) => String::new(),
            }
        } else {
            String::from_utf8_lossy(&self.body).into_owned()
        }
    }
}

/// An HTTP response (any status; non-2xx is not an error at this layer).
#[derive(Debug, Clone, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResponse {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        HttpResponse { status, headers: Vec::new(), body: body.into() }
    }

    /// Case-insensitive header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Connection-level failure (DNS, TLS, refused, timeout, …).
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct TransportError(pub String);

pub type TransportFuture = Pin<Box<dyn Future<Output = Result<HttpResponse, TransportError>> + Send>>;

/// Sends HTTP requests. Implementations must be cheap to call concurrently.
pub trait Transport: Send + Sync + 'static {
    fn send(&self, req: HttpRequest) -> TransportFuture;
}

// ---------------------------------------------------------------------------------------------
// reqwest
// ---------------------------------------------------------------------------------------------

/// The real transport: reqwest + rustls, HTTP/1.1 only (the EDDN docs say HTTP/2 is not
/// supported), no certificate pinning.
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new(user_agent: &str) -> Result<Self, TransportError> {
        let client = reqwest::Client::builder()
            .http1_only()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| TransportError(format!("cannot create HTTP client: {e}")))?;
        Ok(ReqwestTransport { client })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, req: HttpRequest) -> TransportFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let mut rb = match req.method {
                Method::Get => client.get(&req.url),
                Method::Post => client.post(&req.url),
            };
            for (k, v) in &req.headers {
                rb = rb.header(k.as_str(), v.as_str());
            }
            if req.method == Method::Post {
                rb = rb.body(req.body);
            }
            let resp = rb.timeout(req.timeout).send().await.map_err(|e| TransportError(error_chain(&e)))?;
            let status = resp.status().as_u16();
            let headers = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
                .collect();
            let body = resp.text().await.map_err(|e| TransportError(format!("reading response: {}", error_chain(&e))))?;
            Ok(HttpResponse { status, headers, body })
        })
    }
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut src = e.source();
    while let Some(inner) = src {
        let t = inner.to_string();
        if !s.contains(&t) {
            s.push_str(": ");
            s.push_str(&t);
        }
        src = inner.source();
    }
    s
}

/// Used when no HTTP client could be created: every request fails (and is retried later).
pub(crate) struct FailingTransport(pub String);

impl Transport for FailingTransport {
    fn send(&self, _req: HttpRequest) -> TransportFuture {
        let msg = self.0.clone();
        Box::pin(async move { Err(TransportError(msg)) })
    }
}

// ---------------------------------------------------------------------------------------------
// Recording transport (tests, dry runs)
// ---------------------------------------------------------------------------------------------

type Responder = dyn Fn(&HttpRequest) -> Result<HttpResponse, TransportError> + Send + Sync;

/// Records every request and answers with a scripted response. Nothing leaves the machine.
pub struct RecordingTransport {
    requests: Mutex<Vec<HttpRequest>>,
    responder: Box<Responder>,
}

impl RecordingTransport {
    pub fn new(responder: impl Fn(&HttpRequest) -> Result<HttpResponse, TransportError> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(RecordingTransport { requests: Mutex::new(Vec::new()), responder: Box::new(responder) })
    }

    /// Answers like the real services do when everything is accepted.
    pub fn simulated() -> Arc<Self> {
        Self::new(simulated_response)
    }

    /// Every request so far.
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().clone()
    }

    /// Requests whose URL contains `needle`.
    pub fn requests_to(&self, needle: &str) -> Vec<HttpRequest> {
        self.requests.lock().iter().filter(|r| r.url.contains(needle)).cloned().collect()
    }
}

impl Transport for RecordingTransport {
    fn send(&self, req: HttpRequest) -> TransportFuture {
        let res = (self.responder)(&req);
        self.requests.lock().push(req);
        Box::pin(async move { res })
    }
}

/// Plausible "everything accepted" answers for each service.
pub fn simulated_response(req: &HttpRequest) -> Result<HttpResponse, TransportError> {
    let url = req.url.as_str();
    if url.contains("eddn.edcd.io") {
        return Ok(HttpResponse::new(200, "OK"));
    }
    if url.contains("edsm.net") {
        if url.ends_with("/discard") {
            return Ok(HttpResponse::new(200, r#"["Music","Fileheader","FSSSignalDiscovered","ShipLocker","ReceiveText","Market","Outfitting","Shipyard","NavRoute","NavRouteClear","Status"]"#));
        }
        let form = form_urldecode(&req.body_text());
        let n = form
            .iter()
            .find(|(k, _)| k == "message")
            .and_then(|(_, v)| serde_json::from_str::<Value>(v).ok())
            .and_then(|v| v.as_array().map(Vec::len))
            .unwrap_or(0);
        let events: Vec<Value> = (0..n).map(|_| json!({"msgnum": 100, "msg": "OK"})).collect();
        return Ok(HttpResponse::new(200, json!({"msgnum": 100, "msg": "OK", "events": events}).to_string()));
    }
    if url.contains("inara.cz") {
        let n = serde_json::from_str::<Value>(&req.body_text())
            .ok()
            .and_then(|v| v.get("events").and_then(Value::as_array).map(Vec::len))
            .unwrap_or(0);
        let events: Vec<Value> = (0..n).map(|_| json!({"eventStatus": 200})).collect();
        return Ok(HttpResponse::new(
            200,
            json!({"header": {"eventStatus": 200, "eventData": {"userID": 1, "userName": "Test"}}, "events": events}).to_string(),
        ));
    }
    if url.contains("edastro.com") {
        if url.ends_with("/api/accepting") {
            return Ok(HttpResponse::new(200, r#"["CarrierJumpRequest","CarrierStats","Docked","ScanOrganic"]"#));
        }
        return Ok(HttpResponse::new(200, r#"{"status":200,"message":"OK"}"#));
    }
    Ok(HttpResponse::new(404, "not found"))
}
