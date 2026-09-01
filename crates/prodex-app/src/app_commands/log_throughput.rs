use crate::reports::InfoTokenUsageEvent;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const OUTPUT_THROUGHPUT_WINDOW: Duration = Duration::from_secs(2);
const OUTPUT_THROUGHPUT_MIN_SAMPLE: Duration = Duration::from_millis(250);
const OUTPUT_THROUGHPUT_MAX_STREAMS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OutputThroughputKey {
    log_path: PathBuf,
    profile: String,
    request: Option<u64>,
}

#[derive(Debug, Default, Clone)]
struct OutputThroughputStream {
    samples: VecDeque<(Instant, u64)>,
    active: bool,
    last_known_rate: Option<f64>,
    last_event_at: Option<Instant>,
}

/// Tracks authoritative output-token deltas for active log streams.
///
/// The viewer only renders this value when at least two monotonic samples span the minimum
/// interval. Final-only usage records therefore remain a completion metric, not a fabricated
/// live rate.
#[derive(Debug, Default, Clone)]
pub(crate) struct OutputThroughput {
    streams: BTreeMap<OutputThroughputKey, OutputThroughputStream>,
    last_known_rates: BTreeMap<PathBuf, f64>,
    last_event_path: Option<PathBuf>,
    historical_rate_timestamp: Option<String>,
}

impl OutputThroughput {
    pub(super) fn observe_token_usage(
        &mut self,
        log_path: &Path,
        event: &InfoTokenUsageEvent,
        observed_at: Instant,
    ) {
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        self.last_event_path = Some(log_path.to_path_buf());
        let rate = {
            let stream = self.stream(&key);
            stream.last_event_at = Some(observed_at);
            if stream
                .samples
                .back()
                .is_some_and(|(_, previous)| event.output_tokens < *previous)
            {
                stream.samples.clear();
                stream.active = false;
            }
            if stream
                .samples
                .back()
                .is_none_or(|(_, previous)| event.output_tokens > *previous)
            {
                stream.samples.push_back((observed_at, event.output_tokens));
                stream.active = event.output_tokens > 0;
            }
            prune_output_throughput_samples(stream, observed_at);
            output_throughput_stream_rate(stream)
        };
        if let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate >= 0.0) {
            self.record_rate(log_path, rate);
        }
    }

    #[cfg(test)]
    fn observe_delta(
        &mut self,
        log_path: &Path,
        profile: &str,
        request: Option<u64>,
        output_tokens: u64,
        observed_at: Instant,
    ) {
        if output_tokens == 0 {
            return;
        }
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: profile.to_string(),
            request,
        };
        self.last_event_path = Some(log_path.to_path_buf());
        let rate = {
            let stream = self.stream(&key);
            let cumulative = stream
                .samples
                .back()
                .map_or(Some(output_tokens), |(_, previous)| {
                    previous.checked_add(output_tokens)
                });
            let Some(cumulative) = cumulative else {
                return;
            };
            stream.last_event_at = Some(observed_at);
            stream.samples.push_back((observed_at, cumulative));
            stream.active = output_tokens > 0;
            prune_output_throughput_samples(stream, observed_at);
            output_throughput_stream_rate(stream)
        };
        if let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate >= 0.0) {
            self.record_rate(log_path, rate);
        }
    }

    pub(super) fn finish(&mut self, log_path: &Path, event: &InfoTokenUsageEvent) {
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        let rate = if let Some(stream) = self.streams.get_mut(&key) {
            stream.active = false;
            let rate = valid_output_rate(event)
                .or_else(|| output_throughput_stream_rate(stream))
                .or(stream.last_known_rate);
            if let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate >= 0.0) {
                stream.last_known_rate = Some(rate);
                Some(rate)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(rate) = rate {
            self.record_rate(log_path, rate);
            self.historical_rate_timestamp = None;
        }
    }

    pub(super) fn active_rate(&mut self, now: Instant) -> Option<f64> {
        let mut active = Vec::new();
        for (key, stream) in &mut self.streams {
            if !stream.active {
                continue;
            }
            prune_output_throughput_samples(stream, now);
            if let Some(rate) = output_throughput_stream_rate(stream) {
                active.push((key.log_path.clone(), stream.last_event_at, rate));
            }
        }
        let selected_path = active
            .iter()
            .max_by_key(|(_, last_event_at, _)| *last_event_at)
            .map(|(path, _, _)| path.clone());
        let selected_path = selected_path?;
        let rate = active
            .into_iter()
            .filter(|(path, _, _)| *path == selected_path)
            .map(|(_, _, rate)| rate)
            .sum::<f64>();
        if rate.is_finite() && rate >= 0.0 {
            self.record_rate(&selected_path, rate);
            Some(rate)
        } else {
            None
        }
    }

    pub(super) fn display_rate(&mut self, now: Instant) -> Option<f64> {
        if let Some(rate) = self.active_rate(now) {
            return Some(rate);
        }
        self.last_event_path
            .as_ref()
            .and_then(|path| self.last_known_rates.get(path).copied())
    }

    pub(super) fn observe_historical(&mut self, log_path: &Path, event: &InfoTokenUsageEvent) {
        let Some(rate) = valid_output_rate(event) else {
            return;
        };
        if self
            .historical_rate_timestamp
            .as_deref()
            .is_some_and(|timestamp| timestamp > event.timestamp.as_str())
        {
            return;
        }
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        let stream = self.stream(&key);
        stream.active = false;
        stream.last_known_rate = Some(rate);
        self.last_known_rates.insert(log_path.to_path_buf(), rate);
        self.last_event_path = Some(log_path.to_path_buf());
        self.historical_rate_timestamp = Some(event.timestamp.clone());
    }

    fn record_rate(&mut self, log_path: &Path, rate: f64) {
        self.last_known_rates.insert(log_path.to_path_buf(), rate);
        self.last_event_path = Some(log_path.to_path_buf());
        self.historical_rate_timestamp = None;
    }

    fn stream(&mut self, key: &OutputThroughputKey) -> &mut OutputThroughputStream {
        if !self.streams.contains_key(key)
            && self.streams.len() >= OUTPUT_THROUGHPUT_MAX_STREAMS
            && let Some(oldest) = self.streams.keys().next().cloned()
        {
            self.streams.remove(&oldest);
        }
        self.streams.entry(key.clone()).or_default()
    }
}

fn valid_output_rate(event: &InfoTokenUsageEvent) -> Option<f64> {
    event
        .output_tokens_per_second
        .filter(|rate| rate.is_finite() && *rate >= 0.0)
        .filter(|_| event.output_tokens > 0)
        .filter(|_| event.generation_ms.is_none_or(|duration| duration > 0))
}

fn prune_output_throughput_samples(stream: &mut OutputThroughputStream, now: Instant) {
    while stream.samples.front().is_some_and(|(sampled_at, _)| {
        now.saturating_duration_since(*sampled_at) > OUTPUT_THROUGHPUT_WINDOW
    }) {
        stream.samples.pop_front();
    }
}

fn output_throughput_stream_rate(stream: &OutputThroughputStream) -> Option<f64> {
    let (first_at, first_tokens) = stream.samples.front()?;
    let (last_at, last_tokens) = stream.samples.back()?;
    let elapsed = last_at.saturating_duration_since(*first_at);
    if elapsed < OUTPUT_THROUGHPUT_MIN_SAMPLE || elapsed.is_zero() {
        return None;
    }
    let tokens = last_tokens.checked_sub(*first_tokens)?;
    if tokens == 0 {
        return None;
    }
    let rate = tokens as f64 / elapsed.as_secs_f64();
    rate.is_finite().then_some(rate)
}

pub(super) fn format_output_tokens_per_second(rate: Option<f64>) -> String {
    let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate >= 0.0) else {
        return "— t/s".to_string();
    };
    if rate >= 100.0 {
        format!("{rate:.0} t/s")
    } else {
        let rounded = (rate * 10.0).round() / 10.0;
        if rounded.fract() == 0.0 {
            format!("{rounded:.0} t/s")
        } else {
            format!("{rounded:.1} t/s")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InfoTokenUsageEvent, OUTPUT_THROUGHPUT_MIN_SAMPLE, OutputThroughput,
        format_output_tokens_per_second,
    };
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn usage(profile: &str, request: Option<u64>, output_tokens: u64) -> InfoTokenUsageEvent {
        InfoTokenUsageEvent {
            profile: profile.to_string(),
            request,
            output_tokens,
            ..InfoTokenUsageEvent::default()
        }
    }

    #[test]
    fn output_throughput_uses_recent_output_deltas_only() {
        let path = Path::new("/tmp/runtime-a.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(7), 50), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(7), 150),
            start + Duration::from_secs(1),
        );

        assert_eq!(
            throughput.active_rate(start + Duration::from_secs(1)),
            Some(100.0)
        );
        assert_eq!(format_output_tokens_per_second(Some(100.0)), "100 t/s");
    }

    #[test]
    fn unrelated_runtime_logs_do_not_contribute_to_one_header_rate() {
        let first = Path::new("/tmp/runtime-process-a.log");
        let second = Path::new("/tmp/runtime-process-b.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_delta(first, "main", Some(1), 100, start);
        throughput.observe_delta(first, "main", Some(1), 100, start + Duration::from_secs(1));
        throughput.observe_delta(second, "main", Some(2), 50, start);
        throughput.observe_delta(second, "main", Some(2), 50, start + Duration::from_secs(1));

        assert_eq!(
            throughput.active_rate(start + Duration::from_secs(1)),
            Some(50.0)
        );
    }

    #[test]
    fn output_throughput_stays_blank_until_a_valid_sample_window() {
        let path = Path::new("/tmp/runtime-b.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(8), 1), start);
        assert_eq!(
            throughput.active_rate(start + Duration::from_millis(1)),
            None
        );
        assert_eq!(format_output_tokens_per_second(None), "— t/s");

        throughput.observe_token_usage(
            path,
            &usage("main", Some(8), 2),
            start + OUTPUT_THROUGHPUT_MIN_SAMPLE,
        );
        assert!(
            throughput
                .active_rate(start + OUTPUT_THROUGHPUT_MIN_SAMPLE)
                .is_some()
        );
    }

    #[test]
    fn output_throughput_rejects_zero_elapsed_and_counter_resets() {
        let path = Path::new("/tmp/runtime-c.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(9), 10), start);
        throughput.observe_token_usage(path, &usage("main", Some(9), 10), start);
        assert_eq!(throughput.active_rate(start), None);

        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(9),
            output_tokens: 100,
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &event, start);
        let reset_event = InfoTokenUsageEvent {
            output_tokens: 0,
            ..event
        };
        throughput.observe_token_usage(path, &reset_event, start + Duration::from_secs(1));
        assert_eq!(throughput.active_rate(start + Duration::from_secs(1)), None);
    }

    #[test]
    fn final_only_usage_retains_a_brief_average_without_claiming_active_generation() {
        let path = Path::new("/tmp/runtime-d.log");
        let now = Instant::now();
        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(10),
            output_tokens: 1_000,
            generation_ms: Some(10_000),
            output_tokens_per_second: Some(100.0),
            ..InfoTokenUsageEvent::default()
        };
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &event, now);
        throughput.finish(path, &event);

        assert_eq!(throughput.active_rate(now), None);
        assert_eq!(throughput.display_rate(Instant::now()), Some(100.0));
        assert_eq!(format_output_tokens_per_second(Some(f64::NAN)), "— t/s");
        assert_eq!(
            format_output_tokens_per_second(Some(f64::INFINITY)),
            "— t/s"
        );
        assert_eq!(format_output_tokens_per_second(Some(-1.0)), "— t/s");
        assert_eq!(format_output_tokens_per_second(Some(0.7)), "0.7 t/s");
    }

    #[test]
    fn completed_rate_remains_visible_until_a_new_valid_rate_replaces_it() {
        let path = Path::new("/tmp/runtime-sticky.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_delta(path, "main", Some(11), 100, start);
        throughput.observe_delta(path, "main", Some(11), 100, start + Duration::from_secs(1));
        assert_eq!(
            throughput.active_rate(start + Duration::from_secs(1)),
            Some(100.0)
        );

        let completed = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(11),
            output_tokens: 100,
            generation_ms: Some(1_500),
            output_tokens_per_second: Some(66.3),
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &completed, start + Duration::from_secs(1));
        throughput.finish(path, &completed);
        assert_eq!(
            throughput.display_rate(start + Duration::from_secs(60)),
            Some(66.3)
        );

        let warming = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(12),
            output_tokens: 1,
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &warming, start + Duration::from_secs(61));
        assert_eq!(
            throughput.display_rate(start + Duration::from_secs(61)),
            Some(66.3)
        );
    }

    #[test]
    fn later_turn_refreshes_the_header_rate() {
        let path = Path::new("/tmp/runtime-later-turn.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(1), 100), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(1), 200),
            start + Duration::from_secs(1),
        );
        throughput.finish(
            path,
            &InfoTokenUsageEvent {
                profile: "main".to_string(),
                request: Some(1),
                output_tokens: 200,
                output_tokens_per_second: Some(100.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        throughput.observe_token_usage(
            path,
            &usage("main", Some(2), 10),
            start + Duration::from_secs(2),
        );
        throughput.observe_token_usage(
            path,
            &usage("main", Some(2), 40),
            start + Duration::from_secs(3),
        );

        assert_eq!(
            throughput.display_rate(start + Duration::from_secs(3)),
            Some(30.0)
        );
    }

    #[test]
    fn profile_rotation_keeps_token_counters_separate() {
        let path = Path::new("/tmp/runtime-profile-rotation.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(3), 100), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(3), 200),
            start + Duration::from_secs(1),
        );
        throughput.finish(
            path,
            &InfoTokenUsageEvent {
                profile: "main".to_string(),
                request: Some(3),
                output_tokens: 200,
                output_tokens_per_second: Some(100.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        throughput.observe_token_usage(
            path,
            &usage("backup", Some(4), 5),
            start + Duration::from_secs(2),
        );
        throughput.observe_token_usage(
            path,
            &usage("backup", Some(4), 25),
            start + Duration::from_secs(3),
        );

        assert_eq!(
            throughput.active_rate(start + Duration::from_secs(3)),
            Some(20.0)
        );
    }

    #[test]
    fn invalid_final_duration_does_not_replace_a_rate() {
        let path = Path::new("/tmp/runtime-invalid-duration.log");
        let now = Instant::now();
        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(5),
            output_tokens: 50,
            generation_ms: Some(0),
            output_tokens_per_second: Some(50.0),
            ..InfoTokenUsageEvent::default()
        };
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &event, now);
        throughput.finish(path, &event);

        assert_eq!(throughput.display_rate(now), None);
    }

    #[test]
    fn overflow_delta_is_ignored_without_saturating_the_stream() {
        let path = Path::new("/tmp/runtime-overflow.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_delta(path, "main", Some(6), u64::MAX, start);
        throughput.observe_delta(path, "main", Some(6), 1, start + Duration::from_secs(1));

        let stream = throughput
            .streams
            .values()
            .next()
            .expect("stream should remain bounded");
        assert_eq!(stream.samples.len(), 1);
        assert_eq!(throughput.active_rate(start + Duration::from_secs(1)), None);
    }

    #[test]
    fn historical_rate_seeds_sticky_display_without_starting_live_sampling() {
        let path = Path::new("/tmp/runtime-history.log");
        let mut throughput = OutputThroughput::default();
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28 12:00:00".to_string(),
                profile: "main".to_string(),
                request: Some(13),
                output_tokens: 500,
                output_tokens_per_second: Some(66.3),
                ..InfoTokenUsageEvent::default()
            },
        );

        assert_eq!(throughput.display_rate(Instant::now()), Some(66.3));
    }
}
