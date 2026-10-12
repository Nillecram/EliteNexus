//! Error type for every Spansh call.

use std::time::Duration;

/// `Result` with [`Error`] as the default error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong talking to Spansh.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The request could not be sent, or the response body could not be read (DNS, TLS,
    /// connection reset, …).
    #[error("Spansh request failed ({url}): {source}")]
    Transport {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// A single HTTP request exceeded the client's request timeout.
    #[error("Spansh request timed out ({url})")]
    Timeout { url: String },

    /// Spansh answered with a non-success HTTP status. `message` is Spansh's own `{"error": …}`
    /// text when it sent one, otherwise the start of the body.
    #[error("Spansh returned HTTP {status} ({url}): {message}")]
    Http {
        status: u16,
        url: String,
        message: String,
    },

    /// The system, body, station or job is unknown to Spansh (HTTP 404).
    #[error("not found on Spansh: {what}")]
    NotFound { what: String },

    /// Spansh accepted the request but reported a failure (`{"status": "error", "error": …}`),
    /// e.g. an unknown starting system or an impossible route.
    #[error("Spansh {endpoint}: {message}")]
    Api { endpoint: String, message: String },

    /// The response was not the JSON we expected. `detail` names the failing field path and
    /// quotes the start of the body.
    #[error("unexpected Spansh response from {url}: {detail}")]
    Decode { url: String, detail: String },

    /// A planner job was still running when the deadline passed.
    #[error("Spansh job {job} did not finish within {}s", waited.as_secs())]
    JobTimeout { job: String, waited: Duration },

    /// A planner job disappeared (HTTP 404) and resubmitting did not help.
    #[error("Spansh job {job} expired before its result could be fetched")]
    JobExpired { job: String },

    /// The call was cancelled through its [`CancellationToken`](crate::CancellationToken).
    #[error("Spansh request cancelled")]
    Cancelled,

    /// The request is invalid; nothing was sent.
    #[error("invalid Spansh request: {0}")]
    InvalidRequest(String),

    /// A journal `Loadout` could not be turned into a ship profile.
    #[error("ship loadout: {0}")]
    Loadout(#[from] crate::ship::LoadoutError),
}

impl Error {
    /// HTTP status, when the error came from one.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Http { status, .. } => Some(*status),
            Error::NotFound { .. } | Error::JobExpired { .. } => Some(404),
            Error::Transport { source, .. } => source.status().map(|s| s.as_u16()),
            _ => None,
        }
    }

    /// Spansh's own error text (`{"error": "…"}`), when it sent one.
    pub fn spansh_message(&self) -> Option<&str> {
        match self {
            Error::Api { message, .. } | Error::Http { message, .. } => Some(message),
            _ => None,
        }
    }

    /// Worth retrying later (network trouble, timeouts, HTTP 429/5xx). Bad input is not.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Transport { .. } | Error::Timeout { .. } | Error::JobTimeout { .. } => true,
            Error::Http { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}
