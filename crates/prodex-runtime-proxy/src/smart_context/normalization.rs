use super::*;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub fn smart_context_hash_text(text: &str) -> String {
    format!("sc:{:016x}", smart_context_fnv1a64(text.as_bytes()))
}

pub fn smart_context_normalized_command_output_hash_text(text: &str) -> String {
    let normalized = smart_context_normalize_volatile_command_output(text);
    format!(
        "scv:{:016x}",
        smart_context_fnv1a64(normalized.as_ref().as_bytes())
    )
}

pub fn smart_context_normalize_volatile_command_output(text: &str) -> Cow<'_, str> {
    let mut normalized = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0usize;

    while index < text.len() {
        let rest = &text[index..];
        let previous = smart_context_previous_char(text, index);

        if let Some(len) = smart_context_ansi_escape_len(rest) {
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_temp_path_len(rest) {
            normalized.push_str("<tmp-path>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_timestamp_len(rest, previous) {
            normalized.push_str("<timestamp>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_progress_counter_len(rest, previous) {
            normalized.push_str("<progress>");
            index += len;
            changed = true;
            continue;
        }
        if let Some((len, replacement)) =
            smart_context_labeled_random_id_replacement(rest, previous)
        {
            normalized.push_str(&replacement);
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_uuid_len(rest, previous) {
            normalized.push_str("<id>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_duration_len(rest, previous) {
            normalized.push_str("<duration>");
            index += len;
            changed = true;
            continue;
        }

        let ch = rest.chars().next().expect("index is within string");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    if changed {
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(text)
    }
}

pub fn smart_context_normalize_volatile_static_context(text: &str) -> Cow<'_, str> {
    let mut normalized = String::with_capacity(text.len());
    let mut changed = false;
    let mut index = 0usize;

    while index < text.len() {
        let rest = &text[index..];
        let previous = smart_context_previous_char(text, index);

        if let Some(len) = smart_context_ansi_escape_len(rest) {
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_temp_path_len(rest) {
            normalized.push_str("<tmp-path>");
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_timestamp_len(rest, previous) {
            normalized.push_str("<timestamp>");
            index += len;
            changed = true;
            continue;
        }
        if let Some((len, replacement)) =
            smart_context_labeled_random_id_replacement(rest, previous)
        {
            normalized.push_str(&replacement);
            index += len;
            changed = true;
            continue;
        }
        if let Some(len) = smart_context_uuid_len(rest, previous) {
            normalized.push_str("<id>");
            index += len;
            changed = true;
            continue;
        }

        let ch = rest.chars().next().expect("index is within string");
        normalized.push(ch);
        index += ch.len_utf8();
    }

    if changed {
        Cow::Owned(normalized)
    } else {
        Cow::Borrowed(text)
    }
}

pub(super) fn smart_context_effective_input_source(
    current_input_tokens: u64,
    estimated_current_request_tokens: u64,
    current_request_accounted_tokens: u64,
    last_accounted_input_tokens: u64,
    effective_input_tokens: u64,
) -> SmartContextTokenAccountingSource {
    if effective_input_tokens == 0 {
        SmartContextTokenAccountingSource::Unknown
    } else if last_accounted_input_tokens > current_request_accounted_tokens {
        SmartContextTokenAccountingSource::ObservedHistory
    } else if current_input_tokens >= estimated_current_request_tokens && current_input_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestTokens
    } else if estimated_current_request_tokens > 0 {
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    } else if last_accounted_input_tokens > 0 {
        SmartContextTokenAccountingSource::ObservedHistory
    } else {
        SmartContextTokenAccountingSource::Unknown
    }
}

pub(super) fn smart_context_token_accounting_risks(
    model_context_window_tokens: Option<u64>,
    reserved_output_tokens: u64,
    effective_input_source: SmartContextTokenAccountingSource,
) -> Vec<SmartContextTokenAccountingRisk> {
    let mut risks = Vec::new();

    match model_context_window_tokens {
        Some(0) => risks.push(SmartContextTokenAccountingRisk::ZeroContextWindow),
        Some(window) if reserved_output_tokens >= window => {
            risks.push(SmartContextTokenAccountingRisk::ReservedOutputConsumesWindow);
        }
        Some(_) => {}
        None => risks.push(SmartContextTokenAccountingRisk::UnknownTokenWindow),
    }
    if effective_input_source == SmartContextTokenAccountingSource::Unknown {
        risks.push(SmartContextTokenAccountingRisk::UnknownCurrentRequestAccounting);
    }

    risks
}

pub(super) fn smart_context_u64_budget_tier(available_tokens: u64) -> SmartContextTokenBudgetTier {
    if available_tokens > usize::MAX as u64 {
        SmartContextTokenBudgetTier::Exact
    } else {
        smart_context_token_budget_tier(available_tokens as usize)
    }
}

pub(super) fn smart_context_u64_saturating_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

pub(super) fn smart_context_memory_capsule_policy_allows_unbounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && policy.mode == SmartContextBudgetMode::ExactPassThrough
        && policy.tier == SmartContextTokenBudgetTier::Exact
        && policy.reasons == [SmartContextBudgetPolicyReason::PlentyOfBudget]
}

pub(super) fn smart_context_memory_capsule_policy_allows_bounded_budget(
    accounting: &SmartContextObservedTokenAccounting,
    policy: &SmartContextAdaptiveBudgetPolicy,
) -> bool {
    smart_context_accounting_safe_for_adaptive_policy(accounting)
        && !policy.reasons.iter().any(|reason| {
            matches!(
                reason,
                SmartContextBudgetPolicyReason::ExactnessRequired
                    | SmartContextBudgetPolicyReason::StaticContextChanged
                    | SmartContextBudgetPolicyReason::MissingRehydrateRefs
                    | SmartContextBudgetPolicyReason::UnknownTokenWindow
                    | SmartContextBudgetPolicyReason::UnsafeAccounting
            )
        })
}

pub(super) fn smart_context_recent_rewrite_min_saved_tokens(rewrite_count: usize) -> u64 {
    SMART_CONTEXT_RECENT_SAFE_REWRITE_MIN_SAVED_TOKENS
        .saturating_mul(u64::try_from(rewrite_count).unwrap_or(u64::MAX))
}

pub(super) fn smart_context_learned_rewrite_policy_exact(
    base_policy: SmartContextAdaptiveBudgetPolicy,
    reason: SmartContextLearnedRewritePolicyReason,
    matching_telemetry_samples: usize,
    safety_samples: usize,
    available_context_tokens: Option<u64>,
) -> SmartContextLearnedRewritePolicy {
    SmartContextLearnedRewritePolicy {
        policy: SmartContextAdaptiveBudgetPolicy {
            tier: base_policy.tier,
            mode: SmartContextBudgetMode::ExactPassThrough,
            max_inline_bytes: usize::MAX,
            max_inline_tool_output_bytes: usize::MAX,
            max_rehydrate_tokens: available_context_tokens.unwrap_or(u64::MAX),
            reasons: base_policy.reasons,
        },
        decision: SmartContextRewriteBudgetDecision::NoChange,
        reasons: vec![reason],
        matching_telemetry_samples,
        safety_samples,
    }
}

pub(super) fn smart_context_rewrite_policy_bucket_key_complete(
    bucket_key: &SmartContextRewritePolicyBucketKey,
) -> bool {
    bucket_key.route.as_deref().is_some_and(non_empty)
        && bucket_key
            .model
            .as_deref()
            .and_then(|model| smart_context_token_calibration_normalized_model(Some(model)))
            .is_some()
        && bucket_key.profile.as_deref().is_some_and(non_empty)
}

pub(super) fn smart_context_rewrite_policy_bucket_key_matches(
    target: &SmartContextRewritePolicyBucketKey,
    sample: Option<&SmartContextRewritePolicyBucketKey>,
) -> bool {
    let Some(sample) = sample else {
        return false;
    };
    smart_context_rewrite_policy_field_matches(&target.route, &sample.route)
        && smart_context_rewrite_policy_model_matches(&target.model, &sample.model)
        && smart_context_rewrite_policy_field_matches(&target.profile, &sample.profile)
}

pub(super) fn smart_context_rewrite_policy_field_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    target.as_deref().is_some_and(non_empty) && target == sample
}

pub(super) fn smart_context_rewrite_policy_model_matches(
    target: &Option<String>,
    sample: &Option<String>,
) -> bool {
    let Some(target) = smart_context_token_calibration_normalized_model(target.as_deref()) else {
        return false;
    };
    let Some(sample) = smart_context_token_calibration_normalized_model(sample.as_deref()) else {
        return false;
    };
    target == sample
}

pub(super) fn smart_context_rewrite_telemetry_sample_safe_saved(
    sample: &SmartContextRewriteTelemetrySample,
) -> bool {
    sample.safe
        && sample.estimated_tokens_after < sample.estimated_tokens_before
        && sample.body_bytes_after < sample.body_bytes_before
}

pub(super) fn smart_context_rewrite_telemetry_saved_tokens(
    samples: &[SmartContextRewriteTelemetrySample],
) -> u64 {
    samples.iter().fold(0u64, |total, sample| {
        total.saturating_add(
            sample
                .estimated_tokens_before
                .saturating_sub(sample.estimated_tokens_after),
        )
    })
}

pub(super) fn smart_context_rewrite_telemetry_average_body_ratio_percent(
    samples: &[SmartContextRewriteTelemetrySample],
) -> usize {
    if samples.is_empty() {
        return 100;
    }
    samples.iter().fold(0usize, |total, sample| {
        total.saturating_add(smart_context_rewrite_body_ratio_percent(
            sample.body_bytes_before,
            sample.body_bytes_after,
        ))
    }) / samples.len()
}

pub(super) fn smart_context_transform_rewrite_safety_score_empty(
    category: SmartContextTransformCategory,
    safety: &SmartContextRecentRewriteSafety,
) -> SmartContextTransformRewriteSafetyScore {
    let safety_samples = safety
        .safe_rewrites
        .saturating_add(safety.fallback_rewrites);
    let decision = smart_context_recent_rewrite_safety_budget_decision(safety);
    let mut reasons = vec![SmartContextTransformRewriteSafetyReason::NoTransformSamples];
    if safety_samples > 0 {
        reasons.push(SmartContextTransformRewriteSafetyReason::RecentSafetyFallback);
        reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
            decision,
        ));
    }

    SmartContextTransformRewriteSafetyScore {
        category,
        decision,
        safety_score: smart_context_transform_rewrite_safety_score_value(decision),
        telemetry_samples: 0,
        safety_samples,
        safe_samples: 0,
        fallback_samples: 0,
        unsafe_samples: 0,
        weak_savings_samples: 0,
        saved_tokens: 0,
        average_body_ratio_percent: None,
        reasons,
    }
}

pub(super) fn smart_context_transform_rewrite_safety_reasons_for_decision(
    decision: SmartContextRewriteBudgetDecision,
) -> Vec<SmartContextTransformRewriteSafetyReason> {
    match decision {
        SmartContextRewriteBudgetDecision::NoChange => {
            vec![SmartContextTransformRewriteSafetyReason::ModerateSavings]
        }
        SmartContextRewriteBudgetDecision::Relax => {
            vec![SmartContextTransformRewriteSafetyReason::StableSavings]
        }
        SmartContextRewriteBudgetDecision::Tighten => {
            vec![SmartContextTransformRewriteSafetyReason::WeakSavings]
        }
    }
}

pub(super) fn smart_context_transform_rewrite_safety_score_value(
    decision: SmartContextRewriteBudgetDecision,
) -> i32 {
    match decision {
        SmartContextRewriteBudgetDecision::NoChange => 0,
        SmartContextRewriteBudgetDecision::Relax => 100,
        SmartContextRewriteBudgetDecision::Tighten => -100,
    }
}

pub(super) fn smart_context_rewrite_body_ratio_percent(
    body_bytes_before: usize,
    body_bytes_after: usize,
) -> usize {
    if body_bytes_before == 0 {
        return 100;
    }
    body_bytes_after.saturating_mul(100) / body_bytes_before
}

pub(super) fn smart_context_relaxed_inline_budget(
    tier: SmartContextTokenBudgetTier,
    value: usize,
) -> usize {
    if value == 0 || value == usize::MAX {
        return value;
    }

    if tier == SmartContextTokenBudgetTier::Large {
        let cap = 64 * 1024;
        if value >= cap {
            return value;
        }
        return value.saturating_mul(2).min(cap).max(value);
    }

    smart_context_scale_usize_ceil(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR,
    )
    .max(value)
}

pub(super) fn smart_context_relaxed_rehydrate_budget(value: u64) -> u64 {
    if value == 0 || value == u64::MAX {
        return value;
    }
    smart_context_scale_u64_ceil(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_RELAX_DENOMINATOR,
    )
    .max(value)
}

pub(super) fn smart_context_tightened_inline_budget(value: usize) -> usize {
    if value <= SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES {
        return value;
    }
    smart_context_scale_usize_floor(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR,
    )
    .max(SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_INLINE_BYTES)
    .min(value)
}

pub(super) fn smart_context_tightened_rehydrate_budget(value: u64) -> u64 {
    if value <= SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS {
        return value;
    }
    smart_context_scale_u64_floor(
        value,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_NUMERATOR,
        SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_DENOMINATOR,
    )
    .max(SMART_CONTEXT_REWRITE_BUDGET_TIGHTEN_MIN_REHYDRATE_TOKENS)
    .min(value)
}

pub(super) fn smart_context_scale_usize_ceil(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_ceil(value, numerator, denominator))
}

pub(super) fn smart_context_scale_usize_floor(
    value: usize,
    numerator: u64,
    denominator: u64,
) -> usize {
    let value = u64::try_from(value).unwrap_or(u64::MAX);
    smart_context_u64_saturating_usize(smart_context_scale_u64_floor(value, numerator, denominator))
}

pub(super) fn smart_context_scale_u64_ceil(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return value;
    }
    value
        .saturating_mul(numerator)
        .saturating_add(denominator - 1)
        / denominator
}

pub(super) fn smart_context_scale_u64_floor(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return value;
    }
    value.saturating_mul(numerator) / denominator
}

pub(super) fn smart_context_fingerprint_map(
    fingerprints: impl IntoIterator<Item = SmartContextFingerprint>,
) -> BTreeMap<(SmartContextFingerprintKind, String), SmartContextFingerprint> {
    fingerprints
        .into_iter()
        .map(|fingerprint| ((fingerprint.kind, fingerprint.id.clone()), fingerprint))
        .collect()
}

pub(super) fn smart_context_available_artifacts_by_hash_and_len(
    artifacts: impl IntoIterator<Item = SmartContextArtifactRef>,
) -> BTreeMap<(String, usize), SmartContextArtifactRef> {
    let mut artifacts = artifacts
        .into_iter()
        .filter(|artifact| non_empty(&artifact.id) && non_empty(&artifact.content_hash))
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.content_hash
            .cmp(&right.content_hash)
            .then_with(|| left.byte_len.cmp(&right.byte_len))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut available = BTreeMap::new();
    for artifact in artifacts {
        available
            .entry((artifact.content_hash.clone(), artifact.byte_len))
            .or_insert(artifact);
    }

    available
}

pub(super) fn smart_context_command_output_keep_exact(
    record: SmartContextCommandOutputCacheRecord,
    output: String,
    reason: SmartContextCommandOutputCacheKeepReason,
    summary: Option<String>,
) -> SmartContextCommandOutputCacheRewrite {
    SmartContextCommandOutputCacheRewrite {
        record,
        output,
        action: SmartContextCommandOutputCacheAction::KeepExact { reason, summary },
    }
}

pub(super) fn smart_context_command_output_cache_record_valid(
    record: &SmartContextCommandOutputCacheRecord,
) -> bool {
    non_empty(&record.content_hash) && record.byte_len > 0
}

pub(super) fn smart_context_command_output_unchanged_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
    text: &str,
) -> String {
    let mut summary = format!(
        "psc co same id={} ref={} h={} b={} tok={} vn-repeat omitted",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        current.content_hash,
        current.byte_len,
        current.estimated_tokens,
    );

    let critical_signals = smart_context_command_output_critical_signals(text);
    if critical_signals.count > 0 {
        summary.push('\n');
        summary.push_str(&format!(
            "psc co crit n={} h={}",
            critical_signals.count, current.content_hash
        ));
        for sample in critical_signals.samples {
            summary.push('\n');
            summary.push_str("sig: ");
            summary.push_str(&sample);
        }
    }

    summary
}

pub(super) fn smart_context_command_output_changed_summary(
    current: &SmartContextCommandOutputCacheRecord,
    previous: &SmartContextCommandOutputCacheRecord,
) -> String {
    let byte_delta = smart_context_signed_delta(current.byte_len, previous.byte_len);
    let token_delta =
        smart_context_signed_delta_u64(current.estimated_tokens, previous.estimated_tokens);
    format!(
        "psc co delta id={} ref={} old_h={} new_h={} old_b={} new_b={} old_tok={} new_tok={} db={} dtok={} exact kept",
        smart_context_command_output_label(&current.id),
        smart_context_command_output_label(&previous.id),
        previous.content_hash,
        current.content_hash,
        previous.byte_len,
        current.byte_len,
        previous.estimated_tokens,
        current.estimated_tokens,
        byte_delta,
        token_delta,
    )
}

pub(super) fn smart_context_command_output_label(value: &str) -> String {
    let mut label = String::new();
    for value in value.trim().chars() {
        if label.len() >= 96 {
            break;
        }
        let replacement = if value.is_ascii_alphanumeric()
            || matches!(value, ':' | '_' | '-' | '.' | '/' | '#' | '@')
        {
            value
        } else {
            '_'
        };
        label.push(replacement);
    }

    if label.is_empty() {
        "unknown".to_string()
    } else {
        label
    }
}

pub(super) fn smart_context_command_output_line_has_critical_signal(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "denied",
        "not found",
        "segmentation fault",
        "abort",
        "timeout",
    ]
    .iter()
    .any(|signal| line.contains(signal))
}

pub(super) fn smart_context_command_output_signal_sample(line: &str) -> String {
    let line = line.trim();
    let mut sample =
        smart_context_summary_prefix(line, SMART_CONTEXT_COMMAND_OUTPUT_CRITICAL_SAMPLE_BYTES);
    if sample.len() < line.len() {
        sample.push_str("...");
    }
    sample
}

pub(super) fn smart_context_signed_delta(current: usize, previous: usize) -> i128 {
    let current = i128::try_from(current).unwrap_or(i128::MAX);
    let previous = i128::try_from(previous).unwrap_or(i128::MAX);
    current.saturating_sub(previous)
}

pub(super) fn smart_context_signed_delta_u64(current: u64, previous: u64) -> i128 {
    let current = i128::from(current);
    let previous = i128::from(previous);
    current.saturating_sub(previous)
}

pub(super) fn smart_context_stabilize_static_context_id(id: &str) -> String {
    id.trim().replace('\\', "/")
}

pub(super) fn smart_context_static_context_item_order(
    left: &SmartContextStableStaticContextItem,
    right: &SmartContextStableStaticContextItem,
) -> Ordering {
    smart_context_static_context_item_order_key(&left.id)
        .cmp(&smart_context_static_context_item_order_key(&right.id))
        .then_with(|| left.id.cmp(&right.id))
        .then_with(|| left.content_hash.cmp(&right.content_hash))
        .then_with(|| left.byte_len.cmp(&right.byte_len))
        .then_with(|| left.canonical_text.cmp(&right.canonical_text))
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SmartContextStaticContextItemOrderKey {
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: String,
}

pub(super) fn smart_context_static_context_item_order_key(
    id: &str,
) -> SmartContextStaticContextItemOrderKey {
    match id {
        "instructions" => smart_context_static_context_order_key(0, 0, 0, ""),
        "system" => smart_context_static_context_order_key(1, 0, 0, ""),
        "developer" => smart_context_static_context_order_key(2, 0, 0, ""),
        _ => smart_context_input_static_context_order_key(id)
            .unwrap_or_else(|| smart_context_static_context_order_key(100, 0, 0, id)),
    }
}

pub(super) fn smart_context_input_static_context_order_key(
    id: &str,
) -> Option<SmartContextStaticContextItemOrderKey> {
    let rest = id.strip_prefix("input[")?;
    let (index, rest) = rest.split_once("].")?;
    let input_index = index.parse::<usize>().ok()?;
    let role_rank = match rest {
        "system" => 0,
        "developer" => 1,
        _ => return None,
    };
    Some(smart_context_static_context_order_key(
        3,
        input_index,
        role_rank,
        "",
    ))
}

pub(super) fn smart_context_static_context_order_key(
    group: u8,
    input_index: usize,
    role_rank: u8,
    generic_id: &str,
) -> SmartContextStaticContextItemOrderKey {
    SmartContextStaticContextItemOrderKey {
        group,
        input_index,
        role_rank,
        generic_id: generic_id.to_string(),
    }
}

pub(super) fn smart_context_static_context_prompt_cache_payload(
    items: &[SmartContextStableStaticContextItem],
) -> String {
    let mut payload = String::from("prodex-smart-context-static-prompt-cache-v1\n");
    for item in items {
        payload.push_str("id-bytes:");
        payload.push_str(&item.id.len().to_string());
        payload.push('\n');
        payload.push_str(&item.id);
        payload.push('\n');
        payload.push_str("text-bytes:");
        payload.push_str(&item.byte_len.to_string());
        payload.push('\n');
        payload.push_str(&item.canonical_text);
        payload.push('\n');
    }
    payload
}

pub(super) fn smart_context_static_context_noise_line(line: &str) -> bool {
    let mut value = line.trim();
    if let Some(inner) = value
        .strip_prefix("<!--")
        .and_then(|value| value.strip_suffix("-->"))
    {
        value = inner.trim();
    }

    for prefix in ["//", "#", ";"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.trim_start();
            break;
        }
    }

    let Some((key, noise_value)) = value.split_once(':').or_else(|| value.split_once('=')) else {
        return false;
    };
    let key = smart_context_static_context_noise_key(key);
    if !smart_context_static_context_noise_key_is_volatile(&key) {
        return false;
    }

    matches!(
        key.as_str(),
        "run id" | "request id" | "trace id" | "session id"
    ) || smart_context_static_context_noise_value_looks_volatile(noise_value)
}

pub(super) fn smart_context_static_context_noise_key(key: &str) -> String {
    let lower = key.trim().to_ascii_lowercase();
    let mut normalized = String::new();
    let mut previous_space = false;
    for value in lower.chars() {
        let value = match value {
            '-' | '_' => ' ',
            value => value,
        };
        if value.is_whitespace() {
            if !previous_space {
                normalized.push(' ');
                previous_space = true;
            }
        } else {
            normalized.push(value);
            previous_space = false;
        }
    }

    normalized
        .trim()
        .strip_prefix("prodex ")
        .unwrap_or(normalized.trim())
        .to_string()
}

pub(super) fn smart_context_static_context_noise_key_is_volatile(key: &str) -> bool {
    matches!(
        key,
        "generated"
            | "generated at"
            | "generated on"
            | "last generated"
            | "last generated at"
            | "timestamp"
            | "current date"
            | "current time"
            | "current datetime"
            | "as of"
            | "last updated"
            | "updated at"
            | "run id"
            | "request id"
            | "trace id"
            | "session id"
    )
}

pub(super) fn smart_context_static_context_noise_value_looks_volatile(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.chars().any(|value| value.is_ascii_digit()) {
        return true;
    }

    matches!(
        value.to_ascii_lowercase().as_str(),
        "now" | "today" | "yesterday" | "tomorrow"
    )
}

pub(super) fn smart_context_previous_char(text: &str, index: usize) -> Option<char> {
    if index == 0 {
        None
    } else {
        text[..index].chars().next_back()
    }
}

pub(super) fn smart_context_ansi_escape_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.first().copied()? != 0x1b {
        return None;
    }

    match bytes.get(1).copied() {
        Some(b'[') => bytes
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, byte)| (0x40u8..=0x7e).contains(*byte))
            .map(|(index, _)| index + 1)
            .or(Some(bytes.len())),
        Some(b']') => {
            let mut index = 2usize;
            while index < bytes.len() {
                if bytes[index] == 0x07 {
                    return Some(index + 1);
                }
                if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                    return Some(index + 2);
                }
                index += 1;
            }
            Some(bytes.len())
        }
        Some(_) => Some(2.min(bytes.len())),
        None => Some(1),
    }
}

pub(super) fn smart_context_temp_path_len(text: &str) -> Option<usize> {
    for prefix in [
        "/tmp/",
        "/var/tmp/",
        "/private/tmp/",
        "/var/folders/",
        "$TMPDIR/",
        "%TEMP%\\",
        "%TMP%\\",
    ] {
        if text.starts_with(prefix) {
            return Some(smart_context_path_token_len(text));
        }
    }

    let bytes = text.as_bytes();
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        let token_len = smart_context_path_token_len(text);
        let token = text[..token_len].replace('/', "\\").to_ascii_lowercase();
        if token.contains("\\appdata\\local\\temp\\") || token.starts_with("c:\\temp\\") {
            return Some(token_len);
        }
    }

    None
}

pub(super) fn smart_context_path_token_len(text: &str) -> usize {
    text.char_indices()
        .find(|(index, ch)| *index > 0 && smart_context_path_token_delimiter(*ch))
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

pub(super) fn smart_context_path_token_delimiter(ch: char) -> bool {
    ch.is_whitespace()
        || ch.is_control()
        || matches!(
            ch,
            '"' | '\'' | '`' | '<' | '>' | '|' | '(' | ')' | '[' | ']' | '{' | '}'
        )
}

pub(super) fn smart_context_timestamp_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let bytes = text.as_bytes();
    if bytes.len() < 16
        || !smart_context_ascii_digits(bytes, 0, 4)
        || bytes[4] != b'-'
        || !smart_context_ascii_digits(bytes, 5, 2)
        || bytes[7] != b'-'
        || !smart_context_ascii_digits(bytes, 8, 2)
        || !matches!(bytes[10], b'T' | b' ')
        || !smart_context_ascii_digits(bytes, 11, 2)
        || bytes[13] != b':'
        || !smart_context_ascii_digits(bytes, 14, 2)
    {
        return None;
    }

    let mut index = 16usize;
    if bytes.get(index) == Some(&b':') {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b'.') {
            let fraction_start = index + 1;
            index = fraction_start;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
            if index == fraction_start {
                return None;
            }
        }
    }

    if bytes.get(index) == Some(&b'Z') {
        index += 1;
    } else if matches!(bytes.get(index), Some(b'+') | Some(b'-')) {
        if !smart_context_ascii_digits(bytes, index + 1, 2) {
            return None;
        }
        index += 3;
        if bytes.get(index) == Some(&b':') {
            if !smart_context_ascii_digits(bytes, index + 1, 2) {
                return None;
            }
            index += 3;
        } else if smart_context_ascii_digits(bytes, index, 2) {
            index += 2;
        }
    }

    smart_context_after_token_boundary(text, index).then_some(index)
}

pub(super) fn smart_context_progress_counter_len(
    text: &str,
    previous: Option<char>,
) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    smart_context_percent_progress_len(text)
        .or_else(|| smart_context_slash_progress_len(text))
        .or_else(|| smart_context_of_progress_len(text))
}

pub(super) fn smart_context_percent_progress_len(text: &str) -> Option<usize> {
    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    if text.as_bytes().get(index) != Some(&b'%') {
        return None;
    }
    index += 1;
    smart_context_after_token_boundary(text, index).then_some(index)
}

pub(super) fn smart_context_slash_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(left_end) != Some(&b'/') {
        return None;
    }
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, left_end + 1)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

pub(super) fn smart_context_of_progress_len(text: &str) -> Option<usize> {
    let (left_end, left) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    let mut index = smart_context_skip_ascii_spaces(text, left_end);
    if !text[index..].starts_with("of") {
        return None;
    }
    index += 2;
    if !text
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        return None;
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let (right_end, right) = smart_context_parse_unsigned_ascii_int(text, index)?;
    if left > right || right == 0 {
        return None;
    }
    smart_context_after_token_boundary(text, right_end).then_some(right_end)
}

pub(super) fn smart_context_labeled_random_id_replacement(
    text: &str,
    previous: Option<char>,
) -> Option<(usize, String)> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let mut separator_index = None;
    for (index, ch) in text.char_indices().take(64) {
        if matches!(ch, ':' | '=') {
            separator_index = Some(index);
            break;
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ' ')) {
            return None;
        }
    }
    let separator_index = separator_index?;
    let key = smart_context_static_context_noise_key(&text[..separator_index]);
    if !smart_context_random_id_key_is_volatile(&key) {
        return None;
    }

    let mut value_start = smart_context_skip_ascii_spaces(text, separator_index + 1);
    if matches!(text.as_bytes().get(value_start), Some(b'"') | Some(b'\'')) {
        value_start += 1;
    }
    let value_len = smart_context_random_id_value_len(&text[value_start..])?;
    let value_end = value_start + value_len;
    let value = &text[value_start..value_end];
    if !smart_context_random_id_value_looks_volatile_for_key(&key, value) {
        return None;
    }

    let mut replacement = String::with_capacity(value_start + 4);
    replacement.push_str(&text[..value_start]);
    replacement.push_str("<id>");
    Some((value_end, replacement))
}

pub(super) fn smart_context_uuid_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) || text.len() < 36 {
        return None;
    }
    let candidate = &text[..36];
    if !smart_context_uuid_token_exact(candidate) || !smart_context_after_token_boundary(text, 36) {
        return None;
    }
    Some(36)
}

pub(super) fn smart_context_duration_len(text: &str, previous: Option<char>) -> Option<usize> {
    if !smart_context_token_boundary(previous) {
        return None;
    }

    let (mut index, _) = smart_context_parse_unsigned_ascii_int(text, 0)?;
    if text.as_bytes().get(index) == Some(&b'.') {
        let fraction_start = index + 1;
        index = fraction_start;
        while text.as_bytes().get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == fraction_start {
            return None;
        }
    }
    index = smart_context_skip_ascii_spaces(text, index);
    let unit_len = smart_context_duration_unit_len(&text[index..])?;
    let end = index + unit_len;
    smart_context_after_token_boundary(text, end).then_some(end)
}

pub(super) fn smart_context_duration_unit_len(text: &str) -> Option<usize> {
    for unit in [
        "milliseconds",
        "millisecond",
        "microseconds",
        "microsecond",
        "nanoseconds",
        "nanosecond",
        "seconds",
        "second",
        "minutes",
        "minute",
        "hours",
        "hour",
        "msecs",
        "msec",
        "usecs",
        "usec",
        "nsecs",
        "nsec",
        "secs",
        "sec",
        "mins",
        "min",
        "hrs",
        "hr",
        "ms",
        "us",
        "ns",
        "s",
        "m",
        "h",
    ] {
        if smart_context_ascii_case_prefix(text, unit) {
            return Some(unit.len());
        }
    }
    None
}

pub(super) fn smart_context_random_id_key_is_volatile(key: &str) -> bool {
    matches!(
        key,
        "request id"
            | "x request id"
            | "trace id"
            | "run id"
            | "session id"
            | "conversation id"
            | "turn id"
            | "span id"
            | "correlation id"
            | "invocation id"
            | "execution id"
            | "operation id"
            | "job id"
            | "build id"
            | "uuid"
            | "id"
    )
}

pub(super) fn smart_context_random_id_value_len(text: &str) -> Option<usize> {
    let len = text
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':')))
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    (len > 0).then_some(len)
}

pub(super) fn smart_context_random_id_value_looks_volatile_for_key(key: &str, value: &str) -> bool {
    if smart_context_uuid_token_exact(value) {
        return true;
    }
    if key == "id" && value.len() < 20 {
        return false;
    }
    if value.len() < 12 {
        return false;
    }

    let mut alpha = false;
    let mut digit = false;
    let mut hex_like = true;
    let mut entropy_marks = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_alphabetic() {
            alpha = true;
            if !ch.is_ascii_hexdigit() {
                hex_like = false;
            }
        } else if ch.is_ascii_digit() {
            digit = true;
        } else if matches!(ch, '_' | '-' | '.' | ':') {
            entropy_marks += 1;
        } else {
            return false;
        }
    }

    (hex_like && value.len() >= 16) || (alpha && digit && (value.len() >= 16 || entropy_marks > 0))
}

pub(super) fn smart_context_uuid_token_exact(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub(super) fn smart_context_parse_unsigned_ascii_int(
    text: &str,
    start: usize,
) -> Option<(usize, u64)> {
    let bytes = text.as_bytes();
    let mut index = start;
    let mut value = 0u64;
    let mut digits = 0usize;
    while let Some(byte) = bytes.get(index).copied() {
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .saturating_mul(10)
            .saturating_add(u64::from(byte - b'0'));
        index += 1;
        digits += 1;
    }
    (digits > 0).then_some((index, value))
}

pub(super) fn smart_context_skip_ascii_spaces(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

pub(super) fn smart_context_ascii_digits(bytes: &[u8], start: usize, len: usize) -> bool {
    bytes
        .get(start..start.saturating_add(len))
        .is_some_and(|value| value.iter().all(u8::is_ascii_digit))
}

pub(super) fn smart_context_ascii_case_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
}

pub(super) fn smart_context_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}

pub(super) fn smart_context_after_token_boundary(text: &str, index: usize) -> bool {
    text[index..]
        .chars()
        .next()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '/')))
}

pub(super) fn smart_context_summary_prefix(text: &str, byte_limit: usize) -> String {
    let mut summary = String::new();
    for value in text.chars() {
        let next_len = summary.len() + value.len_utf8();
        if next_len > byte_limit {
            break;
        }
        summary.push(value);
    }
    summary
}

pub(super) fn smart_context_artifact_marker_line(
    kind: &str,
    artifact: &SmartContextArtifactRef,
) -> String {
    let reference = smart_context_short_artifact_ref(&artifact.id);
    let kind = match kind {
        "artifact" => "art",
        other => other,
    };
    format!(
        "psc {kind} {reference} b={} lines=#Lx-Ly",
        artifact.byte_len
    )
}

#[allow(dead_code)]
pub(super) fn smart_context_legacy_artifact_ref(id: &str) -> String {
    format!("prodex-artifact:{id}")
}

pub(super) fn smart_context_short_artifact_label(id: &str) -> &str {
    id.strip_prefix("sc:").unwrap_or(id)
}

pub(super) fn smart_context_capsule_order(
    left: &SmartContextMemoryCapsule,
    right: &SmartContextMemoryCapsule,
) -> Ordering {
    right
        .relevance
        .partial_cmp(&left.relevance)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.token_cost.cmp(&right.token_cost))
        .then_with(|| left.id.cmp(&right.id))
}

pub(super) fn smart_context_fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(super) fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}
