from std.memory import Pointer

comptime UINT64_MAX: UInt64 = 18446744073709551615
comptime UINT32_MAX: UInt64 = 4294967295
comptime SMART_CONTEXT_TOKEN_ACCOUNTING_MAX_COUNT: Int64 = 256

def smart_context_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right

def smart_context_saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return 0
    if left > UINT64_MAX / right:
        return UINT64_MAX
    return left * right

def smart_context_pressure_band(pressure_has_value: Int64, pressure: UInt64) -> Int64:
    if pressure_has_value == 0:
        return 0
    if pressure >= 10000:
        return 5
    if pressure >= 9000:
        return 4
    if pressure >= 7500:
        return 3
    if pressure >= 5000:
        return 2
    return 1

@export("prodex_smart_context_pressure_snapshot")
def prodex_smart_context_pressure_snapshot(
    model_context_window_tokens: UInt64,
    model_context_window_has_value: Int64,
    reserved_output_tokens: UInt64,
    effective_input_tokens: UInt64,
    effective_input_source: Int64,
    unknown_token_window: Int64,
    zero_context_window: Int64,
    reserved_output_consumes_window: Int64,
    effective_usable_context_tokens: Pointer[mut=True, UInt64, _],
    effective_usable_has_value: Pointer[mut=True, Int64, _],
    pressure_basis_points: Pointer[mut=True, UInt64, _],
    pressure_has_value: Pointer[mut=True, Int64, _],
    pressure_band: Pointer[mut=True, Int64, _],
    absolute_safety_floor_tokens: Pointer[mut=True, UInt64, _],
    estimator_confidence: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if model_context_window_has_value < 0 or model_context_window_has_value > 1:
        return 1
    if effective_input_source < 0 or effective_input_source > 3:
        return 1
    if unknown_token_window < 0 or unknown_token_window > 1:
        return 1
    if zero_context_window < 0 or zero_context_window > 1:
        return 1
    if reserved_output_consumes_window < 0 or reserved_output_consumes_window > 1:
        return 1

    var usable: UInt64 = 0
    var usable_has_value: Int64 = 0
    if model_context_window_has_value == 1:
        usable_has_value = 1
        if reserved_output_tokens <= model_context_window_tokens:
            usable_has_value = 1
            usable = model_context_window_tokens - reserved_output_tokens
        else:
            usable_has_value = 0
            usable = 0

    effective_usable_context_tokens[0] = usable
    effective_usable_has_value[0] = usable_has_value

    var pressure: UInt64 = 0
    var has_pressure: Int64 = 0
    if usable > 0:
        has_pressure = 1
        pressure = smart_context_saturating_mul(effective_input_tokens, 10000) / usable
        if pressure > UINT32_MAX:
            pressure = UINT32_MAX

    pressure_basis_points[0] = pressure
    pressure_has_value[0] = has_pressure
    pressure_band[0] = smart_context_pressure_band(has_pressure, pressure)

    var floor: UInt64 = 2000
    if model_context_window_has_value == 1:
        floor = usable / 20
        if floor < 1000:
            floor = 1000
        if floor > 8000:
            floor = 8000
    absolute_safety_floor_tokens[0] = floor

    var confidence: Int64 = 2
    if unknown_token_window == 0 and zero_context_window == 0 and reserved_output_consumes_window == 0:
        if effective_input_source == 0 or effective_input_source == 2:
            confidence = 0
        elif effective_input_source == 1:
            confidence = 1
    estimator_confidence[0] = confidence
    return 0

@export("prodex_smart_context_token_usage_summary_batch")
def prodex_smart_context_token_usage_summary_batch(
    input_tokens_address: UInt,
    cached_input_tokens_address: UInt,
    output_tokens_address: UInt,
    reasoning_tokens_address: UInt,
    observed_input_tokens_address: UInt,
    observed_cached_input_tokens_address: UInt,
    observed_output_tokens_address: UInt,
    observed_reasoning_tokens_address: UInt,
    last_input_tokens_address: UInt,
    last_accounted_input_tokens_address: UInt,
    last_observed_context_tokens_address: UInt,
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > SMART_CONTEXT_TOKEN_ACCOUNTING_MAX_COUNT:
        return 1
    if count == 0:
        return 0
    if input_tokens_address == 0 or cached_input_tokens_address == 0 or output_tokens_address == 0 or reasoning_tokens_address == 0 or observed_input_tokens_address == 0 or observed_cached_input_tokens_address == 0 or observed_output_tokens_address == 0 or observed_reasoning_tokens_address == 0 or last_input_tokens_address == 0 or last_accounted_input_tokens_address == 0 or last_observed_context_tokens_address == 0:
        return 1

    var input_tokens = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(input_tokens_address))
    var cached_input_tokens = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(cached_input_tokens_address))
    var output_tokens = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(output_tokens_address))
    var reasoning_tokens = Pointer[mut=False, UInt64, ImmUntrackedOrigin](unsafe_from_address=Int(reasoning_tokens_address))
    var observed_input_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(observed_input_tokens_address))
    var observed_cached_input_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(observed_cached_input_tokens_address))
    var observed_output_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(observed_output_tokens_address))
    var observed_reasoning_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(observed_reasoning_tokens_address))
    var last_input_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(last_input_tokens_address))
    var last_accounted_input_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(last_accounted_input_tokens_address))
    var last_observed_context_tokens = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(last_observed_context_tokens_address))

    var total_input: UInt64 = 0
    var total_cached_input: UInt64 = 0
    var total_output: UInt64 = 0
    var total_reasoning: UInt64 = 0
    var last_input: UInt64 = 0
    var last_accounted: UInt64 = 0
    var last_context: UInt64 = 0
    for index in range(count):
        var input = input_tokens[unsafe_offset=index]
        var cached_input = cached_input_tokens[unsafe_offset=index]
        var output = output_tokens[unsafe_offset=index]
        var reasoning = reasoning_tokens[unsafe_offset=index]
        total_input = smart_context_saturating_add(total_input, input)
        total_cached_input = smart_context_saturating_add(
            total_cached_input, cached_input
        )
        total_output = smart_context_saturating_add(total_output, output)
        total_reasoning = smart_context_saturating_add(total_reasoning, reasoning)
        last_input = input
        if input == 0:
            last_accounted = cached_input
        else:
            last_accounted = input
        var context = smart_context_saturating_add(input, output)
        context = smart_context_saturating_add(context, reasoning)
        if context == 0:
            last_context = cached_input
        else:
            last_context = context

    observed_input_tokens[unsafe_offset=0] = total_input
    observed_cached_input_tokens[unsafe_offset=0] = total_cached_input
    observed_output_tokens[unsafe_offset=0] = total_output
    observed_reasoning_tokens[unsafe_offset=0] = total_reasoning
    last_input_tokens[unsafe_offset=0] = last_input
    last_accounted_input_tokens[unsafe_offset=0] = last_accounted
    last_observed_context_tokens[unsafe_offset=0] = last_context
    return 0


@export("prodex_smart_context_estimate_tokens_from_body_bytes")
def prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: UInt64) abi("C") -> UInt64:
    if body_bytes > 18446744073709551612:
        return 4611686018427387903
    return (body_bytes + 3) / 4
