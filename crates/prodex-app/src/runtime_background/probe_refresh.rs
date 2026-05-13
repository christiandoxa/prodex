#[cfg(test)]
use super::*;

mod apply;
mod attempt;
mod queue;
mod startup;
mod worker;

pub(crate) use apply::apply_runtime_profile_probe_result;
#[cfg(test)]
pub(crate) use attempt::{
    execute_runtime_probe_attempt_inline_for_test, execute_runtime_probe_attempt_queued_for_test,
};
#[allow(unused_imports)]
pub(crate) use queue::note_runtime_probe_refresh_progress;
#[cfg(test)]
pub(crate) use queue::{
    runtime_probe_refresh_nonlocal_upstream_for_test, runtime_probe_refresh_queue_active,
};
pub(crate) use queue::{
    runtime_probe_refresh_queue, runtime_probe_refresh_queue_backlog,
    runtime_probe_refresh_revision, schedule_runtime_probe_refresh,
};
#[allow(unused_imports)]
pub(crate) use startup::runtime_profiles_needing_startup_probe_refresh;
pub(crate) use startup::{run_runtime_probe_jobs_inline, schedule_runtime_startup_probe_warmup};
#[cfg(test)]
pub(crate) use worker::runtime_probe_refresh_take_next_job;

#[cfg(test)]
#[path = "../../tests/src/runtime_background/probe_refresh.rs"]
mod tests;
