use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::sync::{Mutex, mpsc};

use super::{PingProbeOptions, PingResult, PingTarget, probe_ping_target, render_ping_result};

pub(super) fn probe_ping_worker(
    queue: &Mutex<VecDeque<PingTarget>>,
    sender: mpsc::SyncSender<PingResult>,
    options: &PingProbeOptions,
) {
    loop {
        let target = queue.lock().ok().and_then(|mut queue| queue.pop_front());
        let Some(target) = target else {
            return;
        };
        let result = probe_ping_target(target, options);
        if sender.send(result).is_err() {
            return;
        }
    }
}

pub(super) fn collect_ping_results(
    receiver: &mpsc::Receiver<PingResult>,
    target_count: usize,
    json: bool,
) -> Result<Vec<PingResult>> {
    let mut results = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        let result = receiver
            .recv()
            .context("OpenAI application ping worker stopped unexpectedly")?;
        if !json {
            render_ping_result(&result)?;
        }
        results.push(result);
    }
    Ok(results)
}
