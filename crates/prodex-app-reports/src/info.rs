use prodex_quota::{
    MainWindowSnapshot, RuntimeQuotaWindowStatus, UsageResponse, UsageWindow, WindowPair,
    find_main_window, remaining_percent,
};
use prodex_runtime_state::RuntimeProfileUsageSnapshot as RuntimeProfileUsageSnapshotGeneric;
use prodex_shared_types::{
    InfoQuotaAggregate, InfoQuotaSource, InfoQuotaWindow, InfoRuntimeLoadSummary,
    InfoRuntimeQuotaObservation, InfoRunwayEstimate, RunProfileProbeReport,
};
use std::collections::BTreeMap;

pub type RuntimeProfileUsageSnapshot = RuntimeProfileUsageSnapshotGeneric<RuntimeQuotaWindowStatus>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoTokenUsageCounts {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageSummary {
    pub log_count: usize,
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
    pub by_profile: BTreeMap<String, InfoTokenUsageProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InfoTokenUsageProfile {
    pub event_count: usize,
    pub total: InfoTokenUsageCounts,
}

pub fn build_info_quota_aggregate(
    reports: &[RunProfileProbeReport],
    persisted_usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    now: i64,
    stale_grace_seconds: i64,
) -> InfoQuotaAggregate {
    let mut aggregate = InfoQuotaAggregate {
        quota_compatible_profiles: reports
            .iter()
            .filter(|report| report.auth.quota_compatible)
            .count(),
        live_profiles: 0,
        snapshot_profiles: 0,
        unavailable_profiles: 0,
        five_hour_pool_remaining: 0,
        weekly_pool_remaining: 0,
        earliest_five_hour_reset_at: None,
        earliest_weekly_reset_at: None,
    };

    for report in reports {
        if !report.auth.quota_compatible {
            continue;
        }

        let usage = match &report.result {
            Ok(usage) => Some((usage.clone(), InfoQuotaSource::LiveProbe)),
            Err(_) => persisted_usage_snapshots
                .get(&report.name)
                .filter(|snapshot| {
                    runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
                })
                .map(|snapshot| {
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        InfoQuotaSource::PersistedSnapshot,
                    )
                }),
        };

        let Some((usage, source)) = usage else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        let Some((five_hour, weekly)) = info_main_window_snapshots_at(&usage, now) else {
            aggregate.unavailable_profiles += 1;
            continue;
        };

        match source {
            InfoQuotaSource::LiveProbe => aggregate.live_profiles += 1,
            InfoQuotaSource::PersistedSnapshot => aggregate.snapshot_profiles += 1,
        }
        aggregate.five_hour_pool_remaining += five_hour.remaining_percent;
        aggregate.weekly_pool_remaining += weekly.remaining_percent;
        if five_hour.reset_at != i64::MAX {
            aggregate.earliest_five_hour_reset_at = Some(
                aggregate
                    .earliest_five_hour_reset_at
                    .map_or(five_hour.reset_at, |current| {
                        current.min(five_hour.reset_at)
                    }),
            );
        }
        if weekly.reset_at != i64::MAX {
            aggregate.earliest_weekly_reset_at = Some(
                aggregate
                    .earliest_weekly_reset_at
                    .map_or(weekly.reset_at, |current| current.min(weekly.reset_at)),
            );
        }
    }

    aggregate
}

pub fn info_main_window_snapshots_at(
    usage: &UsageResponse,
    now: i64,
) -> Option<(MainWindowSnapshot, MainWindowSnapshot)> {
    Some((
        required_main_window_snapshot_at(usage, "5h", now)?,
        required_main_window_snapshot_at(usage, "weekly", now)?,
    ))
}

pub fn required_main_window_snapshot_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<MainWindowSnapshot> {
    let window = find_main_window(usage.rate_limit.as_ref()?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(MainWindowSnapshot {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub fn usage_from_runtime_usage_snapshot(snapshot: &RuntimeProfileUsageSnapshot) -> UsageResponse {
    UsageResponse {
        email: None,
        plan_type: None,
        rate_limit: Some(WindowPair {
            primary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.five_hour_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.five_hour_reset_at != i64::MAX)
                    .then_some(snapshot.five_hour_reset_at),
                limit_window_seconds: Some(18_000),
            }),
            secondary_window: Some(UsageWindow {
                used_percent: Some((100 - snapshot.weekly_remaining_percent).clamp(0, 100)),
                reset_at: (snapshot.weekly_reset_at != i64::MAX)
                    .then_some(snapshot.weekly_reset_at),
                limit_window_seconds: Some(604_800),
            }),
        }),
        code_review_rate_limit: None,
        additional_rate_limits: Vec::new(),
    }
}

pub fn runtime_usage_snapshot_is_usable(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
    stale_grace_seconds: i64,
) -> bool {
    if runtime_profile_usage_snapshot_hold_active(snapshot, now) {
        return true;
    }
    if runtime_profile_usage_snapshot_hold_expired(snapshot, now) {
        return false;
    }
    now.saturating_sub(snapshot.checked_at) <= stale_grace_seconds
}

pub fn runtime_profile_usage_snapshot_hold_active(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at > now
    })
}

pub fn runtime_profile_usage_snapshot_hold_expired(
    snapshot: &RuntimeProfileUsageSnapshot,
    now: i64,
) -> bool {
    [
        (snapshot.five_hour_status, snapshot.five_hour_reset_at),
        (snapshot.weekly_status, snapshot.weekly_reset_at),
    ]
    .into_iter()
    .any(|(status, reset_at)| {
        matches!(status, RuntimeQuotaWindowStatus::Exhausted)
            && reset_at != i64::MAX
            && reset_at <= now
    })
}

pub fn collect_info_token_usage_summary_from_text(text: &str) -> InfoTokenUsageSummary {
    let mut summary = InfoTokenUsageSummary::default();
    for line in text.lines() {
        let Some((profile, usage)) = info_token_usage_from_line(line) else {
            continue;
        };
        summary.event_count += 1;
        summary.total.add(usage);
        let profile = summary.by_profile.entry(profile).or_default();
        profile.event_count += 1;
        profile.total.add(usage);
    }
    summary
}

impl InfoTokenUsageSummary {
    pub fn merge(&mut self, other: InfoTokenUsageSummary) {
        self.event_count += other.event_count;
        self.total.add(other.total);
        for (name, other_profile) in other.by_profile {
            let profile = self.by_profile.entry(name).or_default();
            profile.event_count += other_profile.event_count;
            profile.total.add(other_profile.total);
        }
    }
}

impl InfoTokenUsageCounts {
    fn add(&mut self, other: InfoTokenUsageCounts) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }
}

pub fn info_token_usage_from_line(line: &str) -> Option<(String, InfoTokenUsageCounts)> {
    if !line.contains("token_usage") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields
            .get("profile")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        InfoTokenUsageCounts {
            input_tokens: fields.get("input_tokens")?.parse::<u64>().ok()?,
            cached_input_tokens: fields
                .get("cached_input_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
            output_tokens: fields
                .get("output_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
            reasoning_tokens: fields
                .get("reasoning_tokens")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_default(),
        },
    ))
}

pub fn collect_info_runtime_load_summary_from_text(
    text: &str,
    now: i64,
    recent_window_seconds: i64,
    lookback_seconds: i64,
) -> InfoRuntimeLoadSummary {
    let recent_cutoff = now.saturating_sub(recent_window_seconds);
    let lookback_cutoff = now.saturating_sub(lookback_seconds);
    let mut summary = InfoRuntimeLoadSummary::default();
    let mut latest_inflight_counts = BTreeMap::new();

    for line in text.lines() {
        if let Some((profile, count)) = info_runtime_inflight_from_line(line) {
            latest_inflight_counts.insert(profile, count);
        }
        let Some(observation) = info_runtime_selection_observation_from_line(line) else {
            continue;
        };
        if observation.timestamp >= lookback_cutoff {
            summary.observations.push(observation.clone());
        }
        if observation.timestamp >= recent_cutoff {
            summary.recent_selection_events += 1;
            summary.recent_first_timestamp = Some(
                summary
                    .recent_first_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.min(observation.timestamp)
                    }),
            );
            summary.recent_last_timestamp = Some(
                summary
                    .recent_last_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.max(observation.timestamp)
                    }),
            );
        }
    }

    summary.active_inflight_units = latest_inflight_counts.values().sum();
    summary
}

pub fn info_runtime_selection_observation_from_line(
    line: &str,
) -> Option<InfoRuntimeQuotaObservation> {
    if !line.contains("selection_pick") && !line.contains("selection_keep_current") {
        return None;
    }
    let timestamp = runtime_log_timestamp_epoch(line)?;
    let fields = info_runtime_parse_fields(line);
    let profile = fields.get("profile")?;
    if profile == "none" {
        return None;
    }
    Some(InfoRuntimeQuotaObservation {
        timestamp,
        profile: profile.clone(),
        five_hour_remaining: fields.get("five_hour_remaining")?.parse::<i64>().ok()?,
        weekly_remaining: fields.get("weekly_remaining")?.parse::<i64>().ok()?,
    })
}

pub fn info_runtime_inflight_from_line(line: &str) -> Option<(String, usize)> {
    if !line.contains("profile_inflight ") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some((
        fields.get("profile")?.clone(),
        fields.get("count")?.parse::<usize>().ok()?,
    ))
}

pub fn runtime_log_timestamp_epoch(line: &str) -> Option<i64> {
    let timestamp = info_runtime_line_timestamp(line)?;
    chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S%.f %:z")
        .or_else(|_| chrono::DateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S %:z"))
        .ok()
        .map(|datetime| datetime.timestamp())
}

pub fn info_runtime_line_timestamp(line: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        return value
            .get("timestamp")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }
    let end = line.find("] ")?;
    line.strip_prefix('[')
        .and_then(|trimmed| trimmed.get(..end.saturating_sub(1)))
        .map(ToString::to_string)
}

pub fn info_runtime_parse_fields(line: &str) -> BTreeMap<String, String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
        && let Some(fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        return fields
            .iter()
            .filter_map(|(key, value)| {
                info_runtime_json_field_value(value).map(|value| (key.clone(), value))
            })
            .collect();
    }
    let message = line
        .split_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
        .trim();
    let mut fields = BTreeMap::new();
    for token in message.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        if key.is_empty() || value.is_empty() {
            continue;
        }
        fields.insert(key.to_string(), value.trim_matches('"').to_string());
    }
    fields
}

fn info_runtime_json_field_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn estimate_info_runway(
    observations: &[InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    current_remaining: i64,
    now: i64,
    min_span_seconds: i64,
) -> Option<InfoRunwayEstimate> {
    if current_remaining <= 0 {
        return Some(InfoRunwayEstimate {
            burn_per_hour: 0.0,
            observed_profiles: 0,
            observed_span_seconds: 0,
            exhaust_at: now,
        });
    }

    let mut by_profile = BTreeMap::<String, Vec<&InfoRuntimeQuotaObservation>>::new();
    for observation in observations {
        by_profile
            .entry(observation.profile.clone())
            .or_default()
            .push(observation);
    }

    let mut burn_per_hour = 0.0;
    let mut observed_profiles = 0;
    let mut earliest = i64::MAX;
    let mut latest = i64::MIN;

    for profile_observations in by_profile.values_mut() {
        profile_observations.sort_by_key(|observation| observation.timestamp);
        let Some((profile_burn_per_hour, start, end)) =
            info_profile_window_burn_rate(profile_observations, window, min_span_seconds)
        else {
            continue;
        };
        burn_per_hour += profile_burn_per_hour;
        observed_profiles += 1;
        earliest = earliest.min(start);
        latest = latest.max(end);
    }

    if burn_per_hour <= 0.0 || observed_profiles == 0 || earliest == i64::MAX || latest == i64::MIN
    {
        return None;
    }

    let seconds_until_exhaustion =
        ((current_remaining as f64 / burn_per_hour) * 3600.0).ceil() as i64;

    Some(InfoRunwayEstimate {
        burn_per_hour,
        observed_profiles,
        observed_span_seconds: latest.saturating_sub(earliest),
        exhaust_at: now.saturating_add(seconds_until_exhaustion.max(0)),
    })
}

pub fn info_profile_window_burn_rate(
    observations: &[&InfoRuntimeQuotaObservation],
    window: InfoQuotaWindow,
    min_span_seconds: i64,
) -> Option<(f64, i64, i64)> {
    if observations.len() < 2 {
        return None;
    }

    let latest = observations.last()?;
    let mut earliest = *latest;
    let mut current_remaining = info_observation_window_remaining(latest, window);

    for observation in observations.iter().rev().skip(1) {
        let remaining = info_observation_window_remaining(observation, window);
        if remaining < current_remaining {
            break;
        }
        earliest = *observation;
        current_remaining = remaining;
    }

    let earliest_remaining = info_observation_window_remaining(earliest, window);
    let latest_remaining = info_observation_window_remaining(latest, window);
    let burned = earliest_remaining.saturating_sub(latest_remaining);
    let span_seconds = latest.timestamp.saturating_sub(earliest.timestamp);
    if burned <= 0 || span_seconds < min_span_seconds {
        return None;
    }

    Some((
        burned as f64 * 3600.0 / span_seconds as f64,
        earliest.timestamp,
        latest.timestamp,
    ))
}

pub fn info_observation_window_remaining(
    observation: &InfoRuntimeQuotaObservation,
    window: InfoQuotaWindow,
) -> i64 {
    match window {
        InfoQuotaWindow::FiveHour => observation.five_hour_remaining,
        InfoQuotaWindow::Weekly => observation.weekly_remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_usage_summary_parses_text_runtime_log_markers() {
        let summary = collect_info_token_usage_summary_from_text(concat!(
            "[2026-04-29 10:00:00.000 +07:00] token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8\n",
            "[2026-04-29 10:00:01.000 +07:00] token_usage request=2 transport=http profile=backup source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=4 reasoning_tokens=1\n",
        ));

        assert_eq!(summary.event_count, 2);
        assert_eq!(summary.total.input_tokens, 110);
        assert_eq!(summary.total.cached_input_tokens, 25);
        assert_eq!(summary.total.output_tokens, 44);
        assert_eq!(summary.total.reasoning_tokens, 9);
        assert_eq!(summary.by_profile["main"].total.output_tokens, 40);
    }

    #[test]
    fn token_usage_summary_parses_json_runtime_log_markers() {
        let summary = collect_info_token_usage_summary_from_text(
            r#"{"timestamp":"2026-04-29 10:00:00.000 +07:00","message":"token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8","event":"token_usage","fields":{"request":"1","transport":"http","profile":"main","source":"responses_unary","input_tokens":"100","cached_input_tokens":"25","output_tokens":"40","reasoning_tokens":"8"}}"#,
        );

        assert_eq!(summary.event_count, 1);
        assert_eq!(summary.total.input_tokens, 100);
    }
}
