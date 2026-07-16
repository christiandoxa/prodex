use std::collections::BTreeMap;

use crate::log_fields::{
    runtime_doctor_ignored_log_value, runtime_doctor_log_message_has_marker,
    runtime_doctor_parse_usize_field, runtime_doctor_split_reason_field,
    runtime_doctor_string_field, runtime_proxy_log_fields,
};
use crate::parsing::RuntimeDoctorParsedLogLine;
#[cfg(test)]
use crate::summarize_runtime_log_tail;

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorSmartContextAutopilotEvent {
    pub request: Option<String>,
    pub transport: Option<String>,
    pub route: Option<String>,
    pub tier: Option<String>,
    pub decision: Option<String>,
    pub reasons: Vec<String>,
    pub token_usage_source: Option<String>,
    pub observed_context_tokens: Option<usize>,
    pub body_bytes_before: Option<usize>,
    pub body_bytes_after: Option<usize>,
    pub body_bytes_saved: Option<usize>,
    pub estimated_tokens_saved: Option<usize>,
    pub rewrite_ratio_percent: Option<usize>,
    pub self_check: Option<String>,
    pub available_tokens: Option<usize>,
    pub policy_mode: Option<String>,
    pub policy_reasons: Vec<String>,
    pub max_inline_tool_output_bytes: Option<usize>,
    pub max_rehydrate_tokens: Option<usize>,
    pub artifacts_stored: Option<usize>,
    pub tool_outputs_condensed: Option<usize>,
    pub duplicate_texts: Option<usize>,
    pub cross_turn_duplicate_texts: Option<usize>,
    pub repeat_tool_output_refs: Option<usize>,
    pub blob_outputs_condensed: Option<usize>,
    pub rehydrated_refs: Option<usize>,
    pub static_context_deltas: Option<usize>,
    pub repo_state_facts: Option<usize>,
}

#[derive(Debug, Clone, Default, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorSmartContextAutopilotSummary {
    pub event_count: usize,
    pub rewrite_count: usize,
    pub fallback_count: usize,
    pub total_body_bytes_before: usize,
    pub total_body_bytes_after: usize,
    pub total_body_bytes_saved: usize,
    pub estimated_tokens_saved: usize,
    pub total_repeat_tool_output_refs: usize,
    pub total_static_context_deltas: usize,
    pub total_repo_state_facts: usize,
    pub decision_counts: BTreeMap<String, usize>,
    pub fallback_reason_counts: BTreeMap<String, usize>,
    pub self_check_counts: BTreeMap<String, usize>,
    pub policy_mode_counts: BTreeMap<String, usize>,
    pub policy_reason_counts: BTreeMap<String, usize>,
    pub token_usage_source_counts: BTreeMap<String, usize>,
    pub latest_event: Option<RuntimeDoctorSmartContextAutopilotEvent>,
}

pub fn runtime_doctor_estimate_smart_context_tokens_saved(saved_bytes: usize) -> usize {
    saved_bytes / 4 + usize::from(!saved_bytes.is_multiple_of(4))
}

pub fn runtime_doctor_parse_smart_context_autopilot_line(
    line: &str,
) -> Option<RuntimeDoctorSmartContextAutopilotEvent> {
    let parsed_line = RuntimeDoctorParsedLogLine::new(line);
    let message = parsed_line.message();
    if !runtime_doctor_log_message_has_marker(&message, "smart_context_autopilot") {
        return None;
    }

    let mut fields = runtime_proxy_log_fields(&message);
    if let Some(value) = parsed_line.json()
        && let Some(json_fields) = value.get("fields").and_then(serde_json::Value::as_object)
    {
        for (key, value) in json_fields {
            if let Some(string_value) = runtime_doctor_json_field_string(value) {
                fields.insert(key.clone(), string_value);
            }
        }
    }

    let body_bytes_before = runtime_doctor_parse_usize_field(&fields, "body_bytes_before");
    let body_bytes_after = runtime_doctor_parse_usize_field(&fields, "body_bytes_after");
    let body_bytes_saved = runtime_doctor_parse_usize_field(&fields, "body_bytes_saved")
        .or_else(|| Some(body_bytes_before?.saturating_sub(body_bytes_after?)));
    let estimated_tokens_saved =
        body_bytes_saved.map(runtime_doctor_estimate_smart_context_tokens_saved);

    Some(RuntimeDoctorSmartContextAutopilotEvent {
        request: runtime_doctor_string_field(&fields, "request"),
        transport: runtime_doctor_string_field(&fields, "transport"),
        route: runtime_doctor_string_field(&fields, "route"),
        tier: runtime_doctor_string_field(&fields, "tier"),
        decision: runtime_doctor_string_field(&fields, "decision"),
        reasons: runtime_doctor_split_reason_field(&fields, "reasons"),
        token_usage_source: runtime_doctor_string_field(&fields, "token_usage_source"),
        observed_context_tokens: runtime_doctor_parse_usize_field(
            &fields,
            "observed_context_tokens",
        ),
        body_bytes_before,
        body_bytes_after,
        body_bytes_saved,
        estimated_tokens_saved,
        rewrite_ratio_percent: runtime_doctor_parse_usize_field(&fields, "rewrite_ratio_percent"),
        self_check: runtime_doctor_string_field(&fields, "self_check"),
        available_tokens: runtime_doctor_parse_usize_field(&fields, "available_tokens"),
        policy_mode: runtime_doctor_string_field(&fields, "budget_mode"),
        policy_reasons: runtime_doctor_split_reason_field(&fields, "policy_reasons"),
        max_inline_tool_output_bytes: runtime_doctor_parse_usize_field(
            &fields,
            "max_inline_tool_output_bytes",
        ),
        max_rehydrate_tokens: runtime_doctor_parse_usize_field(&fields, "max_rehydrate_tokens"),
        artifacts_stored: runtime_doctor_parse_usize_field(&fields, "artifacts_stored"),
        tool_outputs_condensed: runtime_doctor_parse_usize_field(&fields, "tool_outputs_condensed"),
        duplicate_texts: runtime_doctor_parse_usize_field(&fields, "duplicate_texts"),
        cross_turn_duplicate_texts: runtime_doctor_parse_usize_field(
            &fields,
            "cross_turn_duplicate_texts",
        ),
        repeat_tool_output_refs: runtime_doctor_parse_usize_field(
            &fields,
            "repeat_tool_output_refs",
        ),
        blob_outputs_condensed: runtime_doctor_parse_usize_field(&fields, "blob_outputs_condensed"),
        rehydrated_refs: runtime_doctor_parse_usize_field(&fields, "rehydrated_refs"),
        static_context_deltas: runtime_doctor_parse_usize_field(&fields, "static_context_deltas"),
        repo_state_facts: runtime_doctor_parse_usize_field(&fields, "repo_state_facts"),
    })
}

pub fn runtime_doctor_summarize_smart_context_autopilot_tail(
    tail: &[u8],
) -> RuntimeDoctorSmartContextAutopilotSummary {
    let text = String::from_utf8_lossy(tail);
    let mut summary = RuntimeDoctorSmartContextAutopilotSummary::default();
    for line in text.lines() {
        let Some(event) = runtime_doctor_parse_smart_context_autopilot_line(line) else {
            continue;
        };
        runtime_doctor_add_smart_context_autopilot_event(&mut summary, event);
    }
    summary
}

fn runtime_doctor_add_smart_context_autopilot_event(
    summary: &mut RuntimeDoctorSmartContextAutopilotSummary,
    event: RuntimeDoctorSmartContextAutopilotEvent,
) {
    summary.event_count += 1;
    summary.total_body_bytes_before = summary
        .total_body_bytes_before
        .saturating_add(event.body_bytes_before.unwrap_or(0));
    summary.total_body_bytes_after = summary
        .total_body_bytes_after
        .saturating_add(event.body_bytes_after.unwrap_or(0));
    summary.total_body_bytes_saved = summary
        .total_body_bytes_saved
        .saturating_add(event.body_bytes_saved.unwrap_or(0));
    summary.estimated_tokens_saved = summary
        .estimated_tokens_saved
        .saturating_add(event.estimated_tokens_saved.unwrap_or(0));
    summary.total_repeat_tool_output_refs = summary
        .total_repeat_tool_output_refs
        .saturating_add(event.repeat_tool_output_refs.unwrap_or(0));
    summary.total_static_context_deltas = summary
        .total_static_context_deltas
        .saturating_add(event.static_context_deltas.unwrap_or(0));
    summary.total_repo_state_facts = summary
        .total_repo_state_facts
        .saturating_add(event.repo_state_facts.unwrap_or(0));

    if let Some(decision) = event.decision.as_deref() {
        runtime_doctor_increment_string_count(&mut summary.decision_counts, decision);
        if decision == "rewritten" {
            summary.rewrite_count += 1;
        } else if runtime_doctor_smart_context_decision_is_fallback(decision) {
            summary.fallback_count += 1;
            runtime_doctor_count_smart_context_fallback_reasons(summary, decision, &event);
        }
    }
    if let Some(self_check) = event.self_check.as_deref() {
        runtime_doctor_increment_string_count(&mut summary.self_check_counts, self_check);
    }
    if let Some(policy_mode) = event.policy_mode.as_deref() {
        runtime_doctor_increment_string_count(&mut summary.policy_mode_counts, policy_mode);
    }
    if let Some(source) = event.token_usage_source.as_deref() {
        runtime_doctor_increment_string_count(&mut summary.token_usage_source_counts, source);
    }
    for reason in &event.policy_reasons {
        runtime_doctor_increment_string_count(&mut summary.policy_reason_counts, reason);
    }

    summary.latest_event = Some(event);
}

fn runtime_doctor_smart_context_decision_is_fallback(decision: &str) -> bool {
    !matches!(decision, "rewritten" | "pass_through")
}

fn runtime_doctor_count_smart_context_fallback_reasons(
    summary: &mut RuntimeDoctorSmartContextAutopilotSummary,
    decision: &str,
    event: &RuntimeDoctorSmartContextAutopilotEvent,
) {
    if !event.reasons.is_empty() {
        for reason in &event.reasons {
            runtime_doctor_increment_string_count(&mut summary.fallback_reason_counts, reason);
        }
        return;
    }
    if decision == "self_check_passthrough"
        && let Some(self_check) = event.self_check.as_deref()
    {
        runtime_doctor_increment_string_count(&mut summary.fallback_reason_counts, self_check);
        return;
    }
    runtime_doctor_increment_string_count(&mut summary.fallback_reason_counts, decision);
}

fn runtime_doctor_increment_string_count(counts: &mut BTreeMap<String, usize>, value: &str) {
    if runtime_doctor_ignored_log_value(value) {
        return;
    }
    *counts.entry(value.to_string()).or_insert(0) += 1;
}

fn runtime_doctor_json_field_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../tests/src/smart_context_autopilot.rs"]
mod smart_context_autopilot_tests;
