use super::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES: usize = 1024;
const SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS: u64 = 32_000;
const SMART_CONTEXT_RESERVED_OUTPUT_TOKENS: u64 = 4_096;
const SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT: usize = 8;
const SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT: usize = 4;
const SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES: usize = 12;
const SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES: usize = 256;
const SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS: usize = 220;
const RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS: [&str; 3] =
    ["instructions", "system", "developer"];

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
    cross_turn_duplicate_texts: usize,
    repeat_tool_output_refs: usize,
    blob_outputs_condensed: usize,
    rehydrated_refs: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextTransformOutcome {
    stats: RuntimeSmartContextTransformStats,
    deferred_rehydrate_refs: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RuntimeSmartContextStaticContextObservation {
    changed: bool,
    item_count: usize,
    delta_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeSmartContextRewriteSafetyObservation {
    safe: bool,
    saved_tokens: u64,
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
    model_context_window_tokens: Option<u64>,
    artifacts: RuntimeSmartContextArtifactStore,
    artifact_path: Option<PathBuf>,
    last_token_usage: Option<RuntimeTokenUsage>,
    token_usage_history: Vec<RuntimeTokenUsage>,
    rewrite_safety_history: Vec<RuntimeSmartContextRewriteSafetyObservation>,
    last_static_context_fingerprints: Vec<runtime_proxy_crate::SmartContextFingerprint>,
    last_static_context_prompt_cache_hash: Option<String>,
}

static RUNTIME_SMART_CONTEXT_PROXY_STATES: OnceLock<
    Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState>>,
> = OnceLock::new();

pub(crate) fn register_runtime_smart_context_proxy_state(
    log_path: &Path,
    enabled: bool,
    model_context_window_tokens: Option<u64>,
    artifact_path: Option<PathBuf>,
) {
    let states = RUNTIME_SMART_CONTEXT_PROXY_STATES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut states) = states.lock() else {
        return;
    };
    let artifacts = artifact_path
        .as_deref()
        .filter(|_| enabled)
        .map(RuntimeSmartContextArtifactStore::load_from_path)
        .unwrap_or_default();
    states.insert(
        log_path.to_path_buf(),
        RuntimeSmartContextProxyState {
            enabled,
            model_context_window_tokens,
            artifacts,
            artifact_path,
            last_token_usage: None,
            token_usage_history: Vec::new(),
            rewrite_safety_history: Vec::new(),
            last_static_context_fingerprints: Vec::new(),
            last_static_context_prompt_cache_hash: None,
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
        state.token_usage_history.push(usage);
        if state.token_usage_history.len() > SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT {
            let overflow = state
                .token_usage_history
                .len()
                .saturating_sub(SMART_CONTEXT_TOKEN_USAGE_HISTORY_LIMIT);
            state.token_usage_history.drain(0..overflow);
        }
    }
}

fn observe_runtime_smart_context_rewrite_safety(
    shared: &RuntimeRotationProxyShared,
    observation: RuntimeSmartContextRewriteSafetyObservation,
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
        state.rewrite_safety_history.push(observation);
        if state.rewrite_safety_history.len() > SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT {
            let overflow = state
                .rewrite_safety_history
                .len()
                .saturating_sub(SMART_CONTEXT_REWRITE_SAFETY_HISTORY_LIMIT);
            state.rewrite_safety_history.drain(0..overflow);
        }
    }
}

pub(crate) fn prepare_runtime_smart_context_http_body<'a>(
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
    let budget = runtime_smart_context_budget(
        shared,
        request.body.len(),
        runtime_proxy_crate::SmartContextExactnessGuard {
            decision: runtime_proxy_crate::SmartContextExactnessDecision::Allow,
            reasons: Vec::new(),
        },
        Vec::new(),
        false,
    );
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
            &budget,
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
            missing_rehydrate_refs: missing_rehydrate_refs.clone(),
        },
    );
    let budget = runtime_smart_context_budget(
        shared,
        request.body.len(),
        exactness.clone(),
        missing_rehydrate_refs.clone(),
        runtime_smart_context_observe_static_context(shared, &value).changed,
    );
    let tier = budget.tier;

    if exactness.decision == runtime_proxy_crate::SmartContextExactnessDecision::RequireExact {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "require_exact",
                &runtime_smart_context_reason_labels(&exactness.reasons),
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
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
            &budget,
            "pass_through_exact",
        );
        return Cow::Borrowed(&request.body);
    }

    let Some(outcome) = with_runtime_smart_context_artifacts(shared, |store| {
        let mut outcome = RuntimeSmartContextTransformOutcome::default();
        let rehydrate_plan =
            runtime_smart_context_auto_rehydrate_plan(&value, store, budget.available_tokens, tier);
        outcome.deferred_rehydrate_refs =
            runtime_smart_context_deferred_rehydrate_refs(&rehydrate_plan);
        runtime_smart_context_rehydrate_value_with_plan(
            &mut value,
            store,
            &rehydrate_plan,
            &mut outcome.stats,
        );
        if budget.policy.mode != runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough {
            runtime_smart_context_condense_tool_outputs(
                &mut value,
                store,
                request_id,
                tier,
                budget.policy.max_inline_tool_output_bytes,
                &mut outcome.stats,
            );
            runtime_smart_context_dedupe_input_text(
                &mut value,
                store,
                &exactness,
                &mut outcome.stats,
            );
        }
        outcome
    }) else {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "artifact_store_unavailable",
                "-",
                request.body.len(),
                body.len(),
                RuntimeSmartContextTransformStats::default(),
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
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
            &budget,
            "pass_through",
        );
        return Cow::Borrowed(&request.body);
    };
    let stats = outcome.stats.clone();
    if stats.artifacts_stored > 0 {
        persist_runtime_smart_context_artifacts(shared);
    }

    if stats == RuntimeSmartContextTransformStats::default() {
        if let Some(body) = runtime_smart_context_minified_json_body(&value, &request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "minified",
                "-",
                request.body.len(),
                body.len(),
                stats,
                &budget,
                "ok_minified",
            );
            return Cow::Owned(body);
        }
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
            &budget,
            "noop",
        );
        return Cow::Borrowed(&request.body);
    }

    let Ok(body) = serde_json::to_vec(&value) else {
        return Cow::Borrowed(&request.body);
    };
    let self_check =
        runtime_smart_context_rewrite_self_check(request.body.len(), body.len(), &stats);
    let mut unresolved_rehydrate_refs = missing_rehydrate_refs;
    unresolved_rehydrate_refs.extend(outcome.deferred_rehydrate_refs);
    let regression_check = runtime_smart_context_regression_self_check(
        &request.body,
        &body,
        exactness.clone(),
        unresolved_rehydrate_refs.clone(),
    );
    let critical_signal_check =
        runtime_smart_context_critical_signal_self_check(&request.body, &body);
    if critical_signal_check.has_loss()
        && let Some((repaired_body, repaired_stats)) =
            runtime_smart_context_try_surgical_rehydrate_critical_ranges(
                &value,
                shared,
                &request.body,
                &exactness,
                &unresolved_rehydrate_refs,
                &stats,
            )
    {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: true,
                saved_tokens: runtime_smart_context_saved_tokens(
                    request.body.len(),
                    repaired_body.len(),
                ),
            },
        );
        runtime_smart_context_log(
            request_id,
            shared,
            route_kind,
            transport,
            runtime_smart_context_tier_label(tier),
            "rewritten",
            "surgical_rehydrate",
            request.body.len(),
            repaired_body.len(),
            repaired_stats,
            &budget,
            "ok_surgical_rehydrate",
        );
        return Cow::Owned(repaired_body);
    }
    if let Some(fallback_reason) = runtime_smart_context_fallback_exact_reason(
        &regression_check,
        critical_signal_check,
        &stats,
    ) {
        observe_runtime_smart_context_rewrite_safety(
            shared,
            RuntimeSmartContextRewriteSafetyObservation {
                safe: false,
                saved_tokens: 0,
            },
        );
        if let Some(body) = runtime_smart_context_minified_json_body_from_original(&request.body) {
            runtime_smart_context_log(
                request_id,
                shared,
                route_kind,
                transport,
                runtime_smart_context_tier_label(tier),
                "self_check_passthrough",
                "-",
                request.body.len(),
                body.len(),
                stats,
                &budget,
                fallback_reason,
            );
            return Cow::Owned(body);
        }
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
            &budget,
            fallback_reason,
        );
        return Cow::Borrowed(&request.body);
    }
    observe_runtime_smart_context_rewrite_safety(
        shared,
        RuntimeSmartContextRewriteSafetyObservation {
            safe: true,
            saved_tokens: regression_check.saved_tokens,
        },
    );
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
        &budget,
        self_check,
    );
    Cow::Owned(body)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSmartContextBudget {
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    policy: runtime_proxy_crate::SmartContextAdaptiveBudgetPolicy,
    model_context_window_tokens: u64,
    model_context_window_source: &'static str,
    available_tokens: usize,
    observed_context_tokens: Option<usize>,
    token_usage_source: &'static str,
}

fn runtime_smart_context_budget(
    shared: &RuntimeRotationProxyShared,
    body_bytes: usize,
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
    static_context_changed: bool,
) -> RuntimeSmartContextBudget {
    let (history, configured_context_window_tokens, recent_rewrite_safety) =
        runtime_smart_context_budget_inputs(shared);
    let model_context_window_tokens =
        configured_context_window_tokens.unwrap_or(SMART_CONTEXT_FALLBACK_CONTEXT_WINDOW_TOKENS);
    let observed_context_tokens_u64 = history
        .last()
        .and_then(|usage| runtime_proxy_crate::smart_context_observed_usage_context_tokens(*usage));
    let observed_context_tokens =
        observed_context_tokens_u64.and_then(|tokens| usize::try_from(tokens).ok());
    let current_input_tokens = observed_context_tokens_u64.unwrap_or(0);
    let accounting = runtime_proxy_crate::smart_context_observed_token_accounting(
        runtime_proxy_crate::SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(model_context_window_tokens),
            reserved_output_tokens: SMART_CONTEXT_RESERVED_OUTPUT_TOKENS,
            current_input_tokens,
            current_request_body_bytes: body_bytes,
            observed_usage: history,
        },
    );
    let policy = runtime_proxy_crate::smart_context_adaptive_budget_policy(
        runtime_proxy_crate::SmartContextAdaptiveBudgetPolicyInput {
            exactness_guard,
            accounting,
            recent_rewrite_safety,
            static_context_changed,
            missing_rehydrate_refs,
        },
    );
    let available_tokens = policy
        .max_rehydrate_tokens
        .min(usize::MAX as u64)
        .try_into()
        .unwrap_or(usize::MAX);
    RuntimeSmartContextBudget {
        tier: policy.tier,
        policy,
        model_context_window_tokens,
        model_context_window_source: if configured_context_window_tokens.is_some() {
            "launch_config"
        } else {
            "fallback"
        },
        available_tokens,
        observed_context_tokens,
        token_usage_source: if observed_context_tokens.is_some() {
            "runtime_usage"
        } else {
            "estimated_body"
        },
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

fn runtime_smart_context_budget_inputs(
    shared: &RuntimeRotationProxyShared,
) -> (
    Vec<RuntimeTokenUsage>,
    Option<u64>,
    runtime_proxy_crate::SmartContextRecentRewriteSafety,
) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return (Vec::new(), None, Default::default());
    };
    let Ok(states) = states.lock() else {
        return (Vec::new(), None, Default::default());
    };
    states
        .get(&shared.log_path)
        .map(|state| {
            (
                state.token_usage_history.clone(),
                state.model_context_window_tokens,
                runtime_smart_context_recent_rewrite_safety(&state.rewrite_safety_history),
            )
        })
        .unwrap_or_default()
}

fn runtime_smart_context_recent_rewrite_safety(
    history: &[RuntimeSmartContextRewriteSafetyObservation],
) -> runtime_proxy_crate::SmartContextRecentRewriteSafety {
    let mut safety = runtime_proxy_crate::SmartContextRecentRewriteSafety::default();
    for observation in history {
        if observation.safe {
            safety.safe_rewrites = safety.safe_rewrites.saturating_add(1);
            safety.saved_tokens = safety.saved_tokens.saturating_add(observation.saved_tokens);
        } else {
            safety.fallback_rewrites = safety.fallback_rewrites.saturating_add(1);
        }
    }
    safety
}

fn runtime_smart_context_observe_static_context(
    shared: &RuntimeRotationProxyShared,
    value: &serde_json::Value,
) -> RuntimeSmartContextStaticContextObservation {
    let cache = runtime_proxy_crate::smart_context_static_context_prompt_cache_fingerprint(
        runtime_smart_context_static_context_items(value),
    );
    if cache.items.is_empty() {
        return RuntimeSmartContextStaticContextObservation::default();
    }

    let current = cache
        .items
        .iter()
        .map(|item| runtime_proxy_crate::SmartContextFingerprint {
            id: item.id.clone(),
            kind: runtime_proxy_crate::SmartContextFingerprintKind::StaticContext,
            content_hash: item.content_hash.clone(),
            byte_len: item.byte_len,
        })
        .collect::<Vec<_>>();

    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return RuntimeSmartContextStaticContextObservation {
            changed: false,
            item_count: current.len(),
            delta_count: 0,
        };
    };
    let Ok(mut states) = states.lock() else {
        return RuntimeSmartContextStaticContextObservation {
            changed: false,
            item_count: current.len(),
            delta_count: 0,
        };
    };
    let Some(state) = states.get_mut(&shared.log_path) else {
        return RuntimeSmartContextStaticContextObservation {
            changed: false,
            item_count: current.len(),
            delta_count: 0,
        };
    };

    let delta = if state.last_static_context_fingerprints.is_empty() {
        Vec::new()
    } else {
        runtime_proxy_crate::smart_context_fingerprint_delta(
            state.last_static_context_fingerprints.clone(),
            current.clone(),
        )
    };
    let changed = delta
        .iter()
        .any(runtime_smart_context_fingerprint_change_is_substantive);
    let observation = RuntimeSmartContextStaticContextObservation {
        changed,
        item_count: current.len(),
        delta_count: delta.len(),
    };
    state.last_static_context_fingerprints = current;
    state.last_static_context_prompt_cache_hash = Some(cache.content_hash);
    observation
}

fn runtime_smart_context_fingerprint_change_is_substantive(
    change: &runtime_proxy_crate::SmartContextFingerprintChange,
) -> bool {
    !matches!(
        change,
        runtime_proxy_crate::SmartContextFingerprintChange::Unchanged { .. }
    )
}

fn runtime_smart_context_static_context_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextStaticContextItem> {
    let mut items = Vec::new();
    for key in RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS {
        if let Some(text) = value.get(key).and_then(serde_json::Value::as_str)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: key.to_string(),
                text: text.to_string(),
            });
        }
    }

    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return items;
    };
    for (index, item) in input.iter().enumerate() {
        let Some(object) = item.as_object() else {
            continue;
        };
        let role = object
            .get("role")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if !runtime_smart_context_static_role_is_prompt_prefix(role) {
            continue;
        }
        if let Some(text) = runtime_smart_context_static_message_text(item)
            && !text.trim().is_empty()
        {
            items.push(runtime_proxy_crate::SmartContextStaticContextItem {
                id: format!("input[{index}].{role}"),
                text,
            });
        }
    }
    items.sort_by(|left, right| left.id.cmp(&right.id));
    items
}

fn runtime_smart_context_static_prompt_field_key(key: &str) -> bool {
    RUNTIME_SMART_CONTEXT_STATIC_PROMPT_FIELDS.contains(&key)
}

fn runtime_smart_context_static_role_is_prompt_prefix(role: &str) -> bool {
    matches!(role, "system" | "developer")
}

fn runtime_smart_context_value_is_static_context_item(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("role"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(runtime_smart_context_static_role_is_prompt_prefix)
}

fn runtime_smart_context_static_message_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(text) = object.get("content").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = object.get("input_text").and_then(serde_json::Value::as_str) {
        return Some(text.to_string());
    }

    let content = object.get("content")?;
    let mut parts = Vec::new();
    runtime_smart_context_collect_static_text_parts(content, &mut parts);
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn runtime_smart_context_collect_static_text_parts(
    value: &serde_json::Value,
    parts: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => parts.push(text.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_static_text_parts(item, parts);
            }
        }
        serde_json::Value::Object(object) => {
            for key in ["text", "input_text", "content"] {
                if let Some(item) = object.get(key) {
                    runtime_smart_context_collect_static_text_parts(item, parts);
                }
            }
        }
        _ => {}
    }
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

fn persist_runtime_smart_context_artifacts(shared: &RuntimeRotationProxyShared) {
    let Some(states) = RUNTIME_SMART_CONTEXT_PROXY_STATES.get() else {
        return;
    };
    let Ok(states) = states.lock() else {
        return;
    };
    let Some(state) = states.get(&shared.log_path) else {
        return;
    };
    if !state.enabled {
        return;
    }
    let Some(path) = state.artifact_path.clone() else {
        return;
    };
    let store = state.artifacts.clone();
    schedule_runtime_smart_context_artifact_save(shared, path, store, "smart_context_artifacts");
}

fn runtime_smart_context_exact_header(request: &RuntimeProxyRequest) -> bool {
    runtime_proxy_request_header_value(&request.headers, "x-prodex-smart-context")
        .is_some_and(|value| value.eq_ignore_ascii_case("exact"))
}

fn runtime_smart_context_missing_artifact_refs(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
) -> Vec<String> {
    let ref_ids = runtime_smart_context_collect_rehydratable_artifact_ref_ids(value);
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

fn runtime_smart_context_collect_rehydratable_artifact_ref_ids(
    value: &serde_json::Value,
) -> Vec<String> {
    runtime_smart_context_collect_rehydratable_artifact_refs(value)
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn runtime_smart_context_collect_rehydratable_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_rehydratable_artifact_refs_from_value(value, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_artifact_refs(
    value: &serde_json::Value,
) -> Vec<RuntimeSmartContextArtifactReference> {
    let mut refs = BTreeSet::<RuntimeSmartContextArtifactReference>::new();
    runtime_smart_context_collect_artifact_refs_from_value(value, &mut refs);
    refs.into_iter().collect()
}

fn runtime_smart_context_collect_rehydratable_artifact_refs_from_value(
    value: &serde_json::Value,
    refs: &mut BTreeSet<RuntimeSmartContextArtifactReference>,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(item, refs);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_rehydratable_artifact_refs_from_value(item, refs);
            }
        }
        _ => runtime_smart_context_collect_artifact_refs_from_value(value, refs),
    }
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

fn runtime_smart_context_auto_rehydrate_plan(
    value: &serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    token_budget: usize,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> runtime_proxy_crate::SmartContextRehydratePlan {
    let mut refs_by_id = BTreeMap::<String, usize>::new();
    let mut available_ids = BTreeSet::<String>::new();
    for reference in runtime_smart_context_collect_rehydratable_artifact_refs(value) {
        if store.contains(&reference.id) {
            available_ids.insert(reference.id.clone());
        }
        let token_cost =
            runtime_smart_context_rehydrate_ref_token_cost(&reference, store).unwrap_or(0);
        refs_by_id
            .entry(reference.id)
            .and_modify(|cost| *cost = cost.saturating_add(token_cost))
            .or_insert(token_cost);
    }

    runtime_proxy_crate::smart_context_auto_rehydrate_plan(
        refs_by_id.into_iter().map(|(id, token_cost)| {
            runtime_proxy_crate::SmartContextRehydrateRef {
                id,
                token_cost,
                required: true,
            }
        }),
        available_ids,
        token_budget,
        tier,
    )
}

fn runtime_smart_context_rehydrate_ref_token_cost(
    reference: &RuntimeSmartContextArtifactReference,
    store: &RuntimeSmartContextArtifactStore,
) -> Option<usize> {
    let artifact_text = store.get_text(&reference.id)?;
    let rehydrated =
        runtime_smart_context_rehydrated_artifact_text(&artifact_text, reference.line_range)?;
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(rehydrated.len())
        .try_into()
        .ok()
}

fn runtime_smart_context_deferred_rehydrate_refs(
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
) -> Vec<String> {
    plan.actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { id, .. } => Some(id.clone()),
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { .. } => None,
        })
        .collect()
}

#[cfg(test)]
fn runtime_smart_context_rehydrate_value(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let plan = runtime_smart_context_auto_rehydrate_plan(
        value,
        store,
        usize::MAX,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact,
    );
    runtime_smart_context_rehydrate_value_with_plan(value, store, &plan, stats);
}

fn runtime_smart_context_rehydrate_value_with_plan(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    plan: &runtime_proxy_crate::SmartContextRehydratePlan,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let rehydrate_ids = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextRehydrateAction::Rehydrate { id, .. } => {
                Some(id.clone())
            }
            runtime_proxy_crate::SmartContextRehydrateAction::Defer { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    runtime_smart_context_rehydrate_value_for_ids(value, store, &rehydrate_ids, stats);
}

fn runtime_smart_context_rehydrate_value_for_ids(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    rehydrate_ids: &BTreeSet<String>,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    if runtime_smart_context_value_is_static_context_item(value) {
        return;
    }
    match value {
        serde_json::Value::String(text) => {
            let mut next = text.clone();
            for reference in runtime_smart_context_collect_artifact_refs(
                &serde_json::Value::String(text.clone()),
            ) {
                if rehydrate_ids.contains(&reference.id)
                    && let Some(artifact_text) = store.get_text(&reference.id)
                    && let Some(rehydrated_text) = runtime_smart_context_rehydrated_artifact_text(
                        &artifact_text,
                        reference.line_range,
                    )
                {
                    let marker = format!("prodex-artifact:{}", reference.id);
                    if reference.line_range.is_none()
                        && runtime_smart_context_text_is_artifact_marker_summary(
                            &next,
                            &reference.id,
                        )
                    {
                        next = rehydrated_text;
                        stats.rehydrated_refs += 1;
                        break;
                    } else if next.contains(&reference.marker) {
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
                runtime_smart_context_rehydrate_value_for_ids(item, store, rehydrate_ids, stats);
            }
        }
        serde_json::Value::Object(object) => {
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                runtime_smart_context_rehydrate_value_for_ids(item, store, rehydrate_ids, stats);
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_text_is_artifact_marker_summary(text: &str, id: &str) -> bool {
    let Some(first_line) = text.trim_start().lines().next() else {
        return false;
    };
    (first_line.starts_with("prodex-sc artifact ") || first_line.starts_with("prodex-sc repeat "))
        && first_line.contains(&format!("prodex-artifact:{id}"))
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

fn runtime_smart_context_try_surgical_rehydrate_critical_ranges(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
    request_body: &[u8],
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    unresolved_rehydrate_refs: &[String],
    stats: &RuntimeSmartContextTransformStats,
) -> Option<(Vec<u8>, RuntimeSmartContextTransformStats)> {
    with_runtime_smart_context_artifacts(shared, |store| {
        let mut repaired_value = value.clone();
        let mut repaired_stats = stats.clone();
        if runtime_smart_context_rehydrate_lost_critical_ranges(
            &mut repaired_value,
            store,
            &mut repaired_stats,
        ) == 0
        {
            return None;
        }

        let repaired_body = serde_json::to_vec(&repaired_value).ok()?;
        let regression_check = runtime_smart_context_regression_self_check(
            request_body,
            &repaired_body,
            exactness.clone(),
            unresolved_rehydrate_refs.to_vec(),
        );
        let critical_signal_check =
            runtime_smart_context_critical_signal_self_check(request_body, &repaired_body);
        runtime_smart_context_fallback_exact_reason(
            &regression_check,
            critical_signal_check,
            &repaired_stats,
        )
        .is_none()
        .then_some((repaired_body, repaired_stats))
    })
    .flatten()
}

fn runtime_smart_context_rehydrate_lost_critical_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_rehydrate_lost_critical_ranges_in_text(text, store, stats)
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| runtime_smart_context_rehydrate_lost_critical_ranges(item, store, stats))
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(runtime_smart_context_rehydrate_lost_critical_ranges(
                    item, store, stats,
                ));
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_rehydrate_lost_critical_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(artifact_text) = store.get_text(&id) else {
            continue;
        };
        let artifact_line_index = store.line_index(&id);
        let Some((appendix, range_count)) = runtime_smart_context_missing_critical_range_appendix(
            &id,
            &artifact_text,
            artifact_line_index,
            &next,
        ) else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

enum RuntimeSmartContextIndexedCriticalAppendix {
    Found(String, usize),
    NoLoss,
    Unusable,
}

fn runtime_smart_context_missing_critical_range_appendix(
    artifact_id: &str,
    artifact_text: &str,
    artifact_line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
    current_text: &str,
) -> Option<(String, usize)> {
    if let Some(line_index) = artifact_line_index {
        match runtime_smart_context_missing_indexed_critical_range_appendix(
            artifact_id,
            line_index,
            current_text,
        ) {
            RuntimeSmartContextIndexedCriticalAppendix::Found(appendix, range_count) => {
                return Some((appendix, range_count));
            }
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss => return None,
            RuntimeSmartContextIndexedCriticalAppendix::Unusable => {}
        }
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        artifact_text,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return None;
    }

    let mut rendered = vec!["critical exact ranges:".to_string()];
    let mut range_count = 0usize;
    for range in ranges {
        let exact = runtime_smart_context_rehydrated_artifact_text(
            artifact_text,
            Some(RuntimeSmartContextLineRange {
                start: range.start,
                end: range.end,
            }),
        )?;
        if exact.trim().is_empty() {
            continue;
        }
        rendered.push(format!(
            "prodex-artifact:{artifact_id}#L{}-L{}\n{exact}",
            range.start, range.end
        ));
        range_count = range_count.saturating_add(1);
    }

    (range_count > 0).then_some((rendered.join("\n"), range_count))
}

fn runtime_smart_context_missing_indexed_critical_range_appendix(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
) -> RuntimeSmartContextIndexedCriticalAppendix {
    if line_index.critical_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut synthetic = String::new();
    let mut synthetic_line_to_range = Vec::new();
    for (range_index, range) in line_index.critical_ranges.iter().enumerate() {
        if !runtime_smart_context_artifact_line_index_range_valid(range) {
            return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
        }
        for line in range.text.lines() {
            if !synthetic.is_empty() {
                synthetic.push('\n');
            }
            synthetic.push_str(line);
            synthetic_line_to_range.push(range_index);
        }
    }

    if synthetic_line_to_range.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let lost_synthetic_ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        &synthetic,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 0,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 1,
        },
    );
    if lost_synthetic_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut selected = BTreeSet::<usize>::new();
    for lost_range in lost_synthetic_ranges {
        for synthetic_line in lost_range.start..=lost_range.end {
            let Some(range_index) = synthetic_line_to_range.get(synthetic_line - 1) else {
                return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
            };
            selected.insert(*range_index);
            if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
                break;
            }
        }
        if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
            break;
        }
    }

    let mut rendered = vec!["critical exact ranges:".to_string()];
    for range_index in &selected {
        let range = &line_index.critical_ranges[*range_index];
        if range.text.trim().is_empty() {
            continue;
        }
        rendered.push(format!(
            "prodex-artifact:{artifact_id}#L{}-L{}\n{}",
            range.start, range.end, range.text
        ));
    }

    if rendered.len() <= 1 {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    RuntimeSmartContextIndexedCriticalAppendix::Found(rendered.join("\n"), rendered.len() - 1)
}

fn runtime_smart_context_artifact_line_index_range_valid(
    range: &RuntimeSmartContextArtifactLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}

fn runtime_smart_context_condense_tool_outputs(
    value: &mut serde_json::Value,
    store: &mut RuntimeSmartContextArtifactStore,
    request_id: u64,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    inline_limit: usize,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    let tool_call_metadata = runtime_smart_context_tool_call_metadata_by_call_id(input);
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
        let linked_metadata = runtime_smart_context_tool_call_id(object)
            .and_then(|call_id| tool_call_metadata.get(call_id))
            .map(String::as_str);
        let command_kind_hint =
            runtime_smart_context_tool_output_kind_hint(object, linked_metadata);
        for field in ["output", "content"] {
            let Some(output) = object
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
            else {
                continue;
            };
            if output.len() <= inline_limit.max(256) {
                continue;
            }
            let existing_artifact = store.artifact_ref_for_exact_text(&output);
            let Some(artifact) = existing_artifact
                .clone()
                .or_else(|| store.insert_text(request_id, &output))
            else {
                continue;
            };
            let replacement = if existing_artifact.is_some() {
                stats.repeat_tool_output_refs += 1;
                runtime_smart_context_artifact_reference_summary(&artifact)
            } else {
                let compacted = runtime_smart_context_compact_tool_output_preserving_critical(
                    &output,
                    tier,
                    inline_limit,
                    command_kind_hint,
                );
                runtime_smart_context_artifact_summary(&artifact, &compacted)
            };
            if replacement.len().saturating_mul(100) >= output.len().saturating_mul(90) {
                continue;
            }
            object.insert(field.to_string(), serde_json::Value::String(replacement));
            if existing_artifact.is_none() {
                stats.artifacts_stored += 1;
            }
            stats.tool_outputs_condensed += 1;
            if runtime_smart_context_likely_blob_or_noise(&output) {
                stats.blob_outputs_condensed += 1;
            }
            break;
        }
    }
}

fn runtime_smart_context_compact_tool_output_preserving_critical(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
) -> String {
    let compacted = runtime_smart_context_compact_tool_output(
        text,
        tier,
        preview_byte_limit,
        command_kind_hint,
    );
    runtime_smart_context_append_missing_critical_ranges(
        text,
        compacted,
        SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
    )
}

fn runtime_smart_context_compact_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
) -> String {
    let Some(max_lines) = runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit)
    else {
        return text.to_string();
    };
    prodex_context::compact_command_output_with_options_and_kind_hint(
        text,
        &prodex_context::CommandOutputCompactOptions {
            max_lines,
            head_lines: max_lines.saturating_mul(2) / 3,
            tail_lines: max_lines / 3,
            max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
            ..prodex_context::CommandOutputCompactOptions::default()
        },
        command_kind_hint,
    )
    .output
}

fn runtime_smart_context_tool_preview_max_lines(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
) -> Option<usize> {
    if tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
        && preview_byte_limit == usize::MAX
    {
        return None;
    }

    let budget_lines = preview_byte_limit / SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES;
    let (floor, cap) = match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => (8, 24),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => (24, 80),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => (80, 240),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => (120, 360),
    };
    Some(budget_lines.max(floor).min(cap))
}

fn runtime_smart_context_tool_call_metadata_by_call_id(
    input: &[serde_json::Value],
) -> BTreeMap<String, String> {
    let mut metadata_by_call_id = BTreeMap::new();
    for item in input {
        let Some(object) = item.as_object() else {
            continue;
        };
        let Some(call_id) = runtime_smart_context_tool_call_id(object) else {
            continue;
        };
        let item_type = object
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if item_type.ends_with("_call_output") {
            continue;
        }
        let metadata = runtime_smart_context_tool_output_metadata(object);
        if !metadata.trim().is_empty() {
            metadata_by_call_id.insert(call_id.to_string(), metadata);
        }
    }
    metadata_by_call_id
}

fn runtime_smart_context_tool_output_kind_hint(
    object: &serde_json::Map<String, serde_json::Value>,
    linked_metadata: Option<&str>,
) -> Option<prodex_context::CommandOutputKind> {
    let mut metadata = linked_metadata.unwrap_or_default().to_string();
    runtime_smart_context_push_tool_output_metadata(
        &runtime_smart_context_tool_output_metadata(object),
        &mut metadata,
        SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS,
    );
    prodex_context::infer_command_output_kind_from_metadata(&metadata)
}

const SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS: usize = 8192;

fn runtime_smart_context_tool_output_metadata(
    object: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let mut metadata = String::new();
    for (field, child) in object {
        runtime_smart_context_collect_tool_output_metadata(child, Some(field), 0, &mut metadata);
        if metadata.len() >= SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS {
            break;
        }
    }
    metadata
}

fn runtime_smart_context_tool_call_id(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Option<&str> {
    ["call_id", "tool_call_id", "id"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
        .filter(|value| !value.trim().is_empty())
}

fn runtime_smart_context_collect_tool_output_metadata(
    value: &serde_json::Value,
    key: Option<&str>,
    depth: usize,
    metadata: &mut String,
) {
    const MAX_DEPTH: usize = 8;

    if metadata.len() >= SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS || depth > MAX_DEPTH {
        return;
    }
    if key.is_some_and(runtime_smart_context_tool_output_payload_field) {
        return;
    }
    if let Some(key) = key {
        runtime_smart_context_push_tool_output_metadata(
            key,
            metadata,
            SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS,
        );
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_push_tool_output_metadata(
                text,
                metadata,
                SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS,
            );
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_smart_context_collect_tool_output_metadata(item, None, depth + 1, metadata);
                if metadata.len() >= SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS {
                    break;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (field, child) in map {
                runtime_smart_context_collect_tool_output_metadata(
                    child,
                    Some(field),
                    depth + 1,
                    metadata,
                );
                if metadata.len() >= SMART_CONTEXT_TOOL_METADATA_HINT_MAX_CHARS {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_tool_output_payload_field(key: &str) -> bool {
    matches!(key, "output" | "content")
}

fn runtime_smart_context_push_tool_output_metadata(
    text: &str,
    metadata: &mut String,
    max_chars: usize,
) {
    if metadata.len() >= max_chars {
        return;
    }
    if !metadata.is_empty() {
        metadata.push(' ');
    }
    let remaining = max_chars.saturating_sub(metadata.len());
    metadata.extend(text.chars().take(remaining));
}

fn runtime_smart_context_artifact_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    compacted: &str,
) -> String {
    runtime_proxy_crate::smart_context_artifact_marker(artifact, compacted)
}

fn runtime_smart_context_artifact_reference_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
) -> String {
    format!("prodex-artifact:{}", artifact.id)
}

fn runtime_smart_context_append_missing_critical_ranges(
    original: &str,
    compacted: String,
    max_ranges: usize,
) -> String {
    if !prodex_context::critical_signal_self_check(original, &compacted).has_loss() {
        return compacted;
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        original,
        &compacted,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return compacted;
    }

    let lines = original.lines().collect::<Vec<_>>();
    let mut exact_ranges = Vec::new();
    for range in ranges {
        if range.start == 0 || range.start > lines.len() {
            continue;
        }
        let end = range.end.min(lines.len());
        exact_ranges.push(format!(
            "L{}-L{}:\n{}",
            range.start,
            end,
            lines[range.start - 1..end].join("\n")
        ));
    }
    if exact_ranges.is_empty() {
        return compacted;
    }

    format!(
        "{compacted}\n\ncritical exact ranges:\n{}",
        exact_ranges.join("\n")
    )
}

fn runtime_smart_context_dedupe_input_text(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
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
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        runtime_smart_context_dedupe_value_text(item, index, &mut seen, stats);
    }
    runtime_smart_context_replace_cross_turn_duplicate_refs(value, store, exactness_guard, stats);
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
                seen.insert(hash.clone(), item_index);
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

fn runtime_smart_context_replace_cross_turn_duplicate_refs(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    exactness_guard: &runtime_proxy_crate::SmartContextExactnessGuard,
    stats: &mut RuntimeSmartContextTransformStats,
) {
    let items = runtime_smart_context_collect_large_input_text_items(value);
    if items.is_empty() {
        return;
    }
    let artifacts = runtime_smart_context_available_artifacts_for_text_items(items.iter(), store);
    let plan = runtime_proxy_crate::smart_context_cross_turn_duplicate_ref_plan(
        items,
        artifacts,
        SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES,
        exactness_guard,
    );
    let replacements = plan
        .actions
        .into_iter()
        .filter_map(|action| match action {
            runtime_proxy_crate::SmartContextCrossTurnDuplicateRefAction::ReplaceWithArtifactRef {
                id,
                artifact,
                content_hash,
                byte_len,
            } => Some((
                id,
                format!(
                    "[prodex smart context repeated artifact prodex-artifact:{} content_hash={} original_bytes={}]",
                    artifact.id, content_hash, byte_len
                ),
            )),
            runtime_proxy_crate::SmartContextCrossTurnDuplicateRefAction::Keep { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    if replacements.is_empty() {
        return;
    }
    stats.cross_turn_duplicate_texts +=
        runtime_smart_context_apply_text_replacements(value, &replacements);
}

fn runtime_smart_context_available_artifacts_for_text_items<'a>(
    items: impl IntoIterator<Item = &'a runtime_proxy_crate::SmartContextConversationItem>,
    store: &RuntimeSmartContextArtifactStore,
) -> Vec<runtime_proxy_crate::SmartContextArtifactRef> {
    items
        .into_iter()
        .filter_map(|item| {
            let content_hash = runtime_proxy_crate::smart_context_hash_text(&item.text);
            let artifact_text = store.get_text(&content_hash)?;
            (artifact_text == item.text).then_some(runtime_proxy_crate::SmartContextArtifactRef {
                id: content_hash.clone(),
                byte_len: item.text.len(),
                content_hash,
            })
        })
        .collect()
}

fn runtime_smart_context_collect_large_input_text_items(
    value: &serde_json::Value,
) -> Vec<runtime_proxy_crate::SmartContextConversationItem> {
    let Some(input) = value.get("input").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for (index, item) in input.iter().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        runtime_smart_context_collect_large_text_items_from_value(
            item,
            format!("input[{index}]"),
            &mut items,
        );
    }
    items
}

fn runtime_smart_context_collect_large_text_items_from_value(
    value: &serde_json::Value,
    id: String,
    items: &mut Vec<runtime_proxy_crate::SmartContextConversationItem>,
) {
    match value {
        serde_json::Value::String(text)
            if text.len() >= SMART_CONTEXT_DUPLICATE_TEXT_MIN_BYTES
                && !text.contains("prodex-artifact:")
                && !text.contains("prodex smart context artifact") =>
        {
            items.push(runtime_proxy_crate::SmartContextConversationItem {
                id,
                text: text.clone(),
            });
        }
        serde_json::Value::Array(values) => {
            for (index, item) in values.iter().enumerate() {
                runtime_smart_context_collect_large_text_items_from_value(
                    item,
                    format!("{id}[{index}]"),
                    items,
                );
            }
        }
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(item) = object.get(&key) {
                    runtime_smart_context_collect_large_text_items_from_value(
                        item,
                        format!("{id}.{key}"),
                        items,
                    );
                }
            }
        }
        _ => {}
    }
}

fn runtime_smart_context_apply_text_replacements(
    value: &mut serde_json::Value,
    replacements: &BTreeMap<String, String>,
) -> usize {
    let Some(input) = value
        .get_mut("input")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return 0;
    };
    let mut replaced = 0usize;
    for (index, item) in input.iter_mut().enumerate() {
        if runtime_smart_context_value_is_static_context_item(item) {
            continue;
        }
        replaced += runtime_smart_context_apply_text_replacements_to_value(
            item,
            format!("input[{index}]"),
            replacements,
        );
    }
    replaced
}

fn runtime_smart_context_apply_text_replacements_to_value(
    value: &mut serde_json::Value,
    id: String,
    replacements: &BTreeMap<String, String>,
) -> usize {
    match value {
        serde_json::Value::String(text) => {
            if let Some(replacement) = replacements.get(&id) {
                *text = replacement.clone();
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(values) => values
            .iter_mut()
            .enumerate()
            .map(|(index, item)| {
                runtime_smart_context_apply_text_replacements_to_value(
                    item,
                    format!("{id}[{index}]"),
                    replacements,
                )
            })
            .sum(),
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.into_iter()
                .filter_map(|key| {
                    object.get_mut(&key).map(|item| {
                        runtime_smart_context_apply_text_replacements_to_value(
                            item,
                            format!("{id}.{key}"),
                            replacements,
                        )
                    })
                })
                .sum()
        }
        _ => 0,
    }
}

fn runtime_smart_context_likely_blob_or_noise(text: &str) -> bool {
    prodex_context::is_context_blob_noise(text)
}

fn runtime_smart_context_critical_signal_self_check(
    before: &[u8],
    after: &[u8],
) -> prodex_context::CriticalSignalSelfCheck {
    let before = String::from_utf8_lossy(before);
    let after = String::from_utf8_lossy(after);
    prodex_context::critical_signal_self_check(&before, &after)
}

fn runtime_smart_context_regression_self_check(
    before: &[u8],
    after: &[u8],
    exactness_guard: runtime_proxy_crate::SmartContextExactnessGuard,
    missing_rehydrate_refs: Vec<String>,
) -> runtime_proxy_crate::SmartContextRegressionSelfCheck {
    let before_text = String::from_utf8_lossy(before);
    let after_text = String::from_utf8_lossy(after);
    runtime_proxy_crate::smart_context_regression_self_check(
        runtime_proxy_crate::SmartContextRegressionSelfCheckInput {
            exactness_guard,
            before_hash: runtime_proxy_crate::smart_context_hash_text(&before_text),
            after_hash: runtime_proxy_crate::smart_context_hash_text(&after_text),
            before_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(before.len()),
            after_estimated_tokens:
                runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(after.len()),
            before_critical_signal_count: prodex_context::count_critical_signals(&before_text)
                .total(),
            after_critical_signal_count: prodex_context::count_critical_signals(&after_text)
                .total(),
            missing_rehydrate_refs,
        },
    )
}

fn runtime_smart_context_fallback_exact_reason(
    regression_check: &runtime_proxy_crate::SmartContextRegressionSelfCheck,
    critical_signal_check: prodex_context::CriticalSignalSelfCheck,
    stats: &RuntimeSmartContextTransformStats,
) -> Option<&'static str> {
    if critical_signal_check.has_loss() {
        return Some("critical_signal_loss");
    }
    if runtime_smart_context_rewrite_is_rehydrate_only(stats) {
        return None;
    }
    if regression_check.decision
        == runtime_proxy_crate::SmartContextRegressionSelfCheckDecision::FallbackExact
    {
        return Some(runtime_smart_context_regression_reason_label(
            &regression_check.reasons,
        ));
    }
    None
}

fn runtime_smart_context_rewrite_is_rehydrate_only(
    stats: &RuntimeSmartContextTransformStats,
) -> bool {
    stats.rehydrated_refs > 0
        && stats.tool_outputs_condensed == 0
        && stats.duplicate_texts == 0
        && stats.cross_turn_duplicate_texts == 0
        && stats.repeat_tool_output_refs == 0
}

fn runtime_smart_context_regression_reason_label(
    reasons: &[runtime_proxy_crate::SmartContextRegressionSelfCheckReason],
) -> &'static str {
    if reasons
        .iter()
        .any(|reason| matches!(reason, runtime_proxy_crate::SmartContextRegressionSelfCheckReason::CriticalSignalDropped))
    {
        "critical_signal_loss"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::MissingRehydrateRefs
        )
    }) {
        "missing_rehydrate_refs"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::ExactnessRequiredButPayloadChanged
        )
    }) {
        "exactness_required"
    } else if reasons.iter().any(|reason| {
        matches!(
            reason,
            runtime_proxy_crate::SmartContextRegressionSelfCheckReason::EmptyAfterPayload
        )
    }) {
        "empty_after_payload"
    } else {
        "token_budget_did_not_improve"
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
    budget: &RuntimeSmartContextBudget,
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
                    "model_context_window_tokens",
                    budget.model_context_window_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "model_context_window_source",
                    budget.model_context_window_source,
                ),
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
                runtime_proxy_log_field(
                    "budget_mode",
                    runtime_smart_context_budget_mode_label(budget.policy.mode),
                ),
                runtime_proxy_log_field(
                    "max_inline_tool_output_bytes",
                    budget.policy.max_inline_tool_output_bytes.to_string(),
                ),
                runtime_proxy_log_field(
                    "max_rehydrate_tokens",
                    budget.policy.max_rehydrate_tokens.to_string(),
                ),
                runtime_proxy_log_field(
                    "policy_reasons",
                    runtime_smart_context_budget_policy_reason_labels(&budget.policy.reasons),
                ),
                runtime_proxy_log_field("artifacts_stored", stats.artifacts_stored.to_string()),
                runtime_proxy_log_field(
                    "tool_outputs_condensed",
                    stats.tool_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("duplicate_texts", stats.duplicate_texts.to_string()),
                runtime_proxy_log_field(
                    "cross_turn_duplicate_texts",
                    stats.cross_turn_duplicate_texts.to_string(),
                ),
                runtime_proxy_log_field(
                    "repeat_tool_output_refs",
                    stats.repeat_tool_output_refs.to_string(),
                ),
                runtime_proxy_log_field(
                    "blob_outputs_condensed",
                    stats.blob_outputs_condensed.to_string(),
                ),
                runtime_proxy_log_field("rehydrated_refs", stats.rehydrated_refs.to_string()),
            ],
        ),
    );
}

fn runtime_smart_context_budget_mode_label(
    mode: runtime_proxy_crate::SmartContextBudgetMode,
) -> &'static str {
    match mode {
        runtime_proxy_crate::SmartContextBudgetMode::ExactPassThrough => "exact_pass_through",
        runtime_proxy_crate::SmartContextBudgetMode::LargeLossless => "large_lossless",
        runtime_proxy_crate::SmartContextBudgetMode::ArtifactCondensed => "artifact_condensed",
        runtime_proxy_crate::SmartContextBudgetMode::MinimalRefsOnly => "minimal_refs_only",
    }
}

fn runtime_smart_context_budget_policy_reason_labels(
    reasons: &[runtime_proxy_crate::SmartContextBudgetPolicyReason],
) -> String {
    if reasons.is_empty() {
        return "-".to_string();
    }
    reasons
        .iter()
        .map(|reason| match reason {
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ExactnessRequired => {
                "exactness_required"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::StaticContextChanged => {
                "static_context_changed"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::MissingRehydrateRefs => {
                "missing_rehydrate_refs"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnknownTokenWindow => {
                "unknown_token_window"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::UnsafeAccounting => {
                "unsafe_accounting"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::RecentRewriteSavingsSafe => {
                "recent_rewrite_savings_safe"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::PlentyOfBudget => {
                "plenty_of_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::ModerateBudget => {
                "moderate_budget"
            }
            runtime_proxy_crate::SmartContextBudgetPolicyReason::TightBudget => "tight_budget",
            runtime_proxy_crate::SmartContextBudgetPolicyReason::CriticalBudget => {
                "critical_budget"
            }
        })
        .collect::<Vec<_>>()
        .join(",")
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

fn runtime_smart_context_saved_tokens(body_bytes_before: usize, body_bytes_after: usize) -> u64 {
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_before)
        .saturating_sub(
            runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(body_bytes_after),
        )
}

fn runtime_smart_context_minified_json_body(
    value: &serde_json::Value,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_value_body(original_body, value)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_minified_json_body_from_original(original_body: &[u8]) -> Option<Vec<u8>> {
    let Cow::Owned(body) =
        runtime_proxy_crate::smart_context_structural_minify_json_body(original_body)
    else {
        return None;
    };
    runtime_smart_context_validated_minified_json_body(body, original_body)
}

fn runtime_smart_context_validated_minified_json_body(
    body: Vec<u8>,
    original_body: &[u8],
) -> Option<Vec<u8>> {
    if body.len() >= original_body.len() {
        return None;
    }
    let before = String::from_utf8_lossy(original_body);
    let after = String::from_utf8_lossy(&body);
    prodex_context::critical_signal_self_check(&before, &after)
        .passed()
        .then_some(body)
}

#[cfg(test)]
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
