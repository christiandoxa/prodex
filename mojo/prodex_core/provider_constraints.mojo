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

comptime PROVIDER_REASONING_EFFORT_NONE: Int64 = 0
comptime PROVIDER_REASONING_EFFORT_MINIMAL: Int64 = 1
comptime PROVIDER_REASONING_EFFORT_UNKNOWN: Int64 = 8

def provider_constraint_endpoint_mask_has(mask: UInt64, endpoint: Int64) -> Bool:
    return mask & (UInt64(1) << UInt64(endpoint)) != 0

def provider_constraint_entry_endpoint_supported(
    endpoint_kind: Int64, supported_endpoint_mask: UInt64
) -> Bool:
    if endpoint_kind == 1:
        return provider_constraint_endpoint_mask_has(supported_endpoint_mask, 0) or provider_constraint_endpoint_mask_has(supported_endpoint_mask, 1)
    return provider_constraint_endpoint_mask_has(supported_endpoint_mask, endpoint_kind)

def provider_constraint_feature_supported(
    feature: Int64,
    feature_mask: UInt64,
) -> Bool:
    return feature_mask & (UInt64(1) << UInt64(feature)) != 0

@export("prodex_provider_constraints_resolve_v1")
def prodex_provider_constraints_resolve_v1(
    explicit_output_present: Int64,
    default_output_present: Int64,
    default_output_reserve_tokens: UInt64,
    requested_reasoning_effort: Int64,
    default_reasoning_effort: Int64,
    reasoning_reserve_present: Int64,
    reasoning_reserve_tokens: UInt64,
    reasoning_reserve_by_effort_address: UInt,
    reasoning_reserve_mask: UInt64,
    output_default_output_present: Pointer[mut=True, Int64, _],
    output_default_output_reserve_tokens: Pointer[mut=True, UInt64, _],
    output_reasoning_effort_present: Pointer[mut=True, Int64, _],
    output_reasoning_effort: Pointer[mut=True, Int64, _],
    output_reasoning_reserve_present: Pointer[mut=True, Int64, _],
    output_reasoning_reserve_tokens: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if (
        explicit_output_present < 0
        or explicit_output_present > 1
        or default_output_present < 0
        or default_output_present > 1
        or reasoning_reserve_present < 0
        or reasoning_reserve_present > 1
        or requested_reasoning_effort < -1
        or requested_reasoning_effort > PROVIDER_REASONING_EFFORT_UNKNOWN
        or default_reasoning_effort < -1
        or default_reasoning_effort > PROVIDER_REASONING_EFFORT_UNKNOWN
        or reasoning_reserve_mask > 511
    ):
        return ABI_STATUS_INVALID_INPUT
    if (
        (default_output_present == 0 and default_output_reserve_tokens != 0)
        or (reasoning_reserve_present == 0 and reasoning_reserve_tokens != 0)
        or (reasoning_reserve_mask != 0 and reasoning_reserve_by_effort_address == 0)
    ):
        return ABI_STATUS_INVALID_INPUT

    output_default_output_present[] = 0
    output_default_output_reserve_tokens[] = 0
    if explicit_output_present == 0 and default_output_present == 1:
        output_default_output_present[] = 1
        output_default_output_reserve_tokens[] = default_output_reserve_tokens
    output_reasoning_effort_present[] = 0
    output_reasoning_effort[] = -1
    var selected_reasoning_effort = requested_reasoning_effort
    if selected_reasoning_effort < 0:
        selected_reasoning_effort = default_reasoning_effort
    if selected_reasoning_effort >= 0:
        output_reasoning_effort_present[] = 1
        output_reasoning_effort[] = selected_reasoning_effort

    output_reasoning_reserve_present[] = 0
    output_reasoning_reserve_tokens[] = 0
    if reasoning_reserve_present == 1:
        output_reasoning_reserve_present[] = 1
        output_reasoning_reserve_tokens[] = reasoning_reserve_tokens
    elif selected_reasoning_effort >= 2 and selected_reasoning_effort <= 7 and reasoning_reserve_mask & (UInt64(1) << UInt64(selected_reasoning_effort)) != 0:
        var reserves = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
            unsafe_from_address=Int(reasoning_reserve_by_effort_address)
        )
        output_reasoning_reserve_present[] = 1
        output_reasoning_reserve_tokens[] = reserves[unsafe_offset=selected_reasoning_effort]
    return 0

@export("prodex_provider_constraints_preclassify_v1")
def prodex_provider_constraints_preclassify_v1(
    endpoint_kind: Int64,
    provider_endpoint_supported: Int64,
    catalog_entry_present: Int64,
    provider_streaming_supported: Int64,
    supported_endpoint_mask: UInt64,
    feature_mask: UInt64,
    required_features_address: UInt,
    required_feature_count: Int64,
    reasoning_effort: Int64,
    supported_reasoning_efforts_present: Int64,
    supported_reasoning_efforts: UInt64,
    endpoint_supported: Pointer[mut=True, Int64, _],
    missing_feature_present: Pointer[mut=True, Int64, _],
    missing_feature: Pointer[mut=True, Int64, _],
    reasoning_effort_unsupported: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if (
        endpoint_kind < 0
        or endpoint_kind > 10
        or provider_endpoint_supported < 0
        or provider_endpoint_supported > 1
        or catalog_entry_present < 0
        or catalog_entry_present > 1
        or provider_streaming_supported < 0
        or provider_streaming_supported > 1
        or supported_endpoint_mask > 2047
        or feature_mask > 511
        or required_feature_count < 0
        or required_feature_count > 9
        or reasoning_effort < -1
        or reasoning_effort > PROVIDER_REASONING_EFFORT_UNKNOWN
        or supported_reasoning_efforts_present < 0
        or supported_reasoning_efforts_present > 1
        or supported_reasoning_efforts > 511
    ):
        return ABI_STATUS_INVALID_INPUT
    if required_feature_count > 0 and required_features_address == 0:
        return ABI_STATUS_INVALID_INPUT

    var required_features = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(required_features_address)
    )
    endpoint_supported[] = provider_endpoint_supported
    if catalog_entry_present == 1 and not provider_constraint_entry_endpoint_supported(
        endpoint_kind, supported_endpoint_mask
    ):
        endpoint_supported[] = 0
    missing_feature_present[] = 0
    missing_feature[] = 0
    for index in range(required_feature_count):
        var feature = required_features[unsafe_offset=index]
        if feature < 0 or feature > 8:
            return ABI_STATUS_INVALID_INPUT
        if not provider_constraint_feature_supported(feature, feature_mask):
            missing_feature_present[] = 1
            missing_feature[] = feature
            break
    reasoning_effort_unsupported[] = 0
    if reasoning_effort == PROVIDER_REASONING_EFFORT_UNKNOWN:
        reasoning_effort_unsupported[] = 1
    elif reasoning_effort >= 2 and reasoning_effort <= 7 and supported_reasoning_efforts_present == 1 and supported_reasoning_efforts & (UInt64(1) << UInt64(reasoning_effort)) == 0:
        reasoning_effort_unsupported[] = 1
    return 0


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


comptime GEMINI_REQUEST_PLAN_ABI_VERSION: Int64 = 1
comptime GEMINI_REQUEST_PLAN_STATUS_ABI_MISMATCH: Int64 = 1
comptime GEMINI_REQUEST_PLAN_STATUS_INVALID_INPUT: Int64 = 2
comptime GEMINI_REQUEST_PLAN_STATUS_CAPACITY: Int64 = 3

comptime GEMINI_REQUEST_TARGET_TEMPERATURE: Int64 = 0
comptime GEMINI_REQUEST_TARGET_TOP_P: Int64 = 1
comptime GEMINI_REQUEST_TARGET_MAX_OUTPUT_TOKENS: Int64 = 2
comptime GEMINI_REQUEST_TARGET_STOP_SEQUENCES: Int64 = 3
comptime GEMINI_REQUEST_TARGET_TOP_K: Int64 = 4
comptime GEMINI_REQUEST_TARGET_SEED: Int64 = 5
comptime GEMINI_REQUEST_TARGET_PRESENCE_PENALTY: Int64 = 6
comptime GEMINI_REQUEST_TARGET_FREQUENCY_PENALTY: Int64 = 7
comptime GEMINI_REQUEST_TARGET_RESPONSE_MIME_TYPE: Int64 = 8
comptime GEMINI_REQUEST_TARGET_RESPONSE_SCHEMA: Int64 = 9
comptime GEMINI_REQUEST_TARGET_RESPONSE_JSON_SCHEMA: Int64 = 10
comptime GEMINI_REQUEST_TARGET_RESPONSE_MODALITIES: Int64 = 11
comptime GEMINI_REQUEST_TARGET_MEDIA_RESOLUTION: Int64 = 12
comptime GEMINI_REQUEST_TARGET_AUDIO_TIMESTAMP: Int64 = 13
comptime GEMINI_REQUEST_TARGET_SPEECH_CONFIG: Int64 = 14
comptime GEMINI_REQUEST_TARGET_CANDIDATE_COUNT: Int64 = 15
comptime GEMINI_REQUEST_TARGET_SAFETY_SETTINGS: Int64 = 16
comptime GEMINI_REQUEST_TARGET_CACHED_CONTENT: Int64 = 17
comptime GEMINI_REQUEST_TARGET_LABELS: Int64 = 18

def gemini_request_field_present(mask: UInt64, bit: Int64) -> Bool:
    return mask & (UInt64(1) << UInt64(bit)) != 0

def gemini_request_emit_field(
    target: Int64,
    source: Int64,
    output_targets: Pointer[mut=True, Int64, _],
    output_sources: Pointer[mut=True, Int64, _],
    output_capacity: Int64,
    output_count: Pointer[mut=True, Int64, _],
) -> Bool:
    if output_count[] >= output_capacity:
        return False
    output_targets[unsafe_offset=output_count[]] = target
    output_sources[unsafe_offset=output_count[]] = source
    output_count[] = output_count[] + 1
    return True

@export("prodex_gemini_request_field_plan_v1")
def prodex_gemini_request_field_plan_v1(
    abi_version: Int64,
    basic_fields: UInt64,
    extended_fields: UInt64,
    optional_fields: UInt64,
    output_targets: Pointer[mut=True, Int64, _],
    output_sources: Pointer[mut=True, Int64, _],
    output_capacity: Int64,
    output_count: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if abi_version != GEMINI_REQUEST_PLAN_ABI_VERSION:
        return GEMINI_REQUEST_PLAN_STATUS_ABI_MISMATCH
    if output_capacity < 0 or basic_fields > 63 or extended_fields > 8388607 or optional_fields > 31:
        return GEMINI_REQUEST_PLAN_STATUS_INVALID_INPUT

    output_count[] = 0
    if gemini_request_field_present(basic_fields, 0) and not gemini_request_emit_field(
        GEMINI_REQUEST_TARGET_TEMPERATURE, 0, output_targets, output_sources, output_capacity, output_count
    ):
        return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(basic_fields, 1) and not gemini_request_emit_field(
        GEMINI_REQUEST_TARGET_TOP_P, 1, output_targets, output_sources, output_capacity, output_count
    ):
        return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(basic_fields, 2) and not gemini_request_emit_field(
        GEMINI_REQUEST_TARGET_MAX_OUTPUT_TOKENS, 2, output_targets, output_sources, output_capacity, output_count
    ):
        return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(basic_fields, 3):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_STOP_SEQUENCES, 3, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(basic_fields, 4):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_STOP_SEQUENCES, 4, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(basic_fields, 5):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_STOP_SEQUENCES, 5, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY

    if gemini_request_field_present(extended_fields, 1):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_TOP_K, 7, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 0):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_TOP_K, 6, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 2) and not gemini_request_emit_field(
        GEMINI_REQUEST_TARGET_SEED, 8, output_targets, output_sources, output_capacity, output_count
    ):
        return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 4):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_PRESENCE_PENALTY, 10, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 3):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_PRESENCE_PENALTY, 9, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 6):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_FREQUENCY_PENALTY, 12, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 5):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_FREQUENCY_PENALTY, 11, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 8):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_MIME_TYPE, 14, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 7):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_MIME_TYPE, 13, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 10):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_SCHEMA, 16, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 9):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_SCHEMA, 15, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 12):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_JSON_SCHEMA, 18, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 11):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_JSON_SCHEMA, 17, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 14):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_MODALITIES, 20, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 13):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_RESPONSE_MODALITIES, 19, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 16):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_MEDIA_RESOLUTION, 22, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 15):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_MEDIA_RESOLUTION, 21, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 18):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_AUDIO_TIMESTAMP, 24, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 17):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_AUDIO_TIMESTAMP, 23, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 20):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_SPEECH_CONFIG, 26, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 19):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_SPEECH_CONFIG, 25, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(extended_fields, 21):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_CANDIDATE_COUNT, 27, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(extended_fields, 22):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_CANDIDATE_COUNT, 28, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY

    if gemini_request_field_present(optional_fields, 0):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_SAFETY_SETTINGS, 29, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(optional_fields, 1):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_SAFETY_SETTINGS, 30, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(optional_fields, 2):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_CACHED_CONTENT, 31, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    elif gemini_request_field_present(optional_fields, 3):
        if not gemini_request_emit_field(
            GEMINI_REQUEST_TARGET_CACHED_CONTENT, 32, output_targets, output_sources, output_capacity, output_count
        ):
            return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    if gemini_request_field_present(optional_fields, 4) and not gemini_request_emit_field(
        GEMINI_REQUEST_TARGET_LABELS, 33, output_targets, output_sources, output_capacity, output_count
    ):
        return GEMINI_REQUEST_PLAN_STATUS_CAPACITY
    return 0


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


@fieldwise_init
struct GeminiToolCallStringView(Copyable):
    var ptr: UInt64
    var len: UInt64


@fieldwise_init
struct GeminiToolCallIndexRecord(Copyable):
    var index: UInt64
    var explicit_call_id: Int64
    var done: Int64
    var name_present: Int64
    var name: GeminiToolCallStringView


@fieldwise_init
struct GeminiToolCallIndexBinding(Copyable):
    var id: GeminiToolCallStringView
    var index: UInt64


comptime GEMINI_TOOL_CALL_INDEX_ABI_VERSION: Int64 = 1
comptime GEMINI_TOOL_CALL_INDEX_UINT64_MAX: UInt64 = 18446744073709551615
comptime GEMINI_TOOL_CALL_INDEX_INT64_MAX: UInt64 = 9223372036854775807


def gemini_tool_call_string_view_valid(view: GeminiToolCallStringView) -> Bool:
    return view.len <= GEMINI_TOOL_CALL_INDEX_INT64_MAX and (
        view.len == 0 or view.ptr != 0
    )


def gemini_tool_call_string_view_equals(
    left: GeminiToolCallStringView, right: GeminiToolCallStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    if left.ptr == 0 or right.ptr == 0:
        return False
    var left_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(left.ptr)
    )
    var right_ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(right.ptr)
    )
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


def gemini_tool_call_index_contains(
    records: Pointer[mut=False, GeminiToolCallIndexRecord, _],
    record_count: Int64,
    index: UInt64,
) -> Bool:
    for offset in range(record_count):
        if records[unsafe_offset=offset].index == index:
            return True
    return False


@export("prodex_provider_constraints_gemini_tool_call_index_v1")
def prodex_provider_constraints_gemini_tool_call_index_v1(
    abi_version: Int64,
    part_index: UInt64,
    explicit_call_id_present: Int64,
    explicit_call_id_address: UInt64,
    name_address: UInt64,
    records_address: UInt64,
    record_count: Int64,
    bindings_address: UInt64,
    binding_count: Int64,
    output_index_address: UInt64,
) abi("C") -> Int64:
    if abi_version != GEMINI_TOOL_CALL_INDEX_ABI_VERSION:
        return ABI_STATUS_MISMATCH
    if (
        explicit_call_id_present < 0
        or explicit_call_id_present > 1
        or record_count < 0
        or binding_count < 0
        or explicit_call_id_address == 0
        or name_address == 0
        or output_index_address == 0
    ):
        return ABI_STATUS_INVALID_INPUT
    if record_count > 0 and records_address == 0:
        return ABI_STATUS_INVALID_INPUT
    if binding_count > 0 and bindings_address == 0:
        return ABI_STATUS_INVALID_INPUT

    var explicit_call_id = Pointer[
        mut=False, GeminiToolCallStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(explicit_call_id_address))[].copy()
    var name = Pointer[
        mut=False, GeminiToolCallStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(name_address))[].copy()
    if (
        not gemini_tool_call_string_view_valid(explicit_call_id)
        or not gemini_tool_call_string_view_valid(name)
    ):
        return ABI_STATUS_INVALID_INPUT
    if explicit_call_id_present == 1 and explicit_call_id.len == 0:
        return ABI_STATUS_INVALID_INPUT

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_index_address)
    )
    output[] = 0
    var records = Pointer[
        mut=False, GeminiToolCallIndexRecord, ImmUntrackedOrigin
    ](unsafe_from_address=Int(records_address))
    var bindings = Pointer[
        mut=False, GeminiToolCallIndexBinding, ImmUntrackedOrigin
    ](unsafe_from_address=Int(bindings_address))

    var has_previous_index = False
    var previous_index: UInt64 = 0
    for offset in range(record_count):
        var record = records[unsafe_offset=offset].copy()
        if (
            record.explicit_call_id < 0
            or record.explicit_call_id > 1
            or record.done < 0
            or record.done > 1
            or record.name_present < 0
            or record.name_present > 1
            or has_previous_index and record.index <= previous_index
            or not gemini_tool_call_string_view_valid(record.name)
            or record.name_present == 0 and record.name.len != 0
        ):
            return ABI_STATUS_INVALID_INPUT
        previous_index = record.index
        has_previous_index = True
    for offset in range(binding_count):
        if not gemini_tool_call_string_view_valid(bindings[unsafe_offset=offset].id):
            return ABI_STATUS_INVALID_INPUT
    if explicit_call_id_present == 1:
        for offset in range(binding_count):
            var binding = bindings[unsafe_offset=offset].copy()
            if gemini_tool_call_string_view_equals(binding.id, explicit_call_id):
                output[] = binding.index
                return 0
    elif record_count > 0:
        for offset in range(record_count):
            var record = records[unsafe_offset=offset].copy()
            if (
                record.index == part_index
                and record.done == 0
                and record.explicit_call_id == 0
                and (
                    record.name_present == 0
                    or gemini_tool_call_string_view_equals(record.name, name)
                )
            ):
                output[] = part_index
                return 0
        for offset in range(record_count):
            var record = records[unsafe_offset=offset].copy()
            if (
                record.done == 0
                and record.explicit_call_id == 0
                and record.name_present == 1
                and gemini_tool_call_string_view_equals(record.name, name)
            ):
                output[] = record.index
                return 0
    if not gemini_tool_call_index_contains(records, record_count, part_index):
        output[] = part_index
        return 0

    var candidate = UInt64(record_count)
    var attempts: Int64 = 0
    while attempts <= record_count:
        if not gemini_tool_call_index_contains(records, record_count, candidate):
            output[] = candidate
            return 0
        if candidate == GEMINI_TOOL_CALL_INDEX_UINT64_MAX:
            return ABI_STATUS_INVALID_INPUT
        candidate += 1
        attempts += 1
    return ABI_STATUS_INVALID_INPUT


# The request translator keeps JSON parsing at its Rust boundary, then passes
# canonical JSON fragments to this bounded writer. Mojo owns the recursive
# schema normalization and the wire-shape decisions; Rust still owns media,
# validation, provider adapters, and every host-side effect.
comptime GEMINI_REQUEST_CONTENT_ABI_VERSION: Int64 = 1
comptime GEMINI_REQUEST_CONTENT_STATUS_INVALID: Int64 = 1
comptime GEMINI_REQUEST_CONTENT_STATUS_CAPACITY: Int64 = 2
comptime GEMINI_REQUEST_CONTENT_STATUS_ABI_MISMATCH: Int64 = 3
comptime GEMINI_REQUEST_CONTENT_MAX_BYTES: Int64 = 4_194_304
comptime GEMINI_REQUEST_CONTENT_MAX_DEPTH: Int64 = 256

comptime GEMINI_REQUEST_CONTENT_SANITIZE_SCHEMA: Int64 = 1
comptime GEMINI_REQUEST_CONTENT_SANITIZE_FUNCTION_SCHEMA: Int64 = 2
comptime GEMINI_REQUEST_CONTENT_CONTENT: Int64 = 3
comptime GEMINI_REQUEST_CONTENT_SYSTEM_INSTRUCTION: Int64 = 4
comptime GEMINI_REQUEST_CONTENT_TEXT_PART: Int64 = 5
comptime GEMINI_REQUEST_CONTENT_FUNCTION_CALL_PART: Int64 = 6
comptime GEMINI_REQUEST_CONTENT_FUNCTION_RESPONSE_PART: Int64 = 7
comptime GEMINI_REQUEST_CONTENT_TOOL_DECLARATION: Int64 = 8
comptime GEMINI_REQUEST_CONTENT_TOOL_CONFIG: Int64 = 9
comptime GEMINI_REQUEST_CONTENT_BUILTIN_TOOL: Int64 = 10

@fieldwise_init
struct GeminiRequestContentStringView(Copyable):
    var ptr: UInt64
    var len: UInt64


@fieldwise_init
struct GeminiRequestContentInput(Copyable):
    var operation: Int64
    var primary: GeminiRequestContentStringView
    var secondary: GeminiRequestContentStringView
    var tertiary: GeminiRequestContentStringView
    var quaternary: GeminiRequestContentStringView
    var primary_present: Int64
    var secondary_present: Int64
    var tertiary_present: Int64
    var quaternary_present: Int64
    var kind: Int64


@fieldwise_init
struct GeminiRequestContentWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def gemini_request_content_view_ptr(
    view: GeminiRequestContentStringView,
) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(view.ptr)
    )


def gemini_request_content_view_valid(
    view: GeminiRequestContentStringView,
) -> Bool:
    return view.len <= UInt64(GEMINI_REQUEST_CONTENT_MAX_BYTES) and (
        view.len == 0 or view.ptr != 0
    )


def gemini_request_content_byte(
    view: GeminiRequestContentStringView, index: Int64
) -> UInt8:
    return gemini_request_content_view_ptr(view)[unsafe_offset=index]


def gemini_request_content_put_byte(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def gemini_request_content_put_literal(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not gemini_request_content_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def gemini_request_content_put_range(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var ptr = gemini_request_content_view_ptr(view)
    for index in range(end - start):
        if not gemini_request_content_put_byte(
            writer, ptr[unsafe_offset=start + index]
        ):
            return False
    return True


def gemini_request_content_skip_ws(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end:
        var value = gemini_request_content_byte(view, index)
        if value != 9 and value != 10 and value != 13 and value != 32:
            break
        index += 1
    return index


def gemini_request_content_string_end(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Int64:
    if start < 0 or start >= end or gemini_request_content_byte(view, start) != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = gemini_request_content_byte(view, index)
        if value == 92:
            if index + 1 >= end:
                return -1
            index += 2
        elif value == 34:
            return index + 1
        elif value < 32:
            return -1
        else:
            index += 1
    return -1


def gemini_request_content_value_end(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Int64:
    if depth > GEMINI_REQUEST_CONTENT_MAX_DEPTH:
        return -1
    var index = gemini_request_content_skip_ws(view, start, end)
    if index >= end:
        return -1
    var opening = gemini_request_content_byte(view, index)
    if opening == 34:
        return gemini_request_content_string_end(view, index, end)
    if opening == 91:
        index += 1
        index = gemini_request_content_skip_ws(view, index, end)
        if index < end and gemini_request_content_byte(view, index) == 93:
            return index + 1
        while index < end:
            var value_end = gemini_request_content_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = gemini_request_content_skip_ws(view, value_end, end)
            if index < end and gemini_request_content_byte(view, index) == 44:
                index = gemini_request_content_skip_ws(view, index + 1, end)
                continue
            if index < end and gemini_request_content_byte(view, index) == 93:
                return index + 1
            return -1
        return -1
    if opening == 123:
        index += 1
        index = gemini_request_content_skip_ws(view, index, end)
        if index < end and gemini_request_content_byte(view, index) == 125:
            return index + 1
        while index < end:
            var key_end = gemini_request_content_string_end(view, index, end)
            if key_end < 0:
                return -1
            index = gemini_request_content_skip_ws(view, key_end, end)
            if index >= end or gemini_request_content_byte(view, index) != 58:
                return -1
            index = gemini_request_content_skip_ws(view, index + 1, end)
            var value_end = gemini_request_content_value_end(view, index, end, depth + 1)
            if value_end < 0:
                return -1
            index = gemini_request_content_skip_ws(view, value_end, end)
            if index < end and gemini_request_content_byte(view, index) == 44:
                index = gemini_request_content_skip_ws(view, index + 1, end)
                continue
            if index < end and gemini_request_content_byte(view, index) == 125:
                return index + 1
            return -1
        return -1
    var primitive_start = index
    while index < end:
        var value = gemini_request_content_byte(view, index)
        if value == 9 or value == 10 or value == 13 or value == 32 or value == 44 or value == 93 or value == 125:
            break
        index += 1
    if index == primitive_start:
        return -1
    return index


def gemini_request_content_fragment_valid(
    view: GeminiRequestContentStringView,
) -> Bool:
    if not gemini_request_content_view_valid(view) or view.len == 0:
        return False
    var end = Int64(view.len)
    var value_end = gemini_request_content_value_end(view, 0, end, 0)
    return value_end >= 0 and gemini_request_content_skip_ws(view, value_end, end) == end


def gemini_request_content_raw_equals(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end < start + 2:
        return False
    if gemini_request_content_byte(view, start) != 34 or gemini_request_content_byte(view, end - 1) != 34:
        return False
    var expected_length = Int64(literal.byte_length())
    if end - start - 2 != expected_length:
        return False
    var expected = literal.unsafe_ptr()
    var actual = gemini_request_content_view_ptr(view)
    for index in range(expected_length):
        if actual[unsafe_offset=start + 1 + index] != expected[unsafe_offset=index]:
            return False
    return True


def gemini_request_content_string_equals(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
    fold_ascii: Bool,
) -> Bool:
    if not gemini_request_content_raw_equals(view, start, end, literal):
        if not fold_ascii or start < 0 or end < start + 2:
            return False
        if gemini_request_content_byte(view, start) != 34 or gemini_request_content_byte(view, end - 1) != 34:
            return False
        var expected_length = Int64(literal.byte_length())
        if end - start - 2 != expected_length:
            return False
        var expected = literal.unsafe_ptr()
        var actual = gemini_request_content_view_ptr(view)
        for index in range(expected_length):
            var left = actual[unsafe_offset=start + 1 + index]
            var right = expected[unsafe_offset=index]
            if left >= 65 and left <= 90:
                left += 32
            if right >= 65 and right <= 90:
                right += 32
            if left != right:
                return False
        return True
    return True


def gemini_request_content_object_member(
    view: GeminiRequestContentStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> InlineArray[Int64, 2]:
    var result = InlineArray[Int64, 2](fill=-1)
    if object_start < 0 or object_end > Int64(view.len) or object_end <= object_start + 1:
        return result^
    if gemini_request_content_byte(view, object_start) != 123 or gemini_request_content_byte(view, object_end - 1) != 125:
        return result^
    var index = gemini_request_content_skip_ws(view, object_start + 1, object_end - 1)
    while index < object_end - 1:
        var key_start = index
        var key_end = gemini_request_content_string_end(view, key_start, object_end - 1)
        if key_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        index = gemini_request_content_skip_ws(view, key_end, object_end - 1)
        if index >= object_end - 1 or gemini_request_content_byte(view, index) != 58:
            return InlineArray[Int64, 2](fill=-1)^
        var value_start = gemini_request_content_skip_ws(view, index + 1, object_end - 1)
        var value_end = gemini_request_content_value_end(view, value_start, object_end - 1, 0)
        if value_end < 0:
            return InlineArray[Int64, 2](fill=-1)^
        if gemini_request_content_raw_equals(view, key_start, key_end, key):
            result[0] = value_start
            result[1] = value_end
        index = gemini_request_content_skip_ws(view, value_end, object_end - 1)
        if index < object_end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, object_end - 1)
            continue
        if index == object_end - 1:
            break
        return InlineArray[Int64, 2](fill=-1)^
    return result^


def gemini_request_content_array_nonempty(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end > Int64(view.len) or end <= start + 1:
        return False
    if gemini_request_content_byte(view, start) != 91 or gemini_request_content_byte(view, end - 1) != 93:
        return False
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    return index < end - 1 and gemini_request_content_byte(view, index) != 93


def gemini_request_content_hex(value: UInt8) -> Int64:
    if value >= 48 and value <= 57:
        return Int64(value - 48)
    if value >= 65 and value <= 70:
        return Int64(value - 65) + 10
    if value >= 97 and value <= 102:
        return Int64(value - 97) + 10
    return -1


def gemini_request_content_utf8_width(value: UInt8) -> Int64:
    if value <= 127:
        return 1
    if value <= 223:
        return 2
    if value <= 239:
        return 3
    return 4


def gemini_request_content_codepoint(
    view: GeminiRequestContentStringView, index: Int64, width: Int64
) -> Int64:
    var ptr = gemini_request_content_view_ptr(view)
    var lead = Int64(ptr[unsafe_offset=index])
    if width == 1:
        return lead
    if width == 2:
        return ((lead & 31) << 6) | (Int64(ptr[unsafe_offset=index + 1]) & 63)
    if width == 3:
        return ((lead & 15) << 12) | ((Int64(ptr[unsafe_offset=index + 1]) & 63) << 6) | (Int64(ptr[unsafe_offset=index + 2]) & 63)
    return ((lead & 7) << 18) | ((Int64(ptr[unsafe_offset=index + 1]) & 63) << 12) | ((Int64(ptr[unsafe_offset=index + 2]) & 63) << 6) | (Int64(ptr[unsafe_offset=index + 3]) & 63)


def gemini_request_content_unicode_space(codepoint: Int64) -> Bool:
    return codepoint >= 28 and codepoint <= 31 or codepoint == 9 or codepoint == 10 or codepoint == 11 or codepoint == 12 or codepoint == 13 or codepoint == 32 or codepoint == 133 or codepoint == 160 or codepoint == 5760 or codepoint >= 8192 and codepoint <= 8202 or codepoint == 8232 or codepoint == 8233 or codepoint == 8239 or codepoint == 8287 or codepoint == 12288


def gemini_request_content_string_has_non_space(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end <= start + 1 or gemini_request_content_byte(view, start) != 34:
        return False
    var index = start + 1
    var content_end = end - 1
    while index < content_end:
        var value = gemini_request_content_byte(view, index)
        if value == 92:
            if index + 1 >= content_end:
                return False
            var escaped = gemini_request_content_byte(view, index + 1)
            if escaped == 117 and index + 5 < content_end:
                var codepoint: Int64 = 0
                for offset in range(4):
                    var digit = gemini_request_content_hex(
                        gemini_request_content_byte(view, index + 2 + Int64(offset))
                    )
                    if digit < 0:
                        return True
                    codepoint = (codepoint << 4) | digit
                if not gemini_request_content_unicode_space(codepoint):
                    return True
                index += 6
                continue
            if escaped != 110 and escaped != 116 and escaped != 114 and escaped != 102:
                return True
            index += 2
            continue
        var width = gemini_request_content_utf8_width(value)
        if not gemini_request_content_unicode_space(
            gemini_request_content_codepoint(view, index, width)
        ):
            return True
        index += width
    return False


def gemini_request_content_supported_type(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Int64:
    if gemini_request_content_string_equals(view, start, end, StringSlice("object"), True):
        return 0
    if gemini_request_content_string_equals(view, start, end, StringSlice("array"), True):
        return 1
    if gemini_request_content_string_equals(view, start, end, StringSlice("string"), True):
        return 2
    if gemini_request_content_string_equals(view, start, end, StringSlice("integer"), True):
        return 3
    if gemini_request_content_string_equals(view, start, end, StringSlice("number"), True):
        return 4
    if gemini_request_content_string_equals(view, start, end, StringSlice("boolean"), True):
        return 5
    return -1


def gemini_request_content_put_schema_type(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    schema_kind: Int64,
) -> Bool:
    if schema_kind == 0:
        return gemini_request_content_put_literal(writer, StringSlice("\"object\""))
    if schema_kind == 1:
        return gemini_request_content_put_literal(writer, StringSlice("\"array\""))
    if schema_kind == 2:
        return gemini_request_content_put_literal(writer, StringSlice("\"string\""))
    if schema_kind == 3:
        return gemini_request_content_put_literal(writer, StringSlice("\"integer\""))
    if schema_kind == 4:
        return gemini_request_content_put_literal(writer, StringSlice("\"number\""))
    if schema_kind == 5:
        return gemini_request_content_put_literal(writer, StringSlice("\"boolean\""))
    return False


def gemini_request_content_schema_type(
    view: GeminiRequestContentStringView,
    object_start: Int64,
    object_end: Int64,
    nullable: Pointer[mut=True, Int64, _],
) -> Int64:
    nullable[] = 0
    var type_bounds = gemini_request_content_object_member(
        view, object_start, object_end, StringSlice("type")
    )
    var selected: Int64 = -1
    if type_bounds[0] >= 0:
        if gemini_request_content_byte(view, type_bounds[0]) == 34:
            selected = gemini_request_content_supported_type(
                view, type_bounds[0], type_bounds[1]
            )
        elif gemini_request_content_byte(view, type_bounds[0]) == 91:
            var index = gemini_request_content_skip_ws(view, type_bounds[0] + 1, type_bounds[1] - 1)
            while index < type_bounds[1] - 1:
                var value_end = gemini_request_content_value_end(
                    view, index, type_bounds[1] - 1, 0
                )
                if value_end < 0:
                    return -1
                if gemini_request_content_byte(view, index) == 34:
                    if gemini_request_content_string_equals(view, index, value_end, StringSlice("null"), False):
                        nullable[] = 1
                    elif selected < 0:
                        selected = gemini_request_content_supported_type(
                            view, index, value_end
                        )
                index = gemini_request_content_skip_ws(view, value_end, type_bounds[1] - 1)
                if index < type_bounds[1] - 1 and gemini_request_content_byte(view, index) == 44:
                    index = gemini_request_content_skip_ws(view, index + 1, type_bounds[1] - 1)
                else:
                    break
    if selected >= 0:
        return selected
    if gemini_request_content_object_member(view, object_start, object_end, StringSlice("properties"))[0] >= 0:
        return 0
    if gemini_request_content_object_member(view, object_start, object_end, StringSlice("items"))[0] >= 0:
        return 1
    if gemini_request_content_object_member(view, object_start, object_end, StringSlice("enum"))[0] >= 0 or gemini_request_content_object_member(view, object_start, object_end, StringSlice("const"))[0] >= 0:
        return 2
    return -1


def gemini_request_content_schema_alternative_is_null(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end <= start or gemini_request_content_byte(view, start) != 123:
        return False
    var type_bounds = gemini_request_content_object_member(
        view, start, end, StringSlice("type")
    )
    return type_bounds[0] >= 0 and gemini_request_content_string_equals(
        view, type_bounds[0], type_bounds[1], StringSlice("null"), False
    )


def gemini_request_content_put_schema_field_prefix(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
    key: StringSlice,
) -> Bool:
    if not first[] and not gemini_request_content_put_byte(writer, 44):
        return False
    first[] = False
    return gemini_request_content_put_literal(writer, key)


def gemini_request_content_write_sanitized_schema(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    depth: Int64,
) -> Bool:
    if depth > GEMINI_REQUEST_CONTENT_MAX_DEPTH or start < 0 or end <= start:
        return False
    var value = gemini_request_content_byte(view, start)
    if value == 91:
        if not gemini_request_content_put_byte(writer, 91):
            return False
        var first = True
        var first_ptr = Pointer(to=first)
        var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
        while index < end - 1 and gemini_request_content_byte(view, index) != 93:
            var value_end = gemini_request_content_value_end(view, index, end - 1, depth + 1)
            if value_end < 0:
                return False
            if not first_ptr[] and not gemini_request_content_put_byte(writer, 44):
                return False
            first_ptr[] = False
            if not gemini_request_content_write_sanitized_schema(view, index, value_end, writer, depth + 1):
                return False
            index = gemini_request_content_skip_ws(view, value_end, end - 1)
            if index < end - 1 and gemini_request_content_byte(view, index) == 44:
                index = gemini_request_content_skip_ws(view, index + 1, end - 1)
            else:
                break
        return gemini_request_content_put_byte(writer, 93)
    if value != 123:
        return gemini_request_content_put_range(writer, view, start, end)
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(view, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(view, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(view, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(view, value_start, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not gemini_request_content_raw_equals(view, key_start, key_end, StringSlice("strict")) and not gemini_request_content_raw_equals(view, key_start, key_end, StringSlice("$schema")) and not gemini_request_content_raw_equals(view, key_start, key_end, StringSlice("additionalProperties")):
            if not first_ptr[] and not gemini_request_content_put_byte(writer, 44):
                return False
            first_ptr[] = False
            if not gemini_request_content_put_range(writer, view, key_start, key_end) or not gemini_request_content_put_byte(writer, 58):
                return False
            if not gemini_request_content_write_sanitized_schema(view, value_start, value_end, writer, depth + 1):
                return False
        index = gemini_request_content_skip_ws(view, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, end - 1)
        else:
            break
    return gemini_request_content_put_byte(writer, 125)


def gemini_request_content_count_string_values(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    require_non_space: Bool,
) -> Int64:
    if start < 0 or end <= start + 1 or gemini_request_content_byte(view, start) != 91 or gemini_request_content_byte(view, end - 1) != 93:
        return -1
    var count: Int64 = 0
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(view, index) != 93:
        var value_end = gemini_request_content_value_end(view, index, end - 1, 0)
        if value_end < 0:
            return -1
        if gemini_request_content_byte(view, index) == 34 and (
            not require_non_space or gemini_request_content_string_has_non_space(view, index, value_end)
        ):
            count += 1
        index = gemini_request_content_skip_ws(view, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, end - 1)
        else:
            break
    return count


def gemini_request_content_write_string_values(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    require_non_space: Bool,
) -> Bool:
    if not gemini_request_content_put_byte(writer, 91):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(view, index) != 93:
        var value_end = gemini_request_content_value_end(view, index, end - 1, 0)
        if value_end < 0:
            return False
        if gemini_request_content_byte(view, index) == 34 and (
            not require_non_space or gemini_request_content_string_has_non_space(view, index, value_end)
        ):
            if not first and not gemini_request_content_put_byte(writer, 44):
                return False
            first = False
            if not gemini_request_content_put_range(writer, view, index, value_end):
                return False
        index = gemini_request_content_skip_ws(view, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, end - 1)
        else:
            break
    return gemini_request_content_put_byte(writer, 93)


def gemini_request_content_write_function_schema_array(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    depth: Int64,
) -> Bool:
    if not gemini_request_content_put_byte(writer, 91):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(view, index) != 93:
        var value_end = gemini_request_content_value_end(view, index, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not first and not gemini_request_content_put_byte(writer, 44):
            return False
        first = False
        if not gemini_request_content_write_function_schema(view, index, value_end, writer, depth + 1, False):
            return False
        index = gemini_request_content_skip_ws(view, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, end - 1)
        else:
            break
    return gemini_request_content_put_byte(writer, 93)


def gemini_request_content_write_function_properties(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    depth: Int64,
) -> Bool:
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(view, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(view, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(view, key_start, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(view, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(view, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(view, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(view, value_start, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not first and not gemini_request_content_put_byte(writer, 44):
            return False
        first = False
        if not gemini_request_content_put_range(writer, view, key_start, key_end) or not gemini_request_content_put_byte(writer, 58):
            return False
        if not gemini_request_content_write_function_schema(view, value_start, value_end, writer, depth + 1, False):
            return False
        index = gemini_request_content_skip_ws(view, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, end - 1)
        else:
            break
    return gemini_request_content_put_byte(writer, 125)


def gemini_request_content_write_function_schema(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    depth: Int64,
    force_nullable: Bool,
) -> Bool:
    if depth > GEMINI_REQUEST_CONTENT_MAX_DEPTH or start < 0 or end <= start:
        return False
    if gemini_request_content_byte(view, start) != 123:
        return gemini_request_content_put_literal(writer, StringSlice("{\"type\":\"object\"}"))

    var union_start: Int64 = -1
    var union_end: Int64 = -1
    var any_of = gemini_request_content_object_member(view, start, end, StringSlice("anyOf"))
    if gemini_request_content_array_nonempty(view, any_of[0], any_of[1]):
        union_start = any_of[0]
        union_end = any_of[1]
    else:
        var one_of = gemini_request_content_object_member(view, start, end, StringSlice("oneOf"))
        if gemini_request_content_array_nonempty(view, one_of[0], one_of[1]):
            union_start = one_of[0]
            union_end = one_of[1]
    if union_start >= 0 and gemini_request_content_byte(view, union_start) == 91:
        var alternative_count: Int64 = 0
        var non_null_count: Int64 = 0
        var alternative_start: Int64 = -1
        var alternative_end: Int64 = -1
        var index = gemini_request_content_skip_ws(view, union_start + 1, union_end - 1)
        while index < union_end - 1 and gemini_request_content_byte(view, index) != 93:
            var value_end = gemini_request_content_value_end(view, index, union_end - 1, depth + 1)
            if value_end < 0:
                return False
            alternative_count += 1
            if not gemini_request_content_schema_alternative_is_null(view, index, value_end):
                non_null_count += 1
                alternative_start = index
                alternative_end = value_end
            index = gemini_request_content_skip_ws(view, value_end, union_end - 1)
            if index < union_end - 1 and gemini_request_content_byte(view, index) == 44:
                index = gemini_request_content_skip_ws(view, index + 1, union_end - 1)
            else:
                break
        if non_null_count == 1:
            return gemini_request_content_write_function_schema(
                view,
                alternative_start,
                alternative_end,
                writer,
                depth + 1,
                force_nullable or alternative_count > non_null_count,
            )

    var nullable: Int64 = 0
    var nullable_ptr = Pointer(to=nullable)
    var schema_kind = gemini_request_content_schema_type(view, start, end, nullable_ptr)
    var any_composition = False
    var any_of_bounds = gemini_request_content_object_member(view, start, end, StringSlice("anyOf"))
    var one_of_bounds = gemini_request_content_object_member(view, start, end, StringSlice("oneOf"))
    var all_of_bounds = gemini_request_content_object_member(view, start, end, StringSlice("allOf"))
    if gemini_request_content_array_nonempty(view, any_of_bounds[0], any_of_bounds[1]) or gemini_request_content_array_nonempty(view, one_of_bounds[0], one_of_bounds[1]) or gemini_request_content_array_nonempty(view, all_of_bounds[0], all_of_bounds[1]):
        any_composition = True
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if schema_kind >= 0:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"type\":")):
            return False
        if not gemini_request_content_put_schema_type(writer, schema_kind):
            return False
    if nullable == 1 or force_nullable:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"nullable\":true")):
            return False
    var description_bounds = gemini_request_content_object_member(view, start, end, StringSlice("description"))
    if description_bounds[0] >= 0 and gemini_request_content_byte(view, description_bounds[0]) == 34 and gemini_request_content_string_has_non_space(view, description_bounds[0], description_bounds[1]):
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"description\":")) or not gemini_request_content_put_range(writer, view, description_bounds[0], description_bounds[1]):
            return False
    var format_bounds = gemini_request_content_object_member(view, start, end, StringSlice("format"))
    if format_bounds[0] >= 0 and gemini_request_content_byte(view, format_bounds[0]) == 34 and gemini_request_content_string_has_non_space(view, format_bounds[0], format_bounds[1]):
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"format\":")) or not gemini_request_content_put_range(writer, view, format_bounds[0], format_bounds[1]):
            return False
    var enum_bounds = gemini_request_content_object_member(view, start, end, StringSlice("enum"))
    if enum_bounds[0] >= 0 and gemini_request_content_count_string_values(view, enum_bounds[0], enum_bounds[1], False) > 0:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"enum\":")) or not gemini_request_content_write_string_values(view, enum_bounds[0], enum_bounds[1], writer, False):
            return False
    var properties_bounds = gemini_request_content_object_member(view, start, end, StringSlice("properties"))
    if properties_bounds[0] >= 0 and gemini_request_content_byte(view, properties_bounds[0]) == 123:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"properties\":")) or not gemini_request_content_write_function_properties(view, properties_bounds[0], properties_bounds[1], writer, depth + 1):
            return False
    var required_bounds = gemini_request_content_object_member(view, start, end, StringSlice("required"))
    if required_bounds[0] >= 0 and gemini_request_content_count_string_values(view, required_bounds[0], required_bounds[1], True) > 0:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"required\":")) or not gemini_request_content_write_string_values(view, required_bounds[0], required_bounds[1], writer, True):
            return False
    var items_bounds = gemini_request_content_object_member(view, start, end, StringSlice("items"))
    if items_bounds[0] >= 0:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"items\":")) or not gemini_request_content_write_function_schema(view, items_bounds[0], items_bounds[1], writer, depth + 1, False):
            return False
    if gemini_request_content_array_nonempty(view, any_of_bounds[0], any_of_bounds[1]):
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"anyOf\":")) or not gemini_request_content_write_function_schema_array(view, any_of_bounds[0], any_of_bounds[1], writer, depth + 1):
            return False
    if gemini_request_content_array_nonempty(view, one_of_bounds[0], one_of_bounds[1]):
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"oneOf\":")) or not gemini_request_content_write_function_schema_array(view, one_of_bounds[0], one_of_bounds[1], writer, depth + 1):
            return False
    if gemini_request_content_array_nonempty(view, all_of_bounds[0], all_of_bounds[1]):
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"allOf\":")) or not gemini_request_content_write_function_schema_array(view, all_of_bounds[0], all_of_bounds[1], writer, depth + 1):
            return False
    if schema_kind < 0 and not any_composition:
        if not gemini_request_content_put_schema_field_prefix(writer, first_ptr, StringSlice("\"type\":\"object\"")):
            return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_request_content_write_operation(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    input: GeminiRequestContentInput,
) -> Bool:
    var primary = input.primary.copy()
    var secondary = input.secondary.copy()
    var tertiary = input.tertiary.copy()
    if input.operation == GEMINI_REQUEST_CONTENT_SANITIZE_SCHEMA:
        var end = Int64(primary.len)
        var value_end = gemini_request_content_value_end(primary, 0, end, 0)
        return value_end == end and gemini_request_content_write_sanitized_schema(primary, 0, end, writer, 0)
    if input.operation == GEMINI_REQUEST_CONTENT_SANITIZE_FUNCTION_SCHEMA:
        var end = Int64(primary.len)
        var value_end = gemini_request_content_value_end(primary, 0, end, 0)
        if value_end != end:
            return False
        return gemini_request_content_write_function_schema(primary, 0, end, writer, 0, False)
    if input.operation == GEMINI_REQUEST_CONTENT_CONTENT:
        return gemini_request_content_put_literal(writer, StringSlice("{\"role\":")) and gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) and gemini_request_content_put_literal(writer, StringSlice(",\"parts\":")) and gemini_request_content_put_range(writer, secondary, 0, Int64(secondary.len)) and gemini_request_content_put_byte(writer, 125)
    if input.operation == GEMINI_REQUEST_CONTENT_SYSTEM_INSTRUCTION:
        return gemini_request_content_put_literal(writer, StringSlice("{\"parts\":[{\"text\":")) and gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) and gemini_request_content_put_literal(writer, StringSlice("}]")) and gemini_request_content_put_byte(writer, 125)
    if input.operation == GEMINI_REQUEST_CONTENT_TEXT_PART:
        return gemini_request_content_put_literal(writer, StringSlice("{\"text\":")) and gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) and gemini_request_content_put_byte(writer, 125)
    if input.operation == GEMINI_REQUEST_CONTENT_FUNCTION_CALL_PART:
        if not gemini_request_content_put_literal(writer, StringSlice("{\"functionCall\":{\"name\":")) or not gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) or not gemini_request_content_put_literal(writer, StringSlice(",\"args\":")) or not gemini_request_content_put_range(writer, secondary, 0, Int64(secondary.len)):
            return False
        if input.tertiary_present == 1 and (
            not gemini_request_content_put_literal(writer, StringSlice(",\"id\":")) or not gemini_request_content_put_range(writer, tertiary, 0, Int64(tertiary.len))
        ):
            return False
        return gemini_request_content_put_literal(writer, StringSlice("}}"))
    if input.operation == GEMINI_REQUEST_CONTENT_FUNCTION_RESPONSE_PART:
        if not gemini_request_content_put_literal(writer, StringSlice("{\"functionResponse\":{\"name\":")) or not gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) or not gemini_request_content_put_literal(writer, StringSlice(",\"response\":")) or not gemini_request_content_put_range(writer, secondary, 0, Int64(secondary.len)):
            return False
        if input.tertiary_present == 1 and (
            not gemini_request_content_put_literal(writer, StringSlice(",\"id\":")) or not gemini_request_content_put_range(writer, tertiary, 0, Int64(tertiary.len))
        ):
            return False
        return gemini_request_content_put_literal(writer, StringSlice("}}"))
    if input.operation == GEMINI_REQUEST_CONTENT_TOOL_DECLARATION:
        if not gemini_request_content_put_literal(writer, StringSlice("{\"name\":")) or not gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)):
            return False
        if input.secondary_present == 1 and (
            not gemini_request_content_put_literal(writer, StringSlice(",\"description\":")) or not gemini_request_content_put_range(writer, secondary, 0, Int64(secondary.len))
        ):
            return False
        return gemini_request_content_put_literal(writer, StringSlice(",\"parameters\":")) and gemini_request_content_put_range(writer, tertiary, 0, Int64(tertiary.len)) and gemini_request_content_put_byte(writer, 125)
    if input.operation == GEMINI_REQUEST_CONTENT_TOOL_CONFIG:
        if not gemini_request_content_put_literal(writer, StringSlice("{\"functionCallingConfig\":{\"mode\":")) or not gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)):
            return False
        if input.secondary_present == 1 and (
            not gemini_request_content_put_literal(writer, StringSlice(",\"allowedFunctionNames\":[")) or not gemini_request_content_put_range(writer, secondary, 0, Int64(secondary.len)) or not gemini_request_content_put_byte(writer, 93)
        ):
            return False
        return gemini_request_content_put_literal(writer, StringSlice("}}"))
    if input.operation == GEMINI_REQUEST_CONTENT_BUILTIN_TOOL:
        if input.kind == 1:
            return gemini_request_content_put_literal(writer, StringSlice("{\"computerUse\":")) and gemini_request_content_put_range(writer, primary, 0, Int64(primary.len)) and gemini_request_content_put_byte(writer, 125)
        if input.kind == 2:
            return gemini_request_content_put_literal(writer, StringSlice("{\"codeExecution\":{}}"))
        if input.kind == 3:
            return gemini_request_content_put_literal(writer, StringSlice("{\"googleSearch\":{}}"))
        if input.kind == 4:
            return gemini_request_content_put_literal(writer, StringSlice("{\"urlContext\":{}}"))
    return False


def gemini_request_content_input_valid(input: GeminiRequestContentInput) -> Bool:
    if input.operation < GEMINI_REQUEST_CONTENT_SANITIZE_SCHEMA or input.operation > GEMINI_REQUEST_CONTENT_BUILTIN_TOOL:
        return False
    if input.primary_present < 0 or input.primary_present > 1 or input.secondary_present < 0 or input.secondary_present > 1 or input.tertiary_present < 0 or input.tertiary_present > 1 or input.quaternary_present < 0 or input.quaternary_present > 1:
        return False
    if not gemini_request_content_view_valid(input.primary) or not gemini_request_content_view_valid(input.secondary) or not gemini_request_content_view_valid(input.tertiary) or not gemini_request_content_view_valid(input.quaternary):
        return False
    if input.primary_present == 0 and input.primary.len != 0 or input.secondary_present == 0 and input.secondary.len != 0 or input.tertiary_present == 0 and input.tertiary.len != 0 or input.quaternary_present == 0 and input.quaternary.len != 0:
        return False
    if input.operation == GEMINI_REQUEST_CONTENT_BUILTIN_TOOL and (input.kind < 1 or input.kind > 4):
        return False
    if input.operation != GEMINI_REQUEST_CONTENT_BUILTIN_TOOL and input.kind != 0:
        return False
    return True


@export("prodex_gemini_request_content_kernel_v1")
def prodex_gemini_request_content_kernel_v1(
    abi_version: Int64,
    input_address: UInt64,
    output_address: UInt64,
    output_capacity: Int64,
    written_address: UInt64,
) abi("C") -> Int64:
    if abi_version != GEMINI_REQUEST_CONTENT_ABI_VERSION:
        return GEMINI_REQUEST_CONTENT_STATUS_ABI_MISMATCH
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return GEMINI_REQUEST_CONTENT_STATUS_INVALID
    var input = Pointer[
        mut=False, GeminiRequestContentInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))[].copy()
    if not gemini_request_content_input_valid(input):
        return GEMINI_REQUEST_CONTENT_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = GeminiRequestContentWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if not gemini_request_content_write_operation(writer_ptr, input):
        written[] = writer.written
        if writer.written >= output_capacity:
            return GEMINI_REQUEST_CONTENT_STATUS_CAPACITY
        return GEMINI_REQUEST_CONTENT_STATUS_INVALID
    written[] = writer.written
    return 0
