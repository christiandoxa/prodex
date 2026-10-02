#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextRegressionPlanInput {
    pub exactness_required: bool,
    pub payload_changed: bool,
    pub before_tokens: u64,
    pub after_tokens: u64,
    pub tokenizer_counted: bool,
    pub future_retrieval_overhead_tokens: u64,
    pub injected_protocol_overhead_tokens: u64,
    pub expected_recovery_overhead_tokens: u64,
    pub before_critical_signal_count: u64,
    pub after_critical_signal_count: u64,
    pub missing_rehydrate_refs: bool,
    pub unresolved_refs_segment_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextRegressionPlan {
    pub fallback_exact: bool,
    pub reason_bits: u64,
    pub saved_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextRolloutPlan {
    pub mode: i64,
    pub canary_percent: u8,
    pub reason: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextFingerprintDeltaInput {
    pub key: u64,
    pub content_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextFingerprintDeltaPlanItem {
    pub action: i64,
    pub previous_index: Option<usize>,
    pub current_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextStaticItemPlanInput<'a> {
    pub id: &'a str,
    pub content_hash: &'a str,
    pub canonical_text: &'a str,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextStaticItemPlan {
    pub selected_indices: Vec<usize>,
    pub retained: Vec<bool>,
}

/// Text input to the Mojo-owned request-local duplicate-reference planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextDuplicateTextCandidate<'a> {
    pub digest: String,
    pub text: &'a str,
    pub role: &'a str,
    pub input_index: usize,
}

/// Candidate count at which the duplicate precheck preserves its existing fail-open behavior.
pub const SMART_CONTEXT_DUPLICATE_SCAN_MAX_CANDIDATES: usize = 256;
const SMART_CONTEXT_DUPLICATE_PLAN_MAX_CANDIDATES: usize = 50_000;

/// Controls whether duplicate planning finds candidates or returns rewrite targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextDuplicateTextMode {
    /// Apply the 256-candidate probe limit and verify full text on digest matches.
    Probe,
    /// Plan replacements using the full digest, matching the request rewrite contract.
    Rewrite,
}

/// Result of probing or planning request-local duplicate text references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextDuplicateTextPlan {
    pub replacement_sources: Vec<Option<usize>>,
    pub candidate_limit_exceeded: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct StaticItem {
    id: StringView,
    content_hash: StringView,
    canonical_text: StringView,
    byte_len: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DuplicateTextCandidate {
    digest: StringView,
    text: StringView,
    role: StringView,
    input_index: i64,
}

unsafe extern "C" {
    fn prodex_smart_context_regression_plan_v1(
        exactness_required: i64,
        payload_changed: i64,
        before_tokens: u64,
        after_tokens: u64,
        tokenizer_counted: i64,
        future_retrieval_overhead_tokens: u64,
        injected_protocol_overhead_tokens: u64,
        expected_recovery_overhead_tokens: u64,
        before_critical_signal_count: u64,
        after_critical_signal_count: u64,
        missing_rehydrate_refs: i64,
        unresolved_refs_segment_local: i64,
        decision: *mut i64,
        reason_bits: *mut u64,
        saved_tokens: *mut u64,
    ) -> i64;
    fn prodex_smart_context_affinity_rewrite_allowed_v1(
        exactness_required: i64,
        exactness_reason_bits: u64,
        policy_reason_bits: u64,
    ) -> i64;
    fn prodex_smart_context_rollout_plan_v1(
        enabled: i64,
        explicit_exact_mode: i64,
        shadow_mode: i64,
        canary_percent_input: i64,
        canary_bucket: i64,
        mode: *mut i64,
        canary_percent: *mut i64,
        reason: *mut i64,
    ) -> i64;
    fn prodex_smart_context_fingerprint_delta_plan_v1(
        previous_keys_address: u64,
        previous_hashes_address: u64,
        current_keys_address: u64,
        current_hashes_address: u64,
        action_address: u64,
        previous_index_address: u64,
        current_index_address: u64,
        previous_count: i64,
        current_count: i64,
        key_count: i64,
    ) -> i64;
    fn prodex_smart_context_static_item_plan_v1(
        items_address: u64,
        selected_indices_address: u64,
        retained_address: u64,
        selected_count_address: u64,
        count: i64,
        maximum_items: i64,
    ) -> i64;
    fn prodex_smart_context_duplicate_text_plan_v1(
        candidates_address: u64,
        replacement_sources_address: u64,
        bucket_slots_address: u64,
        bucket_slot_count: i64,
        duplicate_count_address: u64,
        candidate_limit_exceeded_address: u64,
        count: i64,
        mode: i64,
    ) -> i64;
}

pub fn smart_context_regression_plan(
    input: SmartContextRegressionPlanInput,
) -> Result<SmartContextRegressionPlan, crate::MojoError> {
    let mut decision = -1_i64;
    let mut reason_bits = 0_u64;
    let mut saved_tokens = 0_u64;
    let status = unsafe {
        prodex_smart_context_regression_plan_v1(
            i64::from(input.exactness_required),
            i64::from(input.payload_changed),
            input.before_tokens,
            input.after_tokens,
            i64::from(input.tokenizer_counted),
            input.future_retrieval_overhead_tokens,
            input.injected_protocol_overhead_tokens,
            input.expected_recovery_overhead_tokens,
            input.before_critical_signal_count,
            input.after_critical_signal_count,
            i64::from(input.missing_rehydrate_refs),
            i64::from(input.unresolved_refs_segment_local),
            &mut decision,
            &mut reason_bits,
            &mut saved_tokens,
        )
    };
    if status != 0 || !matches!(decision, 0 | 1) || reason_bits > 127 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextRegressionPlan {
        fallback_exact: decision == 1,
        reason_bits,
        saved_tokens,
    })
}

pub fn smart_context_affinity_rewrite_allowed(
    exactness_required: bool,
    exactness_reason_bits: u64,
    policy_reason_bits: u64,
) -> Result<bool, crate::MojoError> {
    match unsafe {
        prodex_smart_context_affinity_rewrite_allowed_v1(
            i64::from(exactness_required),
            exactness_reason_bits,
            policy_reason_bits,
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn smart_context_rollout_plan(
    enabled: bool,
    explicit_exact_mode: bool,
    shadow_mode: bool,
    canary_percent: u8,
    canary_bucket: u16,
) -> Result<SmartContextRolloutPlan, crate::MojoError> {
    let mut mode = -1_i64;
    let mut normalized_percent = -1_i64;
    let mut reason = -1_i64;
    let status = unsafe {
        prodex_smart_context_rollout_plan_v1(
            i64::from(enabled),
            i64::from(explicit_exact_mode),
            i64::from(shadow_mode),
            i64::from(canary_percent),
            i64::from(canary_bucket),
            &mut mode,
            &mut normalized_percent,
            &mut reason,
        )
    };
    if status != 0
        || !matches!(mode, 0..=2)
        || !matches!(normalized_percent, 0..=100)
        || !matches!(reason, 0..=5)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextRolloutPlan {
        mode,
        canary_percent: normalized_percent as u8,
        reason,
    })
}

pub fn smart_context_fingerprint_delta_plan(
    previous: &[SmartContextFingerprintDeltaInput],
    current: &[SmartContextFingerprintDeltaInput],
    key_count: usize,
) -> Result<Vec<SmartContextFingerprintDeltaPlanItem>, crate::MojoError> {
    let previous_keys = previous.iter().map(|item| item.key).collect::<Vec<_>>();
    let previous_hashes = previous
        .iter()
        .map(|item| item.content_hash)
        .collect::<Vec<_>>();
    let current_keys = current.iter().map(|item| item.key).collect::<Vec<_>>();
    let current_hashes = current
        .iter()
        .map(|item| item.content_hash)
        .collect::<Vec<_>>();
    let mut actions = vec![-1_i64; key_count];
    let mut previous_indices = vec![-1_i64; key_count];
    let mut current_indices = vec![-1_i64; key_count];
    let status = unsafe {
        prodex_smart_context_fingerprint_delta_plan_v1(
            previous_keys.as_ptr() as usize as u64,
            previous_hashes.as_ptr() as usize as u64,
            current_keys.as_ptr() as usize as u64,
            current_hashes.as_ptr() as usize as u64,
            actions.as_mut_ptr() as usize as u64,
            previous_indices.as_mut_ptr() as usize as u64,
            current_indices.as_mut_ptr() as usize as u64,
            i64::try_from(previous.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(current.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(key_count).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    actions
        .into_iter()
        .zip(previous_indices)
        .zip(current_indices)
        .map(|((action, previous_index), current_index)| {
            if !matches!(action, 0..=3) {
                return Err(crate::MojoError::InvalidOutput);
            }
            Ok(SmartContextFingerprintDeltaPlanItem {
                action,
                previous_index: optional_index(previous_index, previous.len())?,
                current_index: optional_index(current_index, current.len())?,
            })
        })
        .collect()
}

fn optional_index(index: i64, count: usize) -> Result<Option<usize>, crate::MojoError> {
    if index == -1 {
        return Ok(None);
    }
    usize::try_from(index)
        .ok()
        .filter(|index| *index < count)
        .map(Some)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn smart_context_static_item_plan(
    items: &[SmartContextStaticItemPlanInput<'_>],
    maximum_items: usize,
) -> Result<SmartContextStaticItemPlan, crate::MojoError> {
    if maximum_items > items.len() {
        return Err(crate::MojoError::InvalidInput);
    }
    let items = items
        .iter()
        .map(|item| StaticItem {
            id: string_view(item.id),
            content_hash: string_view(item.content_hash),
            canonical_text: string_view(item.canonical_text),
            byte_len: item.byte_len,
        })
        .collect::<Vec<_>>();
    let mut selected_indices = vec![-1_i64; items.len()];
    let mut retained = vec![0_i64; items.len()];
    let mut selected_count = 0_i64;
    let status = unsafe {
        prodex_smart_context_static_item_plan_v1(
            items.as_ptr() as usize as u64,
            selected_indices.as_mut_ptr() as usize as u64,
            retained.as_mut_ptr() as usize as u64,
            &mut selected_count as *mut i64 as usize as u64,
            i64::try_from(items.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(maximum_items).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    let selected_count = usize::try_from(selected_count).ok();
    if status != 0
        || selected_count.is_none_or(|count| count != maximum_items)
        || retained.iter().any(|value| !matches!(value, 0 | 1))
        || retained.iter().filter(|value| **value == 1).count() != maximum_items
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let mut seen = vec![false; items.len()];
    let selected_indices = selected_indices[..selected_count.unwrap()]
        .iter()
        .map(|index| {
            let index = usize::try_from(*index)
                .ok()
                .filter(|index| *index < items.len() && retained[*index] == 1)
                .ok_or(crate::MojoError::InvalidOutput)?;
            if seen[index] {
                return Err(crate::MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmartContextStaticItemPlan {
        selected_indices,
        retained: retained.into_iter().map(|value| value == 1).collect(),
    })
}

pub fn smart_context_duplicate_text_plan(
    inputs: &[SmartContextDuplicateTextCandidate<'_>],
    mode: SmartContextDuplicateTextMode,
) -> Result<SmartContextDuplicateTextPlan, crate::MojoError> {
    if inputs.len() > SMART_CONTEXT_DUPLICATE_PLAN_MAX_CANDIDATES {
        return Err(crate::MojoError::InvalidInput);
    }
    let bucket_slot_count = inputs
        .len()
        .max(1)
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(crate::MojoError::InvalidInput)?;
    let candidates = inputs
        .iter()
        .map(|input| {
            Ok(DuplicateTextCandidate {
                digest: string_view(&input.digest),
                text: string_view(input.text),
                role: string_view(input.role),
                input_index: i64::try_from(input.input_index)
                    .map_err(|_| crate::MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, crate::MojoError>>()?;
    let mut replacement_sources = vec![-1_i64; inputs.len()];
    let mut bucket_slots = vec![-1_i64; bucket_slot_count];
    let mut duplicate_count = 0_i64;
    let mut candidate_limit_exceeded = 0_i64;
    let mode = match mode {
        SmartContextDuplicateTextMode::Rewrite => 0,
        SmartContextDuplicateTextMode::Probe => 1,
    };
    let status = unsafe {
        prodex_smart_context_duplicate_text_plan_v1(
            candidates.as_ptr() as usize as u64,
            replacement_sources.as_mut_ptr() as usize as u64,
            bucket_slots.as_mut_ptr() as usize as u64,
            i64::try_from(bucket_slot_count).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut duplicate_count as *mut i64 as usize as u64,
            &mut candidate_limit_exceeded as *mut i64 as usize as u64,
            i64::try_from(candidates.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            mode,
        )
    };
    if status != 0
        || duplicate_count < 0
        || usize::try_from(duplicate_count)
            .ok()
            .is_none_or(|count| count > inputs.len())
        || !matches!(candidate_limit_exceeded, 0 | 1)
        || candidate_limit_exceeded == 1 && mode != 1
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    if candidate_limit_exceeded == 1 {
        return Ok(SmartContextDuplicateTextPlan {
            replacement_sources: vec![None; inputs.len()],
            candidate_limit_exceeded: true,
        });
    }
    let replacement_sources = replacement_sources
        .into_iter()
        .zip(inputs)
        .map(|(source, candidate)| {
            if source == -1 {
                return Ok(None);
            }
            usize::try_from(source)
                .ok()
                .filter(|source| *source < candidate.input_index)
                .map(Some)
                .ok_or(crate::MojoError::InvalidOutput)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if replacement_sources
        .iter()
        .filter(|source| source.is_some())
        .count()
        != duplicate_count as usize
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(SmartContextDuplicateTextPlan {
        replacement_sources,
        candidate_limit_exceeded: candidate_limit_exceeded == 1,
    })
}

fn string_view(value: &str) -> StringView {
    StringView {
        ptr: value.as_ptr() as usize as u64,
        len: value.len() as u64,
    }
}

const _: () = {
    assert!(std::mem::size_of::<StringView>() == 16);
    assert!(std::mem::align_of::<StringView>() == 8);
    assert!(std::mem::size_of::<StaticItem>() == 56);
    assert!(std::mem::align_of::<StaticItem>() == 8);
    assert!(std::mem::size_of::<DuplicateTextCandidate>() == 56);
    assert!(std::mem::align_of::<DuplicateTextCandidate>() == 8);
};

pub fn smart_context_policy_self_test() -> bool {
    smart_context_regression_plan(SmartContextRegressionPlanInput {
        exactness_required: false,
        payload_changed: true,
        before_tokens: 1_000,
        after_tokens: 700,
        tokenizer_counted: true,
        future_retrieval_overhead_tokens: 20,
        injected_protocol_overhead_tokens: 10,
        expected_recovery_overhead_tokens: 0,
        before_critical_signal_count: 1,
        after_critical_signal_count: 1,
        missing_rehydrate_refs: false,
        unresolved_refs_segment_local: false,
    })
    .is_ok_and(|plan| !plan.fallback_exact && plan.saved_tokens == 270)
        && smart_context_affinity_rewrite_allowed(true, 2, 1).is_ok_and(|allowed| allowed)
        && smart_context_rollout_plan(true, false, false, 10, 999)
            .is_ok_and(|plan| plan.mode == 0 && plan.reason == 5)
}

#[cfg(test)]
mod duplicate_text_tests {
    use super::*;

    fn candidate<'a>(
        digest: &str,
        text: &'a str,
        input_index: usize,
    ) -> SmartContextDuplicateTextCandidate<'a> {
        SmartContextDuplicateTextCandidate {
            digest: digest.to_string(),
            text,
            role: "user",
            input_index,
        }
    }

    fn candidate_with_role<'a>(
        digest: &str,
        text: &'a str,
        role: &'a str,
        input_index: usize,
    ) -> SmartContextDuplicateTextCandidate<'a> {
        SmartContextDuplicateTextCandidate {
            digest: digest.to_string(),
            text,
            role,
            input_index,
        }
    }

    #[test]
    fn smart_context_duplicate_text_plan_keeps_first_input_source() {
        let digest = format!("sc2:{}", "0".repeat(64));
        let text = "same ".repeat(300);
        let candidates = [
            candidate(&digest, &text, 0),
            candidate(&digest, &text, 0),
            candidate(&digest, &text, 1),
            candidate(&digest, &text, 2),
        ];

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)
                .unwrap();

        assert_eq!(plan.replacement_sources, vec![None, None, Some(0), Some(0)]);
        assert!(!plan.candidate_limit_exceeded);
    }

    #[test]
    fn smart_context_duplicate_plan_handles_digest_bucket_collisions() {
        let digest_prefix = format!("sc2:{}", "a".repeat(16));
        let first_digest = format!("{digest_prefix}{}", "0".repeat(48));
        let colliding_digest = format!("{digest_prefix}{}", "1".repeat(48));
        let text = "long text ".repeat(110);
        let different_text = "different long text ".repeat(60);
        let candidates = [
            candidate(&first_digest, &text, 0),
            candidate(&colliding_digest, &different_text, 1),
            candidate(&first_digest, &text, 2),
        ];

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)
                .unwrap();

        assert_eq!(plan.replacement_sources, vec![None, None, Some(0)]);
    }

    #[test]
    fn smart_context_duplicate_rewrite_checks_text_after_digest_match() {
        let digest = format!("sc2:{}", "1".repeat(64));
        let same = "same exact text ".repeat(80);
        let different = "different text ".repeat(80);
        let candidates = [
            candidate(&digest, &same, 0),
            candidate(&digest, &different, 1),
            candidate(&digest, &same, 2),
        ];

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)
                .unwrap();

        assert_eq!(plan.replacement_sources, vec![None, None, Some(0)]);
    }

    #[test]
    fn smart_context_duplicate_probe_checks_text_after_digest_match() {
        let digest = format!("sc2:{}", "1".repeat(64));
        let same = "same ".repeat(300);
        let different = "different ".repeat(200);
        let candidates = [
            candidate(&digest, &same, 0),
            candidate(&digest, &different, 1),
            candidate(&digest, &same, 2),
        ];

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Probe)
                .unwrap();

        assert_eq!(plan.replacement_sources, vec![None, None, Some(0)]);
        assert!(!plan.candidate_limit_exceeded);
    }

    #[test]
    fn smart_context_duplicate_probe_stops_at_candidate_limit() {
        let text = "x".repeat(1024);
        let candidates = (0..=SMART_CONTEXT_DUPLICATE_SCAN_MAX_CANDIDATES)
            .map(|input_index| candidate("", &text, input_index))
            .collect::<Vec<_>>();

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Probe)
                .unwrap();

        assert!(plan.candidate_limit_exceeded);
        assert!(plan.replacement_sources.iter().all(Option::is_none));
    }

    #[test]
    fn smart_context_duplicate_rewrite_skips_static_prompt_roles() {
        let digest = format!("sc2:{}", "3".repeat(64));
        let text = "static instruction ".repeat(80);
        let candidates = [
            candidate_with_role(&digest, &text, "system", 0),
            candidate_with_role(&digest, &text, "developer", 1),
            candidate_with_role(&digest, &text, "user", 2),
        ];

        let plan =
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)
                .unwrap();

        assert_eq!(plan.replacement_sources, vec![None, None, None]);
    }

    #[test]
    fn smart_context_duplicate_rewrite_accepts_long_non_prompt_roles() {
        let digest = format!("sc2:{}", "4".repeat(64));
        let text = "long text ".repeat(110);
        let role = "other".repeat(103);
        let candidates = [candidate_with_role(&digest, &text, &role, 0)];

        assert!(
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite)
                .is_ok()
        );
    }

    #[test]
    fn smart_context_duplicate_rewrite_rejects_more_than_json_node_ceiling() {
        let digest = format!("sc2:{}", "2".repeat(64));
        let candidates = vec![candidate(&digest, "text", 0); 50_001];

        assert_eq!(
            smart_context_duplicate_text_plan(&candidates, SmartContextDuplicateTextMode::Rewrite,),
            Err(crate::MojoError::InvalidInput)
        );
    }
}
