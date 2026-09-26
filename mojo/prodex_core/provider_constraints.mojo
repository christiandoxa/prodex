from std.collections import Array

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

comptime COMBO_ABI_VERSION: Int64 = 1
comptime COMBO_MAX_CANDIDATES: Int64 = 256
comptime COMBO_INPUT_I64_FIELDS: Int64 = 5
comptime COMBO_INPUT_U64_FIELDS: Int64 = 5
comptime COMBO_OUTPUT_I64_FIELDS: Int64 = 2
comptime COMBO_OUTPUT_U64_FIELDS: Int64 = 3
comptime COMBO_INPUT_I64_ELIGIBLE: Int64 = 0
comptime COMBO_INPUT_I64_OUTPUT_FIELD: Int64 = 1
comptime COMBO_INPUT_I64_ADJUSTMENT_PRESENT: Int64 = 2
comptime COMBO_INPUT_I64_EXPLICIT_PRESENT: Int64 = 3
comptime COMBO_INPUT_I64_REASONING_PRESENT: Int64 = 4
comptime COMBO_INPUT_U64_ADJUSTMENT_REQUESTED: Int64 = 0
comptime COMBO_INPUT_U64_ADJUSTMENT_APPLIED: Int64 = 1
comptime COMBO_INPUT_U64_EXPLICIT: Int64 = 2
comptime COMBO_INPUT_U64_ESTIMATED_INPUT: Int64 = 3
comptime COMBO_INPUT_U64_REASONING_RESERVE: Int64 = 4
comptime COMBO_OUTPUT_I64_CHANGED: Int64 = 0
comptime COMBO_OUTPUT_I64_FIELD: Int64 = 1
comptime COMBO_OUTPUT_U64_REQUESTED: Int64 = 0
comptime COMBO_OUTPUT_U64_APPLIED: Int64 = 1
comptime COMBO_OUTPUT_U64_TOTAL: Int64 = 2


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


@export("prodex_provider_constraints_normalize_combo_v1")
def prodex_provider_constraints_normalize_combo_v1(
    abi_version: Int64,
    input_i64_address: UInt,
    input_i64_count: Int64,
    input_u64_address: UInt,
    input_u64_count: Int64,
    output_i64_address: UInt,
    output_i64_count: Int64,
    output_u64_address: UInt,
    output_u64_count: Int64,
    candidate_count: Int64,
) abi("C") -> Int64:
    if abi_version != COMBO_ABI_VERSION:
        return ABI_STATUS_MISMATCH
    if candidate_count < 0 or candidate_count > COMBO_MAX_CANDIDATES:
        return ABI_STATUS_INVALID_INPUT
    if (
        input_i64_count != candidate_count * COMBO_INPUT_I64_FIELDS
        or input_u64_count != candidate_count * COMBO_INPUT_U64_FIELDS
        or output_i64_count != candidate_count * COMBO_OUTPUT_I64_FIELDS
        or output_u64_count != candidate_count * COMBO_OUTPUT_U64_FIELDS
    ):
        return ABI_STATUS_MISMATCH
    if candidate_count > 0 and (
        input_i64_address == 0
        or input_u64_address == 0
        or output_i64_address == 0
        or output_u64_address == 0
    ):
        return ABI_STATUS_INVALID_INPUT

    var input_i64 = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_i64_address)
    )
    var input_u64 = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_u64_address)
    )
    var output_i64 = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_i64_address)
    )
    var output_u64 = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_u64_address)
    )
    var has_applied = False
    var minimum_applied: UInt64 = UINT64_MAX
    for index in range(candidate_count):
        var input_i64_base = index * COMBO_INPUT_I64_FIELDS
        var input_u64_base = index * COMBO_INPUT_U64_FIELDS
        var eligible = input_i64[unsafe_offset=input_i64_base + COMBO_INPUT_I64_ELIGIBLE]
        var output_field = input_i64[unsafe_offset=input_i64_base + COMBO_INPUT_I64_OUTPUT_FIELD]
        var adjustment_present = input_i64[
            unsafe_offset=input_i64_base + COMBO_INPUT_I64_ADJUSTMENT_PRESENT
        ]
        var explicit_present = input_i64[
            unsafe_offset=input_i64_base + COMBO_INPUT_I64_EXPLICIT_PRESENT
        ]
        var reasoning_present = input_i64[
            unsafe_offset=input_i64_base + COMBO_INPUT_I64_REASONING_PRESENT
        ]
        if (
            eligible < 0
            or eligible > 1
            or output_field < -1
            or output_field > 2
            or adjustment_present < 0
            or adjustment_present > 1
            or explicit_present < 0
            or explicit_present > 1
            or reasoning_present < 0
            or reasoning_present > 1
        ):
            return ABI_STATUS_INVALID_INPUT
        if adjustment_present == 0 and (
            input_u64[
                unsafe_offset=input_u64_base + COMBO_INPUT_U64_ADJUSTMENT_REQUESTED
            ] != 0
            or input_u64[
                unsafe_offset=input_u64_base + COMBO_INPUT_U64_ADJUSTMENT_APPLIED
            ] != 0
        ):
            return ABI_STATUS_INVALID_INPUT
        if explicit_present == 0 and input_u64[
            unsafe_offset=input_u64_base + COMBO_INPUT_U64_EXPLICIT
        ] != 0:
            return ABI_STATUS_INVALID_INPUT
        if reasoning_present == 0 and input_u64[
            unsafe_offset=input_u64_base + COMBO_INPUT_U64_REASONING_RESERVE
        ] != 0:
            return ABI_STATUS_INVALID_INPUT
        if eligible == 1 and adjustment_present == 1:
            var applied = input_u64[
                unsafe_offset=input_u64_base + COMBO_INPUT_U64_ADJUSTMENT_APPLIED
            ]
            if not has_applied or applied < minimum_applied:
                minimum_applied = applied
                has_applied = True

    for index in range(candidate_count):
        var input_i64_base = index * COMBO_INPUT_I64_FIELDS
        var input_u64_base = index * COMBO_INPUT_U64_FIELDS
        var output_i64_base = index * COMBO_OUTPUT_I64_FIELDS
        var output_u64_base = index * COMBO_OUTPUT_U64_FIELDS
        output_i64[unsafe_offset=output_i64_base + COMBO_OUTPUT_I64_CHANGED] = 0
        output_i64[unsafe_offset=output_i64_base + COMBO_OUTPUT_I64_FIELD] = -1
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_REQUESTED] = 0
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_APPLIED] = 0
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_TOTAL] = 0
        if not has_applied:
            continue
        var eligible = input_i64[unsafe_offset=input_i64_base + COMBO_INPUT_I64_ELIGIBLE]
        var output_field = input_i64[unsafe_offset=input_i64_base + COMBO_INPUT_I64_OUTPUT_FIELD]
        if eligible == 0 or output_field < 0:
            continue
        var adjustment_present = input_i64[
            unsafe_offset=input_i64_base + COMBO_INPUT_I64_ADJUSTMENT_PRESENT
        ]
        var explicit_present = input_i64[
            unsafe_offset=input_i64_base + COMBO_INPUT_I64_EXPLICIT_PRESENT
        ]
        var requested = minimum_applied
        if adjustment_present == 1:
            requested = input_u64[
                unsafe_offset=input_u64_base + COMBO_INPUT_U64_ADJUSTMENT_REQUESTED
            ]
        elif explicit_present == 1:
            requested = input_u64[
                unsafe_offset=input_u64_base + COMBO_INPUT_U64_EXPLICIT
            ]
        var total = provider_constraint_saturating_add(
            input_u64[unsafe_offset=input_u64_base + COMBO_INPUT_U64_ESTIMATED_INPUT],
            minimum_applied,
        )
        if input_i64[unsafe_offset=input_i64_base + COMBO_INPUT_I64_REASONING_PRESENT] == 1:
            total = provider_constraint_saturating_add(
                total,
                input_u64[unsafe_offset=input_u64_base + COMBO_INPUT_U64_REASONING_RESERVE],
            )
        output_i64[unsafe_offset=output_i64_base + COMBO_OUTPUT_I64_CHANGED] = 1
        output_i64[unsafe_offset=output_i64_base + COMBO_OUTPUT_I64_FIELD] = output_field
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_REQUESTED] = requested
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_APPLIED] = minimum_applied
        output_u64[unsafe_offset=output_u64_base + COMBO_OUTPUT_U64_TOTAL] = total
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
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    if object_start < 0 or object_end > Int64(view.len) or object_end <= object_start + 1:
        return result^
    if gemini_request_content_byte(view, object_start) != 123 or gemini_request_content_byte(view, object_end - 1) != 125:
        return result^
    var index = gemini_request_content_skip_ws(view, object_start + 1, object_end - 1)
    while index < object_end - 1:
        var key_start = index
        var key_end = gemini_request_content_string_end(view, key_start, object_end - 1)
        if key_end < 0:
            return Array[Int64, 2](fill=-1)^
        index = gemini_request_content_skip_ws(view, key_end, object_end - 1)
        if index >= object_end - 1 or gemini_request_content_byte(view, index) != 58:
            return Array[Int64, 2](fill=-1)^
        var value_start = gemini_request_content_skip_ws(view, index + 1, object_end - 1)
        var value_end = gemini_request_content_value_end(view, value_start, object_end - 1, 0)
        if value_end < 0:
            return Array[Int64, 2](fill=-1)^
        if gemini_request_content_raw_equals(view, key_start, key_end, key):
            result[0] = value_start
            result[1] = value_end
        index = gemini_request_content_skip_ws(view, value_end, object_end - 1)
        if index < object_end - 1 and gemini_request_content_byte(view, index) == 44:
            index = gemini_request_content_skip_ws(view, index + 1, object_end - 1)
            continue
        if index == object_end - 1:
            break
        return Array[Int64, 2](fill=-1)^
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


# The bridge request adapter owns only deterministic JSON decisions that are
# not part of the existing request-content kernel. JSON bytes stay borrowed
# across this boundary; Rust retains serialization, policy callbacks, media,
# credentials, and transport effects.
comptime GEMINI_BRIDGE_REQUEST_ABI_VERSION: Int64 = 1
comptime GEMINI_BRIDGE_REQUEST_STATUS_INVALID: Int64 = 1
comptime GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY: Int64 = 2
comptime GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH: Int64 = 3
comptime GEMINI_BRIDGE_REQUEST_MAX_BYTES: Int64 = 4_194_304

comptime GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_REQUEST: Int64 = 1
comptime GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_BODY: Int64 = 2
comptime GEMINI_BRIDGE_REQUEST_GENERATION_CONFIG: Int64 = 3
comptime GEMINI_BRIDGE_REQUEST_NATIVE_PROJECT: Int64 = 4
comptime GEMINI_BRIDGE_REQUEST_WITHOUT_TOOL: Int64 = 5
comptime GEMINI_BRIDGE_REQUEST_SIMPLE: Int64 = 6
comptime GEMINI_BRIDGE_REQUEST_VALIDATE_CANDIDATE_COUNT: Int64 = 7
comptime GEMINI_BRIDGE_REQUEST_TOOL_CONFIG: Int64 = 8
comptime GEMINI_BRIDGE_REQUEST_TEXT_CONTENTS: Int64 = 9
comptime GEMINI_BRIDGE_REQUEST_RAW_TRANSLATOR: Int64 = 10
comptime GEMINI_BRIDGE_REQUEST_VALIDATE_TRANSLATOR: Int64 = 11

@fieldwise_init
struct GeminiBridgeRequestInput(Copyable):
    var operation: Int64
    var primary: GeminiRequestContentStringView
    var secondary: GeminiRequestContentStringView
    var tertiary: GeminiRequestContentStringView
    var quaternary: GeminiRequestContentStringView
    var quinary: GeminiRequestContentStringView
    var senary: GeminiRequestContentStringView
    var septenary: GeminiRequestContentStringView
    var octonary: GeminiRequestContentStringView
    var primary_present: Int64
    var secondary_present: Int64
    var tertiary_present: Int64
    var quaternary_present: Int64
    var quinary_present: Int64
    var senary_present: Int64
    var septenary_present: Int64
    var octonary_present: Int64
    var kind: Int64


def gemini_bridge_request_input_flag_valid(value: Int64) -> Bool:
    return value == 0 or value == 1


def gemini_bridge_request_input_valid(input: GeminiBridgeRequestInput) -> Bool:
    if input.operation < GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_REQUEST or input.operation > GEMINI_BRIDGE_REQUEST_VALIDATE_TRANSLATOR:
        return False
    if not gemini_bridge_request_input_flag_valid(input.primary_present) or not gemini_bridge_request_input_flag_valid(input.secondary_present) or not gemini_bridge_request_input_flag_valid(input.tertiary_present) or not gemini_bridge_request_input_flag_valid(input.quaternary_present) or not gemini_bridge_request_input_flag_valid(input.quinary_present) or not gemini_bridge_request_input_flag_valid(input.senary_present) or not gemini_bridge_request_input_flag_valid(input.septenary_present) or not gemini_bridge_request_input_flag_valid(input.octonary_present):
        return False
    if not gemini_request_content_view_valid(input.primary) or not gemini_request_content_view_valid(input.secondary) or not gemini_request_content_view_valid(input.tertiary) or not gemini_request_content_view_valid(input.quaternary) or not gemini_request_content_view_valid(input.quinary) or not gemini_request_content_view_valid(input.senary) or not gemini_request_content_view_valid(input.septenary) or not gemini_request_content_view_valid(input.octonary):
        return False
    if input.primary_present == 0 and input.primary.len != 0 or input.secondary_present == 0 and input.secondary.len != 0 or input.tertiary_present == 0 and input.tertiary.len != 0 or input.quaternary_present == 0 and input.quaternary.len != 0 or input.quinary_present == 0 and input.quinary.len != 0 or input.senary_present == 0 and input.senary.len != 0 or input.septenary_present == 0 and input.septenary.len != 0 or input.octonary_present == 0 and input.octonary.len != 0:
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_REQUEST and (
        input.tertiary_present == 0 or input.senary_present == 0
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_BODY and (
        input.primary_present == 0 or input.secondary_present == 0 or input.tertiary_present == 0 or input.kind < 0 or input.kind > 1
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_GENERATION_CONFIG and (
        input.primary_present == 0 or input.secondary_present == 0 or input.tertiary_present == 0 or input.kind < 0 or input.kind > 1
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_NATIVE_PROJECT and (
        input.primary_present == 0 or input.secondary_present == 0
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_WITHOUT_TOOL and (
        input.primary_present == 0 or input.secondary_present == 0
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_SIMPLE and input.primary_present == 0:
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_VALIDATE_CANDIDATE_COUNT and input.primary_present == 0:
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_TOOL_CONFIG and input.primary_present == 0:
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_TEXT_CONTENTS and input.primary_present == 0:
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_RAW_TRANSLATOR and (
        input.primary_present == 0
        or input.tertiary_present == 0
        or input.senary_present == 0
    ):
        return False
    if input.operation == GEMINI_BRIDGE_REQUEST_VALIDATE_TRANSLATOR and input.primary_present == 0:
        return False
    return True


def gemini_bridge_request_value_bounds(
    view: GeminiRequestContentStringView,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    if not gemini_request_content_view_valid(view) or view.len == 0:
        return result^
    var end = Int64(view.len)
    var start = gemini_request_content_skip_ws(view, 0, end)
    var value_end = gemini_request_content_value_end(view, start, end, 0)
    if value_end < 0 or gemini_request_content_skip_ws(view, value_end, end) != end:
        return result^
    result[0] = start
    result[1] = value_end
    return result^


def gemini_bridge_request_object_member(
    view: GeminiRequestContentStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> Array[Int64, 2]:
    return gemini_request_content_object_member(view, object_start, object_end, key)


def gemini_bridge_request_is_object(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    return start >= 0 and end > start and gemini_request_content_byte(view, start) == 123 and gemini_request_content_byte(view, end - 1) == 125


def gemini_bridge_request_is_array(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    return start >= 0 and end > start and gemini_request_content_byte(view, start) == 91 and gemini_request_content_byte(view, end - 1) == 93


def gemini_bridge_request_literal_equals(
    view: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end < start or end - start != Int64(literal.byte_length()):
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(end - start):
        if gemini_request_content_byte(view, start + offset) != expected[unsafe_offset=offset]:
            return False
    return True


def gemini_bridge_request_is_null(
    view: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    return gemini_bridge_request_literal_equals(view, start, end, StringSlice("null"))


def gemini_bridge_request_put_field_prefix(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
    key: StringSlice,
) -> Bool:
    if not first[] and not gemini_request_content_put_byte(writer, 44):
        return False
    first[] = False
    return gemini_request_content_put_literal(writer, key)


def gemini_bridge_request_put_raw_field(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
    key: StringSlice,
    value: GeminiRequestContentStringView,
) -> Bool:
    return gemini_bridge_request_put_field_prefix(writer, first, key) and gemini_request_content_put_range(writer, value, 0, Int64(value.len))


def gemini_bridge_request_put_literal_field(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
    value: StringSlice,
) -> Bool:
    if not first[] and not gemini_request_content_put_byte(writer, 44):
        return False
    first[] = False
    return gemini_request_content_put_literal(writer, value)


def gemini_bridge_request_write_optional_original_fields(
    original: GeminiRequestContentStringView,
    original_bounds: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if not gemini_bridge_request_is_object(original, original_bounds[0], original_bounds[1]):
        return True
    var safety = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("safety_settings")
    )
    if safety[0] < 0:
        safety = gemini_bridge_request_object_member(
            original, original_bounds[0], original_bounds[1], StringSlice("safetySettings")
        )
    if safety[0] >= 0:
        var safety_view = GeminiRequestContentStringView(
            original.ptr + UInt64(safety[0]), UInt64(safety[1] - safety[0])
        )
        if not gemini_bridge_request_put_raw_field(
            writer, first, StringSlice("\"safetySettings\":"), safety_view
        ):
            return False
    var cached = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("cached_content")
    )
    if cached[0] < 0:
        cached = gemini_bridge_request_object_member(
            original, original_bounds[0], original_bounds[1], StringSlice("cachedContent")
        )
    if cached[0] >= 0 and not gemini_bridge_request_is_null(original, cached[0], cached[1]):
        var cached_view = GeminiRequestContentStringView(
            original.ptr + UInt64(cached[0]), UInt64(cached[1] - cached[0])
        )
        if not gemini_bridge_request_put_raw_field(
            writer, first, StringSlice("\"cachedContent\":"), cached_view
        ):
            return False
    var labels = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("labels")
    )
    if labels[0] >= 0 and not gemini_bridge_request_is_null(original, labels[0], labels[1]):
        var labels_view = GeminiRequestContentStringView(
            original.ptr + UInt64(labels[0]), UInt64(labels[1] - labels[0])
        )
        if not gemini_bridge_request_put_raw_field(
            writer, first, StringSlice("\"labels\":"), labels_view
        ):
            return False
    return True


def gemini_bridge_request_write_content_request(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    var original = input.primary.copy()
    var original_bounds = gemini_bridge_request_value_bounds(original)
    if not gemini_request_content_fragment_valid(original):
        return False
    if not gemini_request_content_fragment_valid(input.tertiary) or not gemini_request_content_fragment_valid(input.senary):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if not gemini_request_content_put_byte(writer, 123):
        return False
    if input.secondary_present == 1 and not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"systemInstruction\":"), input.secondary
    ):
        return False
    if not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"contents\":"), input.tertiary
    ):
        return False
    if input.quaternary_present == 1 and not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"tools\":"), input.quaternary
    ):
        return False
    if input.quinary_present == 1 and not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"toolConfig\":"), input.quinary
    ):
        return False
    if not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"generationConfig\":"), input.senary
    ):
        return False
    if not gemini_bridge_request_write_optional_original_fields(
        original, original_bounds, writer, first_ptr
    ):
        return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_content_body(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary) or not gemini_request_content_fragment_valid(input.secondary) or not gemini_request_content_fragment_valid(input.tertiary):
        return False
    if input.kind == 0:
        return gemini_request_content_put_range(writer, input.tertiary, 0, Int64(input.tertiary.len))
    return gemini_request_content_put_literal(writer, StringSlice("{\"model\":")) and gemini_request_content_put_range(writer, input.primary, 0, Int64(input.primary.len)) and gemini_request_content_put_literal(writer, StringSlice(",\"project\":")) and gemini_request_content_put_range(writer, input.secondary, 0, Int64(input.secondary.len)) and gemini_request_content_put_literal(writer, StringSlice(",\"request\":")) and gemini_request_content_put_range(writer, input.tertiary, 0, Int64(input.tertiary.len)) and gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_source_bounds(
    source: GeminiRequestContentStringView,
    object_bounds: Array[Int64, 2],
    index: Int64,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    if index < 0 or not gemini_bridge_request_is_object(
        source, object_bounds[0], object_bounds[1]
    ):
        return result^
    if index == 0:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("temperature"))
    if index == 1:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("top_p"))
    if index == 2:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("max_tokens"))
    if index == 3:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("stop"))
    if index == 4:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("stop_sequences"))
    if index == 5:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("stopSequences"))
    if index == 6:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("top_k"))
    if index == 7:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("topK"))
    if index == 8:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("seed"))
    if index == 9:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("presence_penalty"))
    if index == 10:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("presencePenalty"))
    if index == 11:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("frequency_penalty"))
    if index == 12:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("frequencyPenalty"))
    if index == 13:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("response_mime_type"))
    if index == 14:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("responseMimeType"))
    if index == 15:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("response_schema"))
    if index == 16:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("responseSchema"))
    if index == 17:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("response_json_schema"))
    if index == 18:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("responseJsonSchema"))
    if index == 19:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("response_modalities"))
    if index == 20:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("responseModalities"))
    if index == 21:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("media_resolution"))
    if index == 22:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("mediaResolution"))
    if index == 23:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("audio_timestamp"))
    if index == 24:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("audioTimestamp"))
    if index == 25:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("speech_config"))
    if index == 26:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("speechConfig"))
    if index == 27:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("candidateCount"))
    if index == 28:
        return gemini_bridge_request_object_member(source, object_bounds[0], object_bounds[1], StringSlice("candidate_count"))
    return result^


def gemini_bridge_request_value_view(
    source: GeminiRequestContentStringView, bounds: Array[Int64, 2]
) -> GeminiRequestContentStringView:
    return GeminiRequestContentStringView(
        source.ptr + UInt64(bounds[0]), UInt64(bounds[1] - bounds[0])
    )


def gemini_bridge_request_put_source_pair(
    source: GeminiRequestContentStringView,
    object_bounds: Array[Int64, 2],
    first_source: Int64,
    second_source: Int64,
    target: StringSlice,
    omit_null: Bool,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    var bounds = gemini_bridge_request_source_bounds(source, object_bounds, first_source)
    if bounds[0] < 0:
        bounds = gemini_bridge_request_source_bounds(source, object_bounds, second_source)
    if bounds[0] < 0 or omit_null and gemini_bridge_request_is_null(source, bounds[0], bounds[1]):
        return True
    return gemini_bridge_request_put_raw_field(
        writer, first, target, gemini_bridge_request_value_view(source, bounds)
    )


def gemini_bridge_request_put_source_triple(
    source: GeminiRequestContentStringView,
    object_bounds: Array[Int64, 2],
    first_source: Int64,
    second_source: Int64,
    third_source: Int64,
    target: StringSlice,
    omit_null: Bool,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    var bounds = gemini_bridge_request_source_bounds(source, object_bounds, first_source)
    if bounds[0] < 0:
        bounds = gemini_bridge_request_source_bounds(source, object_bounds, second_source)
    if bounds[0] < 0:
        bounds = gemini_bridge_request_source_bounds(source, object_bounds, third_source)
    if bounds[0] < 0 or omit_null and gemini_bridge_request_is_null(source, bounds[0], bounds[1]):
        return True
    return gemini_bridge_request_put_raw_field(
        writer, first, target, gemini_bridge_request_value_view(source, bounds)
    )


def gemini_bridge_request_ascii_fold(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def gemini_bridge_request_string_equals(
    view: GeminiRequestContentStringView, literal: StringSlice
) -> Bool:
    return gemini_request_content_string_equals(
        view, 0, Int64(view.len), literal, True
    )


def gemini_bridge_request_string_contains(
    view: GeminiRequestContentStringView, literal: StringSlice
) -> Bool:
    if view.len < 2 or gemini_request_content_byte(view, 0) != 34 or gemini_request_content_byte(view, Int64(view.len) - 1) != 34:
        return False
    var expected = literal.unsafe_ptr()
    var expected_length = Int64(literal.byte_length())
    if expected_length == 0:
        return True
    var actual_length = Int64(view.len) - 2
    if expected_length > actual_length:
        return False
    var start: Int64 = 0
    while start <= actual_length - expected_length:
        var matched = True
        for offset in range(expected_length):
            var left = gemini_bridge_request_ascii_fold(
                gemini_request_content_byte(view, 1 + start + offset)
            )
            var right = gemini_bridge_request_ascii_fold(expected[unsafe_offset=offset])
            if left != right:
                matched = False
                break
        if matched:
            return True
        start += 1
    return False


def gemini_bridge_request_text_format_kind(
    original: GeminiRequestContentStringView,
    original_bounds: Array[Int64, 2],
) -> Int64:
    if not gemini_bridge_request_is_object(original, original_bounds[0], original_bounds[1]):
        return 0
    var text = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("text")
    )
    if not gemini_bridge_request_is_object(original, text[0], text[1]):
        return 0
    var format = gemini_bridge_request_object_member(
        original, text[0], text[1], StringSlice("format")
    )
    if not gemini_bridge_request_is_object(original, format[0], format[1]):
        return 0
    var kind = gemini_bridge_request_object_member(
        original, format[0], format[1], StringSlice("type")
    )
    if kind[0] < 0 or gemini_request_content_byte(original, kind[0]) != 34:
        return 0
    if gemini_request_content_string_equals(
        original, kind[0], kind[1], StringSlice("json_object"), False
    ):
        return 1
    if gemini_request_content_string_equals(
        original, kind[0], kind[1], StringSlice("json_schema"), False
    ):
        return 2
    return 0


def gemini_bridge_request_write_text_format(
    original: GeminiRequestContentStringView,
    original_bounds: Array[Int64, 2],
    format_kind: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if format_kind == 1:
        return gemini_bridge_request_put_literal_field(
            writer, first, StringSlice("\"responseMimeType\":\"application/json\"")
        )
    if format_kind != 2:
        return True
    if not gemini_bridge_request_put_literal_field(
        writer, first, StringSlice("\"responseMimeType\":\"application/json\"")
    ):
        return False
    var text = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("text")
    )
    var format = gemini_bridge_request_object_member(
        original, text[0], text[1], StringSlice("format")
    )
    var schema = gemini_bridge_request_object_member(
        original, format[0], format[1], StringSlice("schema")
    )
    if schema[0] < 0:
        schema = gemini_bridge_request_object_member(
            original, format[0], format[1], StringSlice("json_schema")
        )
    if schema[0] >= 0:
        return gemini_bridge_request_put_raw_field(
            writer,
            first,
            StringSlice("\"responseJsonSchema\":"),
            gemini_bridge_request_value_view(original, schema),
        )
    return True


def gemini_bridge_request_reasoning_effort(
    original: GeminiRequestContentStringView,
    original_bounds: Array[Int64, 2],
) -> GeminiRequestContentStringView:
    var empty = GeminiRequestContentStringView(0, 0)
    if not gemini_bridge_request_is_object(original, original_bounds[0], original_bounds[1]):
        return empty^
    var reasoning = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("reasoning")
    )
    if not gemini_bridge_request_is_object(original, reasoning[0], reasoning[1]):
        return empty^
    var effort = gemini_bridge_request_object_member(
        original, reasoning[0], reasoning[1], StringSlice("effort")
    )
    if effort[0] < 0 or gemini_request_content_byte(original, effort[0]) != 34:
        return empty^
    return gemini_bridge_request_value_view(original, effort).copy()


def gemini_bridge_request_write_thinking_config(
    original: GeminiRequestContentStringView,
    original_bounds: Array[Int64, 2],
    model: GeminiRequestContentStringView,
    budget: GeminiRequestContentStringView,
    budget_present: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    var effort = gemini_bridge_request_reasoning_effort(original, original_bounds)
    var disabled = gemini_bridge_request_string_equals(effort, StringSlice("none")) or gemini_bridge_request_string_equals(effort, StringSlice("minimal"))
    if disabled:
        return gemini_bridge_request_put_literal_field(
            writer, first, StringSlice("\"thinkingConfig\":{\"includeThoughts\":false,\"thinkingBudget\":0}")
        )
    var thinking_level = gemini_bridge_request_string_contains(model, StringSlice("gemini-3")) or gemini_bridge_request_string_contains(model, StringSlice("gemma-3")) or gemini_bridge_request_string_contains(model, StringSlice("gemma-4"))
    if thinking_level:
        var level = StringSlice("HIGH")
        if gemini_bridge_request_string_equals(effort, StringSlice("low")):
            level = StringSlice("LOW")
        elif gemini_bridge_request_string_equals(effort, StringSlice("medium")):
            level = StringSlice("MEDIUM")
        return gemini_bridge_request_put_literal_field(
            writer,
            first,
            StringSlice("\"thinkingConfig\":{\"includeThoughts\":true,\"thinkingLevel\":\""),
        ) and gemini_request_content_put_literal(writer, level) and gemini_request_content_put_literal(writer, StringSlice("\"}"))
    if budget_present == 1:
        if not gemini_request_content_fragment_valid(budget):
            return False
        if not gemini_bridge_request_put_field_prefix(
            writer,
            first,
            StringSlice("\"thinkingConfig\":{\"includeThoughts\":true,\"thinkingBudget\":"),
        ):
            return False
        return gemini_request_content_put_range(writer, budget, 0, Int64(budget.len)) and gemini_request_content_put_byte(writer, 125)
    var value = StringSlice("\"thinkingConfig\":{\"includeThoughts\":true,\"thinkingBudget\":8192}")
    if gemini_bridge_request_string_equals(effort, StringSlice("low")):
        value = StringSlice("\"thinkingConfig\":{\"includeThoughts\":true,\"thinkingBudget\":1024}")
    elif gemini_bridge_request_string_equals(effort, StringSlice("xhigh")):
        value = StringSlice("\"thinkingConfig\":{\"includeThoughts\":true,\"thinkingBudget\":24576}")
    return gemini_bridge_request_put_literal_field(
        writer, first, value,
    )


def gemini_bridge_request_write_generation_config(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary) or not gemini_request_content_fragment_valid(input.secondary) or not gemini_request_content_fragment_valid(input.tertiary):
        return False
    var original = input.primary.copy()
    var chat = input.secondary.copy()
    var original_bounds = gemini_bridge_request_value_bounds(original)
    var chat_bounds = gemini_bridge_request_value_bounds(chat)
    var model = input.tertiary.copy()
    if not gemini_request_content_fragment_valid(model) or gemini_request_content_byte(model, 0) != 34:
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if not gemini_request_content_put_byte(writer, 123):
        return False
    if gemini_bridge_request_is_object(chat, chat_bounds[0], chat_bounds[1]):
        if not gemini_bridge_request_put_source_pair(
            chat, chat_bounds, 0, -1, StringSlice("\"temperature\":"), False, writer, first_ptr
        ) or not gemini_bridge_request_put_source_pair(
            chat, chat_bounds, 1, -1, StringSlice("\"topP\":"), False, writer, first_ptr
        ) or not gemini_bridge_request_put_source_pair(
            chat, chat_bounds, 2, -1, StringSlice("\"maxOutputTokens\":"), False, writer, first_ptr
        ):
            return False
    if not gemini_bridge_request_is_object(original, original_bounds[0], original_bounds[1]):
        return gemini_request_content_put_byte(writer, 125)

    var format_kind = gemini_bridge_request_text_format_kind(original, original_bounds)
    if not gemini_bridge_request_put_source_pair(
        original, original_bounds, 7, 6, StringSlice("\"topK\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 8, -1, StringSlice("\"seed\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 10, 9, StringSlice("\"presencePenalty\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 12, 11, StringSlice("\"frequencyPenalty\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 14, 13, StringSlice("\"responseMimeType\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 16, 15, StringSlice("\"responseSchema\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 18, 17, StringSlice("\"responseJsonSchema\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 20, 19, StringSlice("\"responseModalities\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 22, 21, StringSlice("\"mediaResolution\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 24, 23, StringSlice("\"audioTimestamp\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 26, 25, StringSlice("\"speechConfig\":"), True, writer, first_ptr
    ) or not gemini_bridge_request_put_source_pair(
        original, original_bounds, 27, 28, StringSlice("\"candidateCount\":"), True, writer, first_ptr
    ):
        return False
    var stop = gemini_bridge_request_object_member(
        original, original_bounds[0], original_bounds[1], StringSlice("stop")
    )
    if stop[0] < 0:
        stop = gemini_bridge_request_object_member(
            original, original_bounds[0], original_bounds[1], StringSlice("stop_sequences")
        )
    if stop[0] < 0:
        stop = gemini_bridge_request_object_member(
            original, original_bounds[0], original_bounds[1], StringSlice("stopSequences")
        )
    if stop[0] >= 0 and not gemini_bridge_request_is_null(original, stop[0], stop[1]) and not gemini_bridge_request_put_raw_field(
        writer, first_ptr, StringSlice("\"stopSequences\":"), gemini_bridge_request_value_view(original, stop)
    ):
        return False
    if not gemini_bridge_request_write_text_format(
        original, original_bounds, format_kind, writer, first_ptr
    ):
        return False
    if not gemini_bridge_request_write_thinking_config(
        original,
        original_bounds,
        model,
        input.quaternary,
        input.quaternary_present,
        writer,
        first_ptr,
    ):
        return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_native_metadata(
    source: GeminiRequestContentStringView,
    object_start: Int64,
    object_end: Int64,
    project: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_bridge_request_is_object(source, object_start, object_end):
        return gemini_request_content_put_range(writer, source, object_start, object_end)
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(source, object_start + 1, object_end - 1)
    while index < object_end - 1 and gemini_request_content_byte(source, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(source, key_start, object_end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(source, key_end, object_end - 1)
        if index >= object_end - 1 or gemini_request_content_byte(source, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(source, index + 1, object_end - 1)
        var value_end = gemini_request_content_value_end(source, value_start, object_end - 1, 0)
        if value_end < 0:
            return False
        if not first and not gemini_request_content_put_byte(writer, 44):
            return False
        first = False
        if not gemini_request_content_put_range(writer, source, key_start, key_end) or not gemini_request_content_put_byte(writer, 58):
            return False
        if gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("duetProject")):
            if not gemini_request_content_put_range(writer, project, 0, Int64(project.len)):
                return False
        else:
            if not gemini_request_content_put_range(writer, source, value_start, value_end):
                return False
        index = gemini_request_content_skip_ws(source, value_end, object_end - 1)
        if index < object_end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, object_end - 1)
        elif index != object_end - 1:
            return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_native_value(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    project: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    depth: Int64,
) -> Bool:
    if depth > GEMINI_REQUEST_CONTENT_MAX_DEPTH or start < 0 or end <= start:
        return False
    if not gemini_bridge_request_is_object(source, start, end):
        return gemini_request_content_put_range(writer, source, start, end)
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(source, key_start, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(source, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(source, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(source, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(source, value_start, end - 1, depth + 1)
        if value_end < 0:
            return False
        if not first and not gemini_request_content_put_byte(writer, 44):
            return False
        first = False
        if not gemini_request_content_put_range(writer, source, key_start, key_end) or not gemini_request_content_put_byte(writer, 58):
            return False
        if gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("project")) or gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("projectId")) or gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("cloudaicompanionProject")):
            if not gemini_request_content_put_range(writer, project, 0, Int64(project.len)):
                return False
        elif gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("metadata")) and gemini_bridge_request_is_object(source, value_start, value_end):
            if not gemini_bridge_request_write_native_metadata(source, value_start, value_end, project, writer):
                return False
        elif gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("request")) and gemini_bridge_request_is_object(source, value_start, value_end):
            if not gemini_bridge_request_write_native_value(source, value_start, value_end, project, writer, depth + 1):
                return False
        elif not gemini_request_content_put_range(writer, source, value_start, value_end):
            return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_native_project(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary) or not gemini_request_content_fragment_valid(input.secondary):
        return gemini_request_content_put_range(writer, input.primary, 0, Int64(input.primary.len))
    var bounds = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, bounds[0], bounds[1]):
        return gemini_request_content_put_range(writer, input.primary, bounds[0], bounds[1])
    return gemini_bridge_request_write_native_value(
        input.primary, bounds[0], bounds[1], input.secondary, writer, 0
    )


def gemini_bridge_request_tool_key_matches(
    source: GeminiRequestContentStringView,
    key_start: Int64,
    key_end: Int64,
    name: GeminiRequestContentStringView,
) -> Bool:
    if key_end < key_start + 2 or gemini_request_content_byte(source, key_start) != 34 or gemini_request_content_byte(source, key_end - 1) != 34:
        return False
    if key_end - key_start - 2 != Int64(name.len):
        return False
    for offset in range(Int64(name.len)):
        if gemini_request_content_byte(source, key_start + 1 + offset) != gemini_request_content_byte(name, offset):
            return False
    return True


def gemini_bridge_request_scan_tool_array(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    name: GeminiRequestContentStringView,
    removed: Pointer[mut=True, Int64, _],
    kept: Pointer[mut=True, Int64, _],
) -> Bool:
    if not gemini_bridge_request_is_array(source, start, end):
        return False
    removed[] = 0
    kept[] = 0
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(source, index, end - 1, 0)
        if value_end < 0:
            return False
        var drop = False
        if gemini_bridge_request_is_object(source, index, value_end):
            var object_end = value_end
            var key_probe = gemini_request_content_skip_ws(source, index + 1, object_end - 1)
            while key_probe < object_end - 1 and gemini_request_content_byte(source, key_probe) != 125:
                var key_start = key_probe
                var key_end = gemini_request_content_string_end(source, key_start, object_end - 1)
                if key_end < 0:
                    return False
                key_probe = gemini_request_content_skip_ws(source, key_end, object_end - 1)
                if key_probe >= object_end - 1 or gemini_request_content_byte(source, key_probe) != 58:
                    return False
                var member_start = gemini_request_content_skip_ws(source, key_probe + 1, object_end - 1)
                var member_end = gemini_request_content_value_end(source, member_start, object_end - 1, 0)
                if member_end < 0:
                    return False
                if gemini_bridge_request_tool_key_matches(source, key_start, key_end, name):
                    drop = True
                key_probe = gemini_request_content_skip_ws(source, member_end, object_end - 1)
                if key_probe < object_end - 1 and gemini_request_content_byte(source, key_probe) == 44:
                    key_probe = gemini_request_content_skip_ws(source, key_probe + 1, object_end - 1)
                elif key_probe != object_end - 1:
                    return False
        if drop:
            removed[] = removed[] + 1
        else:
            kept[] = kept[] + 1
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return True


def gemini_bridge_request_write_filtered_tools(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    name: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_put_byte(writer, 91):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(source, index, end - 1, 0)
        if value_end < 0:
            return False
        var drop = False
        if gemini_bridge_request_is_object(source, index, value_end):
            var probe = gemini_request_content_skip_ws(source, index + 1, value_end - 1)
            while probe < value_end - 1 and gemini_request_content_byte(source, probe) != 125:
                var key_start = probe
                var key_end = gemini_request_content_string_end(source, key_start, value_end - 1)
                if key_end < 0:
                    return False
                probe = gemini_request_content_skip_ws(source, key_end, value_end - 1)
                if probe >= value_end - 1 or gemini_request_content_byte(source, probe) != 58:
                    return False
                var member_start = gemini_request_content_skip_ws(source, probe + 1, value_end - 1)
                var member_end = gemini_request_content_value_end(source, member_start, value_end - 1, 0)
                if member_end < 0:
                    return False
                if gemini_bridge_request_tool_key_matches(source, key_start, key_end, name):
                    drop = True
                probe = gemini_request_content_skip_ws(source, member_end, value_end - 1)
                if probe < value_end - 1 and gemini_request_content_byte(source, probe) == 44:
                    probe = gemini_request_content_skip_ws(source, probe + 1, value_end - 1)
                elif probe != value_end - 1:
                    return False
        if not drop:
            if not first and not gemini_request_content_put_byte(writer, 44):
                return False
            first = False
            if not gemini_request_content_put_range(writer, source, index, value_end):
                return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return gemini_request_content_put_byte(writer, 93)


def gemini_bridge_request_write_filtered_object(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    name: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    changed: Pointer[mut=True, Int64, _],
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(source, key_start, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(source, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(source, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(source, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(source, value_start, end - 1, 0)
        if value_end < 0:
            return False
        var is_tools = gemini_request_content_raw_equals(
            source, key_start, key_end, StringSlice("tools")
        )
        var removed: Int64 = 0
        var kept: Int64 = 0
        var removed_ptr = Pointer(to=removed)
        var kept_ptr = Pointer(to=kept)
        if is_tools and not gemini_bridge_request_scan_tool_array(
            source, value_start, value_end, name, removed_ptr, kept_ptr
        ):
            return False
        if is_tools and removed > 0:
            changed[] = 1
            if kept == 0:
                index = gemini_request_content_skip_ws(source, value_end, end - 1)
                if index < end - 1 and gemini_request_content_byte(source, index) == 44:
                    index = gemini_request_content_skip_ws(source, index + 1, end - 1)
                elif index != end - 1:
                    return False
                continue
            if not first and not gemini_request_content_put_byte(writer, 44):
                return False
            first = False
            if not gemini_request_content_put_range(writer, source, key_start, key_end) or not gemini_request_content_put_byte(writer, 58) or not gemini_bridge_request_write_filtered_tools(source, value_start, value_end, name, writer):
                return False
        else:
            if not first and not gemini_request_content_put_byte(writer, 44):
                return False
            first = False
            if not gemini_request_content_put_range(writer, source, key_start, key_end) or not gemini_request_content_put_byte(writer, 58) or not gemini_request_content_put_range(writer, source, value_start, value_end):
                return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_root_without_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    name: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    changed: Pointer[mut=True, Int64, _],
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    var request = gemini_bridge_request_object_member(
        source, start, end, StringSlice("request")
    )
    if request[0] < 0:
        return gemini_bridge_request_write_filtered_object(
            source, start, end, name, writer, changed
        )
    if not gemini_bridge_request_is_object(source, request[0], request[1]):
        return False
    if not gemini_request_content_put_byte(writer, 123):
        return False
    var first = True
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 125:
        var key_start = index
        var key_end = gemini_request_content_string_end(source, key_start, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(source, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(source, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(source, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(source, value_start, end - 1, 0)
        if value_end < 0:
            return False
        if not first and not gemini_request_content_put_byte(writer, 44):
            return False
        first = False
        if not gemini_request_content_put_range(writer, source, key_start, key_end) or not gemini_request_content_put_byte(writer, 58):
            return False
        if gemini_request_content_raw_equals(source, key_start, key_end, StringSlice("request")):
            if not gemini_bridge_request_write_filtered_object(
                source, value_start, value_end, name, writer, changed
            ):
                return False
        elif not gemini_request_content_put_range(writer, source, value_start, value_end):
            return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return gemini_request_content_put_byte(writer, 125)


def gemini_bridge_request_write_without_tool(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary) or input.secondary.len == 0:
        return False
    var bounds = gemini_bridge_request_value_bounds(input.primary)
    var changed: Int64 = 0
    var changed_ptr = Pointer(to=changed)
    if not gemini_bridge_request_write_root_without_tool(
        input.primary, bounds[0], bounds[1], input.secondary, writer, changed_ptr
    ):
        return False
    if changed == 0:
        writer[].written = 0
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    return True


def gemini_bridge_request_raw_values_equal(
    source: GeminiRequestContentStringView,
    left: Array[Int64, 2],
    right: Array[Int64, 2],
) -> Bool:
    if left[0] < 0 or right[0] < 0 or left[1] - left[0] != right[1] - right[0]:
        return False
    for offset in range(left[1] - left[0]):
        if gemini_request_content_byte(source, left[0] + offset) != gemini_request_content_byte(source, right[0] + offset):
            return False
    return True


def gemini_bridge_request_write_candidate_validation(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary):
        return False
    var bounds = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, bounds[0], bounds[1]):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    var snake = gemini_bridge_request_object_member(
        input.primary, bounds[0], bounds[1], StringSlice("candidate_count")
    )
    var camel = gemini_bridge_request_object_member(
        input.primary, bounds[0], bounds[1], StringSlice("candidateCount")
    )
    var snake_active = snake[0] >= 0 and not gemini_bridge_request_is_null(input.primary, snake[0], snake[1])
    var camel_active = camel[0] >= 0 and not gemini_bridge_request_is_null(input.primary, camel[0], camel[1])
    if snake_active and camel_active:
        if not gemini_bridge_request_raw_values_equal(input.primary, snake, camel):
            return gemini_request_content_put_literal(writer, StringSlice("\"invalid_candidate_count: Gemini request fields `candidate_count` and `candidateCount` conflict\""))
    if snake_active and not gemini_bridge_request_literal_equals(input.primary, snake[0], snake[1], StringSlice("1")):
        return gemini_request_content_put_literal(writer, StringSlice("\"invalid_candidate_count: Gemini request field `candidate_count` must be omitted, null, or 1\""))
    if camel_active and not gemini_bridge_request_literal_equals(input.primary, camel[0], camel[1], StringSlice("1")):
        return gemini_request_content_put_literal(writer, StringSlice("\"invalid_candidate_count: Gemini request field `candidateCount` must be omitted, null, or 1\""))
    return gemini_request_content_put_literal(writer, StringSlice("null"))


def gemini_bridge_request_write_tool_config(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary):
        return False
    var bounds = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, bounds[0], bounds[1]):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    var choice = gemini_bridge_request_object_member(
        input.primary, bounds[0], bounds[1], StringSlice("tool_choice")
    )
    if choice[0] < 0:
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    if gemini_request_content_string_equals(
        input.primary, choice[0], choice[1], StringSlice("auto"), False
    ):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    if gemini_request_content_string_equals(
        input.primary, choice[0], choice[1], StringSlice("none"), False
    ):
        return gemini_request_content_put_literal(
            writer, StringSlice('{"functionCallingConfig":{"mode":"NONE"}}')
        )
    if gemini_request_content_string_equals(
        input.primary, choice[0], choice[1], StringSlice("required"), False
    ):
        return gemini_request_content_put_literal(
            writer, StringSlice('{"functionCallingConfig":{"mode":"ANY"}}')
        )
    if not gemini_bridge_request_is_object(input.primary, choice[0], choice[1]):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    var function = gemini_bridge_request_object_member(
        input.primary, choice[0], choice[1], StringSlice("function")
    )
    var name = gemini_bridge_request_object_member(
        input.primary, choice[0], choice[1], StringSlice("name")
    )
    if gemini_bridge_request_is_object(input.primary, function[0], function[1]):
        var function_name = gemini_bridge_request_object_member(
            input.primary, function[0], function[1], StringSlice("name")
        )
        if (
            function_name[0] >= 0
            and gemini_request_content_byte(input.primary, function_name[0]) == 34
        ):
            name = function_name.copy()
    if name[0] < 0 or gemini_request_content_byte(input.primary, name[0]) != 34:
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    return (
        gemini_request_content_put_literal(writer, StringSlice('{"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":['))
        and gemini_request_content_put_range(writer, input.primary, name[0], name[1])
        and gemini_request_content_put_literal(writer, StringSlice("]}}"))
    )


def gemini_bridge_request_string_starts_with(
    view: GeminiRequestContentStringView, literal: StringSlice
) -> Bool:
    if view.len < UInt64(literal.byte_length()) + 2 or gemini_request_content_byte(view, 0) != 34:
        return False
    var expected = literal.unsafe_ptr()
    for offset in range(Int64(literal.byte_length())):
        if gemini_request_content_byte(view, 1 + offset) != expected[unsafe_offset=offset]:
            return False
    return True


def gemini_bridge_request_builtin_tool(
    source: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    var type = gemini_bridge_request_object_member(
        source, start, end, StringSlice("type")
    )
    if type[0] >= 0 and gemini_request_content_byte(source, type[0]) == 34:
        var value = gemini_bridge_request_value_view(source, type)
        if gemini_bridge_request_string_equals(value, StringSlice("web_search")) or gemini_bridge_request_string_equals(value, StringSlice("web_search_preview")) or gemini_bridge_request_string_starts_with(value, StringSlice("web_search_preview_")) or gemini_bridge_request_string_equals(value, StringSlice("code_interpreter")) or gemini_bridge_request_string_equals(value, StringSlice("code_execution")) or gemini_bridge_request_string_equals(value, StringSlice("codeExecution")) or gemini_bridge_request_string_equals(value, StringSlice("computer")) or gemini_bridge_request_string_equals(value, StringSlice("computer_use")) or gemini_bridge_request_string_equals(value, StringSlice("computerUse")) or gemini_bridge_request_string_equals(value, StringSlice("computer_use_preview")) or gemini_bridge_request_string_starts_with(value, StringSlice("computer_")) or gemini_bridge_request_string_equals(value, StringSlice("web_fetch")) or gemini_bridge_request_string_equals(value, StringSlice("url_context")) or gemini_bridge_request_string_equals(value, StringSlice("urlContext")) or gemini_bridge_request_string_equals(value, StringSlice("web_fetch_preview")) or gemini_bridge_request_string_starts_with(value, StringSlice("web_fetch_preview_")):
            return True
    return gemini_bridge_request_object_member(source, start, end, StringSlice("computerUse"))[0] >= 0 or gemini_bridge_request_object_member(source, start, end, StringSlice("codeExecution"))[0] >= 0 or gemini_bridge_request_object_member(source, start, end, StringSlice("urlContext"))[0] >= 0


def gemini_bridge_request_builtin_tool_choice(
    source: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if gemini_bridge_request_is_null(source, start, end) or gemini_request_content_byte(source, start) != 34:
        return True
    return gemini_request_content_string_equals(source, start, end, StringSlice("auto"), False) or gemini_request_content_string_equals(source, start, end, StringSlice("none"), False)


def gemini_bridge_request_write_decoded_byte(
    output: Pointer[mut=True, UInt8, _],
    position: Pointer[mut=True, Int64, _],
    capacity: Int64,
    value: UInt8,
) -> Bool:
    if position[] < 0 or position[] >= capacity:
        return False
    output[unsafe_offset=position[]] = value
    position[] += 1
    return True


def gemini_bridge_request_write_codepoint(
    output: Pointer[mut=True, UInt8, _],
    position: Pointer[mut=True, Int64, _],
    capacity: Int64,
    codepoint: Int64,
) -> Bool:
    if codepoint < 0 or codepoint > 1_114_111 or codepoint >= 55_296 and codepoint <= 57_343:
        return False
    if codepoint <= 127:
        return gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(codepoint))
    if codepoint <= 2_047:
        return gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(192 | (codepoint >> 6))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | (codepoint & 63)))
    if codepoint <= 65_535:
        return gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(224 | (codepoint >> 12))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | ((codepoint >> 6) & 63))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | (codepoint & 63)))
    return gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(240 | (codepoint >> 18))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | ((codepoint >> 12) & 63))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | ((codepoint >> 6) & 63))) and gemini_bridge_request_write_decoded_byte(output, position, capacity, UInt8(128 | (codepoint & 63)))


def gemini_bridge_request_decode_json_string(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
) -> Int64:
    if start < 0 or end <= start + 1 or gemini_request_content_byte(source, start) != 34 or gemini_request_content_byte(source, end - 1) != 34:
        return -1
    var position: Int64 = 0
    var index = start + 1
    while index < end - 1:
        var value = gemini_request_content_byte(source, index)
        if value == 92:
            if index + 1 >= end - 1:
                return -1
            var escaped = gemini_request_content_byte(source, index + 1)
            if escaped == 34 or escaped == 92 or escaped == 47:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, escaped):
                    return -1
                index += 2
            elif escaped == 98:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, 8):
                    return -1
                index += 2
            elif escaped == 102:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, 12):
                    return -1
                index += 2
            elif escaped == 110:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, 10):
                    return -1
                index += 2
            elif escaped == 114:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, 13):
                    return -1
                index += 2
            elif escaped == 116:
                if not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, 9):
                    return -1
                index += 2
            elif escaped == 117:
                if index + 5 >= end:
                    return -1
                var codepoint: Int64 = 0
                for offset in range(4):
                    var digit = gemini_request_content_hex(
                        gemini_request_content_byte(source, index + 2 + Int64(offset))
                    )
                    if digit < 0:
                        return -1
                    codepoint = (codepoint << 4) | digit
                index += 6
                if codepoint >= 55_296 and codepoint <= 56_319:
                    if index + 5 >= end or gemini_request_content_byte(source, index) != 92 or gemini_request_content_byte(source, index + 1) != 117:
                        return -1
                    var low: Int64 = 0
                    for offset in range(4):
                        var digit = gemini_request_content_hex(
                            gemini_request_content_byte(source, index + 2 + Int64(offset))
                        )
                        if digit < 0:
                            return -1
                        low = (low << 4) | digit
                    if low < 56_320 or low > 57_343:
                        return -1
                    codepoint = 65_536 + ((codepoint - 55_296) << 10) + low - 56_320
                    index += 6
                elif codepoint >= 56_320 and codepoint <= 57_343:
                    return -1
                if not gemini_bridge_request_write_codepoint(output, Pointer(to=position), capacity, codepoint):
                    return -1
            else:
                return -1
        else:
            if value < 32 or not gemini_bridge_request_write_decoded_byte(output, Pointer(to=position), capacity, value):
                return -1
            index += 1
    return position


def gemini_bridge_request_simple_content_item(
    source: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    var text = gemini_bridge_request_object_member(source, start, end, StringSlice("text"))
    var content = gemini_bridge_request_object_member(source, start, end, StringSlice("content"))
    var text_string = text[0] >= 0 and gemini_request_content_byte(source, text[0]) == 34
    var content_string = content[0] >= 0 and gemini_request_content_byte(source, content[0]) == 34
    if not text_string and not content_string:
        return False
    var type = gemini_bridge_request_object_member(source, start, end, StringSlice("type"))
    if type[0] >= 0 and not (
        gemini_request_content_string_equals(source, type[0], type[1], StringSlice("input_text"), False) or gemini_request_content_string_equals(source, type[0], type[1], StringSlice("output_text"), False) or gemini_request_content_string_equals(source, type[0], type[1], StringSlice("text"), False)
    ):
        return False
    return True


def gemini_bridge_request_simple_tool_call(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    scratch_address: UInt64,
    scratch: Pointer[mut=True, UInt8, _],
    scratch_capacity: Int64,
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    var id = gemini_bridge_request_object_member(source, start, end, StringSlice("id"))
    if id[0] >= 0 and gemini_request_content_byte(source, id[0]) != 34:
        return False
    var function = gemini_bridge_request_object_member(source, start, end, StringSlice("function"))
    if not gemini_bridge_request_is_object(source, function[0], function[1]):
        return False
    var name = gemini_bridge_request_object_member(source, function[0], function[1], StringSlice("name"))
    var arguments = gemini_bridge_request_object_member(source, function[0], function[1], StringSlice("arguments"))
    if name[0] < 0 or gemini_request_content_byte(source, name[0]) != 34 or arguments[0] < 0 or gemini_request_content_byte(source, arguments[0]) != 34:
        return False
    var decoded = gemini_bridge_request_decode_json_string(
        source, arguments[0], arguments[1], scratch, scratch_capacity
    )
    if decoded < 0:
        return False
    return gemini_request_content_fragment_valid(
        GeminiRequestContentStringView(scratch_address, UInt64(decoded))
    )


def gemini_bridge_request_simple_tool_calls(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    scratch_address: UInt64,
    scratch: Pointer[mut=True, UInt8, _],
    scratch_capacity: Int64,
) -> Bool:
    if not gemini_bridge_request_is_array(source, start, end):
        return False
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(source, index, end - 1, 0)
        if value_end < 0 or not gemini_bridge_request_simple_tool_call(
            source, index, value_end, scratch_address, scratch, scratch_capacity
        ):
            return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return True


def gemini_bridge_request_simple_role(
    source: GeminiRequestContentStringView, start: Int64, end: Int64
) -> Int64:
    var role = gemini_bridge_request_object_member(source, start, end, StringSlice("role"))
    if role[0] < 0 or gemini_request_content_byte(source, role[0]) != 34:
        return 1
    if gemini_request_content_string_equals(source, role[0], role[1], StringSlice("system"), False):
        return 0
    if gemini_request_content_string_equals(source, role[0], role[1], StringSlice("user"), False):
        return 1
    if gemini_request_content_string_equals(source, role[0], role[1], StringSlice("assistant"), False):
        return 2
    if gemini_request_content_string_equals(source, role[0], role[1], StringSlice("tool"), False):
        return 3
    return -1


def gemini_bridge_request_simple_input_item(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    scratch_address: UInt64,
    scratch: Pointer[mut=True, UInt8, _],
    scratch_capacity: Int64,
) -> Bool:
    if not gemini_bridge_request_is_object(source, start, end):
        return False
    var type = gemini_bridge_request_object_member(source, start, end, StringSlice("type"))
    if type[0] >= 0 and not gemini_request_content_string_equals(source, type[0], type[1], StringSlice("message"), False):
        return False
    var role = gemini_bridge_request_simple_role(source, start, end)
    if role < 0:
        return False
    var content = gemini_bridge_request_object_member(source, start, end, StringSlice("content"))
    if content[0] >= 0:
        if gemini_request_content_byte(source, content[0]) != 34 and gemini_bridge_request_is_array(source, content[0], content[1]):
            var index = gemini_request_content_skip_ws(source, content[0] + 1, content[1] - 1)
            while index < content[1] - 1 and gemini_request_content_byte(source, index) != 93:
                var value_end = gemini_request_content_value_end(source, index, content[1] - 1, 0)
                if value_end < 0 or not gemini_bridge_request_simple_content_item(source, index, value_end):
                    return False
                index = gemini_request_content_skip_ws(source, value_end, content[1] - 1)
                if index < content[1] - 1 and gemini_request_content_byte(source, index) == 44:
                    index = gemini_request_content_skip_ws(source, index + 1, content[1] - 1)
                elif index != content[1] - 1:
                    return False
        elif gemini_request_content_byte(source, content[0]) != 34:
            return False
    if gemini_bridge_request_object_member(source, start, end, StringSlice("gemini_native_parts"))[0] >= 0:
        return False
    var tool_calls = gemini_bridge_request_object_member(source, start, end, StringSlice("tool_calls"))
    if role == 2:
        if tool_calls[0] >= 0 and not gemini_bridge_request_simple_tool_calls(
            source, tool_calls[0], tool_calls[1], scratch_address, scratch, scratch_capacity
        ):
            return False
        return True
    if role == 3:
        var tool_call_id = gemini_bridge_request_object_member(source, start, end, StringSlice("tool_call_id"))
        var name = gemini_bridge_request_object_member(source, start, end, StringSlice("name"))
        return (tool_call_id[0] < 0 or gemini_request_content_byte(source, tool_call_id[0]) == 34) and (name[0] < 0 or gemini_request_content_byte(source, name[0]) == 34)
    return tool_calls[0] < 0


def gemini_bridge_request_simple_input_items(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    scratch_address: UInt64,
    scratch: Pointer[mut=True, UInt8, _],
    scratch_capacity: Int64,
) -> Bool:
    if not gemini_bridge_request_is_array(source, start, end):
        return False
    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(source, index, end - 1, 0)
        if value_end < 0 or not gemini_bridge_request_simple_input_item(
            source, index, value_end, scratch_address, scratch, scratch_capacity
        ):
            return False
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
        elif index != end - 1:
            return False
    return True


def gemini_bridge_request_simple(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    scratch_address: UInt64,
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary):
        return False
    var bounds = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, bounds[0], bounds[1]):
        return False
    var tools = gemini_bridge_request_object_member(input.primary, bounds[0], bounds[1], StringSlice("tools"))
    if tools[0] >= 0:
        if not gemini_bridge_request_is_array(input.primary, tools[0], tools[1]):
            return False
        var index = gemini_request_content_skip_ws(input.primary, tools[0] + 1, tools[1] - 1)
        while index < tools[1] - 1 and gemini_request_content_byte(input.primary, index) != 93:
            var value_end = gemini_request_content_value_end(input.primary, index, tools[1] - 1, 0)
            if value_end < 0 or not gemini_bridge_request_builtin_tool(input.primary, index, value_end):
                return False
            index = gemini_request_content_skip_ws(input.primary, value_end, tools[1] - 1)
            if index < tools[1] - 1 and gemini_request_content_byte(input.primary, index) == 44:
                index = gemini_request_content_skip_ws(input.primary, index + 1, tools[1] - 1)
            elif index != tools[1] - 1:
                return False
        var choice = gemini_bridge_request_object_member(input.primary, bounds[0], bounds[1], StringSlice("tool_choice"))
        if choice[0] >= 0 and not gemini_bridge_request_builtin_tool_choice(input.primary, choice[0], choice[1]):
            return False
    var request_input = gemini_bridge_request_object_member(input.primary, bounds[0], bounds[1], StringSlice("input"))
    if request_input[0] < 0:
        return False
    if gemini_request_content_byte(input.primary, request_input[0]) == 34:
        return True
    return gemini_bridge_request_simple_input_items(
        input.primary,
        request_input[0],
        request_input[1],
        scratch_address,
        writer[].output,
        writer[].capacity,
    )



def gemini_bridge_text_raw_string(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    return (
        bounds[0] >= 0
        and bounds[1] > bounds[0]
        and gemini_request_content_byte(source, bounds[0]) == 34
        and gemini_request_content_byte(source, bounds[1] - 1) == 34
    )


def gemini_bridge_text_role_code(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Int64:
    var role = gemini_bridge_request_object_member(
        source, start, end, StringSlice("role")
    )
    if role[0] < 0:
        return 1
    if not gemini_bridge_text_raw_string(source, role):
        return -1
    if gemini_request_content_string_equals(
        source, role[0], role[1], StringSlice("system"), False
    ):
        return 0
    if gemini_request_content_string_equals(
        source, role[0], role[1], StringSlice("user"), False
    ):
        return 1
    if gemini_request_content_string_equals(
        source, role[0], role[1], StringSlice("assistant"), False
    ):
        return 2
    return -1


def gemini_bridge_text_media_like_object(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    for key in [
        StringSlice("path"),
        StringSlice("file_path"),
        StringSlice("filePath"),
        StringSlice("image_url"),
        StringSlice("imageUrl"),
        StringSlice("file_data"),
        StringSlice("fileData"),
        StringSlice("inline_data"),
        StringSlice("inlineData"),
    ]:
        if gemini_bridge_request_object_member(source, start, end, key)[0] >= 0:
            return True
    var type = gemini_bridge_request_object_member(
        source, start, end, StringSlice("type")
    )
    if not gemini_bridge_text_raw_string(source, type):
        return False
    for media_type in [
        StringSlice("input_image"),
        StringSlice("image_url"),
        StringSlice("input_file"),
        StringSlice("file"),
        StringSlice("media"),
        StringSlice("input_audio"),
        StringSlice("input_video"),
    ]:
        if gemini_request_content_string_equals(
            source, type[0], type[1], media_type, False
        ):
            return True
    return False


def gemini_bridge_text_object_value(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Array[Int64, 2]:
    var missing = Array[Int64, 2](fill=-1)
    if not gemini_bridge_request_is_object(source, start, end):
        return missing^
    if gemini_bridge_text_media_like_object(source, start, end):
        return missing^
    var text = gemini_bridge_request_object_member(
        source, start, end, StringSlice("text")
    )
    if gemini_bridge_text_raw_string(source, text):
        return text^
    var content = gemini_bridge_request_object_member(
        source, start, end, StringSlice("content")
    )
    if gemini_bridge_text_raw_string(source, content):
        return content^
    return missing^


def gemini_bridge_text_content_supported(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if bounds[0] < 0:
        return True
    var opening = gemini_request_content_byte(source, bounds[0])
    if opening == 34:
        return True
    if opening != 91:
        return False
    var index = gemini_request_content_skip_ws(
        source, bounds[0] + 1, bounds[1] - 1
    )
    while index < bounds[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(
            source, index, bounds[1] - 1, 0
        )
        if value_end < 0:
            return False
        var value = gemini_request_content_byte(source, index)
        if value != 34 and (
            value != 123
            or gemini_bridge_text_object_value(source, index, value_end)[0] < 0
        ):
            return False
        index = gemini_request_content_skip_ws(source, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, bounds[1] - 1)
        elif index != bounds[1] - 1:
            return False
    return True


def gemini_bridge_text_contextual_prefix(
    view: GeminiRequestContentStringView,
) -> Bool:
    for prefix in [
        StringSlice("# AGENTS.md instructions for "),
        StringSlice("<environment_context>"),
        StringSlice("<permissions instructions>"),
        StringSlice("<collaboration_mode>"),
        StringSlice("<skills_instructions>"),
        StringSlice("<plugins_instructions>"),
        StringSlice("<model_switch>"),
        StringSlice("<personality_spec>"),
        StringSlice("<realtime_conversation>"),
    ]:
        if gemini_bridge_request_string_starts_with(view, prefix):
            return True
    return False


def gemini_bridge_text_content_has_contextual_prefix(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if bounds[0] < 0:
        return False
    if gemini_bridge_text_raw_string(source, bounds):
        return gemini_bridge_text_contextual_prefix(
            gemini_bridge_request_value_view(source, bounds)
        )
    if gemini_request_content_byte(source, bounds[0]) != 91:
        return False
    var index = gemini_request_content_skip_ws(
        source, bounds[0] + 1, bounds[1] - 1
    )
    while index < bounds[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(
            source, index, bounds[1] - 1, 0
        )
        if value_end < 0:
            return False
        var text = Array[Int64, 2](fill=-1)
        if gemini_request_content_byte(source, index) == 34:
            text[0] = index
            text[1] = value_end
        elif gemini_request_content_byte(source, index) == 123:
            text = gemini_bridge_text_object_value(source, index, value_end)
        if gemini_bridge_text_raw_string(source, text) and gemini_bridge_text_contextual_prefix(
            gemini_bridge_request_value_view(source, text)
        ):
            return True
        index = gemini_request_content_skip_ws(source, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, bounds[1] - 1)
        else:
            break
    return False

def gemini_bridge_text_copy_inner(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    return gemini_request_content_put_range(
        writer, source, bounds[0] + 1, bounds[1] - 1
    )


def gemini_bridge_text_write_joined_inner(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    found: Pointer[mut=True, Int64, _],
    separator: StringSlice,
) -> Bool:
    if bounds[0] < 0:
        return True
    if gemini_bridge_text_raw_string(source, bounds):
        if not gemini_request_content_string_has_non_space(
            source, bounds[0], bounds[1]
        ):
            return True
        if found[] == 1 and not gemini_request_content_put_literal(writer, separator):
            return False
        found[] = 1
        return gemini_bridge_text_copy_inner(writer, source, bounds)
    if gemini_request_content_byte(source, bounds[0]) != 91:
        return False
    var index = gemini_request_content_skip_ws(
        source, bounds[0] + 1, bounds[1] - 1
    )
    while index < bounds[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(
            source, index, bounds[1] - 1, 0
        )
        if value_end < 0:
            return False
        var text = Array[Int64, 2](fill=-1)
        if gemini_request_content_byte(source, index) == 34:
            text[0] = index
            text[1] = value_end
        elif gemini_request_content_byte(source, index) == 123:
            text = gemini_bridge_text_object_value(source, index, value_end)
        if gemini_bridge_text_raw_string(source, text) and gemini_request_content_string_has_non_space(
            source, text[0], text[1]
        ):
            if found[] == 1 and not gemini_request_content_put_literal(writer, separator):
                return False
            found[] = 1
            if not gemini_bridge_text_copy_inner(writer, source, text):
                return False
        index = gemini_request_content_skip_ws(source, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, bounds[1] - 1)
        elif index != bounds[1] - 1:
            return False
    return True


def gemini_bridge_text_request_supported(
    source: GeminiRequestContentStringView,
    root: Array[Int64, 2],
) -> Bool:
    var input = gemini_bridge_request_object_member(
        source, root[0], root[1], StringSlice("input")
    )
    if input[0] < 0 or gemini_bridge_text_raw_string(source, input):
        return True
    if not gemini_bridge_request_is_array(source, input[0], input[1]):
        return False
    var index = gemini_request_content_skip_ws(
        source, input[0] + 1, input[1] - 1
    )
    while index < input[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var item_end = gemini_request_content_value_end(
            source, index, input[1] - 1, 0
        )
        if item_end < 0 or not gemini_bridge_request_is_object(
            source, index, item_end
        ):
            return False
        var role = gemini_bridge_text_role_code(source, index, item_end)
        if role < 0 or role == 3:
            return False
        if gemini_bridge_request_object_member(
            source, index, item_end, StringSlice("tool_calls")
        )[0] >= 0 or gemini_bridge_request_object_member(
            source, index, item_end, StringSlice("gemini_native_parts")
        )[0] >= 0:
            return False
        var content = gemini_bridge_request_object_member(
            source, index, item_end, StringSlice("content")
        )
        if not gemini_bridge_text_content_supported(source, content):
            return False
        if role == 1 and gemini_bridge_text_content_has_contextual_prefix(
            source, content
        ):
            return False
        index = gemini_request_content_skip_ws(source, item_end, input[1] - 1)
        if index < input[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, input[1] - 1)
        elif index != input[1] - 1:
            return False
    return True



def gemini_bridge_text_content_nonempty(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if bounds[0] < 0:
        return False
    if gemini_bridge_text_raw_string(source, bounds):
        return gemini_request_content_string_has_non_space(
            source, bounds[0], bounds[1]
        )
    if gemini_request_content_byte(source, bounds[0]) != 91:
        return False
    var index = gemini_request_content_skip_ws(
        source, bounds[0] + 1, bounds[1] - 1
    )
    while index < bounds[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(
            source, index, bounds[1] - 1, 0
        )
        if value_end < 0:
            return False
        var text = Array[Int64, 2](fill=-1)
        if gemini_request_content_byte(source, index) == 34:
            text[0] = index
            text[1] = value_end
        elif gemini_request_content_byte(source, index) == 123:
            text = gemini_bridge_text_object_value(source, index, value_end)
        if gemini_bridge_text_raw_string(source, text) and gemini_request_content_string_has_non_space(
            source, text[0], text[1]
        ):
            return True
        index = gemini_request_content_skip_ws(source, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, bounds[1] - 1)
        else:
            break
    return False


def gemini_bridge_text_write_parts(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_put_byte(writer, 91):
        return False
    var first: Int64 = 1
    if gemini_bridge_text_raw_string(source, bounds):
        if gemini_request_content_string_has_non_space(
            source, bounds[0], bounds[1]
        ):
            if (
                not gemini_request_content_put_literal(writer, StringSlice('{"text":'))
                or not gemini_request_content_put_range(
                    writer, source, bounds[0], bounds[1]
                )
                or not gemini_request_content_put_byte(writer, 125)
            ):
                return False
        return gemini_request_content_put_byte(writer, 93)
    if bounds[0] < 0 or gemini_request_content_byte(source, bounds[0]) != 91:
        return gemini_request_content_put_byte(writer, 93)
    var index = gemini_request_content_skip_ws(
        source, bounds[0] + 1, bounds[1] - 1
    )
    while index < bounds[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var value_end = gemini_request_content_value_end(
            source, index, bounds[1] - 1, 0
        )
        if value_end < 0:
            return False
        var text = Array[Int64, 2](fill=-1)
        if gemini_request_content_byte(source, index) == 34:
            text[0] = index
            text[1] = value_end
        elif gemini_request_content_byte(source, index) == 123:
            text = gemini_bridge_text_object_value(source, index, value_end)
        if gemini_bridge_text_raw_string(source, text) and gemini_request_content_string_has_non_space(
            source, text[0], text[1]
        ):
            if first == 0 and not gemini_request_content_put_byte(writer, 44):
                return False
            first = 0
            if (
                not gemini_request_content_put_literal(writer, StringSlice('{"text":'))
                or not gemini_request_content_put_range(
                    writer, source, text[0], text[1]
                )
                or not gemini_request_content_put_byte(writer, 125)
            ):
                return False
        index = gemini_request_content_skip_ws(source, value_end, bounds[1] - 1)
        if index < bounds[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, bounds[1] - 1)
        elif index != bounds[1] - 1:
            return False
    return gemini_request_content_put_byte(writer, 93)


def gemini_bridge_text_write_system_instruction(
    source: GeminiRequestContentStringView,
    input: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    var has_system = False
    if gemini_bridge_request_is_array(source, input[0], input[1]):
        var probe = gemini_request_content_skip_ws(
            source, input[0] + 1, input[1] - 1
        )
        while probe < input[1] - 1 and gemini_request_content_byte(source, probe) != 93:
            var item_end = gemini_request_content_value_end(
                source, probe, input[1] - 1, 0
            )
            if item_end < 0:
                return False
            if gemini_bridge_text_role_code(source, probe, item_end) == 0:
                var content = gemini_bridge_request_object_member(
                    source, probe, item_end, StringSlice("content")
                )
                if gemini_bridge_text_content_nonempty(source, content):
                    has_system = True
                    break
            probe = gemini_request_content_skip_ws(source, item_end, input[1] - 1)
            if probe < input[1] - 1 and gemini_request_content_byte(source, probe) == 44:
                probe = gemini_request_content_skip_ws(source, probe + 1, input[1] - 1)
            else:
                break
    if not has_system:
        return gemini_request_content_put_literal(writer, StringSlice("null"))

    if not gemini_request_content_put_literal(
        writer, StringSlice('{"parts":[{"text":"')
    ):
        return False
    var found: Int64 = 0
    var found_ptr = Pointer(to=found)
    var index = gemini_request_content_skip_ws(
        source, input[0] + 1, input[1] - 1
    )
    while index < input[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var item_end = gemini_request_content_value_end(
            source, index, input[1] - 1, 0
        )
        if item_end < 0:
            return False
        if gemini_bridge_text_role_code(source, index, item_end) == 0:
            var content = gemini_bridge_request_object_member(
                source, index, item_end, StringSlice("content")
            )
            if not gemini_bridge_text_write_joined_inner(
                source,
                content,
                writer,
                found_ptr,
                StringSlice("\n\n"),
            ):
                return False
        index = gemini_request_content_skip_ws(source, item_end, input[1] - 1)
        if index < input[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, input[1] - 1)
        else:
            break
    return gemini_request_content_put_literal(writer, StringSlice('"}]}'))


def gemini_bridge_text_write_contents(
    source: GeminiRequestContentStringView,
    input: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_put_byte(writer, 91):
        return False

    if input[0] < 0:
        return (
            gemini_request_content_put_literal(
                writer, StringSlice('{"role":"user","parts":[{"text":""}]}')
            )
            and gemini_request_content_put_byte(writer, 93)
        )

    if gemini_bridge_text_raw_string(source, input):
        return (
            gemini_request_content_put_literal(
                writer, StringSlice('{"role":"user","parts":[{"text":')
            )
            and gemini_request_content_put_range(
                writer, source, input[0], input[1]
            )
            and gemini_request_content_put_literal(writer, StringSlice("}]}"))
            and gemini_request_content_put_byte(writer, 93)
        )

    var first: Int64 = 1
    var index = gemini_request_content_skip_ws(
        source, input[0] + 1, input[1] - 1
    )
    while index < input[1] - 1 and gemini_request_content_byte(source, index) != 93:
        var item_end = gemini_request_content_value_end(
            source, index, input[1] - 1, 0
        )
        if item_end < 0:
            return False
        var role = gemini_bridge_text_role_code(source, index, item_end)
        if role == 1 or role == 2:
            var content = gemini_bridge_request_object_member(
                source, index, item_end, StringSlice("content")
            )
            if gemini_bridge_text_content_nonempty(source, content):
                if first == 0 and not gemini_request_content_put_byte(writer, 44):
                    return False
                first = 0
                if role == 2:
                    if not gemini_request_content_put_literal(
                        writer, StringSlice('{"role":"model","parts":')
                    ):
                        return False
                else:
                    if not gemini_request_content_put_literal(
                        writer, StringSlice('{"role":"user","parts":')
                    ):
                        return False
                if (
                    not gemini_bridge_text_write_parts(source, content, writer)
                    or not gemini_request_content_put_byte(writer, 125)
                ):
                    return False
        index = gemini_request_content_skip_ws(source, item_end, input[1] - 1)
        if index < input[1] - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, input[1] - 1)
        elif index != input[1] - 1:
            return False

    if first == 1 and not gemini_request_content_put_literal(
        writer, StringSlice('{"role":"user","parts":[{"text":""}]}')
    ):
        return False
    return gemini_request_content_put_byte(writer, 93)


def gemini_bridge_request_write_text_contents(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary):
        return False
    var root = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, root[0], root[1]):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    if not gemini_bridge_text_request_supported(input.primary, root):
        return gemini_request_content_put_literal(writer, StringSlice("null"))
    var request_input = gemini_bridge_request_object_member(
        input.primary, root[0], root[1], StringSlice("input")
    )
    if (
        not gemini_request_content_put_literal(
            writer, StringSlice('{"systemInstruction":')
        )
        or not gemini_bridge_text_write_system_instruction(
            input.primary, request_input, writer
        )
        or not gemini_request_content_put_literal(
            writer, StringSlice(',"contents":')
        )
        or not gemini_bridge_text_write_contents(
            input.primary, request_input, writer
        )
    ):
        return False
    return gemini_request_content_put_byte(writer, 125)


comptime GEMINI_TRANSLATOR_VALIDATION_OK: Int64 = 0
comptime GEMINI_TRANSLATOR_VALIDATION_LOCAL_MEDIA: Int64 = 1
comptime GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_CONFLICT: Int64 = 2
comptime GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_SNAKE: Int64 = 3
comptime GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_CAMEL: Int64 = 4
comptime GEMINI_TRANSLATOR_VALIDATION_TOOLS_ARRAY: Int64 = 5
comptime GEMINI_TRANSLATOR_VALIDATION_TOOL_OBJECT: Int64 = 6
comptime GEMINI_TRANSLATOR_VALIDATION_FUNCTION_OBJECT: Int64 = 7
comptime GEMINI_TRANSLATOR_VALIDATION_FUNCTION_NAME_WRAPPED: Int64 = 8
comptime GEMINI_TRANSLATOR_VALIDATION_FUNCTION_NAME_FLAT: Int64 = 9
comptime GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_REQUIRED_WRAPPED: Int64 = 10
comptime GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_REQUIRED_FLAT: Int64 = 11
comptime GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_OBJECT_WRAPPED: Int64 = 12
comptime GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_OBJECT_FLAT: Int64 = 13
comptime GEMINI_TRANSLATOR_VALIDATION_DESCRIPTION_STRING_WRAPPED: Int64 = 14
comptime GEMINI_TRANSLATOR_VALIDATION_DESCRIPTION_STRING_FLAT: Int64 = 15
comptime GEMINI_TRANSLATOR_VALIDATION_RUST_TOOL_BRIDGE: Int64 = 16
comptime GEMINI_TRANSLATOR_VALIDATION_RESPONSE_FORMAT: Int64 = 17

def gemini_bridge_request_put_nonnegative_i64(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    value: Int64,
) -> Bool:
    if value < 0:
        return (
            gemini_request_content_put_byte(writer, 45)
            and gemini_request_content_put_byte(writer, 49)
        )
    if value == 0:
        return gemini_request_content_put_byte(writer, 48)
    var divisor: Int64 = 1
    while value / divisor >= 10:
        divisor *= 10
    var remaining = value
    while divisor > 0:
        if not gemini_request_content_put_byte(
            writer, UInt8(remaining / divisor) + 48
        ):
            return False
        remaining %= divisor
        divisor /= 10
    return True

def gemini_bridge_validation_put_plan(
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    source: GeminiRequestContentStringView,
    tag: Int64,
    index: Int64 = -1,
    detail_start: Int64 = -1,
    detail_end: Int64 = -1,
) -> Bool:
    if not gemini_request_content_put_literal(writer, StringSlice('{"tag":')):
        return False
    if not gemini_bridge_request_put_nonnegative_i64(writer, tag):
        return False
    if not gemini_request_content_put_literal(writer, StringSlice(',"index":')):
        return False
    if not gemini_bridge_request_put_nonnegative_i64(writer, index):
        return False
    if not gemini_request_content_put_literal(writer, StringSlice(',"detail":')):
        return False
    if detail_start >= 0 and detail_end > detail_start:
        if not gemini_request_content_put_range(
            writer, source, detail_start, detail_end
        ):
            return False
    elif not gemini_request_content_put_literal(writer, StringSlice("null")):
        return False
    return gemini_request_content_put_byte(writer, 125)

def gemini_bridge_validation_media_type(
    source: GeminiRequestContentStringView,
    bounds: Array[Int64, 2],
) -> Bool:
    if bounds[0] < 0 or gemini_request_content_byte(source, bounds[0]) != 34:
        return False
    return (
        gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("input_image"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("image_url"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("input_file"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("file"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("media"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("input_audio"), False
        )
        or gemini_request_content_string_equals(
            source, bounds[0], bounds[1], StringSlice("input_video"), False
        )
    )

def gemini_bridge_validation_has_local_media(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    depth: Int64,
) -> Bool:
    if depth > GEMINI_REQUEST_CONTENT_MAX_DEPTH or start < 0 or end <= start:
        return False
    var opening = gemini_request_content_byte(source, start)
    if opening == 91:
        var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
        while index < end - 1:
            var value_end = gemini_request_content_value_end(
                source, index, end - 1, depth + 1
            )
            if value_end < 0:
                return False
            if gemini_bridge_validation_has_local_media(
                source, index, value_end, depth + 1
            ):
                return True
            index = gemini_request_content_skip_ws(source, value_end, end - 1)
            if index < end - 1 and gemini_request_content_byte(source, index) == 44:
                index = gemini_request_content_skip_ws(source, index + 1, end - 1)
                continue
            break
        return False
    if opening != 123:
        return False

    var kind = gemini_bridge_request_object_member(
        source, start, end, StringSlice("type")
    )
    if gemini_bridge_validation_media_type(source, kind):
        if (
            gemini_bridge_request_object_member(
                source, start, end, StringSlice("path")
            )[0] >= 0
            or gemini_bridge_request_object_member(
                source, start, end, StringSlice("file_path")
            )[0] >= 0
            or gemini_bridge_request_object_member(
                source, start, end, StringSlice("filePath")
            )[0] >= 0
        ):
            return True

    var index = gemini_request_content_skip_ws(source, start + 1, end - 1)
    while index < end - 1:
        var key_end = gemini_request_content_string_end(source, index, end - 1)
        if key_end < 0:
            return False
        index = gemini_request_content_skip_ws(source, key_end, end - 1)
        if index >= end - 1 or gemini_request_content_byte(source, index) != 58:
            return False
        var value_start = gemini_request_content_skip_ws(source, index + 1, end - 1)
        var value_end = gemini_request_content_value_end(
            source, value_start, end - 1, depth + 1
        )
        if value_end < 0:
            return False
        if gemini_bridge_validation_has_local_media(
            source, value_start, value_end, depth + 1
        ):
            return True
        index = gemini_request_content_skip_ws(source, value_end, end - 1)
        if index < end - 1 and gemini_request_content_byte(source, index) == 44:
            index = gemini_request_content_skip_ws(source, index + 1, end - 1)
            continue
        break
    return False

def gemini_bridge_validation_function_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    index: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Int64:
    var function = gemini_bridge_request_object_member(
        source, start, end, StringSlice("function")
    )
    var wrapped = function[0] >= 0
    var target_start = start
    var target_end = end
    if wrapped:
        if not gemini_bridge_request_is_object(
            source, function[0], function[1]
        ):
            if gemini_bridge_validation_put_plan(
                writer,
                source,
                GEMINI_TRANSLATOR_VALIDATION_FUNCTION_OBJECT,
                index,
            ):
                return 1
            return -1
        target_start = function[0]
        target_end = function[1]

    var name = gemini_bridge_request_object_member(
        source, target_start, target_end, StringSlice("name")
    )
    if (
        name[0] < 0
        or gemini_request_content_byte(source, name[0]) != 34
        or not gemini_request_content_string_has_non_space(
            source, name[0], name[1]
        )
    ):
        var tag = GEMINI_TRANSLATOR_VALIDATION_FUNCTION_NAME_FLAT
        if wrapped:
            tag = GEMINI_TRANSLATOR_VALIDATION_FUNCTION_NAME_WRAPPED
        if gemini_bridge_validation_put_plan(
            writer, source, tag, index
        ):
            return 1
        return -1

    var parameters = gemini_bridge_request_object_member(
        source, target_start, target_end, StringSlice("parameters")
    )
    if parameters[0] < 0:
        var tag = GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_REQUIRED_FLAT
        if wrapped:
            tag = GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_REQUIRED_WRAPPED
        if gemini_bridge_validation_put_plan(
            writer, source, tag, index
        ):
            return 1
        return -1
    if not gemini_bridge_request_is_object(
        source, parameters[0], parameters[1]
    ):
        var tag = GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_OBJECT_FLAT
        if wrapped:
            tag = GEMINI_TRANSLATOR_VALIDATION_PARAMETERS_OBJECT_WRAPPED
        if gemini_bridge_validation_put_plan(
            writer, source, tag, index
        ):
            return 1
        return -1

    var description = gemini_bridge_request_object_member(
        source, target_start, target_end, StringSlice("description")
    )
    if (
        description[0] >= 0
        and not gemini_bridge_request_is_null(
            source, description[0], description[1]
        )
        and gemini_request_content_byte(source, description[0]) != 34
    ):
        var tag = GEMINI_TRANSLATOR_VALIDATION_DESCRIPTION_STRING_FLAT
        if wrapped:
            tag = GEMINI_TRANSLATOR_VALIDATION_DESCRIPTION_STRING_WRAPPED
        if gemini_bridge_validation_put_plan(
            writer, source, tag, index
        ):
            return 1
        return -1
    return 0

def gemini_bridge_request_validate_translator(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if not gemini_request_content_fragment_valid(input.primary):
        return False
    var root = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(
        input.primary, root[0], root[1]
    ):
        return gemini_bridge_validation_put_plan(
            writer, input.primary, GEMINI_TRANSLATOR_VALIDATION_OK
        )

    if gemini_bridge_validation_has_local_media(
        input.primary, root[0], root[1], 0
    ):
        return gemini_bridge_validation_put_plan(
            writer, input.primary, GEMINI_TRANSLATOR_VALIDATION_LOCAL_MEDIA
        )

    var snake = gemini_bridge_request_object_member(
        input.primary, root[0], root[1], StringSlice("candidate_count")
    )
    var camel = gemini_bridge_request_object_member(
        input.primary, root[0], root[1], StringSlice("candidateCount")
    )
    var snake_active = (
        snake[0] >= 0
        and not gemini_bridge_request_is_null(
            input.primary, snake[0], snake[1]
        )
    )
    var camel_active = (
        camel[0] >= 0
        and not gemini_bridge_request_is_null(
            input.primary, camel[0], camel[1]
        )
    )
    if (
        snake_active
        and camel_active
        and not gemini_bridge_request_raw_values_equal(
            input.primary, snake, camel
        )
    ):
        return gemini_bridge_validation_put_plan(
            writer,
            input.primary,
            GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_CONFLICT,
        )
    if (
        snake_active
        and not gemini_bridge_request_literal_equals(
            input.primary, snake[0], snake[1], StringSlice("1")
        )
    ):
        return gemini_bridge_validation_put_plan(
            writer,
            input.primary,
            GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_SNAKE,
        )
    if (
        camel_active
        and not gemini_bridge_request_literal_equals(
            input.primary, camel[0], camel[1], StringSlice("1")
        )
    ):
        return gemini_bridge_validation_put_plan(
            writer,
            input.primary,
            GEMINI_TRANSLATOR_VALIDATION_CANDIDATE_CAMEL,
        )

    var response_format = gemini_bridge_request_object_member(
        input.primary, root[0], root[1], StringSlice("response_format")
    )
    if gemini_bridge_request_is_object(
        input.primary, response_format[0], response_format[1]
    ):
        var format_type = gemini_bridge_request_object_member(
            input.primary,
            response_format[0],
            response_format[1],
            StringSlice("type"),
        )
        if (
            format_type[0] >= 0
            and gemini_request_content_byte(input.primary, format_type[0]) == 34
            and not gemini_request_content_string_equals(
                input.primary,
                format_type[0],
                format_type[1],
                StringSlice("text"),
                False,
            )
            and not gemini_request_content_string_equals(
                input.primary,
                format_type[0],
                format_type[1],
                StringSlice("json_object"),
                False,
            )
            and not gemini_request_content_string_equals(
                input.primary,
                format_type[0],
                format_type[1],
                StringSlice("json_schema"),
                False,
            )
        ):
            return gemini_bridge_validation_put_plan(
                writer,
                input.primary,
                GEMINI_TRANSLATOR_VALIDATION_RESPONSE_FORMAT,
                -1,
                format_type[0],
                format_type[1],
            )

    var tools = gemini_bridge_request_object_member(
        input.primary, root[0], root[1], StringSlice("tools")
    )
    if tools[0] < 0:
        return gemini_bridge_validation_put_plan(
            writer, input.primary, GEMINI_TRANSLATOR_VALIDATION_OK
        )
    if not gemini_bridge_request_is_array(
        input.primary, tools[0], tools[1]
    ):
        return gemini_bridge_validation_put_plan(
            writer,
            input.primary,
            GEMINI_TRANSLATOR_VALIDATION_TOOLS_ARRAY,
        )

    var item_index: Int64 = 0
    var cursor = gemini_request_content_skip_ws(
        input.primary, tools[0] + 1, tools[1] - 1
    )
    while cursor < tools[1] - 1:
        var item_end = gemini_request_content_value_end(
            input.primary, cursor, tools[1] - 1, 0
        )
        if item_end < 0:
            return False
        if not gemini_bridge_request_is_object(
            input.primary, cursor, item_end
        ):
            return gemini_bridge_validation_put_plan(
                writer,
                input.primary,
                GEMINI_TRANSLATOR_VALIDATION_TOOL_OBJECT,
                item_index,
            )

        var function = gemini_bridge_request_object_member(
            input.primary, cursor, item_end, StringSlice("function")
        )
        var tool_type = gemini_bridge_request_object_member(
            input.primary, cursor, item_end, StringSlice("type")
        )
        var is_function = function[0] >= 0 or (
            tool_type[0] >= 0
            and gemini_request_content_string_equals(
                input.primary,
                tool_type[0],
                tool_type[1],
                StringSlice("function"),
                False,
            )
        )
        if is_function:
            if function[0] < 0:
                return gemini_bridge_validation_put_plan(
                    writer,
                    input.primary,
                    GEMINI_TRANSLATOR_VALIDATION_FUNCTION_OBJECT,
                    item_index,
                )
            var result = gemini_bridge_validation_function_tool(
                input.primary, cursor, item_end, item_index, writer
            )
            if result != 0:
                return result > 0
        elif not gemini_bridge_request_builtin_tool(
            input.primary, cursor, item_end
        ):
            return gemini_bridge_validation_put_plan(
                writer,
                input.primary,
                GEMINI_TRANSLATOR_VALIDATION_RUST_TOOL_BRIDGE,
                item_index,
            )

        item_index += 1
        cursor = gemini_request_content_skip_ws(
            input.primary, item_end, tools[1] - 1
        )
        if (
            cursor < tools[1] - 1
            and gemini_request_content_byte(input.primary, cursor) == 44
        ):
            cursor = gemini_request_content_skip_ws(
                input.primary, cursor + 1, tools[1] - 1
            )
            continue
        if cursor != tools[1] - 1:
            return False
        break

    return gemini_bridge_validation_put_plan(
        writer, input.primary, GEMINI_TRANSLATOR_VALIDATION_OK
    )

def gemini_translator_response_format_kind(
    source: GeminiRequestContentStringView,
    root: Array[Int64, 2],
) -> Int64:
    var response_format = gemini_bridge_request_object_member(
        source, root[0], root[1], StringSlice("response_format")
    )
    if not gemini_bridge_request_is_object(
        source, response_format[0], response_format[1]
    ):
        return 0
    var kind = gemini_bridge_request_object_member(
        source, response_format[0], response_format[1], StringSlice("type")
    )
    if (
        kind[0] < 0
        or gemini_request_content_byte(source, kind[0]) != 34
        or gemini_request_content_string_equals(
            source, kind[0], kind[1], StringSlice("text"), False
        )
    ):
        return 0
    if gemini_request_content_string_equals(
        source, kind[0], kind[1], StringSlice("json_object"), False
    ):
        return 1
    if gemini_request_content_string_equals(
        source, kind[0], kind[1], StringSlice("json_schema"), False
    ):
        return 2
    return 0

def gemini_translator_write_response_format(
    source: GeminiRequestContentStringView,
    root: Array[Int64, 2],
    kind: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first: Pointer[mut=True, Bool, _],
) -> Bool:
    if kind == 0:
        return True
    if not gemini_bridge_request_put_literal_field(
        writer, first, StringSlice('"responseMimeType":"application/json"')
    ):
        return False
    if kind != 2:
        return True
    var response_format = gemini_bridge_request_object_member(
        source, root[0], root[1], StringSlice("response_format")
    )
    var schema = gemini_bridge_request_object_member(
        source, response_format[0], response_format[1], StringSlice("schema")
    )
    if schema[0] < 0:
        return True
    if not gemini_bridge_request_put_field_prefix(
        writer, first, StringSlice('"responseJsonSchema":')
    ):
        return False
    return gemini_request_content_write_sanitized_schema(
        source, schema[0], schema[1], writer, 0
    )

def gemini_translator_write_generation_config(
    source: GeminiRequestContentStringView,
    model: GeminiRequestContentStringView,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    var root = gemini_bridge_request_value_bounds(source)
    if not gemini_bridge_request_is_object(source, root[0], root[1]):
        return False
    var first = True
    var first_ptr = Pointer(to=first)
    if not gemini_request_content_put_byte(writer, 123):
        return False

    if (
        not gemini_bridge_request_put_source_pair(
            source, root, 0, -1, StringSlice('"temperature":'), False, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 1, -1, StringSlice('"topP":'), False, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 2, -1, StringSlice('"maxOutputTokens":'), False, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 7, 6, StringSlice('"topK":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 8, -1, StringSlice('"seed":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 10, 9, StringSlice('"presencePenalty":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 12, 11, StringSlice('"frequencyPenalty":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 14, 13, StringSlice('"responseMimeType":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 16, 15, StringSlice('"responseSchema":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 18, 17, StringSlice('"responseJsonSchema":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 20, 19, StringSlice('"responseModalities":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 22, 21, StringSlice('"mediaResolution":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 24, 23, StringSlice('"audioTimestamp":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 26, 25, StringSlice('"speechConfig":'), True, writer, first_ptr
        )
        or not gemini_bridge_request_put_source_pair(
            source, root, 27, 28, StringSlice('"candidateCount":'), True, writer, first_ptr
        )
    ):
        return False

    var stop = gemini_bridge_request_object_member(
        source, root[0], root[1], StringSlice("stop")
    )
    if stop[0] < 0:
        stop = gemini_bridge_request_object_member(
            source, root[0], root[1], StringSlice("stop_sequences")
        )
    if stop[0] < 0:
        stop = gemini_bridge_request_object_member(
            source, root[0], root[1], StringSlice("stopSequences")
        )
    if (
        stop[0] >= 0
        and not gemini_bridge_request_is_null(source, stop[0], stop[1])
        and not gemini_bridge_request_put_raw_field(
            writer,
            first_ptr,
            StringSlice('"stopSequences":'),
            gemini_bridge_request_value_view(source, stop),
        )
    ):
        return False

    var text_format = gemini_bridge_request_text_format_kind(source, root)
    var response_format = gemini_translator_response_format_kind(source, root)
    if response_format == 2:
        if not gemini_translator_write_response_format(
            source, root, response_format, writer, first_ptr
        ):
            return False
    else:
        if not gemini_bridge_request_write_text_format(
            source, root, text_format, writer, first_ptr
        ):
            return False
        if response_format == 1 and text_format == 0:
            if not gemini_translator_write_response_format(
                source, root, response_format, writer, first_ptr
            ):
                return False

    if not gemini_bridge_request_write_thinking_config(
        source,
        root,
        model,
        GeminiRequestContentStringView(0, 0),
        0,
        writer,
        first_ptr,
    ):
        return False

    return gemini_request_content_put_byte(writer, 125)




def gemini_translator_type_is(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var kind = gemini_bridge_request_object_member(
        source, start, end, StringSlice("type")
    )
    return (
        kind[0] >= 0
        and gemini_request_content_byte(source, kind[0]) == 34
        and gemini_request_content_string_equals(
            source, kind[0], kind[1], literal, False
        )
    )

def gemini_translator_type_starts_with(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    prefix: StringSlice,
) -> Bool:
    var kind = gemini_bridge_request_object_member(
        source, start, end, StringSlice("type")
    )
    if kind[0] < 0:
        return False
    return gemini_bridge_request_string_starts_with(
        gemini_bridge_request_value_view(source, kind), prefix
    )

def gemini_translator_is_computer_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return (
        gemini_translator_type_is(
            source, start, end, StringSlice("computer")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("computer_use")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("computerUse")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("computer_use_preview")
        )
        or gemini_translator_type_starts_with(
            source, start, end, StringSlice("computer_")
        )
        or gemini_bridge_request_object_member(
            source, start, end, StringSlice("computerUse")
        )[0] >= 0
    )

def gemini_translator_is_code_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return (
        gemini_translator_type_is(
            source, start, end, StringSlice("code_interpreter")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("code_execution")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("codeExecution")
        )
        or gemini_bridge_request_object_member(
            source, start, end, StringSlice("codeExecution")
        )[0] >= 0
    )

def gemini_translator_is_web_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return (
        gemini_translator_type_is(
            source, start, end, StringSlice("web_search")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("web_search_preview")
        )
        or gemini_translator_type_starts_with(
            source, start, end, StringSlice("web_search_preview_")
        )
    )

def gemini_translator_is_url_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return (
        gemini_translator_type_is(
            source, start, end, StringSlice("web_fetch")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("url_context")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("urlContext")
        )
        or gemini_translator_type_is(
            source, start, end, StringSlice("web_fetch_preview")
        )
        or gemini_translator_type_starts_with(
            source, start, end, StringSlice("web_fetch_preview_")
        )
        or gemini_bridge_request_object_member(
            source, start, end, StringSlice("urlContext")
        )[0] >= 0
    )

def gemini_translator_is_function_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    return gemini_bridge_request_object_member(
        source, start, end, StringSlice("function")
    )[0] >= 0

def gemini_translator_write_computer_tool(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    var config_start = start
    var config_end = end
    var config = gemini_bridge_request_object_member(
        source, start, end, StringSlice("computerUse")
    )
    if config[0] < 0:
        config = gemini_bridge_request_object_member(
            source, start, end, StringSlice("computer_use")
        )
    if config[0] >= 0:
        config_start = config[0]
        config_end = config[1]

    var environment = gemini_bridge_request_object_member(
        source, config_start, config_end, StringSlice("environment")
    )
    if (
        environment[0] < 0
        or gemini_request_content_byte(source, environment[0]) != 34
        or not gemini_request_content_string_has_non_space(
            source, environment[0], environment[1]
        )
    ):
        environment = Array[Int64, 2](fill=-1)

    var excluded = gemini_bridge_request_object_member(
        source,
        config_start,
        config_end,
        StringSlice("excludedPredefinedFunctions"),
    )
    if excluded[0] < 0:
        excluded = gemini_bridge_request_object_member(
            source,
            config_start,
            config_end,
            StringSlice("excluded_predefined_functions"),
        )

    if not gemini_request_content_put_literal(
        writer, StringSlice('{"computerUse":{"environment":')
    ):
        return False
    if environment[0] >= 0:
        if not gemini_request_content_put_range(
            writer, source, environment[0], environment[1]
        ):
            return False
    elif not gemini_request_content_put_literal(
        writer, StringSlice('"ENVIRONMENT_BROWSER"')
    ):
        return False
    if (
        excluded[0] >= 0
        and not gemini_bridge_request_is_null(
            source, excluded[0], excluded[1]
        )
    ):
        if (
            not gemini_request_content_put_literal(
                writer, StringSlice(',"excludedPredefinedFunctions":')
            )
            or not gemini_request_content_put_range(
                writer, source, excluded[0], excluded[1]
            )
        ):
            return False
    return gemini_request_content_put_literal(writer, StringSlice("}}"))

def gemini_translator_write_function_declaration(
    source: GeminiRequestContentStringView,
    start: Int64,
    end: Int64,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    var function = gemini_bridge_request_object_member(
        source, start, end, StringSlice("function")
    )
    if not gemini_bridge_request_is_object(
        source, function[0], function[1]
    ):
        return False
    var name = gemini_bridge_request_object_member(
        source, function[0], function[1], StringSlice("name")
    )
    var description = gemini_bridge_request_object_member(
        source, function[0], function[1], StringSlice("description")
    )
    var parameters = gemini_bridge_request_object_member(
        source, function[0], function[1], StringSlice("parameters")
    )
    if (
        not gemini_request_content_put_literal(writer, StringSlice('{"name":'))
        or not gemini_request_content_put_range(
            writer, source, name[0], name[1]
        )
        or not gemini_request_content_put_literal(
            writer, StringSlice(',"description":')
        )
    ):
        return False
    if description[0] >= 0:
        if not gemini_request_content_put_range(
            writer, source, description[0], description[1]
        ):
            return False
    elif not gemini_request_content_put_literal(writer, StringSlice('""')):
        return False
    if not gemini_request_content_put_literal(
        writer, StringSlice(',"parameters":')
    ):
        return False
    if not gemini_request_content_write_sanitized_schema(
        source, parameters[0], parameters[1], writer, 0
    ):
        return False
    return gemini_request_content_put_byte(writer, 125)

def gemini_translator_write_tools_field(
    source: GeminiRequestContentStringView,
    root: Array[Int64, 2],
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
    first_field: Pointer[mut=True, Bool, _],
) -> Bool:
    var tools = gemini_bridge_request_object_member(
        source, root[0], root[1], StringSlice("tools")
    )
    if tools[0] < 0:
        return True
    if not gemini_bridge_request_is_array(source, tools[0], tools[1]):
        return False

    var computer_start: Int64 = -1
    var computer_end: Int64 = -1
    var has_code = False
    var has_web = False
    var has_url = False
    var has_function = False
    var cursor = gemini_request_content_skip_ws(
        source, tools[0] + 1, tools[1] - 1
    )
    while cursor < tools[1] - 1:
        var item_end = gemini_request_content_value_end(
            source, cursor, tools[1] - 1, 0
        )
        if item_end < 0:
            return False
        if gemini_translator_is_computer_tool(
            source, cursor, item_end
        ) and computer_start < 0:
            computer_start = cursor
            computer_end = item_end
        if gemini_translator_is_code_tool(source, cursor, item_end):
            has_code = True
        if gemini_translator_is_web_tool(source, cursor, item_end):
            has_web = True
        if gemini_translator_is_url_tool(source, cursor, item_end):
            has_url = True
        if gemini_translator_is_function_tool(
            source, cursor, item_end
        ):
            has_function = True
        cursor = gemini_request_content_skip_ws(
            source, item_end, tools[1] - 1
        )
        if (
            cursor < tools[1] - 1
            and gemini_request_content_byte(source, cursor) == 44
        ):
            cursor = gemini_request_content_skip_ws(
                source, cursor + 1, tools[1] - 1
            )
            continue
        if cursor != tools[1] - 1:
            return False
        break

    if (
        computer_start < 0
        and not has_code
        and not has_web
        and not has_url
        and not has_function
    ):
        return True

    if not gemini_bridge_request_put_field_prefix(
        writer, first_field, StringSlice('"tools":')
    ):
        return False
    if not gemini_request_content_put_byte(writer, 91):
        return False

    var first_tool = True
    if computer_start >= 0:
        if not gemini_translator_write_computer_tool(
            source, computer_start, computer_end, writer
        ):
            return False
        first_tool = False

    if has_code:
        if (
            not first_tool
            and not gemini_request_content_put_byte(writer, 44)
        ):
            return False
        if not gemini_request_content_put_literal(
            writer, StringSlice('{"codeExecution":{}}')
        ):
            return False
        first_tool = False

    if has_web:
        if (
            not first_tool
            and not gemini_request_content_put_byte(writer, 44)
        ):
            return False
        if not gemini_request_content_put_literal(
            writer, StringSlice('{"googleSearch":{}}')
        ):
            return False
        first_tool = False

    if has_url:
        if (
            not first_tool
            and not gemini_request_content_put_byte(writer, 44)
        ):
            return False
        if not gemini_request_content_put_literal(
            writer, StringSlice('{"urlContext":{}}')
        ):
            return False
        first_tool = False

    if has_function:
        if (
            not first_tool
            and not gemini_request_content_put_byte(writer, 44)
        ):
            return False
        if not gemini_request_content_put_literal(
            writer, StringSlice('{"functionDeclarations":[')
        ):
            return False
        var first_function = True
        cursor = gemini_request_content_skip_ws(
            source, tools[0] + 1, tools[1] - 1
        )
        while cursor < tools[1] - 1:
            var item_end = gemini_request_content_value_end(
                source, cursor, tools[1] - 1, 0
            )
            if item_end < 0:
                return False
            if gemini_translator_is_function_tool(
                source, cursor, item_end
            ):
                if (
                    not first_function
                    and not gemini_request_content_put_byte(writer, 44)
                ):
                    return False
                if not gemini_translator_write_function_declaration(
                    source, cursor, item_end, writer
                ):
                    return False
                first_function = False
            cursor = gemini_request_content_skip_ws(
                source, item_end, tools[1] - 1
            )
            if (
                cursor < tools[1] - 1
                and gemini_request_content_byte(source, cursor) == 44
            ):
                cursor = gemini_request_content_skip_ws(
                    source, cursor + 1, tools[1] - 1
                )
                continue
            if cursor != tools[1] - 1:
                return False
            break
        if not gemini_request_content_put_literal(
            writer, StringSlice("]}")
        ):
            return False

    return gemini_request_content_put_byte(writer, 93)

def gemini_bridge_request_write_raw_translator(
    input: GeminiBridgeRequestInput,
    writer: Pointer[mut=True, GeminiRequestContentWriter, _],
) -> Bool:
    if (
        not gemini_request_content_fragment_valid(input.primary)
        or not gemini_request_content_fragment_valid(input.tertiary)
        or not gemini_request_content_fragment_valid(input.senary)
        or gemini_request_content_byte(input.senary, 0) != 34
    ):
        return False
    if input.secondary_present == 1 and not gemini_request_content_fragment_valid(input.secondary):
        return False
    if input.quaternary_present == 1 and not gemini_request_content_fragment_valid(input.quaternary):
        return False
    if input.quinary_present == 1 and not gemini_request_content_fragment_valid(input.quinary):
        return False

    var root = gemini_bridge_request_value_bounds(input.primary)
    if not gemini_bridge_request_is_object(input.primary, root[0], root[1]):
        return False

    if (
        not gemini_request_content_put_literal(writer, StringSlice('{"model":'))
        or not gemini_request_content_put_range(
            writer, input.senary, 0, Int64(input.senary.len)
        )
        or not gemini_request_content_put_literal(writer, StringSlice(',"request":{'))
    ):
        return False

    var first = True
    var first_ptr = Pointer(to=first)
    if input.secondary_present == 1 and not gemini_bridge_request_put_raw_field(
        writer,
        first_ptr,
        StringSlice('"systemInstruction":'),
        input.secondary,
    ):
        return False
    if not gemini_bridge_request_put_raw_field(
        writer,
        first_ptr,
        StringSlice('"contents":'),
        input.tertiary,
    ):
        return False

    if not gemini_bridge_request_put_field_prefix(
        writer, first_ptr, StringSlice('"generationConfig":')
    ):
        return False
    if not gemini_translator_write_generation_config(
        input.primary, input.senary, writer
    ):
        return False

    if input.quaternary_present == 1:
        if not gemini_bridge_request_put_raw_field(
            writer,
            first_ptr,
            StringSlice('"tools":'),
            input.quaternary,
        ):
            return False
    elif not gemini_translator_write_tools_field(
        input.primary, root, writer, first_ptr
    ):
        return False
    if input.quinary_present == 1 and not gemini_bridge_request_put_raw_field(
        writer,
        first_ptr,
        StringSlice('"toolConfig":'),
        input.quinary,
    ):
        return False

    if not gemini_bridge_request_write_optional_original_fields(
        input.primary, root, writer, first_ptr
    ):
        return False

    return (
        gemini_request_content_put_byte(writer, 125)
        and gemini_request_content_put_byte(writer, 125)
    )


@export("prodex_gemini_bridge_request_kernel_v1")
def prodex_gemini_bridge_request_kernel_v1(
    abi_version: Int64,
    input_address: UInt64,
    output_address: UInt64,
    output_capacity: Int64,
    written_address: UInt64,
) abi("C") -> Int64:
    if abi_version != GEMINI_BRIDGE_REQUEST_ABI_VERSION:
        return GEMINI_BRIDGE_REQUEST_STATUS_ABI_MISMATCH
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return GEMINI_BRIDGE_REQUEST_STATUS_INVALID
    var input = Pointer[
        mut=False, GeminiBridgeRequestInput, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input_address))[].copy()
    if not gemini_bridge_request_input_valid(input):
        return GEMINI_BRIDGE_REQUEST_STATUS_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = GeminiRequestContentWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    var ok = False
    if input.operation == GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_REQUEST:
        ok = gemini_bridge_request_write_content_request(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_GENERATE_CONTENT_BODY:
        ok = gemini_bridge_request_write_content_body(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_GENERATION_CONFIG:
        ok = gemini_bridge_request_write_generation_config(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_NATIVE_PROJECT:
        ok = gemini_bridge_request_write_native_project(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_WITHOUT_TOOL:
        ok = gemini_bridge_request_write_without_tool(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_SIMPLE:
        var result = gemini_bridge_request_simple(input, writer_ptr, output_address)
        writer.written = 0
        if result:
            ok = gemini_request_content_put_literal(writer_ptr, StringSlice("true"))
        else:
            ok = gemini_request_content_put_literal(writer_ptr, StringSlice("false"))
    elif input.operation == GEMINI_BRIDGE_REQUEST_VALIDATE_CANDIDATE_COUNT:
        ok = gemini_bridge_request_write_candidate_validation(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_TOOL_CONFIG:
        ok = gemini_bridge_request_write_tool_config(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_TEXT_CONTENTS:
        ok = gemini_bridge_request_write_text_contents(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_VALIDATE_TRANSLATOR:
        ok = gemini_bridge_request_validate_translator(input, writer_ptr)
    elif input.operation == GEMINI_BRIDGE_REQUEST_RAW_TRANSLATOR:
        ok = gemini_bridge_request_write_raw_translator(input, writer_ptr)
    if not ok:
        written[] = writer.written
        if writer.written >= output_capacity:
            return GEMINI_BRIDGE_REQUEST_STATUS_CAPACITY
        return GEMINI_BRIDGE_REQUEST_STATUS_INVALID
    written[] = writer.written
    return 0
