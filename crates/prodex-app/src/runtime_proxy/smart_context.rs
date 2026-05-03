use super::*;
use std::borrow::Cow;
use std::path::Path;

const SMART_CONTEXT_TOOL_OUTPUT_INLINE_LIMIT: usize = 4 * 1024;
const SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES: usize = 1024;
const SMART_CONTEXT_ESTIMATED_CONTEXT_WINDOW: usize = 32_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextTransport {
    Http,
    Websocket,
}

impl RuntimeSmartContextTransport {
    fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Websocket => "websocket",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextTransformStats {
    artifacts_stored: usize,
    tool_outputs_condensed: usize,
    duplicate_texts: usize,
    rehydrated_refs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeSmartContextLineRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeSmartContextArtifactReference {
    id: String,
    marker: String,
    line_range: Option<RuntimeSmartContextLineRange>,
}

#[derive(Debug, Default)]
struct RuntimeSmartContextProxyState {
    enabled: bool,
    artifacts: RuntimeSmartContextArtifactStore,
    last_token_usage: Option<RuntimeTokenUsage>,
}

static RUNTIME_SMART_CONTEXT_PROXY_STATES: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState>>,
> = OnceLock::new();

pub(crate) fn register_runtime_smart_context_proxy_state(log_path: &Path, enabled: bool) {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut states) = states.lock() else {
        return;
    };
    states.insert(
        log_path.to_path_buf(),
        RuntimeSmartContextProxyState {
            enabled,
            artifacts: RuntimeSmartContextArtifactStore::default(),
            last_token_usage: None,
        },
    );
}

pub(crate) fn observe_runtime_smart_context_token_usage(
    shared: &RuntimeRotationProxyShared,
    usage: RuntimeTokenUsage,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(mut states) = states.lock() else {
        return;
    };
    if let Some(state) = states.get_mut(&shared.log_path)
        && state.enabled
    {
        state.last_token_usage = Some(usage);
    }
}

pub(super) fn prepare_runtime_smart_context_http_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
) -> Cow<'a, [u8]> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(&request.body);
    }

    prepare_runtime_smart_context_body(
        request_id,
        request,
        shared,
        route_kind,
        RuntimeSmartContextTransport::Http,
    )
}

pub(super) fn prepare_runtime_smart_context_websocket_text<'a>(
    request_id: u64,
    request_text: &'a str,
    handshake_request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Cow<'a, str> {
    if !runtime_smart_context_enabled(shared) {
        return Cow::Borrowed(request_text);
    }

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: handshake_request.path_and_query.clone(),
        headers: handshake_request.headers.clone(),
        body: request_text.as_bytes().to_vec(),
    };
    match prepare_runtime_smart_context_body(
        request_id,
        &request,
        shared,
        RuntimeRouteKind::Websocket,
        RuntimeSmartContextTransport::Websocket,
    ) {
        Cow::Borrowed(_) => Cow::Borrowed(request_text),
        Cow::Owned(body) => String::from_utf8(body)
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(request_text)),
    }
}

fn prepare_runtime_smart_context_body<'a>(
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
) -> Cow<'a, [u8]> {
    let budget = runtime_smart_context_budget(shared, request.body.len());
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            "invalid_json",
            "pass_through",
            "-",
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };

    let missing_rehydrate_refs = runtime_smart_context_missing_artifact_refs(&value, shared);
    let exactness = runtime_proxy_crate::smart_context_exactness_guard(
        runtime_proxy_crate::SmartContextExactnessInput {
            exact_mode: runtime_smart_context_exact_header(request),
            previous_response_id: runtime_request_previous_response_id(request),
            turn_state: runtime_request_turn_state(request),
            session_id: runtime_request_session_id(request),
            tool_output_without_artifact: false,
            missing_rehydrate_refs,
        },
    );
    let tier = budget.tier;

    if exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "require_exact",
            &runtime_smart_context_reason_labels(&exactness.reasons),
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            budget,
            "pass_through_exact",
        );
        return Cow::Borrowed(&request.body);
    }

    let Some(stats) = with_runtime_smart_context_artifacts(shared, |store| {
        let mut stats = RuntimeSmartContextTransformStats::default();
        runtime_smart_context_rehydrate_value(&mut value, store, &mut stats);
        if tier != runtime_proxy_crate::SmartContextTokenBudgetTier::Exact {
            runtime_smart_context_condense_tool_outputs(
                &mut value, store, request_id, tier, &mut stats,
            );
            runtime_smart_context_dedupe_input_text(&mut value, &mut stats);
        }
        stats
    }) else {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "artifact_store_unavailable",
            "-",
            request.body.len(),
            request.body.len(),
            RuntimeSmartContextTransformStats::default(),
            budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };

    if stats == RuntimeSmartContextTransformStats::default() {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "pass_through",
            "-",
            request.body.len(),
            request.body.len(),
            stats,
            budget,
            "noop",
        );
        return Cow::Borrowed(&request.body);
    }

    let Ok(body) = serde_json::to_vec(&value) else {
        return Cow::Borrowed(&request.body);
    };
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    if runtime_smart_context_should_pass_through_after_self_check(
        request.body.len(),
        body.len(),
        &stats,
    ) {
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "self_check_passthrough",
            "-",
            request.body.len(),
            request.body.len(),
            stats,
            budget,
            self_check,
        );
        return Cow::Borrowed(&request.body);
    }
    runtime_smart_context_log(
        request_id,
        shared,
        route_kind,
        transport,
        runtime_smart_context_tier_label(tier),
        "rewritten",
        "-",
        request.body.len(),
        body.len(),
        stats,
        budget,
        self_check,
    );
    Cow::Owned(body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSmartContextBudget {
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    available_tokens: usize,
    observed_context_tokens: Option<usize>,
    token_usage_source: &'static str,
}

fn runtime_smart_context_budget(
    shared: &RuntimeRotationProxyShared,
    body_bytes: usize,
) -> RuntimeSmartContextBudget {
    let estimated_tokens = body_bytes.div_ceil(4);
    let observed_context_tokens =
        runtime_smart_context_last_token_usage(shared).and_then(runtime_smart_context_usage_tokens);
    let effective_tokens = observed_context_tokens.unwrap_or(estimated_tokens);
    let available_tokens = SMART_CONTEXT_ESTIMATED_CONTEXT_WINDOW.saturating_sub(effective_tokens);
    RuntimeSmartContextBudget {
        tier: runtime_proxy_crate::smart_context_token_budget_tier(available_tokens),
        available_tokens,
        observed_context_tokens,
        token_usage_source: if observed_context_tokens.is_some() {
            "runtime_usage"
        } else {
            "estimated_body"
        },
    }
}

fn runtime_smart_context_usage_tokens(usage: RuntimeTokenUsage) -> Option<usize> {
    let observed = usage
        .input_tokens
        .saturating_add(usage.output_tokens)
        .saturating_add(usage.reasoning_tokens);
    let observed = if observed == 0 {
        usage.cached_input_tokens
    } else {
        observed
    };
    if observed == 0 {
        None
    } else {
        Some(usize::try_from(observed).unwrap_or(usize::MAX))
    }
}

fn runtime_smart_context_enabled(shared: &RuntimeRotationProxyShared) -> bool {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return false;
    };
    let Ok(states) = states.lock() else {
        return false;
    };
    states
        .get(&shared.log_path)
        .is_some_and(|state| state.enabled)
}

fn runtime_smart_context_last_token_usage(
    shared: &RuntimeRotationProxyShared,
) -> Option<RuntimeTokenUsage> {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get()?;
    let states = states.lock().ok()?;
    states.get(&shared.log_path)?.last_token_usage
}

fn with_runtime_smart_context_artifacts<R>(
    shared: &RuntimeRotationProxyShared,
    action: impl FnOnce(&mut RuntimeSmartContextArtifactStore) -> R,
) -> Option<R> {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get()?;
    let mut states = states.lock().ok()?;
    let state = states.get_mut(&shared.log_path)?;
    state.enabled.then(|| action(&mut state.artifacts))
}

fn runtime_smart_context_exact_header(request: &RuntimeProxyRequest) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

fn runtime_smart_context_missing_artifact_refs(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
) -> Vec<String> {
    let ref_ids = runtime_smart_context_collect_artifact_ref_ids(value);
    if ref_ids.is_empty() {
        return Vec::new();
    }
    with_runtime_smart_context_artifacts(shared, |store| {
        ref_ids
            .into_iter()
            .filter(|id| !store.contains(id))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default()
}

fn runtime_smart_context_collect_artifact_ref_ids(value: &serde_json::Value) -> Vec<String> {
    runtime_smart_context_collect_artifact_refs(value)
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn runtime_smart_context_collect_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_artifact_refs_from_value(value, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_artifact_refs_from_value(
    value: &serde_json::Value,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    match value {
        serde_json::Value::String(text)
            if text.contains("prodex-artifact:")
                || text.contains("prodex smart context artifact") =>
        {
            for token in
                text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ')' | ']' | '}'))
            {
                if let Some(reference) = runtime_smart_context_parse_artifact_reference(token) {
                    refs.insert(reference);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_artifact_refs_from_value(item, refs);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values() {
                runtime_smart_context_collect_artifact_refs_from_value(item, refs);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_parse_artifact_reference(
    token: &str,
) -> Option<RuntimeSmartContextArtifactReference> {
    let token =
        token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`' | ':' | ';' | '.' | '!' | '?'));
    let raw = token.strip_prefix("prodex-artifact:").unwrap_or(token);
    if !raw.starts_with("sc:") {
        return None;
    }

    let mut id_end = 3usize;
    for (offset, ch) in raw[3..].char_indices() {
        if ch.is_ascii_hexdigit() {
            id_end = 3 + offset + ch.len_utf8();
        } else {
            break;
        }
    }
    if id_end == 3 {
        return None;
    }

    Some(RuntimeSmartContextArtifactReference {
        id: raw[..id_end].to_string(),
        marker: token.to_string(),
        line_range: runtime_smart_context_parse_line_range(&raw[id_end..]),
    })
}

fn runtime_smart_context_parse_line_range(suffix: &str) -> Option<RuntimeSmartContextLineRange> {
    let suffix = suffix
        .strip_prefix('#')
        .or_else(|| suffix.strip_prefix(':'))
        .or_else(|| suffix.strip_prefix('?'))?;
    let suffix = suffix.strip_prefix("lines=").unwrap_or(suffix);
    let suffix = suffix
        .strip_prefix('L')
        .or_else(|| suffix.strip_prefix('l'))
        .unwrap_or(suffix);
    let (start, end) = suffix.split_once('-').unwrap_or((suffix, suffix));
    let start = runtime_smart_context_parse_line_number(start)?;
    let end = runtime_smart_context_parse_line_number(end)?;
    (start > 0 && end >= start).then_some(RuntimeSmartContextLineRange { start, end })
}

fn runtime_smart_context_parse_line_number(value: &str) -> Option<usize> {
    value
        .strip_prefix('L')
        .or_else(|| value.strip_prefix('l'))
        .unwrap_or(value)
        .parse::<usize>()
        .ok()
}

fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    match value {
        serde_json::Value::String(text) => {
            let mut next = text.clone();
            for reference in runtime_smart_context_collect_artifact_refs(
                &serde_json::Value::String(text.clone()),
            ) {
                if let Some(artifact_text) = store.get_text(&reference.id)
                    && let Some(rehydrated_text) = runtime_smart_context_rehydrated_artifact_text(
                        &artifact_text,
                        reference.line_range,
                    )
                {
                    let marker = format!("prodex-artifact:{}", reference.id);
                    if next.contains(&reference.marker) {
                        next = next.replace(&reference.marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.contains(&marker) {
                        next = next.replace(&marker, &rehydrated_text);
                        stats.rehydrated_refs += 1;
                    } else if next.trim() == reference.id {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                    }
                }
            }
            *text = next;
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_rehydrate_value(item, store, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_rehydrate_value(item, store, stats);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_rehydrated_artifact_text(
    artifact_text: &str,
    line_range: Option<RuntimeSmartContextLineRange>,
) -> Option<String> {
    let Some(line_range) = line_range else {
        return Some(artifact_text.to_string());
    };
    let lines = artifact_text.lines().collect::<Vec<_>>();
    if line_range.start > lines.len() {
        return None;
    }
    let end = line_range.end.min(lines.len());
    Some(lines[line_range.start - 1..end].join("\n"))
}

fn runtime_smart_context_condense_tool_outputs(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for item in input {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !item_type.ends_with("_call_output") {
            continue;
        }
        for field in ["output", "content"] {
            let Some(output) = object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if output.len() <= SMART_CONTEXT_TOOL_OUTPUT_INLINE_LIMIT {
                continue;
            }
            let Some(artifact) = store.insert_text(request_id, &output) else {
                continue;
            };
            let compacted = runtime_smart_context_compact_tool_output(&output, tier);
            let replacement = runtime_smart_context_artifact_summary(&artifact, &compacted);
            if replacement.len().saturating_mul(100) >= output.len().saturating_mul(90) {
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            stats.artifacts_stored += 1;
            stats.tool_outputs_condensed += 1;
            break;
        }
    }
}

fn runtime_smart_context_compact_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> String {
    let max_lines = match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => 120,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => 80,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => 40,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => return text.to_string(),
    };
    prodex_context::compact_command_output_with_options(
        text,
        &prodex_context::CommandOutputCompactOptions {
            max_lines,
            head_lines: max_lines.saturating_mul(2) / 3,
            tail_lines: max_lines / 3,
            max_line_chars: 220,
            ..prodex_context::CommandOutputCompactOptions::default()
        },
    )
    .output
}

fn runtime_smart_context_artifact_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    compacted: &str,
) -> String {
    format!(
        "# prodex smart context artifact\nartifact_id: prodex-artifact:{}\noriginal_bytes: {}\ncontent_hash: {}\nrehydrate: automatic when exact content is referenced; use prodex-artifact:{}#Lstart-Lend for exact line range\n\n{}",
        artifact.id, artifact.byte_len, artifact.content_hash, artifact.id, compacted
    )
}

fn runtime_smart_context_dedupe_input_text(
    value: &mut serde_json::Value,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let mut seen = BTreeMap::<String, usize>::new();
    for (index, item) in input.iter_mut().enumerate() {
        runtime_smart_context_dedupe_value_text(item, index, &mut seen, stats);
    }
}

fn runtime_smart_context_dedupe_value_text(
    value: &mut serde_json::Value,
    item_index: usize,
    seen: &mut BTreeMap<String, usize>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    match value {
        serde_json::Value::String(text) => {
            if text.len() < SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES {
                return;
            }
            let hash = runtime_proxy_crate::smart_context_hash_text(text);
            if let Some(first_index) = seen.get(&hash) {
                *text = format!(
                    "[prodex smart context duplicate of input[{first_index}] content_hash={hash}]"
                );
                stats.duplicate_texts += 1;
            } else {
                seen.insert(hash, item_index);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for item in object.values_mut() {
                runtime_smart_context_dedupe_value_text(item, item_index, seen, stats);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn runtime_smart_context_log(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    tier: &str,
    decision: &str,
    reasons: &str,
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: RuntimeSmartContextTransformStats,
    budget: RuntimeSmartContextBudget,
    self_check: &'static str,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_autopilot",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport.label()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("tier", tier),
                runtime_proxy_log_field("decision", decision),
                runtime_proxy_log_field("reasons", reasons),
                runtime_proxy_log_field("token_usage_source", budget.token_usage_source),
                runtime_proxy_log_field(
                    "observed_context_tokens",
                    budget
                        .observed_context_tokens
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                ),
                runtime_proxy_log_field("body_bytes_before", body_bytes_before.to_string()),
                runtime_proxy_log_field("body_bytes_after", body_bytes_after.to_string()),
                runtime_proxy_log_field(
                    "body_bytes_saved",
                    body_bytes_before
                        .saturating_sub(body_bytes_after)
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rewrite_ratio_percent",
                    runtime_smart_context_rewrite_ratio_percent(
                        body_bytes_before,
                        body_bytes_after,
                    )
                    .to_string(),
                ),
                runtime_proxy_log_field("self_check", self_check),
                runtime_proxy_log_field("available_tokens", budget.available_tokens.to_string()),
                runtime_proxy_log_field("artifacts_stored", stats.artifacts_stored.to_string()),
                runtime_proxy_log_field(
                    "tool_outputs_condensed",
                    stats.tool_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("duplicate_texts", stats.duplicate_texts.to_string()),
                runtime_proxy_log_field("rehydrated_refs", stats.rehydrated_refs.to_string()),
            ],
        ),
    );
}

fn runtime_smart_context_rewrite_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> &'static str {
    if stats.rehydrated_refs > 0 && stats.tool_outputs_condensed == 0 && stats.duplicate_texts == 0
    {
        "ok_rehydrate_exact"
    } else if body_bytes_after < body_bytes_before {
        "ok_saved"
    } else if body_bytes_after == body_bytes_before {
        "zero_savings"
    } else {
        "growth"
    }
}

fn runtime_smart_context_should_pass_through_after_self_check(
    body_bytes_before: usize,
    body_bytes_after: usize,
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs == 0 && body_bytes_after >= body_bytes_before
}

fn runtime_smart_context_rewrite_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}

fn runtime_smart_context_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextExactnessReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextExactnessReason::ExplicitExactMode => "exact_mode",
            runtime_proxy_crate::SmartContextExactnessReason::PreviousResponseAffinity => {
                "previous_response"
            }
            runtime_proxy_crate::SmartContextExactnessReason::TurnStateAffinity => "turn_state",
            runtime_proxy_crate::SmartContextExactnessReason::SessionAffinity => "session",
            runtime_proxy_crate::SmartContextExactnessReason::ToolOutputWithoutArtifact => {
                "tool_output_without_artifact"
            }
            runtime_proxy_crate::SmartContextExactnessReason::RehydrateRequired => "rehydrate",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn runtime_smart_context_tier_label(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> &'static str {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => "exact",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => "large",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => "condensed",
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => "minimal",
    }
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/smart_context.rs"]
mod tests;
