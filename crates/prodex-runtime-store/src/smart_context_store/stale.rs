use super::*;

pub fn runtime_smart_context_stale_context_pruning_decision(
    input: RuntimeSmartContextStaleContextPruningInput<'_>,
) -> RuntimeSmartContextStaleContextPruningDecision {
    let current_hash = runtime_smart_context_normalized_hash(input.current.hash);
    let previous = input.previous;
    let previous_hash = previous.and_then(|snapshot| {
        runtime_smart_context_normalized_hash(snapshot.hash).map(str::to_string)
    });
    let current_hash_owned = current_hash.map(str::to_string);
    let previous_byte_len = previous.map(|snapshot| snapshot.byte_len);
    let previous_token_len = previous.map(|snapshot| snapshot.token_len);

    let current_is_small = input.current.byte_len
        < crate::RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_BYTES
        && input.current.token_len < crate::RUNTIME_SMART_CONTEXT_STALE_CONTEXT_MIN_TOKENS;
    if current_is_small {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::TooSmall,
            summary: format!(
                "static-context: no-op reason=too_small current_hash={} current_bytes={} current_tokens={}",
                runtime_smart_context_hash_summary(current_hash),
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: 0,
            reusable_token_len: 0,
        };
    }

    let Some(previous) = previous else {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::NoPrevious,
            summary: format!(
                "static-context: no-op reason=no_previous current_hash={} current_bytes={} current_tokens={}",
                runtime_smart_context_hash_summary(current_hash),
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: 0,
            reusable_token_len: 0,
        };
    };

    let previous_hash_ref = runtime_smart_context_normalized_hash(previous.hash);
    let hashes_match = previous_hash_ref
        .zip(current_hash)
        .is_some_and(|(previous_hash, current_hash)| previous_hash == current_hash);
    let hash_mismatch = previous_hash_ref
        .zip(current_hash)
        .is_some_and(|(previous_hash, current_hash)| previous_hash != current_hash);
    let caller_reports_unchanged_with_matching_size = !input.changed
        && !hash_mismatch
        && previous.byte_len == input.current.byte_len
        && previous.token_len == input.current.token_len;

    if hashes_match || caller_reports_unchanged_with_matching_size {
        return RuntimeSmartContextStaleContextPruningDecision {
            kind: RuntimeSmartContextStaleContextPruningKind::ExactReuse,
            summary: format!(
                "static-context: reuse hash={} bytes={} tokens={} saved_bytes={} saved_tokens={}",
                runtime_smart_context_hash_summary(current_hash.or(previous_hash_ref)),
                input.current.byte_len,
                input.current.token_len,
                input.current.byte_len,
                input.current.token_len
            ),
            previous_hash,
            current_hash: current_hash_owned,
            previous_byte_len,
            current_byte_len: input.current.byte_len,
            previous_token_len,
            current_token_len: input.current.token_len,
            reusable_byte_len: input.current.byte_len,
            reusable_token_len: input.current.token_len,
        };
    }

    RuntimeSmartContextStaleContextPruningDecision {
        kind: RuntimeSmartContextStaleContextPruningKind::Changed,
        summary: format!(
            "static-context: changed previous_hash={} current_hash={} previous_bytes={} current_bytes={} previous_tokens={} current_tokens={} byte_delta={} token_delta={}",
            runtime_smart_context_hash_summary(previous_hash_ref),
            runtime_smart_context_hash_summary(current_hash),
            previous.byte_len,
            input.current.byte_len,
            previous.token_len,
            input.current.token_len,
            runtime_smart_context_signed_delta(input.current.byte_len, previous.byte_len),
            runtime_smart_context_signed_delta(input.current.token_len, previous.token_len)
        ),
        previous_hash,
        current_hash: current_hash_owned,
        previous_byte_len,
        current_byte_len: input.current.byte_len,
        previous_token_len,
        current_token_len: input.current.token_len,
        reusable_byte_len: 0,
        reusable_token_len: 0,
    }
}

pub(super) fn runtime_smart_context_normalized_hash(hash: Option<&str>) -> Option<&str> {
    hash.map(str::trim).filter(|hash| !hash.is_empty())
}

pub(super) fn runtime_smart_context_hash_summary(hash: Option<&str>) -> &str {
    hash.unwrap_or("none")
}

pub(super) fn runtime_smart_context_signed_delta(current: usize, previous: usize) -> String {
    if current >= previous {
        format!("+{}", current - previous)
    } else {
        format!("-{}", previous - current)
    }
}
