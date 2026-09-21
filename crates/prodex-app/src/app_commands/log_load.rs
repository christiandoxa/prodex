use super::super::log_tui::contains_ignore_ascii_case;
use super::log_transcript::TranscriptEvent;
use std::collections::BTreeMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub(crate) struct LogLoadObservation {
    pub(crate) event: TranscriptEvent,
    pub(crate) event_name: String,
    pub(crate) fields: BTreeMap<String, String>,
    pub(crate) run_id: Option<String>,
}

pub(crate) fn is_routine_load_event(event_name: &str) -> bool {
    matches!(
        event_name,
        "profile_inflight_saturated"
            | "runtime_proxy_active_limit_reached"
            | "runtime_proxy_lane_limit_reached"
    )
}

#[derive(Debug, Clone)]
pub(crate) struct LogLoadAggregate {
    pub(crate) event: TranscriptEvent,
    pub(crate) key: String,
    pub(crate) occurrences: usize,
    pub(crate) unique_runs: Vec<String>,
    pub(crate) run_count_overflow: bool,
    pub(crate) last_seen: Instant,
}

impl LogLoadAggregate {
    pub(crate) fn new(
        event: TranscriptEvent,
        key: String,
        run_id: Option<String>,
        now: Instant,
    ) -> Self {
        let mut aggregate = Self {
            event,
            key,
            occurrences: 1,
            unique_runs: Vec::new(),
            run_count_overflow: false,
            last_seen: now,
        };
        aggregate.add_run(run_id);
        aggregate
    }

    pub(crate) fn observe(&mut self, event: TranscriptEvent, run_id: Option<String>, now: Instant) {
        self.event = event;
        self.occurrences = self.occurrences.saturating_add(1);
        self.last_seen = now;
        self.add_run(run_id);
    }

    pub(crate) fn as_transcript(&self) -> TranscriptEvent {
        let run_count = if self.run_count_overflow {
            format!("{}+", self.unique_runs.len())
        } else {
            self.unique_runs.len().to_string()
        };
        let mut event = self.event.clone();
        event.text = format!(
            "{} · ×{} · {} runs",
            event.text, self.occurrences, run_count
        );
        event
    }

    fn add_run(&mut self, run_id: Option<String>) {
        const MAX_UNIQUE_RUNS: usize = 256;
        let Some(run_id) = run_id else {
            return;
        };
        if self.unique_runs.iter().any(|current| current == &run_id) {
            return;
        }
        if self.unique_runs.len() >= MAX_UNIQUE_RUNS {
            self.run_count_overflow = true;
            return;
        }
        self.unique_runs.push(run_id);
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let event = self.as_transcript();
        contains_ignore_ascii_case(
            &format!("{} {} {}", event.timestamp, event.source, event.text),
            query,
        ) || self
            .unique_runs
            .iter()
            .any(|run| contains_ignore_ascii_case(run, query))
    }
}
