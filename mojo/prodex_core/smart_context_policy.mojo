from std.memory import Pointer

comptime UINT64_MAX: UInt64 = 18446744073709551615

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
