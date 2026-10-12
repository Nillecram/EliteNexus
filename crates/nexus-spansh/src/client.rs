//! The [`Spansh`] client: configuration and HTTP plumbing shared by every endpoint.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::job::JobOptions;
use crate::util::Form;
use crate::{CancellationToken, Error, Result};

/// API root used by the website itself.
pub const DEFAULT_BASE_URL: &str = "https://spansh.co.uk/api";

/// Descriptive User-Agent, as Spansh etiquette asks.
pub const USER_AGENT: &str = "EliteNexus/0.1 (Elite Dangerous companion)";

/// Async Spansh API client. Cheap to clone (an `Arc` plus per-handle job options); all clones
/// share one connection pool and one concurrency limit (2 requests in flight by default).
///
/// ```no_run
/// # async fn demo() -> nexus_spansh::Result<()> {
/// use nexus_spansh::{NeutronRouteRequest, Spansh};
/// let spansh = Spansh::new();
/// let plotted = spansh.neutron_route(&NeutronRouteRequest::new("Sol", "Colonia", 60.0)).await?;
/// println!("{} waypoints", plotted.route.system_jumps.len());
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Spansh {
    inner: Arc<Inner>,
    pub(crate) job: JobOptions,
}

struct Inner {
    http: reqwest::Client,
    base_url: String,
    limiter: Semaphore,
    record_dir: Option<PathBuf>,
    record_seq: AtomicU32,
}

impl fmt::Debug for Spansh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Spansh")
            .field("base_url", &self.inner.base_url)
            .field("record_dir", &self.inner.record_dir)
            .field("job", &self.job)
            .finish_non_exhaustive()
    }
}

impl Default for Spansh {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for [`Spansh`]; start with [`Spansh::builder`].
#[derive(Debug, Clone)]
pub struct SpanshBuilder {
    base_url: String,
    user_agent: String,
    request_timeout: Duration,
    connect_timeout: Duration,
    max_concurrent: usize,
    job: JobOptions,
    record_dir: Option<PathBuf>,
    http: Option<reqwest::Client>,
}

impl Default for SpanshBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            user_agent: USER_AGENT.to_owned(),
            request_timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(15),
            max_concurrent: 2,
            job: JobOptions::default(),
            record_dir: None,
            http: None,
        }
    }
}

impl SpanshBuilder {
    /// API root (default [`DEFAULT_BASE_URL`]); handy for tests against a local mock.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_owned();
        self
    }

    /// User-Agent header (default [`USER_AGENT`]).
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = ua.into();
        self
    }

    /// Timeout for one HTTP request including the body (default 60 s).
    pub fn request_timeout(mut self, t: Duration) -> Self {
        self.request_timeout = t;
        self
    }

    /// TCP/TLS connect timeout (default 15 s).
    pub fn connect_timeout(mut self, t: Duration) -> Self {
        self.connect_timeout = t;
        self
    }

    /// Requests in flight at once across all clones (default 2, minimum 1).
    pub fn max_concurrent_requests(mut self, n: usize) -> Self {
        self.max_concurrent = n.max(1);
        self
    }

    /// Default polling/timeout/cancellation behaviour for route jobs.
    pub fn job_options(mut self, opts: JobOptions) -> Self {
        self.job = opts;
        self
    }

    /// Write every response body to `dir` as `NNN_<endpoint>.json` (development aid for
    /// capturing fixtures; off by default).
    pub fn record_responses(mut self, dir: impl Into<PathBuf>) -> Self {
        self.record_dir = Some(dir.into());
        self
    }

    /// Use an existing `reqwest::Client` (its own User-Agent/timeouts then apply).
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http = Some(client);
        self
    }

    pub fn build(self) -> Result<Spansh> {
        let http = match self.http.clone() {
            Some(client) => client,
            None => {
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    reqwest::header::ACCEPT,
                    reqwest::header::HeaderValue::from_static("application/json"),
                );
                reqwest::Client::builder()
                    .user_agent(self.user_agent.clone())
                    .default_headers(headers)
                    .gzip(true)
                    .timeout(self.request_timeout)
                    .connect_timeout(self.connect_timeout)
                    .build()
                    .map_err(|source| Error::Transport {
                        url: self.base_url.clone(),
                        source,
                    })?
            }
        };
        Ok(self.finish(http))
    }

    fn finish(self, http: reqwest::Client) -> Spansh {
        Spansh {
            inner: Arc::new(Inner {
                http,
                base_url: self.base_url,
                limiter: Semaphore::new(self.max_concurrent),
                record_dir: self.record_dir,
                record_seq: AtomicU32::new(0),
            }),
            job: self.job,
        }
    }
}

/// A raw HTTP reply.
pub(crate) struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Spansh {
    /// Client with the default configuration.
    pub fn new() -> Self {
        match Self::builder().build() {
            Ok(client) => client,
            Err(e) => {
                // Only TLS backend initialisation can fail here; reqwest's defaults are the
                // best remaining option.
                tracing::error!(error = %e, "Spansh HTTP client setup failed; using reqwest defaults");
                SpanshBuilder::default().finish(reqwest::Client::new())
            }
        }
    }

    pub fn builder() -> SpanshBuilder {
        SpanshBuilder::default()
    }

    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// Job options used by this handle.
    pub fn job_options(&self) -> &JobOptions {
        &self.job
    }

    /// A handle sharing the same connection pool but using `opts` for route jobs.
    pub fn with_job_options(&self, opts: JobOptions) -> Spansh {
        Spansh {
            inner: Arc::clone(&self.inner),
            job: opts,
        }
    }

    /// A handle whose requests and jobs stop with [`Error::Cancelled`] once `token` is
    /// cancelled. (Dropping the future cancels as well.)
    pub fn with_cancellation(&self, token: CancellationToken) -> Spansh {
        let mut job = self.job.clone();
        job.cancel = Some(token);
        self.with_job_options(job)
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}/{}", self.inner.base_url, path.trim_start_matches('/'))
    }

    /// Send a request (respecting the concurrency limit and cancellation) and read the body.
    async fn execute(&self, request: reqwest::RequestBuilder, url: &str) -> Result<Reply> {
        let work = async {
            let _permit = self
                .inner
                .limiter
                .acquire()
                .await
                .map_err(|_| Error::Cancelled)?;
            let started = Instant::now();
            let response = request.send().await.map_err(|e| transport(url, e))?;
            let status = response.status().as_u16();
            let body = Vec::from(response.bytes().await.map_err(|e| transport(url, e))?);
            tracing::debug!(%url, status, bytes = body.len(), ms = started.elapsed().as_millis() as u64, "spansh response");
            self.record(url, status, &body);
            Ok(Reply { status, body })
        };
        match &self.job.cancel {
            Some(token) => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => Err(Error::Cancelled),
                    reply = work => reply,
                }
            }
            None => work.await,
        }
    }

    pub(crate) async fn get_raw(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<(String, Reply)> {
        let url = self.url(path);
        let mut request = self.inner.http.get(&url);
        if !query.is_empty() {
            request = request.query(query);
        }
        let reply = self.execute(request, &url).await?;
        Ok((url, reply))
    }

    /// GET and decode; 404 becomes [`Error::NotFound`] naming `what`.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        what: &str,
    ) -> Result<T> {
        let (url, reply) = self.get_raw(path, query).await?;
        check(&reply, &url, what)?;
        decode(&url, &reply.body)
    }

    pub(crate) async fn post_form_raw(&self, path: &str, form: &Form) -> Result<(String, Reply)> {
        let url = self.url(path);
        let reply = self
            .execute(self.inner.http.post(&url).form(form), &url)
            .await?;
        Ok((url, reply))
    }

    pub(crate) async fn post_json_raw(&self, path: &str, body: &Value) -> Result<(String, Reply)> {
        let url = self.url(path);
        let reply = self
            .execute(self.inner.http.post(&url).json(body), &url)
            .await?;
        Ok((url, reply))
    }

    /// POST a JSON body and decode the JSON answer.
    pub(crate) async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &Value,
        what: &str,
    ) -> Result<T> {
        let (url, reply) = self.post_json_raw(path, body).await?;
        check(&reply, &url, what)?;
        decode(&url, &reply.body)
    }

    /// Cancellable sleep.
    pub(crate) async fn pause(&self, d: Duration) -> Result<()> {
        match &self.job.cancel {
            Some(token) => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => Err(Error::Cancelled),
                    _ = tokio::time::sleep(d) => Ok(()),
                }
            }
            None => {
                tokio::time::sleep(d).await;
                Ok(())
            }
        }
    }

    pub(crate) fn check_cancelled(&self) -> Result<()> {
        match &self.job.cancel {
            Some(token) if token.is_cancelled() => Err(Error::Cancelled),
            _ => Ok(()),
        }
    }

    fn record(&self, url: &str, status: u16, body: &[u8]) {
        let Some(dir) = &self.inner.record_dir else {
            return;
        };
        let n = self.inner.record_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let tail = url
            .strip_prefix(self.inner.base_url.as_str())
            .unwrap_or(url);
        let mut slug: String = tail
            .trim_matches('/')
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        slug.truncate(80);
        let name = if status == 200 {
            format!("{n:03}_{slug}.json")
        } else {
            format!("{n:03}_{slug}.http{status}.json")
        };
        if let Err(e) =
            std::fs::create_dir_all(dir).and_then(|()| std::fs::write(dir.join(&name), body))
        {
            tracing::warn!(error = %e, file = %name, "could not record Spansh response");
        }
    }
}

fn transport(url: &str, e: reqwest::Error) -> Error {
    if e.is_timeout() {
        Error::Timeout {
            url: url.to_owned(),
        }
    } else {
        Error::Transport {
            url: url.to_owned(),
            source: e,
        }
    }
}

/// Map a non-2xx reply to an error.
pub(crate) fn check(reply: &Reply, url: &str, what: &str) -> Result<()> {
    match reply.status {
        200..=299 => Ok(()),
        404 => Err(Error::NotFound {
            what: what.to_owned(),
        }),
        status => Err(Error::Http {
            status,
            url: url.to_owned(),
            message: error_message(&reply.body),
        }),
    }
}

/// Spansh's `{"error": "…"}` text, or the start of the body.
pub(crate) fn error_message(body: &[u8]) -> String {
    if let Ok(Value::Object(m)) = serde_json::from_slice::<Value>(body) {
        for key in ["error", "message", "detail"] {
            if let Some(Value::String(s)) = m.get(key) {
                return s.clone();
            }
        }
    }
    snippet(body)
}

/// First ~240 bytes of a body, printable.
pub(crate) fn snippet(body: &[u8]) -> String {
    const MAX: usize = 240;
    let text = String::from_utf8_lossy(&body[..body.len().min(MAX)]);
    let mut s: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if body.len() > MAX {
        s.push('…');
    }
    if s.trim().is_empty() {
        "(empty body)".to_owned()
    } else {
        s
    }
}

/// Decode a JSON body, reporting the failing field path.
pub(crate) fn decode<T: DeserializeOwned>(url: &str, body: &[u8]) -> Result<T> {
    let mut de = serde_json::Deserializer::from_slice(body);
    serde_path_to_error::deserialize(&mut de).map_err(|e| Error::Decode {
        url: url.to_owned(),
        detail: format!(
            "{} at `{}`; body starts: {}",
            e.inner(),
            e.path(),
            snippet(body)
        ),
    })
}

/// Decode an already-parsed value; `prefix` names where it came from (e.g. `result`).
pub(crate) fn decode_value<T: DeserializeOwned>(
    url: &str,
    prefix: &str,
    value: Value,
) -> Result<T> {
    serde_path_to_error::deserialize(value).map_err(|e| {
        let path = e.path().to_string();
        let at = if path == "." {
            prefix.to_owned()
        } else {
            format!("{prefix}.{path}")
        };
        Error::Decode {
            url: url.to_owned(),
            detail: format!("{} at `{at}`", e.inner()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages() {
        assert_eq!(
            error_message(br#"{"error":"Could not find starting system"}"#),
            "Could not find starting system"
        );
        assert_eq!(
            error_message(b"<html>Bad Gateway</html>"),
            "<html>Bad Gateway</html>"
        );
        assert_eq!(error_message(b""), "(empty body)");
    }

    #[test]
    fn decode_reports_paths() {
        #[derive(Debug, serde::Deserialize)]
        struct Inner {
            #[allow(dead_code)]
            x: f64,
        }
        #[derive(Debug, serde::Deserialize)]
        struct Outer {
            #[allow(dead_code)]
            jumps: Vec<Inner>,
        }
        let err =
            decode::<Outer>("u", br#"{"jumps":[{"x":1},{"x":"nope"}]}"#).expect_err("must fail");
        assert!(err.to_string().contains("jumps[1].x"), "{err}");
        let err = decode_value::<Outer>("u", "result", serde_json::json!({"jumps": [{"x": []}]}))
            .expect_err("must fail");
        assert!(err.to_string().contains("result.jumps[0].x"), "{err}");
    }

    #[test]
    fn handles_are_cheap_and_share_state() {
        let a = Spansh::builder()
            .base_url("http://localhost:1/api/")
            .build()
            .expect("client");
        let b = a.with_cancellation(CancellationToken::new());
        assert!(Arc::ptr_eq(&a.inner, &b.inner));
        assert_eq!(b.url("/route"), "http://localhost:1/api/route");
        assert!(a.job.cancel.is_none() && b.job.cancel.is_some());
    }
}
