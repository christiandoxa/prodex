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
