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
    last_output_tokens: Option<u64>,
    active: bool,
    last_completed_rate: Option<(Instant, f64)>,
}

/// Tracks authoritative output-token deltas for active log streams.
///
/// The viewer only renders this value when at least two monotonic samples span the minimum
/// interval. Final-only usage records therefore remain a completion metric, not a fabricated
/// live rate.
#[derive(Debug, Default, Clone)]
pub(super) struct OutputThroughput {
    streams: BTreeMap<OutputThroughputKey, OutputThroughputStream>,
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
        let stream = self.stream(&key);
        if stream
            .last_output_tokens
            .is_some_and(|previous| event.output_tokens < previous)
        {
            stream.samples.clear();
        }
        if let Some(previous) = stream.last_output_tokens
            && let Some(delta) = event.output_tokens.checked_sub(previous)
            && delta > 0
        {
            stream.samples.push_back((observed_at, delta));
            stream.active = true;
        }
        stream.last_output_tokens = Some(event.output_tokens);
        prune_output_throughput_samples(stream, observed_at);
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
        let stream = self.stream(&key);
        stream.samples.push_back((observed_at, output_tokens));
        stream.active = true;
        prune_output_throughput_samples(stream, observed_at);
    }

    pub(super) fn finish(&mut self, log_path: &Path, event: &InfoTokenUsageEvent) {
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        if let Some(stream) = self.streams.get_mut(&key) {
            stream.active = false;
            let rate = event
                .output_tokens_per_second
                .filter(|rate| rate.is_finite() && *rate >= 0.0)
                .or_else(|| output_throughput_stream_rate(stream));
            if let Some(rate) = rate {
                stream.last_completed_rate = Some((Instant::now(), rate));
            }
        }
    }

    pub(super) fn active_rate(&mut self, now: Instant) -> Option<f64> {
        let mut total = 0.0;
        let mut found = false;
        for stream in self.streams.values_mut().filter(|stream| stream.active) {
            prune_output_throughput_samples(stream, now);
            if let Some(rate) = output_throughput_stream_rate(stream) {
                total += rate;
                found = true;
            }
        }
        (found && total.is_finite() && total >= 0.0).then_some(total)
    }

    pub(super) fn display_rate(&mut self, now: Instant) -> Option<f64> {
        if let Some(rate) = self.active_rate(now) {
            return Some(rate);
        }
        let mut total = 0.0;
        let mut found = false;
        for stream in self.streams.values_mut() {
            let Some((completed_at, rate)) = stream.last_completed_rate else {
                continue;
            };
            if now.saturating_duration_since(completed_at) <= Duration::from_secs(1)
                && rate.is_finite()
                && rate >= 0.0
            {
                total += rate;
                found = true;
            } else {
                stream.last_completed_rate = None;
            }
        }
        (found && total.is_finite() && total >= 0.0).then_some(total)
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

fn prune_output_throughput_samples(stream: &mut OutputThroughputStream, now: Instant) {
    while stream.samples.front().is_some_and(|(sampled_at, _)| {
        now.saturating_duration_since(*sampled_at) > OUTPUT_THROUGHPUT_WINDOW
    }) {
        stream.samples.pop_front();
    }
}

fn output_throughput_stream_rate(stream: &OutputThroughputStream) -> Option<f64> {
    let (first_at, _) = stream.samples.front()?;
    let (last_at, _) = stream.samples.back()?;
    let elapsed = last_at.saturating_duration_since(*first_at);
    if elapsed < OUTPUT_THROUGHPUT_MIN_SAMPLE || elapsed.is_zero() {
        return None;
    }
    let tokens = stream
        .samples
        .iter()
        .map(|(_, tokens)| *tokens)
        .sum::<u64>();
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

    #[test]
    fn output_throughput_uses_recent_output_deltas_only() {
        let path = Path::new("/tmp/runtime-a.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_delta(path, "main", Some(7), 50, start);
        throughput.observe_delta(path, "main", Some(7), 50, start + Duration::from_secs(1));

        assert_eq!(
            throughput.active_rate(start + Duration::from_secs(1)),
            Some(100.0)
        );
        assert_eq!(format_output_tokens_per_second(Some(100.0)), "100 t/s");
    }

    #[test]
    fn output_throughput_stays_blank_until_a_valid_sample_window() {
        let path = Path::new("/tmp/runtime-b.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_delta(path, "main", Some(8), 1, start);
        assert_eq!(
            throughput.active_rate(start + Duration::from_millis(1)),
            None
        );
        assert_eq!(format_output_tokens_per_second(None), "— t/s");

        throughput.observe_delta(
            path,
            "main",
            Some(8),
            1,
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
        throughput.observe_delta(path, "main", Some(9), 10, start);
        throughput.observe_delta(path, "main", Some(9), 10, start);
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
    }
}
