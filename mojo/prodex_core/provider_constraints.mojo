from std.memory import Pointer

comptime DECISION_COMPATIBLE: Int64 = 0
comptime DECISION_ENDPOINT_UNSUPPORTED: Int64 = 1
comptime DECISION_REQUIRED_CAPABILITY_MISSING: Int64 = 2
comptime DECISION_CATALOG_UNAVAILABLE: Int64 = 3
comptime DECISION_CONTEXT_UNKNOWN: Int64 = 4
comptime DECISION_CONTEXT_EXCEEDED: Int64 = 5
comptime DECISION_OUTPUT_UNKNOWN: Int64 = 6
comptime DECISION_OUTPUT_EXCEEDS_LIMIT: Int64 = 7
comptime DECISION_REASONING_UNSUPPORTED: Int64 = 8
comptime DECISION_REASONING_EXCESSIVE: Int64 = 9
comptime DECISION_OUTPUT_CLAMPED: Int64 = 11

comptime FEATURE_REASONING: Int64 = 5
comptime UNKNOWN_ALLOW: Int64 = 0
comptime UNKNOWN_SAFE_WINDOW: Int64 = 1
comptime UNKNOWN_REJECT: Int64 = 2
comptime OVERSIZED_PASSTHROUGH: Int64 = 0
comptime OVERSIZED_REJECT: Int64 = 1
comptime OVERSIZED_CLAMP: Int64 = 2

comptime WARNING_CONTEXT_UNKNOWN: UInt64 = 1 << 4
comptime WARNING_OUTPUT_UNKNOWN: UInt64 = 1 << 6
comptime WARNING_OUTPUT_EXCEEDS_LIMIT: UInt64 = 1 << 7
comptime WARNING_CATALOG_UNAVAILABLE: UInt64 = 1 << 3

comptime UINT64_MAX: UInt64 = 18446744073709551615

def provider_constraint_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right

def provider_constraint_total(
    estimated_input_tokens: UInt64,
    explicit_output_tokens: UInt64,
    explicit_output_present: Int64,
    default_output_reserve_tokens: UInt64,
    default_output_present: Int64,
    reasoning_reserve_tokens: UInt64,
    reasoning_reserve_present: Int64,
) -> UInt64:
    var total = estimated_input_tokens
    if explicit_output_present == 1:
        total = provider_constraint_saturating_add(total, explicit_output_tokens)
    elif default_output_present == 1:
        total = provider_constraint_saturating_add(total, default_output_reserve_tokens)
    if reasoning_reserve_present == 1:
        total = provider_constraint_saturating_add(total, reasoning_reserve_tokens)
    return total

@export("prodex_provider_constraints_evaluate")
def prodex_provider_constraints_evaluate(
    policy_enabled: Int64,
    endpoint_supported: Int64,
    catalog_entry_present: Int64,
    embeddings_endpoint: Int64,
    missing_feature_present: Int64,
    missing_feature: Int64,
    reasoning_effort_unsupported: Int64,
    estimated_input_tokens: UInt64,
    explicit_output_tokens: UInt64,
    explicit_output_present: Int64,
    default_output_reserve_tokens: UInt64,
    default_output_present: Int64,
    reasoning_reserve_tokens: UInt64,
    reasoning_reserve_present: Int64,
    max_output_tokens: UInt64,
    max_output_present: Int64,
    context_window_tokens: UInt64,
    context_window_present: Int64,
    unknown_context_policy: Int64,
    safe_window_tokens: UInt64,
    oversized_output_policy: Int64,
    output_limit_field: Int64,
    output_limit_field_present: Int64,
    decision: Pointer[mut=True, Int64, _],
    eligible: Pointer[mut=True, Int64, _],
    result_missing_feature_present: Pointer[mut=True, Int64, _],
    result_missing_feature: Pointer[mut=True, Int64, _],
    adjusted_output_tokens: Pointer[mut=True, UInt64, _],
    adjusted_output_present: Pointer[mut=True, Int64, _],
    total_required_tokens: Pointer[mut=True, UInt64, _],
    available_context_tokens: Pointer[mut=True, UInt64, _],
    available_context_present: Pointer[mut=True, Int64, _],
    result_max_output_tokens: Pointer[mut=True, UInt64, _],
    result_max_output_present: Pointer[mut=True, Int64, _],
    warnings: Pointer[mut=True, UInt64, _],
    adjustment_field: Pointer[mut=True, Int64, _],
    adjustment_field_present: Pointer[mut=True, Int64, _],
    adjustment_reason: Pointer[mut=True, Int64, _],
    adjustment_reason_present: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if policy_enabled < 0 or policy_enabled > 1 or endpoint_supported < 0 or endpoint_supported > 1 or catalog_entry_present < 0 or catalog_entry_present > 1 or embeddings_endpoint < 0 or embeddings_endpoint > 1:
        return 1
    if missing_feature_present < 0 or missing_feature_present > 1 or missing_feature < 0 or missing_feature > 8 or reasoning_effort_unsupported < 0 or reasoning_effort_unsupported > 1:
        return 1
    if explicit_output_present < 0 or explicit_output_present > 1 or default_output_present < 0 or default_output_present > 1 or reasoning_reserve_present < 0 or reasoning_reserve_present > 1:
        return 1
    if max_output_present < 0 or max_output_present > 1 or context_window_present < 0 or context_window_present > 1:
        return 1
    if unknown_context_policy < UNKNOWN_ALLOW or unknown_context_policy > UNKNOWN_REJECT or oversized_output_policy < OVERSIZED_PASSTHROUGH or oversized_output_policy > OVERSIZED_CLAMP:
        return 1
    if output_limit_field_present < 0 or output_limit_field_present > 1 or output_limit_field < 0 or output_limit_field > 2:
        return 1

    var total = provider_constraint_total(
        estimated_input_tokens,
        explicit_output_tokens,
        explicit_output_present,
        default_output_reserve_tokens,
        default_output_present,
        reasoning_reserve_tokens,
        reasoning_reserve_present,
    )
    var result_decision: Int64 = DECISION_COMPATIBLE
    var result_eligible: Int64 = 1
    var result_warnings: UInt64 = 0
    var result_adjusted_output_present: Int64 = 0
    var result_adjusted_output = explicit_output_tokens
    var result_available_context_present = context_window_present
    var result_available_context = context_window_tokens
    var result_adjustment_field_present: Int64 = 0
    var result_adjustment_field: Int64 = 0
    var result_adjustment_reason_present: Int64 = 0
    var result_adjustment_reason: Int64 = 0
    var result_output_rejected: Int64 = 0

    if policy_enabled == 0:
        decision[unsafe_offset=0] = result_decision
        eligible[unsafe_offset=0] = result_eligible
        result_missing_feature_present[unsafe_offset=0] = 0
        result_missing_feature[unsafe_offset=0] = missing_feature
        adjusted_output_tokens[unsafe_offset=0] = result_adjusted_output
        adjusted_output_present[unsafe_offset=0] = 0
        total_required_tokens[unsafe_offset=0] = total
        available_context_tokens[unsafe_offset=0] = result_available_context
        available_context_present[unsafe_offset=0] = result_available_context_present
        result_max_output_tokens[unsafe_offset=0] = max_output_tokens
        result_max_output_present[unsafe_offset=0] = max_output_present
        warnings[unsafe_offset=0] = 0
        adjustment_field[unsafe_offset=0] = 0
        adjustment_field_present[unsafe_offset=0] = 0
        adjustment_reason[unsafe_offset=0] = 0
        adjustment_reason_present[unsafe_offset=0] = 0
        return 0

    if endpoint_supported == 0 or (embeddings_endpoint == 1 and catalog_entry_present == 0):
        result_decision = DECISION_ENDPOINT_UNSUPPORTED
        result_eligible = 0
    elif catalog_entry_present == 0:
        result_decision = DECISION_CATALOG_UNAVAILABLE
        if unknown_context_policy == UNKNOWN_REJECT:
            result_eligible = 0
        else:
            result_eligible = 1
        result_available_context_present = 0
        if unknown_context_policy == UNKNOWN_SAFE_WINDOW:
            result_available_context = safe_window_tokens
            result_available_context_present = 1
            if total > safe_window_tokens:
                result_decision = DECISION_CONTEXT_EXCEEDED
                result_eligible = 0
            else:
                result_warnings = result_warnings | WARNING_CATALOG_UNAVAILABLE
        elif result_eligible == 1:
            result_warnings = result_warnings | WARNING_CATALOG_UNAVAILABLE
    elif missing_feature_present == 1:
        if missing_feature == FEATURE_REASONING:
            result_decision = DECISION_REASONING_UNSUPPORTED
        else:
            result_decision = DECISION_REQUIRED_CAPABILITY_MISSING
        result_eligible = 0
    elif reasoning_effort_unsupported == 1:
        result_decision = DECISION_REASONING_UNSUPPORTED
        result_eligible = 0
    else:
        if explicit_output_present == 1:
            if max_output_present == 1 and explicit_output_tokens > max_output_tokens:
                if oversized_output_policy == OVERSIZED_PASSTHROUGH:
                    result_decision = DECISION_OUTPUT_EXCEEDS_LIMIT
                    result_warnings = result_warnings | WARNING_OUTPUT_EXCEEDS_LIMIT
                elif oversized_output_policy == OVERSIZED_REJECT:
                    result_decision = DECISION_OUTPUT_EXCEEDS_LIMIT
                    result_eligible = 0
                    result_output_rejected = 1
                else:
                    result_decision = DECISION_OUTPUT_CLAMPED
                    result_adjusted_output = max_output_tokens
                    result_adjusted_output_present = 1
                    if output_limit_field_present == 1:
                        result_adjustment_field = output_limit_field
                    else:
                        result_adjustment_field = 0
                    result_adjustment_field_present = 1
                    result_adjustment_reason = DECISION_OUTPUT_CLAMPED
                    result_adjustment_reason_present = 1
                    total = provider_constraint_total(
                        estimated_input_tokens,
                        max_output_tokens,
                        1,
                        default_output_reserve_tokens,
                        default_output_present,
                        reasoning_reserve_tokens,
                        reasoning_reserve_present,
                    )
            elif max_output_present == 0:
                result_decision = DECISION_OUTPUT_UNKNOWN
                result_warnings = result_warnings | WARNING_OUTPUT_UNKNOWN
        if result_output_rejected == 0:
            if context_window_present == 1:
                if total > context_window_tokens:
                    if reasoning_reserve_present == 1 and reasoning_reserve_tokens > 0 and total - reasoning_reserve_tokens <= context_window_tokens:
                        result_decision = DECISION_REASONING_EXCESSIVE
                    else:
                        result_decision = DECISION_CONTEXT_EXCEEDED
                    result_eligible = 0
            elif unknown_context_policy == UNKNOWN_ALLOW:
                result_decision = DECISION_CONTEXT_UNKNOWN
                result_warnings = result_warnings | WARNING_CONTEXT_UNKNOWN
            elif unknown_context_policy == UNKNOWN_REJECT:
                result_decision = DECISION_CONTEXT_UNKNOWN
                result_eligible = 0
            else:
                result_available_context = safe_window_tokens
                result_available_context_present = 1
                if total > safe_window_tokens:
                    result_decision = DECISION_CONTEXT_EXCEEDED
                    result_eligible = 0
                else:
                    result_decision = DECISION_CONTEXT_UNKNOWN
                    result_warnings = result_warnings | WARNING_CONTEXT_UNKNOWN
    decision[unsafe_offset=0] = result_decision
    eligible[unsafe_offset=0] = result_eligible
    result_missing_feature_present[unsafe_offset=0] = missing_feature_present
    result_missing_feature[unsafe_offset=0] = missing_feature
    adjusted_output_tokens[unsafe_offset=0] = result_adjusted_output
    adjusted_output_present[unsafe_offset=0] = result_adjusted_output_present
    total_required_tokens[unsafe_offset=0] = total
    available_context_tokens[unsafe_offset=0] = result_available_context
    available_context_present[unsafe_offset=0] = result_available_context_present
    result_max_output_tokens[unsafe_offset=0] = max_output_tokens
    result_max_output_present[unsafe_offset=0] = max_output_present
    warnings[unsafe_offset=0] = result_warnings
    adjustment_field[unsafe_offset=0] = result_adjustment_field
    adjustment_field_present[unsafe_offset=0] = result_adjustment_field_present
    adjustment_reason[unsafe_offset=0] = result_adjustment_reason
    adjustment_reason_present[unsafe_offset=0] = result_adjustment_reason_present
    return 0
