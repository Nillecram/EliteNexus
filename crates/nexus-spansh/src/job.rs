//! Route-planner jobs: submit → poll `/api/results/{job}` with exponential backoff → typed route.
//!
//! Spansh answers a planner submit with `202 {"job": "…", "status": "queued"}`. Polling returns
//! `202` while the job is queued/started, `200` with `status: "ok"` and the `result` when done,
//! and `404` once the job has expired (we then resubmit once, like the website does).

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{decode, decode_value, error_message, snippet};
use crate::util::WEBSITE;
use crate::{CancellationToken, Error, Result, Spansh};

/// Overall deadline for a planner job unless the planner or [`JobOptions::timeout`] says
/// otherwise.
pub const DEFAULT_JOB_TIMEOUT: Duration = Duration::from_secs(120);

/// Progress callback; called from the polling task (keep it cheap, e.g. store into a mutex or
/// send on a channel).
pub type ProgressFn = Arc<dyn Fn(&JobProgress) + Send + Sync>;

/// How route jobs are polled. Set per client ([`SpanshBuilder::job_options`](crate::SpanshBuilder::job_options))
/// or per call site ([`Spansh::with_job_options`]).
#[derive(Clone)]
pub struct JobOptions {
    /// Delay before the first poll (default 500 ms); doubles after every poll …
    pub first_poll: Duration,
    /// … up to this interval (default 8 s).
    pub max_poll_interval: Duration,
    /// Overall deadline; `None` uses the planner's default (120 s, galaxy plotter `max_time` + 60 s).
    pub timeout: Option<Duration>,
    /// Resubmit the original request once if the job expires (HTTP 404) while polling.
    pub resubmit_expired: bool,
    /// Cancels polling and in-flight requests with [`Error::Cancelled`].
    pub cancel: Option<CancellationToken>,
    /// Called after submission and on every poll.
    pub progress: Option<ProgressFn>,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            first_poll: Duration::from_millis(500),
            max_poll_interval: Duration::from_secs(8),
            timeout: None,
            resubmit_expired: true,
            cancel: None,
            progress: None,
        }
    }
}

impl fmt::Debug for JobOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobOptions")
            .field("first_poll", &self.first_poll)
            .field("max_poll_interval", &self.max_poll_interval)
            .field("timeout", &self.timeout)
            .field("resubmit_expired", &self.resubmit_expired)
            .field(
                "cancel",
                &self.cancel.as_ref().map(CancellationToken::is_cancelled),
            )
            .field("progress", &self.progress.is_some())
            .finish()
    }
}

impl JobOptions {
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = Some(t);
        self
    }

    pub fn poll_interval(mut self, first: Duration, max: Duration) -> Self {
        self.first_poll = first;
        self.max_poll_interval = max.max(first);
        self
    }

    pub fn cancel(mut self, token: CancellationToken) -> Self {
        self.cancel = Some(token);
        self
    }

    pub fn on_progress(mut self, f: impl Fn(&JobProgress) + Send + Sync + 'static) -> Self {
        self.progress = Some(Arc::new(f));
        self
    }
}

/// Where a job is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Submitted,
    Queued,
    Started,
    Completed,
    Other(String),
}

impl JobState {
    fn parse(state: Option<&str>) -> Self {
        match state.unwrap_or("queued") {
            "queued" => JobState::Queued,
            "started" | "running" => JobState::Started,
            "completed" | "ok" => JobState::Completed,
            other => JobState::Other(other.to_owned()),
        }
    }
}

/// Snapshot passed to [`JobOptions::progress`].
#[derive(Debug, Clone, PartialEq)]
pub struct JobProgress {
    pub job: String,
    pub state: JobState,
    pub polls: u32,
    pub elapsed: Duration,
}

/// What Spansh returns from a planner submit and from `/api/results/{job}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobEnvelope {
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub job: Option<String>,
    /// `queued`, `ok` or `error`.
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub status: Option<String>,
    /// `queued`, `started`, `completed`.
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub state: Option<String>,
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub error: Option<String>,
    /// UTC without suffix (`2026-10-12T09:17:41`), see [`parse_timestamp`](crate::parse_timestamp).
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "crate::de::opt_string")]
    pub updated_at: Option<String>,
    /// Echo of the submitted parameters (values as strings).
    pub parameters: Value,
    /// Planner-specific result; decode with [`JobEnvelope::route`].
    pub result: Option<Value>,
}

impl JobEnvelope {
    /// The failure message when Spansh reports one.
    pub fn failure(&self) -> Option<String> {
        let errored =
            self.status.as_deref() == Some("error") || self.state.as_deref() == Some("failed");
        match (&self.error, errored) {
            (Some(msg), _) if !msg.trim().is_empty() => Some(msg.clone()),
            (_, true) => Some("planner reported an error".to_owned()),
            _ => None,
        }
    }

    /// Finished (successfully or not)?
    pub fn is_finished(&self) -> bool {
        self.result.is_some()
            || self.state.as_deref() == Some("completed")
            || self.failure().is_some()
    }

    /// Decode the `result` into a planner's route type.
    pub fn route<R: DeserializeOwned>(&self) -> Result<R> {
        let url = format!("results/{}", self.job.as_deref().unwrap_or("?"));
        match &self.result {
            Some(v) if !v.is_null() => decode_value(&url, "result", v.clone()),
            _ => Err(Error::Api {
                endpoint: url,
                message: "job has no result".to_owned(),
            }),
        }
    }
}

/// Request body of a planner.
#[derive(Debug, Clone, PartialEq)]
pub enum PlannerBody {
    /// `application/x-www-form-urlencoded`; arrays as repeated keys.
    Form(Vec<(&'static str, String)>),
    /// JSON (engineer plotter).
    Json(Value),
}

/// A Spansh route planner request. Implemented by every `*RouteRequest` in this crate; implement
/// it yourself to drive a planner this crate does not wrap yet.
pub trait RoutePlanner {
    /// Typed `result` of the job.
    type Route: DeserializeOwned;
    /// API path below `/api`, e.g. `route` or `generic/route`.
    fn endpoint(&self) -> &'static str;
    /// Website section for result links, e.g. `plotter` → `https://spansh.co.uk/plotter/results/{job}`.
    fn web_path(&self) -> &'static str;
    fn body(&self) -> PlannerBody;
    /// Checked before anything is sent.
    fn validate(&self) -> Result<()> {
        Ok(())
    }
    /// Default overall deadline.
    fn timeout(&self) -> Duration {
        DEFAULT_JOB_TIMEOUT
    }
    /// How long the job is expected to run when that is known up front (the galaxy plotter
    /// uses its whole `max_time`). Polling then visits ¼, ½, ¾ and just past this time instead
    /// of every few seconds.
    fn expected_duration(&self) -> Option<Duration> {
        None
    }
}

/// Delay until the next poll: exponential backoff capped at `max_interval`; with an expected
/// run time, skip ahead to the next quarter checkpoint while the job must still be running.
fn next_delay(
    prev: Duration,
    elapsed: Duration,
    max_interval: Duration,
    expected: Option<Duration>,
) -> Duration {
    let backoff = (prev * 2).min(max_interval);
    let Some(expected) = expected else {
        return backoff;
    };
    let next = elapsed + backoff;
    [
        expected / 4,
        expected / 2,
        expected * 3 / 4,
        expected + Duration::from_secs(1),
    ]
    .into_iter()
    .find(|checkpoint| *checkpoint > next)
    .map_or(backoff, |checkpoint| checkpoint - elapsed)
}

/// A finished planner job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plotted<R> {
    pub job: String,
    pub route: R,
    /// Spansh's echo of the submitted parameters.
    pub parameters: Value,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Result polls made (excluding the submit).
    pub polls: u32,
    /// Wall time from submit to result.
    pub elapsed: Duration,
    /// The same result on the Spansh website.
    pub web_url: String,
}

enum Poll {
    Running(JobEnvelope),
    Done(JobEnvelope),
    Expired,
}

impl Spansh {
    /// Run any planner: submit, poll until done (see [`JobOptions`]), decode the result.
    pub async fn plot<P: RoutePlanner + ?Sized>(&self, req: &P) -> Result<Plotted<P::Route>> {
        req.validate()?;
        self.check_cancelled()?;
        let limit = self.job.timeout.unwrap_or_else(|| req.timeout());
        let expected = req.expected_duration();
        let max_interval = self.job.max_poll_interval.max(self.job.first_poll);
        let started = Instant::now();
        let mut polls = 0u32;
        let mut resubmitted = false;
        let mut transient_failures = 0u32;
        let mut delay = self.job.first_poll;

        let mut env = self.submit(req).await?;
        let mut job = env.job.clone().unwrap_or_default();
        self.report(&job, JobState::Submitted, polls, started);
        loop {
            if env.is_finished() {
                self.report(&job, JobState::Completed, polls, started);
                return finish(req.web_path(), env, polls, started);
            }
            if job.is_empty() {
                return Err(Error::Decode {
                    url: self.url(req.endpoint()),
                    detail: "submit response carries neither a job id nor a result".to_owned(),
                });
            }
            let elapsed = started.elapsed();
            if elapsed >= limit {
                return Err(Error::JobTimeout {
                    job,
                    waited: elapsed,
                });
            }
            self.pause(delay.min(limit - elapsed)).await?;
            polls += 1;
            match self.poll_job(&job).await {
                Ok(Poll::Done(done)) => {
                    env = done;
                    continue;
                }
                Ok(Poll::Running(running)) => {
                    transient_failures = 0;
                    self.report(
                        &job,
                        JobState::parse(running.state.as_deref()),
                        polls,
                        started,
                    );
                }
                Ok(Poll::Expired) if self.job.resubmit_expired && !resubmitted => {
                    tracing::info!(%job, "Spansh job expired while polling; resubmitting");
                    resubmitted = true;
                    env = self.submit(req).await?;
                    job = env.job.clone().unwrap_or_default();
                    delay = self.job.first_poll;
                    self.report(&job, JobState::Submitted, polls, started);
                    continue;
                }
                Ok(Poll::Expired) => return Err(Error::JobExpired { job }),
                Err(e) if e.is_transient() && transient_failures < 3 => {
                    transient_failures += 1;
                    tracing::warn!(%job, error = %e, "transient error polling Spansh job; will retry");
                }
                Err(e) => return Err(e),
            }
            delay = next_delay(delay, started.elapsed(), max_interval, expected);
        }
    }

    /// One look at a job (e.g. to resume a route after a restart). Unknown/expired jobs give
    /// [`Error::NotFound`].
    pub async fn job_status(&self, job: &str) -> Result<JobEnvelope> {
        match self.poll_job(job).await? {
            Poll::Running(env) | Poll::Done(env) => Ok(env),
            Poll::Expired => Err(Error::NotFound {
                what: format!("job {job}"),
            }),
        }
    }

    async fn submit<P: RoutePlanner + ?Sized>(&self, req: &P) -> Result<JobEnvelope> {
        let endpoint = req.endpoint();
        let (url, reply) = match req.body() {
            PlannerBody::Form(form) => self.post_form_raw(endpoint, &form).await?,
            PlannerBody::Json(body) => self.post_json_raw(endpoint, &body).await?,
        };
        let env = serde_json::from_slice::<JobEnvelope>(&reply.body).ok();
        if !(200..300).contains(&reply.status) {
            let message = env
                .as_ref()
                .and_then(JobEnvelope::failure)
                .unwrap_or_else(|| error_message(&reply.body));
            let rejected =
                (400..500).contains(&reply.status) && !matches!(reply.status, 404 | 408 | 429);
            return Err(if rejected && env.is_some() {
                Error::Api {
                    endpoint: endpoint.to_owned(),
                    message,
                }
            } else {
                Error::Http {
                    status: reply.status,
                    url,
                    message,
                }
            });
        }
        let env = env.ok_or_else(|| Error::Decode {
            url: url.clone(),
            detail: format!("not a job envelope; body starts: {}", snippet(&reply.body)),
        })?;
        if let Some(message) = env.failure() {
            return Err(Error::Api {
                endpoint: endpoint.to_owned(),
                message,
            });
        }
        tracing::debug!(
            endpoint,
            job = env.job.as_deref().unwrap_or(""),
            "Spansh job submitted"
        );
        Ok(env)
    }

    async fn poll_job(&self, job: &str) -> Result<Poll> {
        // Job ids are UUIDs; never let one reshape the request path.
        if job.is_empty() || !job.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(Error::Decode {
                url: self.url("results"),
                detail: format!("unexpected job id {job:?}"),
            });
        }
        let (url, reply) = self.get_raw(&format!("results/{job}"), &[]).await?;
        match reply.status {
            404 => Ok(Poll::Expired),
            // Still queued/started; the body is informational only.
            202 => Ok(Poll::Running(
                serde_json::from_slice(&reply.body).unwrap_or_default(),
            )),
            200..=299 => {
                let env: JobEnvelope = decode(&url, &reply.body)?;
                if let Some(message) = env.failure() {
                    return Err(Error::Api {
                        endpoint: format!("job {job}"),
                        message,
                    });
                }
                Ok(if env.is_finished() {
                    Poll::Done(env)
                } else {
                    Poll::Running(env)
                })
            }
            status => Err(Error::Http {
                status,
                url,
                message: error_message(&reply.body),
            }),
        }
    }

    fn report(&self, job: &str, state: JobState, polls: u32, started: Instant) {
        if let Some(progress) = &self.job.progress {
            progress(&JobProgress {
                job: job.to_owned(),
                state,
                polls,
                elapsed: started.elapsed(),
            });
        }
    }
}

fn finish<R: DeserializeOwned>(
    web_path: &str,
    env: JobEnvelope,
    polls: u32,
    started: Instant,
) -> Result<Plotted<R>> {
    let route = env.route()?;
    let job = env.job.unwrap_or_default();
    Ok(Plotted {
        web_url: format!("{WEBSITE}/{web_path}/results/{job}"),
        job,
        route,
        parameters: env.parameters,
        created_at: env.created_at,
        updated_at: env.updated_at,
        polls,
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_states() {
        let queued: JobEnvelope =
            serde_json::from_str(r#"{"job":"ABC","status":"queued"}"#).expect("parse");
        assert!(!queued.is_finished());
        assert_eq!(queued.failure(), None);
        let failed: JobEnvelope =
            serde_json::from_str(r#"{"status":"error","error":"Could not find starting system"}"#)
                .expect("parse");
        assert_eq!(
            failed.failure().as_deref(),
            Some("Could not find starting system")
        );
        let done: JobEnvelope = serde_json::from_str(
            r#"{"job":"ABC","state":"completed","status":"ok","result":[1,2]}"#,
        )
        .expect("parse");
        assert!(done.is_finished());
        assert_eq!(done.route::<Vec<u8>>().expect("route"), vec![1, 2]);
        let empty: JobEnvelope = serde_json::from_str(
            r#"{"job":"ABC","state":"completed","status":"ok","result":null}"#,
        )
        .expect("parse");
        assert!(empty.is_finished());
        assert!(empty.route::<Vec<u8>>().is_err());
    }

    /// Poll times (s) for a job that never finishes, up to `until`.
    fn schedule(first: f64, max: f64, expected: Option<f64>, until: f64) -> Vec<f64> {
        let (mut t, mut delay, mut out) = (0.0, Duration::from_secs_f64(first), Vec::new());
        loop {
            t += delay.as_secs_f64();
            if t > until {
                return out;
            }
            out.push((t * 10.0).round() / 10.0);
            delay = next_delay(
                delay,
                Duration::from_secs_f64(t),
                Duration::from_secs_f64(max),
                expected.map(Duration::from_secs_f64),
            );
        }
    }

    #[test]
    fn poll_schedule() {
        // Plain exponential backoff.
        assert_eq!(
            schedule(0.5, 8.0, None, 40.0),
            [0.5, 1.5, 3.5, 7.5, 15.5, 23.5, 31.5, 39.5]
        );
        // Galaxy plotter with max_time 60 s: quarter checkpoints, then backoff past the end.
        assert_eq!(
            schedule(0.5, 8.0, Some(60.0), 80.0),
            [0.5, 15.0, 30.0, 45.0, 61.0, 69.0, 77.0]
        );
        // Short expectations: straight to just past the end, then backoff.
        assert_eq!(schedule(0.5, 8.0, Some(2.0), 10.0), [0.5, 3.0, 8.0]);
    }

    #[test]
    fn job_state_parse() {
        assert_eq!(JobState::parse(Some("started")), JobState::Started);
        assert_eq!(JobState::parse(None), JobState::Queued);
        assert_eq!(
            JobState::parse(Some("weird")),
            JobState::Other("weird".into())
        );
    }
}
