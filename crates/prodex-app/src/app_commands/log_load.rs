use super::super::log_tui::contains_ignore_ascii_case;
use super::log_transcript::TranscriptEvent;
use prodex_mojo_core::log_load::{LogLoadAggregateInput, LogLoadAggregatePlan};
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
    prodex_mojo_core::log_load::is_routine_event(event_name)
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
    pub(crate) fn plan_observation(
        previous: Option<&Self>,
        event_name: &str,
        key: &str,
        run_id: Option<&str>,
        now: Instant,
    ) -> LogLoadAggregatePlan {
        let elapsed_ns = previous.map_or(0, |aggregate| {
            u64::try_from(
                now.saturating_duration_since(aggregate.last_seen)
                    .as_nanos(),
            )
            .unwrap_or(u64::MAX)
        });
        prodex_mojo_core::log_load::aggregate_update(LogLoadAggregateInput {
            event_name,
            previous_key: previous.map(|aggregate| aggregate.key.as_str()),
            observation_key: key,
            elapsed_ns,
            occurrences: previous.map_or(0, |aggregate| aggregate.occurrences as u64),
            unique_run_ids: previous.map_or(&[], |aggregate| aggregate.unique_runs.as_slice()),
            run_count_overflow: previous.is_some_and(|aggregate| aggregate.run_count_overflow),
            run_id,
        })
        .unwrap_or_else(|error| panic!("Mojo log-load aggregate plan failed: {error:?}"))
    }

    pub(crate) fn from_plan(
        event: TranscriptEvent,
        key: String,
        run_id: Option<String>,
        now: Instant,
        plan: LogLoadAggregatePlan,
    ) -> Self {
        assert!(!plan.routine && !plan.coalesce);
        let mut aggregate = Self {
            event,
            key,
            occurrences: usize::try_from(plan.occurrences)
                .expect("Mojo log-load occurrence count does not fit usize"),
            unique_runs: Vec::new(),
            run_count_overflow: plan.run_count_overflow,
            last_seen: now,
        };
        if plan.append_run {
            aggregate
                .unique_runs
                .push(run_id.expect("Mojo requested an absent log-load run ID"));
        }
        aggregate
    }

    pub(crate) fn apply_plan(
        &mut self,
        event: TranscriptEvent,
        key: String,
        run_id: Option<String>,
        now: Instant,
        plan: LogLoadAggregatePlan,
    ) {
        assert!(!plan.routine && plan.coalesce);
        self.event = event;
        self.key = key;
        self.occurrences = usize::try_from(plan.occurrences)
            .expect("Mojo log-load occurrence count does not fit usize");
        self.last_seen = now;
        self.run_count_overflow = plan.run_count_overflow;
        if plan.append_run {
            self.unique_runs
                .push(run_id.expect("Mojo requested an absent log-load run ID"));
        }
    }

    pub(crate) fn as_transcript(&self) -> TranscriptEvent {
        let summary = prodex_mojo_core::log_load::aggregate_summary(
            self.occurrences,
            self.unique_runs.len(),
            self.run_count_overflow,
        )
        .unwrap_or_else(|error| panic!("Mojo log-load summary failed: {error:?}"));
        let mut event = self.event.clone();
        event.text.push_str(&summary);
        event
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        let event = self.as_transcript();
        prodex_mojo_core::log::log_virtual_fields_contains(
            &[&event.timestamp, &event.source, &event.text],
            query,
        )
        .expect("compiled Mojo log-load virtual-field search returned invalid output")
            || self
                .unique_runs
                .iter()
                .any(|run| contains_ignore_ascii_case(run, query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_log_load_search_preserves_cross_field_and_run_id_matching() {
        let aggregate = LogLoadAggregate {
            event: TranscriptEvent {
                timestamp: "2026-10-21".to_owned(),
                source: "main".to_owned(),
                text: "payload".to_owned(),
            },
            key: "payload-key".to_owned(),
            occurrences: 1,
            unique_runs: vec!["run-A".to_owned()],
            run_count_overflow: false,
            last_seen: Instant::now(),
        };
        assert!(aggregate.matches("2026-10-21 MAIN"));
        assert!(aggregate.matches("main PAYLOAD"));
        assert!(!aggregate.matches("main  payload"));
        assert!(aggregate.matches("RUN-A"));
        assert!(!aggregate.matches("payload run-A"));
        assert!(aggregate.matches(""));
    }
}
