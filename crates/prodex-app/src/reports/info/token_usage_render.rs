//! Token-usage parsing and rendering.

use super::{
    InfoTokenUsageCounts, InfoTokenUsageEvent, InfoTokenUsageSummary, info_runtime_parse_fields,
};

pub fn collect_info_token_usage_summary_from_texts<I, S>(
    log_count: usize,
    texts: I,
) -> InfoTokenUsageSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut records = Vec::new();
    for text in texts {
        records.extend(text.as_ref().lines().filter_map(info_token_usage_from_line));
    }
    let facts = records
        .iter()
        .map(
            |(profile, total)| prodex_mojo_core::info_render::InfoTokenUsageProfile {
                profile,
                input_tokens: total.input_tokens,
                cached_input_tokens: total.cached_input_tokens,
                output_tokens: total.output_tokens,
                reasoning_tokens: total.reasoning_tokens,
            },
        )
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::info_render::aggregate_info_token_usage(&facts)
        .expect("Mojo info token-usage aggregation returned invalid output");
    InfoTokenUsageSummary {
        log_count,
        event_count: plan.event_count,
        total: plan.total,
        by_profile: plan
            .profiles
            .into_iter()
            .map(|profile| {
                (
                    profile.profile.to_string(),
                    super::InfoTokenUsageProfile {
                        event_count: profile.event_count,
                        total: profile.total,
                    },
                )
            })
            .collect(),
    }
}

pub fn info_token_usage_from_line(line: &str) -> Option<(String, InfoTokenUsageCounts)> {
    let event = info_token_usage_event_from_line(line)?;
    Some((
        event.profile,
        InfoTokenUsageCounts {
            input_tokens: event.input_tokens,
            cached_input_tokens: event.cached_input_tokens,
            output_tokens: event.output_tokens,
            reasoning_tokens: event.reasoning_tokens,
        },
    ))
}

pub fn info_token_usage_event_from_line(line: &str) -> Option<InfoTokenUsageEvent> {
    info_token_usage_event_from_line_with_event(line, "token_usage")
}

pub fn info_token_usage_progress_event_from_line(line: &str) -> Option<InfoTokenUsageEvent> {
    info_token_usage_event_from_line_with_event(line, "token_usage_progress")
}

fn info_token_usage_event_from_line_with_event(
    line: &str,
    event_name: &str,
) -> Option<InfoTokenUsageEvent> {
    if !info_runtime_line_has_event(line, event_name) {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    use prodex_mojo_core::log_throughput_policy::{InfoTokenUsageFields, InfoTokenUsageKind};
    let kind = match event_name {
        "token_usage_progress" => InfoTokenUsageKind::Progress,
        _ => InfoTokenUsageKind::Completed,
    };
    let numbers = prodex_mojo_core::log_throughput_policy::info_token_usage_numbers(
        kind,
        InfoTokenUsageFields {
            input_tokens: fields.get("input_tokens").map(String::as_str),
            cached_input_tokens: fields.get("cached_input_tokens").map(String::as_str),
            output_tokens: fields.get("output_tokens").map(String::as_str),
            reasoning_tokens: fields.get("reasoning_tokens").map(String::as_str),
            request: fields.get("request").map(String::as_str),
            generation_ms: fields.get("generation_ms").map(String::as_str),
            output_tokens_per_second: fields.get("output_tokens_per_second").map(String::as_str),
        },
    )
    .expect("Mojo info token-usage number policy returned invalid output")?;
    Some(InfoTokenUsageEvent {
        timestamp: info_token_usage_timestamp(line),
        request: numbers.request,
        profile: fields
            .get("profile")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        transport: fields
            .get("transport")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        source: fields
            .get("source")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        input_tokens: numbers.input_tokens,
        cached_input_tokens: numbers.cached_input_tokens,
        output_tokens: numbers.output_tokens,
        reasoning_tokens: numbers.reasoning_tokens,
        generation_ms: numbers.generation_ms,
        output_tokens_per_second: numbers.output_tokens_per_second,
    })
}

fn info_runtime_line_has_event(line: &str, expected: &str) -> bool {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        return value.get("event").and_then(serde_json::Value::as_str) == Some(expected);
    }
    runtime_proxy_crate::runtime_proxy_log_event(super::info_runtime_log_message(line))
        == Some(expected)
}

fn info_token_usage_timestamp(line: &str) -> String {
    if let Some(timestamp) = line
        .strip_prefix('[')
        .and_then(|line| line.split_once(']'))
        .map(|(timestamp, _)| timestamp)
    {
        return timestamp.to_string();
    }
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| value.get("timestamp")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

pub fn format_info_token_usage_summary(summary: &InfoTokenUsageSummary) -> String {
    let by_profile =
        summary
            .by_profile
            .iter()
            .map(|(profile, data)| terminal_ui::TokenUsageProfileDisplay {
                profile,
                total: data.total,
            });

    terminal_ui::format_info_token_usage_summary_display(
        summary.event_count,
        summary.log_count,
        summary.total,
        by_profile,
    )
}

#[cfg(test)]
#[path = "token_usage_number_tests.rs"]
mod number_tests;

#[cfg(test)]
#[path = "token_usage_aggregate_tests.rs"]
mod aggregate_tests;
