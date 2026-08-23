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
        total = provider_constraint_saturating_add(
            total, explicit_output_tokens
        )
    elif default_output_present == 1:
        total = provider_constraint_saturating_add(
            total, default_output_reserve_tokens
        )
    if reasoning_reserve_present == 1:
        total = provider_constraint_saturating_add(
            total, reasoning_reserve_tokens
        )
    return total


comptime ABI_VERSION: Int64 = 2
comptime ABI_STATUS_MISMATCH: Int64 = 1
comptime ABI_STATUS_INVALID_INPUT: Int64 = 2

comptime INPUT_I64_FIELD_COUNT: Int64 = 17
comptime INPUT_U64_FIELD_COUNT: Int64 = 7
comptime OUTPUT_I64_FIELD_COUNT: Int64 = 12
comptime OUTPUT_U64_FIELD_COUNT: Int64 = 5

comptime INPUT_I64_ABI_VERSION: Int = 0
comptime INPUT_I64_POLICY_ENABLED: Int = 1
comptime INPUT_I64_ENDPOINT_SUPPORTED: Int = 2
comptime INPUT_I64_CATALOG_ENTRY_PRESENT: Int = 3
comptime INPUT_I64_EMBEDDINGS_ENDPOINT: Int = 4
comptime INPUT_I64_MISSING_FEATURE_PRESENT: Int = 5
comptime INPUT_I64_MISSING_FEATURE: Int = 6
comptime INPUT_I64_REASONING_UNSUPPORTED: Int = 7
comptime INPUT_I64_EXPLICIT_OUTPUT_PRESENT: Int = 8
comptime INPUT_I64_DEFAULT_OUTPUT_PRESENT: Int = 9
comptime INPUT_I64_REASONING_RESERVE_PRESENT: Int = 10
comptime INPUT_I64_MAX_OUTPUT_PRESENT: Int = 11
comptime INPUT_I64_CONTEXT_WINDOW_PRESENT: Int = 12
comptime INPUT_I64_UNKNOWN_CONTEXT_POLICY: Int = 13
comptime INPUT_I64_OVERSIZED_OUTPUT_POLICY: Int = 14
comptime INPUT_I64_OUTPUT_LIMIT_FIELD: Int = 15
comptime INPUT_I64_OUTPUT_LIMIT_FIELD_PRESENT: Int = 16

comptime INPUT_U64_ESTIMATED_INPUT_TOKENS: Int = 0
comptime INPUT_U64_EXPLICIT_OUTPUT_TOKENS: Int = 1
comptime INPUT_U64_DEFAULT_OUTPUT_RESERVE_TOKENS: Int = 2
comptime INPUT_U64_REASONING_RESERVE_TOKENS: Int = 3
comptime INPUT_U64_MAX_OUTPUT_TOKENS: Int = 4
comptime INPUT_U64_CONTEXT_WINDOW_TOKENS: Int = 5
comptime INPUT_U64_SAFE_WINDOW_TOKENS: Int = 6

comptime OUTPUT_I64_ABI_VERSION: Int = 0
comptime OUTPUT_I64_DECISION: Int = 1
comptime OUTPUT_I64_ELIGIBLE: Int = 2
comptime OUTPUT_I64_MISSING_FEATURE_PRESENT: Int = 3
comptime OUTPUT_I64_MISSING_FEATURE: Int = 4
comptime OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT: Int = 5
comptime OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT: Int = 6
comptime OUTPUT_I64_MAX_OUTPUT_PRESENT: Int = 7
comptime OUTPUT_I64_ADJUSTMENT_FIELD: Int = 8
comptime OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT: Int = 9
comptime OUTPUT_I64_ADJUSTMENT_REASON: Int = 10
comptime OUTPUT_I64_ADJUSTMENT_REASON_PRESENT: Int = 11

comptime OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS: Int = 0
comptime OUTPUT_U64_TOTAL_REQUIRED_TOKENS: Int = 1
comptime OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS: Int = 2
comptime OUTPUT_U64_MAX_OUTPUT_TOKENS: Int = 3
comptime OUTPUT_U64_WARNINGS: Int = 4


@export("prodex_provider_constraints_evaluate_v2")
def prodex_provider_constraints_evaluate_v2(
    input_i64: Pointer[mut=False, Int64, _],
    input_i64_count: Int64,
    input_u64: Pointer[mut=False, UInt64, _],
    input_u64_count: Int64,
    output_i64: Pointer[mut=True, Int64, _],
    output_i64_count: Int64,
    output_u64: Pointer[mut=True, UInt64, _],
    output_u64_count: Int64,
) abi("C") -> Int64:
    if (
        input_i64_count != INPUT_I64_FIELD_COUNT
        or input_u64_count != INPUT_U64_FIELD_COUNT
        or output_i64_count != OUTPUT_I64_FIELD_COUNT
        or output_u64_count != OUTPUT_U64_FIELD_COUNT
    ):
        return ABI_STATUS_MISMATCH
    if input_i64[unsafe_offset=INPUT_I64_ABI_VERSION] != ABI_VERSION:
        return ABI_STATUS_MISMATCH

    var policy_enabled = input_i64[unsafe_offset=INPUT_I64_POLICY_ENABLED]
    var endpoint_supported = input_i64[
        unsafe_offset=INPUT_I64_ENDPOINT_SUPPORTED
    ]
    var catalog_entry_present = input_i64[
        unsafe_offset=INPUT_I64_CATALOG_ENTRY_PRESENT
    ]
    var embeddings_endpoint = input_i64[
        unsafe_offset=INPUT_I64_EMBEDDINGS_ENDPOINT
    ]
    var missing_feature_present = input_i64[
        unsafe_offset=INPUT_I64_MISSING_FEATURE_PRESENT
    ]
    var missing_feature = input_i64[unsafe_offset=INPUT_I64_MISSING_FEATURE]
    var reasoning_effort_unsupported = input_i64[
        unsafe_offset=INPUT_I64_REASONING_UNSUPPORTED
    ]
    var explicit_output_present = input_i64[
        unsafe_offset=INPUT_I64_EXPLICIT_OUTPUT_PRESENT
    ]
    var default_output_present = input_i64[
        unsafe_offset=INPUT_I64_DEFAULT_OUTPUT_PRESENT
    ]
    var reasoning_reserve_present = input_i64[
        unsafe_offset=INPUT_I64_REASONING_RESERVE_PRESENT
    ]
    var max_output_present = input_i64[
        unsafe_offset=INPUT_I64_MAX_OUTPUT_PRESENT
    ]
    var context_window_present = input_i64[
        unsafe_offset=INPUT_I64_CONTEXT_WINDOW_PRESENT
    ]
    var unknown_context_policy = input_i64[
        unsafe_offset=INPUT_I64_UNKNOWN_CONTEXT_POLICY
    ]
    var oversized_output_policy = input_i64[
        unsafe_offset=INPUT_I64_OVERSIZED_OUTPUT_POLICY
    ]
    var output_limit_field = input_i64[
        unsafe_offset=INPUT_I64_OUTPUT_LIMIT_FIELD
    ]
    var output_limit_field_present = input_i64[
        unsafe_offset=INPUT_I64_OUTPUT_LIMIT_FIELD_PRESENT
    ]
    var estimated_input_tokens = input_u64[
        unsafe_offset=INPUT_U64_ESTIMATED_INPUT_TOKENS
    ]
    var explicit_output_tokens = input_u64[
        unsafe_offset=INPUT_U64_EXPLICIT_OUTPUT_TOKENS
    ]
    var default_output_reserve_tokens = input_u64[
        unsafe_offset=INPUT_U64_DEFAULT_OUTPUT_RESERVE_TOKENS
    ]
    var reasoning_reserve_tokens = input_u64[
        unsafe_offset=INPUT_U64_REASONING_RESERVE_TOKENS
    ]
    var max_output_tokens = input_u64[unsafe_offset=INPUT_U64_MAX_OUTPUT_TOKENS]
    var context_window_tokens = input_u64[
        unsafe_offset=INPUT_U64_CONTEXT_WINDOW_TOKENS
    ]
    var safe_window_tokens = input_u64[
        unsafe_offset=INPUT_U64_SAFE_WINDOW_TOKENS
    ]

    if (
        policy_enabled < 0
        or policy_enabled > 1
        or endpoint_supported < 0
        or endpoint_supported > 1
        or catalog_entry_present < 0
        or catalog_entry_present > 1
        or embeddings_endpoint < 0
        or embeddings_endpoint > 1
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        missing_feature_present < 0
        or missing_feature_present > 1
        or missing_feature < 0
        or missing_feature > 8
        or reasoning_effort_unsupported < 0
        or reasoning_effort_unsupported > 1
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        explicit_output_present < 0
        or explicit_output_present > 1
        or default_output_present < 0
        or default_output_present > 1
        or reasoning_reserve_present < 0
        or reasoning_reserve_present > 1
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        max_output_present < 0
        or max_output_present > 1
        or context_window_present < 0
        or context_window_present > 1
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        unknown_context_policy < UNKNOWN_ALLOW
        or unknown_context_policy > UNKNOWN_REJECT
        or oversized_output_policy < OVERSIZED_PASSTHROUGH
        or oversized_output_policy > OVERSIZED_CLAMP
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        output_limit_field_present < 0
        or output_limit_field_present > 1
        or output_limit_field < 0
        or output_limit_field > 2
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        (missing_feature_present == 0 and missing_feature != 0)
        or (explicit_output_present == 0 and explicit_output_tokens != 0)
        or (default_output_present == 0 and default_output_reserve_tokens != 0)
        or (reasoning_reserve_present == 0 and reasoning_reserve_tokens != 0)
        or (max_output_present == 0 and max_output_tokens != 0)
        or (context_window_present == 0 and context_window_tokens != 0)
        or (output_limit_field_present == 0 and output_limit_field != 0)
    ):
        return ABI_STATUS_INVALID_INPUT

    output_i64[unsafe_offset=OUTPUT_I64_ABI_VERSION] = ABI_VERSION

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
        output_i64[unsafe_offset=OUTPUT_I64_DECISION] = result_decision
        output_i64[unsafe_offset=OUTPUT_I64_ELIGIBLE] = result_eligible
        output_i64[unsafe_offset=OUTPUT_I64_MISSING_FEATURE_PRESENT] = 0
        output_i64[unsafe_offset=OUTPUT_I64_MISSING_FEATURE] = 0
        output_u64[unsafe_offset=OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS] = 0
        output_i64[unsafe_offset=OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT] = 0
        output_u64[unsafe_offset=OUTPUT_U64_TOTAL_REQUIRED_TOKENS] = total
        output_u64[
            unsafe_offset=OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS
        ] = result_available_context
        output_i64[
            unsafe_offset=OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT
        ] = result_available_context_present
        output_u64[
            unsafe_offset=OUTPUT_U64_MAX_OUTPUT_TOKENS
        ] = max_output_tokens
        output_i64[
            unsafe_offset=OUTPUT_I64_MAX_OUTPUT_PRESENT
        ] = max_output_present
        output_u64[unsafe_offset=OUTPUT_U64_WARNINGS] = 0
        output_i64[unsafe_offset=OUTPUT_I64_ADJUSTMENT_FIELD] = 0
        output_i64[unsafe_offset=OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT] = 0
        output_i64[unsafe_offset=OUTPUT_I64_ADJUSTMENT_REASON] = 0
        output_i64[unsafe_offset=OUTPUT_I64_ADJUSTMENT_REASON_PRESENT] = 0
        return 0

    if endpoint_supported == 0 or (
        embeddings_endpoint == 1 and catalog_entry_present == 0
    ):
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
            if (
                max_output_present == 1
                and explicit_output_tokens > max_output_tokens
            ):
                if oversized_output_policy == OVERSIZED_PASSTHROUGH:
                    result_decision = DECISION_OUTPUT_EXCEEDS_LIMIT
                    result_warnings = (
                        result_warnings | WARNING_OUTPUT_EXCEEDS_LIMIT
                    )
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
                    if (
                        reasoning_reserve_present == 1
                        and reasoning_reserve_tokens > 0
                        and total - reasoning_reserve_tokens
                        <= context_window_tokens
                    ):
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
    output_i64[unsafe_offset=OUTPUT_I64_DECISION] = result_decision
    output_i64[unsafe_offset=OUTPUT_I64_ELIGIBLE] = result_eligible
    output_i64[
        unsafe_offset=OUTPUT_I64_MISSING_FEATURE_PRESENT
    ] = missing_feature_present
    output_i64[unsafe_offset=OUTPUT_I64_MISSING_FEATURE] = missing_feature
    if result_adjusted_output_present == 1:
        output_u64[
            unsafe_offset=OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS
        ] = result_adjusted_output
    else:
        output_u64[unsafe_offset=OUTPUT_U64_ADJUSTED_OUTPUT_TOKENS] = 0
    output_i64[
        unsafe_offset=OUTPUT_I64_ADJUSTED_OUTPUT_PRESENT
    ] = result_adjusted_output_present
    output_u64[unsafe_offset=OUTPUT_U64_TOTAL_REQUIRED_TOKENS] = total
    if result_available_context_present == 1:
        output_u64[
            unsafe_offset=OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS
        ] = result_available_context
    else:
        output_u64[unsafe_offset=OUTPUT_U64_AVAILABLE_CONTEXT_TOKENS] = 0
    output_i64[
        unsafe_offset=OUTPUT_I64_AVAILABLE_CONTEXT_PRESENT
    ] = result_available_context_present
    output_u64[unsafe_offset=OUTPUT_U64_MAX_OUTPUT_TOKENS] = max_output_tokens
    output_i64[unsafe_offset=OUTPUT_I64_MAX_OUTPUT_PRESENT] = max_output_present
    output_u64[unsafe_offset=OUTPUT_U64_WARNINGS] = result_warnings
    output_i64[
        unsafe_offset=OUTPUT_I64_ADJUSTMENT_FIELD
    ] = result_adjustment_field
    output_i64[
        unsafe_offset=OUTPUT_I64_ADJUSTMENT_FIELD_PRESENT
    ] = result_adjustment_field_present
    output_i64[
        unsafe_offset=OUTPUT_I64_ADJUSTMENT_REASON
    ] = result_adjustment_reason
    output_i64[
        unsafe_offset=OUTPUT_I64_ADJUSTMENT_REASON_PRESENT
    ] = result_adjustment_reason_present
    return 0
