use crate::reports::InfoTokenUsageEvent;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const OUTPUT_THROUGHPUT_WINDOW: Duration = Duration::from_secs(2);
const OUTPUT_THROUGHPUT_MIN_SAMPLE: Duration = Duration::from_millis(250);
const OUTPUT_THROUGHPUT_MAX_STREAMS: usize = 64;
const OUTPUT_THROUGHPUT_MAX_OBSERVATIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OutputThroughputKey {
    log_path: PathBuf,
    profile: String,
    request: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OutputThroughputObservation {
    timestamp: String,
    request: Option<u64>,
    profile: String,
    transport: String,
    source: String,
    output_tokens: u64,
    completion: bool,
}

#[derive(Debug, Default, Clone)]
struct OutputThroughputStream {
    samples: VecDeque<(Instant, u64, u64)>,
    active: bool,
    last_known_rate: Option<f64>,
    last_event_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OutputThroughputDisplay {
    Active(f64),
    Last(f64),
}

impl OutputThroughputDisplay {
    #[cfg(test)]
    pub(crate) fn rate(self) -> f64 {
        match self {
            Self::Active(rate) | Self::Last(rate) => rate,
        }
    }
}

/// Tracks authoritative output-token usage and generation timing for log streams.
///
/// Local receipt times never produce a rate. A numeric rate requires output tokens and the
/// producer's positive monotonic generation duration.
#[derive(Debug, Default, Clone)]
pub(crate) struct OutputThroughput {
    streams: BTreeMap<OutputThroughputKey, OutputThroughputStream>,
    last_known_rates: BTreeMap<OutputThroughputKey, f64>,
    last_event_key: Option<OutputThroughputKey>,
    last_event_keys: BTreeMap<String, OutputThroughputKey>,
    historical_rate_timestamp: Option<String>,
    historical_rate_timestamps: BTreeMap<String, String>,
    seen_observations: BTreeMap<OutputThroughputObservation, PathBuf>,
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
        let Some(generation_ms) = event.generation_ms.filter(|duration| *duration > 0) else {
            return;
        };
        if event.output_tokens == 0 {
            return;
        }
        let counter_reset = self
            .streams
            .get(&key)
            .and_then(|stream| stream.samples.back())
            .is_some_and(|(_, previous_tokens, previous_generation_ms)| {
                event.output_tokens < *previous_tokens || generation_ms < *previous_generation_ms
            });
        if counter_reset {
            self.last_known_rates.remove(&key);
        }
        if event.output_tokens > 0 {
            let observation = output_throughput_observation(event);
            if !counter_reset
                && self
                    .duplicate_observation_path(&observation, log_path)
                    .is_some()
            {
                return;
            }
            self.remember_observation(observation, log_path);
        }
        let rate = {
            let stream = self.stream(&key);
            stream.last_event_at = Some(observed_at);
            if counter_reset {
                stream.samples.clear();
                stream.active = false;
                stream.last_known_rate = None;
            }
            if stream
                .samples
                .back()
                .is_none_or(|(_, previous_tokens, _)| event.output_tokens > *previous_tokens)
            {
                stream
                    .samples
                    .push_back((observed_at, event.output_tokens, generation_ms));
            }
            prune_output_throughput_samples(stream, observed_at);
            output_throughput_stream_rate(stream)
        };
        if let Some(rate) = rate
            && let Some(stream) = self.streams.get_mut(&key)
        {
            stream.active = true;
            stream.last_known_rate = Some(rate);
        }
        if let Some(rate) = rate {
            self.record_rate(&key, rate);
        }
    }

    pub(super) fn finish(&mut self, log_path: &Path, event: &InfoTokenUsageEvent) {
        let observation = output_throughput_observation(event);
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        let duplicate_rate = self
            .duplicate_observation_path(&observation, log_path)
            .and_then(|path| {
                self.last_known_rates
                    .get(&OutputThroughputKey {
                        log_path: path.to_path_buf(),
                        profile: event.profile.clone(),
                        request: event.request,
                    })
                    .copied()
            });
        let keyed_rate = self.last_known_rates.get(&key).copied();
        let rate = {
            let stream = self.stream(&key);
            stream.active = false;
            let rate = duplicate_rate
                .or(stream.last_known_rate)
                .or(keyed_rate)
                .or_else(|| valid_output_rate(event));
            if let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate > 0.0) {
                stream.last_known_rate = Some(rate);
                Some(rate)
            } else {
                None
            }
        };
        if let Some(rate) = rate {
            self.record_rate(&key, rate);
        }
    }

    pub(super) fn active_profile(&self) -> Option<String> {
        self.streams
            .iter()
            .filter(|(_, stream)| {
                stream.active
                    && stream
                        .last_event_at
                        .is_some_and(|at| at.elapsed() <= OUTPUT_THROUGHPUT_WINDOW)
            })
            .max_by_key(|(_, stream)| stream.last_event_at)
            .map(|(key, _)| key.profile.clone())
    }

    pub(super) fn active_rate_for_profile(
        &mut self,
        now: Instant,
        preferred_profile: Option<&str>,
    ) -> Option<f64> {
        let mut active = Vec::new();
        for (key, stream) in &mut self.streams {
            if preferred_profile.is_some_and(|profile| profile != key.profile) {
                continue;
            }
            if !stream.active
                || !stream
                    .last_event_at
                    .is_some_and(|at| now.saturating_duration_since(at) <= OUTPUT_THROUGHPUT_WINDOW)
            {
                continue;
            }
            if let Some(rate) = stream.last_known_rate {
                active.push((key.clone(), stream.last_event_at, rate));
            }
        }
        let selected_key = active
            .into_iter()
            .max_by_key(|(_, last_event_at, _)| *last_event_at)
            .map(|(key, _, _)| key)?;
        let rate = self.streams.get(&selected_key)?.last_known_rate?;
        if rate.is_finite() && rate > 0.0 {
            self.record_rate(&selected_key, rate);
            Some(rate)
        } else {
            None
        }
    }

    #[cfg(test)]
    pub(super) fn display_rate_for_profile(
        &mut self,
        now: Instant,
        preferred_profile: Option<&str>,
    ) -> Option<f64> {
        self.display_for_profile(now, preferred_profile)
            .map(OutputThroughputDisplay::rate)
    }

    pub(super) fn display_for_profile(
        &mut self,
        now: Instant,
        preferred_profile: Option<&str>,
    ) -> Option<OutputThroughputDisplay> {
        if let Some(rate) = self.active_rate_for_profile(now, preferred_profile) {
            return Some(OutputThroughputDisplay::Active(rate));
        }
        let key = match preferred_profile {
            Some(profile) => self.last_event_keys.get(profile),
            None => self.last_event_key.as_ref(),
        };
        key.and_then(|key| self.last_known_rates.get(key).copied())
            .map(OutputThroughputDisplay::Last)
    }

    pub(super) fn observe_historical(&mut self, log_path: &Path, event: &InfoTokenUsageEvent) {
        let Some(rate) = valid_output_rate(event) else {
            return;
        };
        if self
            .historical_rate_timestamps
            .get(&event.profile)
            .is_some_and(|timestamp| timestamp > &event.timestamp)
        {
            return;
        }
        let key = OutputThroughputKey {
            log_path: log_path.to_path_buf(),
            profile: event.profile.clone(),
            request: event.request,
        };
        {
            let stream = self.stream(&key);
            stream.active = false;
            stream.last_known_rate = Some(rate);
        }
        let global_latest = self
            .historical_rate_timestamp
            .as_deref()
            .is_none_or(|timestamp| timestamp <= event.timestamp.as_str());
        self.remember_observation(output_throughput_observation(event), log_path);
        self.last_known_rates.insert(key.clone(), rate);
        self.last_event_keys
            .insert(event.profile.clone(), key.clone());
        if global_latest {
            self.last_event_key = Some(key);
            self.historical_rate_timestamp = Some(event.timestamp.clone());
        }
        self.historical_rate_timestamps
            .insert(event.profile.clone(), event.timestamp.clone());
    }

    fn record_rate(&mut self, key: &OutputThroughputKey, rate: f64) {
        self.last_known_rates.insert(key.clone(), rate);
        self.last_event_key = Some(key.clone());
        self.last_event_keys
            .insert(key.profile.clone(), key.clone());
        self.historical_rate_timestamp = None;
        self.historical_rate_timestamps.remove(&key.profile);
    }

    fn stream(&mut self, key: &OutputThroughputKey) -> &mut OutputThroughputStream {
        if !self.streams.contains_key(key)
            && self.streams.len() >= OUTPUT_THROUGHPUT_MAX_STREAMS
            && let Some(oldest) = self.streams.keys().next().cloned()
        {
            self.evict_stream(&oldest);
        }
        self.streams.entry(key.clone()).or_default()
    }

    fn evict_stream(&mut self, key: &OutputThroughputKey) {
        self.streams.remove(key);
        self.last_known_rates.remove(key);
        if self
            .last_event_keys
            .get(&key.profile)
            .is_some_and(|current| current == key)
        {
            self.last_event_keys.remove(&key.profile);
            self.historical_rate_timestamps.remove(&key.profile);
        }
        if self.last_event_key.as_ref() == Some(key) {
            self.repair_global_identity();
        }
    }

    fn repair_global_identity(&mut self) {
        if self.historical_rate_timestamp.is_some() {
            let replacement = self
                .historical_rate_timestamps
                .iter()
                .filter_map(|(profile, timestamp)| {
                    let key = self.last_event_keys.get(profile)?;
                    self.last_known_rates
                        .contains_key(key)
                        .then_some((timestamp, key))
                })
                .max_by(|(left, _), (right, _)| left.cmp(right))
                .map(|(timestamp, key)| (timestamp.clone(), key.clone()));
            if let Some((timestamp, key)) = replacement {
                self.last_event_key = Some(key);
                self.historical_rate_timestamp = Some(timestamp);
            } else {
                self.last_event_key = None;
                self.historical_rate_timestamp = None;
            }
            return;
        }

        self.last_event_key = self
            .last_event_keys
            .values()
            .filter(|key| self.last_known_rates.contains_key(*key))
            .filter(|key| self.streams.contains_key(*key))
            .max_by_key(|key| {
                self.streams
                    .get(*key)
                    .and_then(|stream| stream.last_event_at)
            })
            .cloned();
    }

    fn remember_observation(&mut self, observation: OutputThroughputObservation, log_path: &Path) {
        if self.seen_observations.contains_key(&observation) {
            return;
        }
        // ponytail: keep the replay guard bounded; drop one key after 256 observations.
        if self.seen_observations.len() >= OUTPUT_THROUGHPUT_MAX_OBSERVATIONS
            && let Some(oldest) = self.seen_observations.keys().next().cloned()
        {
            self.seen_observations.remove(&oldest);
        }
        self.seen_observations
            .insert(observation, log_path.to_path_buf());
    }

    fn duplicate_observation_path(
        &self,
        observation: &OutputThroughputObservation,
        log_path: &Path,
    ) -> Option<&Path> {
        self.seen_observations
            .get(observation)
            .filter(|previous_path| {
                previous_path.as_path() != log_path
                    && is_live_log_path(previous_path.as_path()) != is_live_log_path(log_path)
            })
            .map(|path| path.as_path())
    }
}

fn output_throughput_observation(event: &InfoTokenUsageEvent) -> OutputThroughputObservation {
    OutputThroughputObservation {
        timestamp: event.timestamp.clone(),
        request: event.request,
        profile: event.profile.clone(),
        transport: event.transport.clone(),
        source: event.source.clone(),
        output_tokens: event.output_tokens,
        completion: event.generation_ms.is_some(),
    }
}

fn is_live_log_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|path| path.starts_with("broker:") || path.starts_with("direct:"))
}

fn valid_output_rate(event: &InfoTokenUsageEvent) -> Option<f64> {
    let duration = event.generation_ms.filter(|duration| *duration > 0)?;
    if event.output_tokens == 0 {
        return None;
    }
    let rate = event.output_tokens as f64 * 1_000.0 / duration as f64;
    rate.is_finite().then_some(rate)
}

fn prune_output_throughput_samples(stream: &mut OutputThroughputStream, now: Instant) {
    while stream.samples.front().is_some_and(|(sampled_at, _, _)| {
        now.saturating_duration_since(*sampled_at) > OUTPUT_THROUGHPUT_WINDOW
    }) {
        stream.samples.pop_front();
    }
}

fn output_throughput_stream_rate(stream: &OutputThroughputStream) -> Option<f64> {
    let (_, first_tokens, first_generation_ms) = stream.samples.front()?;
    let (_, last_tokens, last_generation_ms) = stream.samples.back()?;
    let elapsed_ms = last_generation_ms.checked_sub(*first_generation_ms)?;
    let elapsed = Duration::from_millis(elapsed_ms);
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

#[cfg(test)]
mod tests {
    use super::{InfoTokenUsageEvent, OUTPUT_THROUGHPUT_MAX_STREAMS, OutputThroughput};
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn active_rate(throughput: &mut OutputThroughput, now: Instant) -> Option<f64> {
        throughput.active_rate_for_profile(now, None)
    }

    fn display_rate(throughput: &mut OutputThroughput, now: Instant) -> Option<f64> {
        throughput.display_rate_for_profile(now, None)
    }

    fn observe_sample(
        throughput: &mut OutputThroughput,
        path: &Path,
        profile: &str,
        request: Option<u64>,
        output_tokens: u64,
        generation_ms: u64,
        observed_at: Instant,
    ) {
        throughput.observe_token_usage(
            path,
            &InfoTokenUsageEvent {
                profile: profile.to_string(),
                request,
                output_tokens,
                generation_ms: Some(generation_ms),
                ..InfoTokenUsageEvent::default()
            },
            observed_at,
        );
    }

    #[test]
    fn unrelated_runtime_logs_do_not_contribute_to_one_header_rate() {
        let first = Path::new("/tmp/runtime-process-a.log");
        let second = Path::new("/tmp/runtime-process-b.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        observe_sample(&mut throughput, first, "main", Some(1), 100, 1_000, start);
        observe_sample(
            &mut throughput,
            first,
            "main",
            Some(1),
            200,
            3_000,
            start + Duration::from_secs(1),
        );
        observe_sample(&mut throughput, second, "main", Some(2), 50, 1_000, start);
        observe_sample(
            &mut throughput,
            second,
            "main",
            Some(2),
            100,
            2_000,
            start + Duration::from_secs(1),
        );

        assert_eq!(
            active_rate(&mut throughput, start + Duration::from_secs(1)),
            Some(50.0)
        );
    }

    #[test]
    fn completed_rate_remains_visible_until_a_new_valid_rate_replaces_it() {
        let path = Path::new("/tmp/runtime-sticky.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        observe_sample(&mut throughput, path, "main", Some(11), 100, 1_000, start);
        observe_sample(
            &mut throughput,
            path,
            "main",
            Some(11),
            200,
            2_000,
            start + Duration::from_secs(1),
        );
        assert_eq!(
            active_rate(&mut throughput, start + Duration::from_secs(1)),
            Some(100.0)
        );

        let completed = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(11),
            output_tokens: 100,
            generation_ms: Some(2_000),
            output_tokens_per_second: Some(50.0),
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &completed, start + Duration::from_secs(1));
        throughput.finish(path, &completed);
        assert_eq!(
            display_rate(&mut throughput, start + Duration::from_secs(60)),
            Some(50.0)
        );

        let warming = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(12),
            output_tokens: 1,
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &warming, start + Duration::from_secs(61));
        assert_eq!(
            display_rate(&mut throughput, start + Duration::from_secs(61)),
            Some(50.0)
        );
    }

    #[test]
    fn counter_reset_is_ignored_without_saturating_the_stream() {
        let path = Path::new("/tmp/runtime-overflow.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        observe_sample(
            &mut throughput,
            path,
            "main",
            Some(6),
            u64::MAX,
            1_000,
            start,
        );
        observe_sample(
            &mut throughput,
            path,
            "main",
            Some(6),
            1,
            2_000,
            start + Duration::from_secs(1),
        );

        let stream = throughput
            .streams
            .values()
            .next()
            .expect("stream should remain bounded");
        assert_eq!(stream.samples.len(), 1);
        assert!(active_rate(&mut throughput, start + Duration::from_secs(1)).is_none());
    }

    #[test]
    fn stream_churn_bounds_identity_maps_and_keeps_latest_samples() {
        let path = Path::new("/home/test-user/runtime-throughput-churn.log");
        let mut throughput = OutputThroughput::default();
        for index in 1_u64..64 {
            throughput.observe_historical(
                path,
                &InfoTokenUsageEvent {
                    timestamp: format!("2026-08-28T00:00:{index:03}Z"),
                    profile: format!("profile-{index:03}"),
                    request: Some(index),
                    output_tokens: index,
                    generation_ms: Some(1_000),
                    output_tokens_per_second: Some(index as f64),
                    ..InfoTokenUsageEvent::default()
                },
            );
        }
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28T00:00:100Z".to_string(),
                profile: "profile-000".to_string(),
                request: Some(100),
                output_tokens: 100,
                generation_ms: Some(1_000),
                output_tokens_per_second: Some(100.0),
                ..InfoTokenUsageEvent::default()
            },
        );
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28T00:00:090Z".to_string(),
                profile: "profile-064".to_string(),
                request: Some(64),
                output_tokens: 64,
                generation_ms: Some(1_000),
                output_tokens_per_second: Some(64.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        assert_eq!(throughput.streams.len(), OUTPUT_THROUGHPUT_MAX_STREAMS);
        assert_eq!(
            throughput.last_known_rates.len(),
            OUTPUT_THROUGHPUT_MAX_STREAMS
        );
        assert_eq!(
            throughput.last_event_keys.len(),
            OUTPUT_THROUGHPUT_MAX_STREAMS
        );
        assert_eq!(
            throughput.historical_rate_timestamps.len(),
            OUTPUT_THROUGHPUT_MAX_STREAMS
        );
        assert_eq!(throughput.seen_observations.len(), 65);
        assert!(!throughput.last_event_keys.contains_key("profile-000"));
        assert!(
            !throughput
                .historical_rate_timestamps
                .contains_key("profile-000")
        );
        assert!(
            throughput
                .last_known_rates
                .keys()
                .all(|key| throughput.streams.contains_key(key))
        );
        assert!(
            throughput
                .last_event_keys
                .values()
                .all(|key| throughput.streams.contains_key(key))
        );
        assert!(
            throughput
                .historical_rate_timestamps
                .keys()
                .all(|profile| throughput.last_event_keys.contains_key(profile))
        );
        assert_eq!(
            throughput.display_rate_for_profile(Instant::now(), Some("profile-000")),
            None
        );
        assert_eq!(
            throughput.display_rate_for_profile(Instant::now(), Some("profile-064")),
            Some(64.0)
        );
        assert_eq!(
            throughput.display_rate_for_profile(Instant::now(), None),
            Some(64.0)
        );
    }

    #[test]
    fn recent_authoritative_delta_wins_over_whole_generation_average() {
        let path = Path::new("/tmp/runtime-recent-authoritative.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();

        observe_sample(&mut throughput, path, "main", Some(17), 540, 9_800, start);
        observe_sample(
            &mut throughput,
            path,
            "main",
            Some(17),
            660,
            10_800,
            start + Duration::from_secs(1),
        );

        assert_eq!(
            throughput.active_rate_for_profile(start + Duration::from_secs(1), Some("main")),
            Some(120.0)
        );

        let completed = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(17),
            output_tokens: 660,
            generation_ms: Some(12_000),
            output_tokens_per_second: Some(55.0),
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &completed, start + Duration::from_secs(2));
        throughput.finish(path, &completed);
        assert_eq!(
            throughput.display_for_profile(start + Duration::from_secs(2), Some("main")),
            Some(super::OutputThroughputDisplay::Last(120.0))
        );
        assert_eq!(super::valid_output_rate(&completed), Some(55.0));
    }
}
