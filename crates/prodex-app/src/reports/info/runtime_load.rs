//! Runtime-load parsing and summaries.

use prodex_mojo_core::info_render::InfoRuntimeLoadConfig;

use super::{
    BTreeMap, InfoQuotaWindow, InfoRuntimeLoadSummary, InfoRuntimeQuotaObservation,
    InfoRunwayEstimate,
};

pub fn collect_info_runtime_load_summary_from_texts<I, S>(
    log_count: usize,
    texts: I,
    now: i64,
    recent_window_seconds: i64,
    lookback_seconds: i64,
) -> InfoRuntimeLoadSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    use prodex_mojo_core::info_render::{InfoRuntimeInflight, InfoRunwayObservation};
    let mut observations = Vec::new();
    let mut inflight_rows = Vec::new();
    for (log, text) in texts.into_iter().enumerate() {
        for line in text.as_ref().lines() {
            if let Some((profile, units)) = info_runtime_inflight_from_line(line) {
                inflight_rows.push((log, profile, units));
            }
            if let Some(observation) = info_runtime_selection_observation_from_line(line) {
                observations.push(observation);
            }
        }
    }
    let observation_facts = observations
        .iter()
        .map(|row| InfoRunwayObservation {
            timestamp: row.timestamp,
            profile: &row.profile,
            five_hour_remaining: row.five_hour_remaining,
            weekly_remaining: row.weekly_remaining,
        })
        .collect::<Vec<_>>();
    let inflight_facts = inflight_rows
        .iter()
        .map(|(log, profile, units)| InfoRuntimeInflight {
            log: *log,
            profile,
            units: *units,
        })
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::info_render::info_runtime_load_plan(
        &observation_facts,
        &inflight_facts,
        InfoRuntimeLoadConfig {
            now,
            recent_window_seconds,
            lookback_seconds,
        },
    )
    .expect("Mojo runtime-load summary policy returned invalid output");
    InfoRuntimeLoadSummary {
        log_count,
        observations: plan
            .observation_indices
            .into_iter()
            .map(|index| observations[index].clone())
            .collect(),
        active_inflight_units: plan.active_inflight_units,
        recent_selection_events: plan.recent_selection_events,
        recent_first_timestamp: plan.recent_first_timestamp,
        recent_last_timestamp: plan.recent_last_timestamp,
    }
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
    runtime_proxy_crate::runtime_proxy_log_fields(info_runtime_log_message(line))
}

pub(crate) fn info_runtime_log_message(line: &str) -> &str {
    line.split_once("] ")
        .map(|(_, message)| message)
        .unwrap_or(line)
        .trim()
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
    let observations = observations
        .iter()
        .map(
            |observation| prodex_mojo_core::info_render::InfoRunwayObservation {
                timestamp: observation.timestamp,
                profile: observation.profile.as_str(),
                five_hour_remaining: observation.five_hour_remaining,
                weekly_remaining: observation.weekly_remaining,
            },
        )
        .collect::<Vec<_>>();
    let window = match window {
        InfoQuotaWindow::FiveHour => 0,
        InfoQuotaWindow::Weekly => 1,
    };
    prodex_mojo_core::info_render::estimate_info_runway(
        &observations,
        window,
        current_remaining,
        now,
        min_span_seconds,
    )
    .unwrap_or_else(|error| panic!("Mojo info runway estimator failed: {error:?}"))
    .map(|estimate| InfoRunwayEstimate {
        burn_per_hour: estimate.burn_per_hour,
        observed_profiles: estimate.observed_profiles,
        observed_span_seconds: estimate.observed_span_seconds,
        exhaust_at: estimate.exhaust_at,
    })
}

#[cfg(test)]
#[path = "runtime_load_plan_tests.rs"]
mod plan_tests;
