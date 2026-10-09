from std.memory import Pointer
from rich_text import rich_view_matches_literal, rich_view_prefix, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime UINT64_MAX: UInt64 = 18446744073709551615
comptime STATIC_ITEM_MAX_BYTES: Int64 = 262144

@fieldwise_init
struct SmartContextStaticItem(Copyable):
    var id: ProdexRichStringView
    var content_hash: ProdexRichStringView
    var canonical_text: ProdexRichStringView
    var byte_len: UInt64

@fieldwise_init
struct SmartContextStaticOrderKey(Copyable):
    var group: UInt64
    var input_index: UInt64
    var role: UInt64

def view_compare(left: ProdexRichStringView, right: ProdexRichStringView) -> Int64:
    var left_bytes = rich_view_ptr(left)
    var right_bytes = rich_view_ptr(right)
    var common = Int64(left.len)
    if right.len < left.len:
        common = Int64(right.len)
    for index in range(common):
        if left_bytes[unsafe_offset=index] < right_bytes[unsafe_offset=index]:
            return -1
        if left_bytes[unsafe_offset=index] > right_bytes[unsafe_offset=index]:
            return 1
    if left.len < right.len:
        return -1
    if left.len > right.len:
        return 1
    return 0

def static_input_order(view: ProdexRichStringView) -> SmartContextStaticOrderKey:
    var key = SmartContextStaticOrderKey(100, 0, 0)
    if not rich_view_prefix["input["](view, False):
        return key^
    var source = rich_view_ptr(view)
    var index = Int64(6)
    if index < Int64(view.len) and source[unsafe_offset=index] == 43:
        index += 1
    var start = index
    var value = UInt64(0)
    while index < Int64(view.len) and source[unsafe_offset=index] >= 48 and source[unsafe_offset=index] <= 57:
        var digit = UInt64(source[unsafe_offset=index] - 48)
        if value > 1844674407370955161 or value == 1844674407370955161 and digit > 5:
            return key^
        value = value * 10 + digit
        index += 1
    if index == start or index + 3 > Int64(view.len) or source[unsafe_offset=index] != 93 or source[unsafe_offset=index + 1] != 46:
        return key^
    index += 2
    var suffix = ProdexRichStringView(ptr=view.ptr + UInt(index), len=view.len - UInt(index))
    var role = UInt64(0)
    if rich_view_matches_literal["system"](suffix, False):
        role = 0
    elif rich_view_matches_literal["developer"](suffix, False):
        role = 1
    else:
        return key^
    key.group = 3
    key.input_index = value
    key.role = role
    return key^

def static_item_order_key(item: SmartContextStaticItem) -> SmartContextStaticOrderKey:
    var key = SmartContextStaticOrderKey(0, 0, 0)
    if rich_view_matches_literal["instructions"](item.id, False):
        return key^
    if rich_view_matches_literal["system"](item.id, False):
        key.group = 1
        return key^
    if rich_view_matches_literal["developer"](item.id, False):
        key.group = 2
        return key^
    return static_input_order(item.id)

def static_item_compare(left: SmartContextStaticItem, right: SmartContextStaticItem) -> Int64:
    var left_key = static_item_order_key(left)
    var right_key = static_item_order_key(right)
    if left_key.group != right_key.group:
        if left_key.group < right_key.group:
            return -1
        return 1
    if left_key.input_index != right_key.input_index:
        if left_key.input_index < right_key.input_index:
            return -1
        return 1
    if left_key.role != right_key.role:
        if left_key.role < right_key.role:
            return -1
        return 1
    if left_key.group == 100:
        var generic = view_compare(left.id, right.id)
        if generic != 0:
            return generic
    var compared = view_compare(left.id, right.id)
    if compared != 0:
        return compared
    compared = view_compare(left.content_hash, right.content_hash)
    if compared != 0:
        return compared
    if left.byte_len < right.byte_len:
        return -1
    if left.byte_len > right.byte_len:
        return 1
    return view_compare(left.canonical_text, right.canonical_text)

@export("prodex_smart_context_static_item_plan_v1")
def prodex_smart_context_static_item_plan_v1(
    items_address: UInt,
    selected_indices_address: UInt,
    retained_address: UInt,
    selected_count_address: UInt,
    count: Int64,
    maximum_items: Int64,
) abi("C") -> Int64:
    if count < 0 or maximum_items < 0 or maximum_items > count:
        return 1
    if count > 0 and (items_address == 0 or selected_indices_address == 0 or retained_address == 0):
        return 1
    if selected_count_address == 0:
        return 1
    var items = Pointer[mut=False, SmartContextStaticItem, ImmUntrackedOrigin](unsafe_from_address=Int(items_address))
    var selected_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_indices_address))
    var retained = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(retained_address))
    var selected_count = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(selected_count_address))
    selected_count[] = 0
    for index in range(count):
        var item = items[unsafe_offset=index].copy()
        if not rich_view_valid(item.id, 512) or not rich_view_valid(item.content_hash, 512) or not rich_view_valid(item.canonical_text, STATIC_ITEM_MAX_BYTES + 512):
            return 1
        retained[unsafe_offset=index] = Int64(index < maximum_items)
        if index < maximum_items:
            continue
        var largest = Int64(-1)
        for candidate in range(index):
            if retained[unsafe_offset=candidate] == 1 and (largest < 0 or static_item_compare(items[unsafe_offset=candidate].copy(), items[unsafe_offset=largest].copy()) >= 0):
                largest = candidate
        if largest >= 0 and static_item_compare(item, items[unsafe_offset=largest].copy()) < 0:
            retained[unsafe_offset=largest] = 0
            retained[unsafe_offset=index] = 1
    for output_index in range(maximum_items):
        var best = Int64(-1)
        for index in range(count):
            if retained[unsafe_offset=index] != 1:
                continue
            var already_output = False
            for previous in range(output_index):
                if selected_indices[unsafe_offset=previous] == index:
                    already_output = True
                    break
            if not already_output and (best < 0 or static_item_compare(items[unsafe_offset=index].copy(), items[unsafe_offset=best].copy()) < 0):
                best = index
        if best < 0:
            break
        selected_indices[unsafe_offset=output_index] = best
        selected_count[] += 1
    return 0

def saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right

def saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return 0
    if left > UINT64_MAX / right:
        return UINT64_MAX
    return left * right

def saturating_sub(left: UInt64, right: UInt64) -> UInt64:
    if right > left:
        return 0
    return left - right

@export("prodex_smart_context_regression_plan_v1")
def prodex_smart_context_regression_plan_v1(
    exactness_required: Int64,
    payload_changed: Int64,
    before_tokens: UInt64,
    after_tokens: UInt64,
    tokenizer_counted: Int64,
    future_retrieval_overhead_tokens: UInt64,
    injected_protocol_overhead_tokens: UInt64,
    expected_recovery_overhead_tokens: UInt64,
    before_critical_signal_count: UInt64,
    after_critical_signal_count: UInt64,
    missing_rehydrate_refs: Int64,
    unresolved_refs_segment_local: Int64,
    decision: Pointer[mut=True, Int64, _],
    reason_bits: Pointer[mut=True, UInt64, _],
    saved_tokens: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if exactness_required < 0 or exactness_required > 1 or payload_changed < 0 or payload_changed > 1 or tokenizer_counted < 0 or tokenizer_counted > 1 or missing_rehydrate_refs < 0 or missing_rehydrate_refs > 1 or unresolved_refs_segment_local < 0 or unresolved_refs_segment_local > 1:
        return 1
    var overhead = saturating_add(
        saturating_add(
            future_retrieval_overhead_tokens,
            injected_protocol_overhead_tokens,
        ),
        expected_recovery_overhead_tokens,
    )
    var net_saved = saturating_sub(
        saturating_sub(before_tokens, after_tokens), overhead
    )
    var required_saved = saturating_add(saturating_mul(before_tokens, 3), 99) / 100
    if required_saved < 128:
        required_saved = 128
    var reasons = UInt64(0)
    if exactness_required == 1 and payload_changed == 1:
        reasons |= 1
    if payload_changed == 1 and tokenizer_counted == 0:
        reasons |= 2
    if payload_changed == 1 and after_tokens >= before_tokens:
        reasons |= 4
    elif payload_changed == 1 and net_saved < required_saved:
        reasons |= 8
    if after_critical_signal_count < before_critical_signal_count:
        reasons |= 16
    if missing_rehydrate_refs == 1 and unresolved_refs_segment_local == 0:
        reasons |= 32
    if before_tokens > 0 and after_tokens == 0:
        reasons |= 64
    decision[] = Int64(reasons != 0)
    reason_bits[] = reasons
    saved_tokens[] = net_saved
    return 0


comptime SMART_CONTEXT_FALLBACK_REASON_NONE: Int64 = 0
comptime SMART_CONTEXT_FALLBACK_REASON_CRITICAL_SIGNAL: Int64 = 1
comptime SMART_CONTEXT_FALLBACK_REASON_MISSING_REHYDRATE_REFS: Int64 = 2
comptime SMART_CONTEXT_FALLBACK_REASON_EXACTNESS_REQUIRED: Int64 = 3
comptime SMART_CONTEXT_FALLBACK_REASON_EMPTY_AFTER_PAYLOAD: Int64 = 4
comptime SMART_CONTEXT_FALLBACK_REASON_TOKEN_SAVINGS: Int64 = 5
comptime SMART_CONTEXT_FALLBACK_REASON_UNSUPPORTED_TOKENIZER: Int64 = 6
comptime SMART_CONTEXT_FALLBACK_REASON_TOKEN_BUDGET: Int64 = 7

@export("prodex_smart_context_rewrite_validation_reason_v1")
def prodex_smart_context_rewrite_validation_reason_v1(
    critical_signal_loss: Int64,
    fallback_exact: Int64,
    reason_bits: UInt64,
    rehydrated_refs: UInt64,
    tool_outputs_condensed: UInt64,
    duplicate_texts: UInt64,
    cross_turn_duplicate_texts: UInt64,
    repeat_tool_output_refs: UInt64,
    static_context_deltas: UInt64,
) abi("C") -> Int64:
    if (
        critical_signal_loss < 0
        or critical_signal_loss > 1
        or fallback_exact < 0
        or fallback_exact > 1
        or reason_bits > 127
    ):
        return -1
    if critical_signal_loss == 1:
        return SMART_CONTEXT_FALLBACK_REASON_CRITICAL_SIGNAL
    var rehydrate_only = (
        rehydrated_refs > 0
        and tool_outputs_condensed == 0
        and duplicate_texts == 0
        and cross_turn_duplicate_texts == 0
        and repeat_tool_output_refs == 0
        and static_context_deltas == 0
    )
    if rehydrate_only or fallback_exact == 0:
        return SMART_CONTEXT_FALLBACK_REASON_NONE
    if reason_bits & UInt64(16):
        return SMART_CONTEXT_FALLBACK_REASON_CRITICAL_SIGNAL
    if reason_bits & UInt64(32):
        return SMART_CONTEXT_FALLBACK_REASON_MISSING_REHYDRATE_REFS
    if reason_bits & UInt64(1):
        return SMART_CONTEXT_FALLBACK_REASON_EXACTNESS_REQUIRED
    if reason_bits & UInt64(64):
        return SMART_CONTEXT_FALLBACK_REASON_EMPTY_AFTER_PAYLOAD
    if reason_bits & UInt64(8):
        return SMART_CONTEXT_FALLBACK_REASON_TOKEN_SAVINGS
    if reason_bits & UInt64(2):
        return SMART_CONTEXT_FALLBACK_REASON_UNSUPPORTED_TOKENIZER
    return SMART_CONTEXT_FALLBACK_REASON_TOKEN_BUDGET

@export("prodex_smart_context_fingerprint_delta_plan_v1")
def prodex_smart_context_fingerprint_delta_plan_v1(
    previous_keys_address: UInt,
    previous_hashes_address: UInt,
    current_keys_address: UInt,
    current_hashes_address: UInt,
    action_address: UInt,
    previous_index_address: UInt,
    current_index_address: UInt,
    previous_count: Int64,
    current_count: Int64,
    key_count: Int64,
) abi("C") -> Int64:
    if previous_count < 0 or current_count < 0 or key_count < 0:
        return 1
    if previous_count > 0 and (previous_keys_address == 0 or previous_hashes_address == 0):
        return 1
    if current_count > 0 and (current_keys_address == 0 or current_hashes_address == 0):
        return 1
    if key_count > 0 and (action_address == 0 or previous_index_address == 0 or current_index_address == 0):
        return 1
    var previous_keys = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(previous_keys_address))
    var previous_hashes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(previous_hashes_address))
    var current_keys = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(current_keys_address))
    var current_hashes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(current_hashes_address))
    var actions = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(action_address))
    var previous_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(previous_index_address))
    var current_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(current_index_address))
    for key in range(key_count):
        var previous_index = Int64(-1)
        var current_index = Int64(-1)
        for index in range(previous_count):
            if previous_keys[unsafe_offset=index] == UInt64(key):
                previous_index = index
        for index in range(current_count):
            if current_keys[unsafe_offset=index] == UInt64(key):
                current_index = index
        previous_indices[unsafe_offset=key] = previous_index
        current_indices[unsafe_offset=key] = current_index
        if previous_index < 0:
            if current_index < 0:
                return 1
            actions[unsafe_offset=key] = 0
        elif current_index < 0:
            actions[unsafe_offset=key] = 1
        elif previous_hashes[unsafe_offset=previous_index] == current_hashes[unsafe_offset=current_index]:
            actions[unsafe_offset=key] = 2
        else:
            actions[unsafe_offset=key] = 3
    return 0

@export("prodex_smart_context_affinity_rewrite_allowed_v1")
def prodex_smart_context_affinity_rewrite_allowed_v1(
    exactness_required: Int64,
    exactness_reason_bits: UInt64,
    policy_reason_bits: UInt64,
) abi("C") -> Int64:
    if exactness_required < 0 or exactness_required > 1 or exactness_reason_bits > 31 or policy_reason_bits > 1023:
        return -1
    var affinity_reasons = exactness_reason_bits & UInt64(14)
    var non_affinity_exactness = exactness_reason_bits & UInt64(17)
    var safety_blocks = policy_reason_bits & UInt64(26)
    return Int64(
        exactness_required == 1
        and affinity_reasons != 0
        and non_affinity_exactness == 0
        and safety_blocks == 0
    )

@export("prodex_smart_context_rollout_plan_v1")
def prodex_smart_context_rollout_plan_v1(
    enabled: Int64,
    explicit_exact_mode: Int64,
    shadow_mode: Int64,
    canary_percent_input: Int64,
    canary_bucket: Int64,
    mode: Pointer[mut=True, Int64, _],
    canary_percent: Pointer[mut=True, Int64, _],
    reason: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if enabled < 0 or enabled > 1 or explicit_exact_mode < 0 or explicit_exact_mode > 1 or shadow_mode < 0 or shadow_mode > 1 or canary_percent_input < 0 or canary_percent_input > 255 or canary_bucket < 0 or canary_bucket >= 10000:
        return 1
    var percent = canary_percent_input
    if percent > 100:
        percent = 100
    canary_percent[] = percent
    if enabled == 0:
        mode[] = 2
        reason[] = 0
    elif explicit_exact_mode == 1:
        mode[] = 2
        reason[] = 1
    elif shadow_mode == 1:
        mode[] = 1
        reason[] = 2
    elif percent < 100 and canary_bucket >= percent * 100:
        mode[] = 2
        reason[] = 3
    else:
        mode[] = 0
        if percent == 100:
            reason[] = 4
        else:
            reason[] = 5
    return 0

# Request admission and rewrite telemetry stay in this policy kernel so the
# Rust caller only acquires JSON/headers and applies the returned plan.
comptime SMART_CONTEXT_REQUEST_PLAN_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_REQUEST_PLAN_OK: Int64 = 0
comptime SMART_CONTEXT_REQUEST_PLAN_INVALID: Int64 = 1
comptime SMART_CONTEXT_REQUEST_PLAN_ABI: Int64 = 4
comptime SMART_CONTEXT_REQUEST_PLAN_HTTP: Int64 = 0
comptime SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET: Int64 = 1
comptime SMART_CONTEXT_REQUEST_PLAN_MIN_BODY_BYTES: UInt64 = 512
comptime SMART_CONTEXT_REQUEST_PLAN_HTTP_MAX_BYTES: UInt64 = 256 * 1024
comptime SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET_MAX_BYTES: UInt64 = 96 * 1024

# Stable body-admission reason tags. Zero means the request may continue.
comptime SMART_CONTEXT_BODY_REASON_ELIGIBLE: Int64 = 0
comptime SMART_CONTEXT_BODY_REASON_UNSUPPORTED_ROUTE: Int64 = 1
comptime SMART_CONTEXT_BODY_REASON_UNSUPPORTED_CONTENT_TYPE: Int64 = 2
comptime SMART_CONTEXT_BODY_REASON_BELOW_MINIMUM: Int64 = 3
comptime SMART_CONTEXT_BODY_REASON_WEBSOCKET_GENERATE_FALSE: Int64 = 4
comptime SMART_CONTEXT_BODY_REASON_WEBSOCKET_LARGE: Int64 = 5
comptime SMART_CONTEXT_BODY_REASON_HTTP_LARGE: Int64 = 6

comptime SMART_CONTEXT_BODY_SHAPE_REASON_ELIGIBLE: Int64 = 0
comptime SMART_CONTEXT_BODY_SHAPE_REASON_INVALID_JSON: Int64 = 1
comptime SMART_CONTEXT_BODY_SHAPE_REASON_JSON_DEPTH: Int64 = 2
comptime SMART_CONTEXT_BODY_SHAPE_REASON_JSON_NODE: Int64 = 3
comptime SMART_CONTEXT_BODY_SHAPE_REASON_NO_CANDIDATE: Int64 = 4

def smart_context_request_plan_bool(value: Int64) -> Bool:
    return value == 0 or value == 1

@export("prodex_smart_context_body_admission_plan_v1")
def prodex_smart_context_body_admission_plan_v1(
    abi_version: Int64,
    body_bytes: UInt64,
    transport: Int64,
    route_supported: Int64,
    content_type_supported: Int64,
    marker_present: Int64,
    static_context_required: Int64,
    websocket_generate_false: Int64,
    reason_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REQUEST_PLAN_ABI_VERSION:
        return SMART_CONTEXT_REQUEST_PLAN_ABI
    if (
        reason_address == 0
        or (transport != SMART_CONTEXT_REQUEST_PLAN_HTTP and transport != SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET)
        or not smart_context_request_plan_bool(route_supported)
        or not smart_context_request_plan_bool(content_type_supported)
        or not smart_context_request_plan_bool(marker_present)
        or not smart_context_request_plan_bool(static_context_required)
        or not smart_context_request_plan_bool(websocket_generate_false)
    ):
        return SMART_CONTEXT_REQUEST_PLAN_INVALID
    var reason = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(reason_address))
    reason[] = SMART_CONTEXT_BODY_REASON_ELIGIBLE
    if route_supported == 0:
        reason[] = SMART_CONTEXT_BODY_REASON_UNSUPPORTED_ROUTE
    elif content_type_supported == 0:
        reason[] = SMART_CONTEXT_BODY_REASON_UNSUPPORTED_CONTENT_TYPE
    elif body_bytes < SMART_CONTEXT_REQUEST_PLAN_MIN_BODY_BYTES and marker_present == 0 and static_context_required == 0:
        reason[] = SMART_CONTEXT_BODY_REASON_BELOW_MINIMUM
    elif transport == SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET and websocket_generate_false == 1:
        reason[] = SMART_CONTEXT_BODY_REASON_WEBSOCKET_GENERATE_FALSE
    elif transport == SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET and body_bytes > SMART_CONTEXT_REQUEST_PLAN_WEBSOCKET_MAX_BYTES and marker_present == 0:
        reason[] = SMART_CONTEXT_BODY_REASON_WEBSOCKET_LARGE
    elif transport == SMART_CONTEXT_REQUEST_PLAN_HTTP and body_bytes > SMART_CONTEXT_REQUEST_PLAN_HTTP_MAX_BYTES and marker_present == 0:
        reason[] = SMART_CONTEXT_BODY_REASON_HTTP_LARGE
    return SMART_CONTEXT_REQUEST_PLAN_OK

@export("prodex_smart_context_body_shape_plan_v1")
def prodex_smart_context_body_shape_plan_v1(
    abi_version: Int64,
    json_valid: Int64,
    json_shape_valid: Int64,
    json_shape_reason: Int64,
    rewrite_candidate: Int64,
    static_context_changed: Int64,
    reason_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REQUEST_PLAN_ABI_VERSION:
        return SMART_CONTEXT_REQUEST_PLAN_ABI
    if (
        reason_address == 0
        or not smart_context_request_plan_bool(json_valid)
        or not smart_context_request_plan_bool(json_shape_valid)
        or json_shape_reason < 0 or json_shape_reason > 2
        or not smart_context_request_plan_bool(rewrite_candidate)
        or not smart_context_request_plan_bool(static_context_changed)
    ):
        return SMART_CONTEXT_REQUEST_PLAN_INVALID
    var reason = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(reason_address))
    reason[] = SMART_CONTEXT_BODY_SHAPE_REASON_ELIGIBLE
    if json_valid == 0:
        reason[] = SMART_CONTEXT_BODY_SHAPE_REASON_INVALID_JSON
    elif json_shape_valid == 0 and json_shape_reason == 1:
        reason[] = SMART_CONTEXT_BODY_SHAPE_REASON_JSON_DEPTH
    elif json_shape_valid == 0:
        reason[] = SMART_CONTEXT_BODY_SHAPE_REASON_JSON_NODE
    elif rewrite_candidate == 0 and static_context_changed == 0:
        reason[] = SMART_CONTEXT_BODY_SHAPE_REASON_NO_CANDIDATE
    return SMART_CONTEXT_REQUEST_PLAN_OK

comptime SMART_CONTEXT_REWRITE_OUTCOME_OK_REHYDRATE_EXACT: Int64 = 0
comptime SMART_CONTEXT_REWRITE_OUTCOME_OK_SAVED: Int64 = 1
comptime SMART_CONTEXT_REWRITE_OUTCOME_ZERO_SAVINGS: Int64 = 2
comptime SMART_CONTEXT_REWRITE_OUTCOME_GROWTH: Int64 = 3

@export("prodex_smart_context_rewrite_outcome_plan_v1")
def prodex_smart_context_rewrite_outcome_plan_v1(
    abi_version: Int64,
    body_bytes_before: UInt64,
    body_bytes_after: UInt64,
    rehydrated_refs: UInt64,
    tool_outputs_condensed: UInt64,
    duplicate_texts: UInt64,
    static_context_deltas: UInt64,
    outcome_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_REQUEST_PLAN_ABI_VERSION:
        return SMART_CONTEXT_REQUEST_PLAN_ABI
    if outcome_address == 0:
        return SMART_CONTEXT_REQUEST_PLAN_INVALID
    var outcome = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(outcome_address))
    if (
        rehydrated_refs > 0
        and tool_outputs_condensed == 0
        and duplicate_texts == 0
        and static_context_deltas == 0
    ):
        outcome[] = SMART_CONTEXT_REWRITE_OUTCOME_OK_REHYDRATE_EXACT
    elif body_bytes_after < body_bytes_before:
        outcome[] = SMART_CONTEXT_REWRITE_OUTCOME_OK_SAVED
    elif body_bytes_after == body_bytes_before:
        outcome[] = SMART_CONTEXT_REWRITE_OUTCOME_ZERO_SAVINGS
    else:
        outcome[] = SMART_CONTEXT_REWRITE_OUTCOME_GROWTH
    return SMART_CONTEXT_REQUEST_PLAN_OK

comptime SMART_CONTEXT_TELEMETRY_LABEL_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_TELEMETRY_LABEL_MAX_BYTES: Int64 = 1024
comptime SMART_CONTEXT_TELEMETRY_LABEL_PRESSURE: Int64 = 0
comptime SMART_CONTEXT_TELEMETRY_LABEL_CONFIDENCE: Int64 = 1
comptime SMART_CONTEXT_TELEMETRY_LABEL_ROLLOUT: Int64 = 2
comptime SMART_CONTEXT_TELEMETRY_LABEL_BUDGET: Int64 = 3
comptime SMART_CONTEXT_TELEMETRY_LABEL_CATEGORIES: Int64 = 4
comptime SMART_CONTEXT_TELEMETRY_LABEL_EXACTNESS_REASONS: Int64 = 5
comptime SMART_CONTEXT_TELEMETRY_LABEL_POLICY_REASONS: Int64 = 6
comptime SMART_CONTEXT_TELEMETRY_LABEL_OUTCOME: Int64 = 7

def smart_context_telemetry_put_byte(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    value: UInt8,
) -> Bool:
    if written[] < 0 or written[] >= capacity:
        return False
    output[unsafe_offset=written[]] = value
    written[] += 1
    return True

def smart_context_telemetry_put_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    var source = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        if not smart_context_telemetry_put_byte(output, capacity, written, source[unsafe_offset=index]):
            return False
    return True

def smart_context_telemetry_put_list_item(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    first: Pointer[mut=True, Bool, _],
    literal: StringSlice,
) -> Bool:
    if not first[] and not smart_context_telemetry_put_byte(output, capacity, written, 44):
        return False
    first[] = False
    return smart_context_telemetry_put_literal(output, capacity, written, literal)

def smart_context_telemetry_write_label(
    kind: Int64,
    value: UInt64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_PRESSURE:
        if value == 0:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("unknown"))
        elif value == 1:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("low"))
        elif value == 2:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("moderate"))
        elif value == 3:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("high"))
        elif value == 4:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("critical"))
        elif value == 5:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("exhausted"))
        return False
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_CONFIDENCE:
        if value == 0:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("high"))
        elif value == 1:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("medium"))
        elif value == 2:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("low"))
        return False
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_ROLLOUT:
        if value == 0:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("apply"))
        elif value == 1:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("shadow"))
        elif value == 2:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("disabled"))
        return False
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_BUDGET:
        if value == 0:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("exact_pass_through"))
        elif value == 1:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("large_lossless"))
        elif value == 2:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("artifact_condensed"))
        elif value == 3:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("minimal_refs_only"))
        return False
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_OUTCOME:
        if value == 0:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("ok_rehydrate_exact"))
        elif value == 1:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("ok_saved"))
        elif value == 2:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("zero_savings"))
        elif value == 3:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("growth"))
        return False
    var first = True
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_CATEGORIES:
        if value & 1 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("tool_output")):
            return False
        if value & 2 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("tool_argument")):
            return False
        if value & 4 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("duplicate_context")):
            return False
        if value & 8 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("repeat_tool_output")):
            return False
        if value & 16 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("blob_output")):
            return False
        if value & 32 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("rehydration")):
            return False
        if value & 64 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("static_context")):
            return False
        if value & 128 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("repo_state")):
            return False
        if first:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("-"))
        return True
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_EXACTNESS_REASONS:
        if value & 1 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("exact_mode")):
            return False
        if value & 2 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("previous_response")):
            return False
        if value & 4 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("turn_state")):
            return False
        if value & 8 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("session")):
            return False
        if value & 16 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("tool_output_without_artifact")):
            return False
        if first:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("-"))
        return True
    if kind == SMART_CONTEXT_TELEMETRY_LABEL_POLICY_REASONS:
        if value & 1 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("exactness_required")):
            return False
        if value & 2 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("static_context_changed")):
            return False
        if value & 4 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("missing_rehydrate_refs")):
            return False
        if value & 8 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("unknown_token_window")):
            return False
        if value & 16 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("unsafe_accounting")):
            return False
        if value & 32 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("recent_rewrite_savings_safe")):
            return False
        if value & 64 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("plenty_of_budget")):
            return False
        if value & 128 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("moderate_budget")):
            return False
        if value & 256 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("tight_budget")):
            return False
        if value & 512 != 0 and not smart_context_telemetry_put_list_item(output, capacity, written, Pointer(to=first), StringSlice("critical_budget")):
            return False
        if first:
            return smart_context_telemetry_put_literal(output, capacity, written, StringSlice("-"))
        return True
    return False

@export("prodex_smart_context_telemetry_label_v1")
def prodex_smart_context_telemetry_label_v1(
    abi_version: Int64,
    kind: Int64,
    value: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_TELEMETRY_LABEL_ABI_VERSION:
        return SMART_CONTEXT_REQUEST_PLAN_ABI
    if output_capacity < 0 or output_capacity > SMART_CONTEXT_TELEMETRY_LABEL_MAX_BYTES or written_address == 0 or output_capacity > 0 and output_address == 0:
        return SMART_CONTEXT_REQUEST_PLAN_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    if not smart_context_telemetry_write_label(kind, value, output, output_capacity, written):
        if written[] >= output_capacity:
            written[] = 0
            return 3
        return SMART_CONTEXT_REQUEST_PLAN_INVALID
    return SMART_CONTEXT_REQUEST_PLAN_OK
