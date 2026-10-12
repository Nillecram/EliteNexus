//! Tiny background-job helper: run a closure on a worker thread, poll the result each frame.
//! Keeps the UI thread free of database queries and network calls.

use crossbeam_channel::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub struct Job<T> {
    rx: Receiver<T>,
    started: Instant,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(ctx: &egui::Context, f: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(f());
            ctx.request_repaint();
        });
        Job { rx, started: Instant::now() }
    }

    /// The result once finished; afterwards the job is spent (keep the value yourself).
    pub fn poll(&mut self) -> Option<T> {
        match self.rx.try_recv() {
            Ok(v) => Some(v),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

impl<T: Send + 'static> Job<T> {
    /// Runs a future on the shared tokio runtime (network calls).
    pub fn spawn_async<F>(ctx: &egui::Context, rt: &tokio::runtime::Handle, fut: F) -> Self
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ctx.clone();
        rt.spawn(async move {
            let _ = tx.send(fut.await);
            ctx.request_repaint();
        });
        Job { rx, started: Instant::now() }
    }
}

/// A job slot plus its last result and how long it took.
pub struct Slot<T> {
    pub job: Option<Job<T>>,
    pub last: Option<T>,
    pub took: Option<Duration>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { job: None, last: None, took: None }
    }
}

impl<T> Slot<T> {
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }
}

impl<T: Send + 'static> Slot<T> {
    pub fn run(&mut self, ctx: &egui::Context, f: impl FnOnce() -> T + Send + 'static) {
        self.job = Some(Job::spawn(ctx, f));
    }

    pub fn run_async<F>(&mut self, ctx: &egui::Context, rt: &tokio::runtime::Handle, fut: F)
    where
        F: std::future::Future<Output = T> + Send + 'static,
    {
        self.job = Some(Job::spawn_async(ctx, rt, fut));
    }

    /// Call every frame; returns true when a new result just arrived.
    pub fn tick(&mut self) -> bool {
        if let Some(job) = self.job.as_mut()
            && let Some(v) = job.poll() {
                self.took = Some(job.elapsed());
                self.last = Some(v);
                self.job = None;
                return true;
            }
        false
    }
}
