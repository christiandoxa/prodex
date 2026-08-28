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


@export("prodex_smart_context_exactness_plan_v1")
def prodex_smart_context_exactness_plan_v1(
    exact_mode: Int64,
    previous_response_present: Int64,
    turn_state_present: Int64,
    session_present: Int64,
    tool_output_without_artifact: Int64,
    decision: Pointer[mut=True, Int64, _],
    reason_bits: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if exact_mode < 0 or exact_mode > 1 or previous_response_present < 0 or previous_response_present > 1 or turn_state_present < 0 or turn_state_present > 1 or session_present < 0 or session_present > 1 or tool_output_without_artifact < 0 or tool_output_without_artifact > 1:
        return 1
    var reasons: UInt64 = 0
    if exact_mode == 1:
        reasons = reasons | UInt64(1 << 0)
    if previous_response_present == 1:
        reasons = reasons | UInt64(1 << 1)
    if turn_state_present == 1:
        reasons = reasons | UInt64(1 << 2)
    if session_present == 1:
        reasons = reasons | UInt64(1 << 3)
    if tool_output_without_artifact == 1:
        reasons = reasons | UInt64(1 << 4)
    decision[] = 0
    if reasons != 0:
        decision[] = 1
    reason_bits[] = reasons
    return 0


def smart_context_calibration_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right


def smart_context_calibration_saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return 0
    if left > UINT64_MAX / right:
        return UINT64_MAX
    return left * right


def smart_context_calibrated_estimate(
    body_bytes: UInt64,
    baseline_estimate: UInt64,
    observed_accounted_input: UInt64,
    observed_present: Int64,
) -> UInt64:
    if baseline_estimate == 0:
        return 0
    if observed_present == 0:
        return baseline_estimate

    var raw_estimate = body_bytes / 4
    if body_bytes % 4 != 0:
        raw_estimate += 1
    var raw_floor = smart_context_calibration_saturating_add(raw_estimate, 1) / 2
    if raw_floor < 1:
        raw_floor = 1
    var raw_floor_with_margin = smart_context_calibration_saturating_add(raw_floor, 64)
    var observed_with_margin = smart_context_calibration_saturating_mul(
        observed_accounted_input,
        9,
    )
    observed_with_margin = smart_context_calibration_saturating_add(
        observed_with_margin,
        7,
    ) / 8
    observed_with_margin = smart_context_calibration_saturating_add(
        observed_with_margin,
        64,
    )
    var calibrated = raw_floor_with_margin
    if observed_with_margin > calibrated:
        calibrated = observed_with_margin
    var inflation_limit = smart_context_calibration_saturating_mul(baseline_estimate, 2)
    if calibrated > inflation_limit:
        return baseline_estimate
    return calibrated


@export("prodex_smart_context_calibrated_estimate_batch")
def prodex_smart_context_calibrated_estimate_batch(
    body_bytes_address: UInt,
    baseline_estimate_address: UInt,
    observed_accounted_input_address: UInt,
    observed_present_address: UInt,
    calibrated_estimate_address: UInt,
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64:
        return 1
    if count == 0:
        return 0
    if body_bytes_address == 0 or baseline_estimate_address == 0 or observed_accounted_input_address == 0 or observed_present_address == 0 or calibrated_estimate_address == 0:
        return 1
    var body_bytes = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(body_bytes_address)
    )
    var baseline_estimate = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(baseline_estimate_address)
    )
    var observed_accounted_input = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(observed_accounted_input_address)
    )
    var observed_present = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(observed_present_address)
    )
    var calibrated_estimate = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(calibrated_estimate_address)
    )
    for index in range(count):
        if observed_present[unsafe_offset=index] < 0 or observed_present[unsafe_offset=index] > 1:
            return 1
        calibrated_estimate[unsafe_offset=index] = smart_context_calibrated_estimate(
            body_bytes[unsafe_offset=index],
            baseline_estimate[unsafe_offset=index],
            observed_accounted_input[unsafe_offset=index],
            observed_present[unsafe_offset=index],
        )
    return 0


def smart_context_budget_scale_ceil(
    value: UInt64, numerator: UInt64, denominator: UInt64
) -> UInt64:
    if value == 0 or value == UINT64_MAX or denominator == 0:
        return value
    var scaled = smart_context_calibration_saturating_mul(value, numerator)
    scaled = smart_context_calibration_saturating_add(scaled, denominator - 1)
    return scaled / denominator


def smart_context_budget_scale_floor(
    value: UInt64, numerator: UInt64, denominator: UInt64
) -> UInt64:
    if value == 0 or value == UINT64_MAX or denominator == 0:
        return value
    return smart_context_calibration_saturating_mul(value, numerator) / denominator


def smart_context_recent_rewrite_decision(
    safe_rewrites: UInt64, fallback_rewrites: UInt64, saved_tokens: UInt64
) -> Int64:
    if fallback_rewrites > 0:
        return 2
    if safe_rewrites == 0:
        return 0
    var required = smart_context_calibration_saturating_mul(safe_rewrites, 256)
    if saved_tokens >= required:
        return 1
    return 2


def smart_context_adaptive_budget_flags(
    available_has_value: Int64,
    exactness_required: Int64,
    static_context_changed: Int64,
    missing_rehydrate_refs: Int64,
    unknown_token_window: Int64,
    unsafe_accounting: Int64,
) -> UInt64:
    var reasons: UInt64 = 0
    if exactness_required == 1:
        reasons = reasons | UInt64(1 << 0)
    if static_context_changed == 1:
        reasons = reasons | UInt64(1 << 1)
    if missing_rehydrate_refs == 1:
        reasons = reasons | UInt64(1 << 2)
    if available_has_value == 0 or unknown_token_window == 1:
        reasons = reasons | UInt64(1 << 3)
    if unsafe_accounting == 1:
        reasons = reasons | UInt64(1 << 4)
    return reasons


@export("prodex_smart_context_adaptive_budget_plan_v1")
def prodex_smart_context_adaptive_budget_plan_v1(
    available_context_tokens: UInt64,
    available_has_value: Int64,
    exactness_required: Int64,
    static_context_changed: Int64,
    missing_rehydrate_refs: Int64,
    unknown_token_window: Int64,
    unsafe_accounting: Int64,
    safe_rewrites: UInt64,
    fallback_rewrites: UInt64,
    saved_tokens: UInt64,
    tier: Pointer[mut=True, Int64, _],
    mode: Pointer[mut=True, Int64, _],
    max_inline_bytes: Pointer[mut=True, UInt64, _],
    max_rehydrate_tokens: Pointer[mut=True, UInt64, _],
    reason_bits: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if available_has_value < 0 or available_has_value > 1 or exactness_required < 0 or exactness_required > 1 or static_context_changed < 0 or static_context_changed > 1 or missing_rehydrate_refs < 0 or missing_rehydrate_refs > 1 or unknown_token_window < 0 or unknown_token_window > 1 or unsafe_accounting < 0 or unsafe_accounting > 1:
        return 1

    var reasons = smart_context_adaptive_budget_flags(
        available_has_value,
        exactness_required,
        static_context_changed,
        missing_rehydrate_refs,
        unknown_token_window,
        unsafe_accounting,
    )
    var selected_tier: Int64 = 0
    if available_has_value == 1:
        if available_context_tokens >= 16000:
            selected_tier = 0
        elif available_context_tokens >= 8000:
            selected_tier = 1
        elif available_context_tokens >= 2000:
            selected_tier = 2
        else:
            selected_tier = 3

    var exact_reasons = UInt64(1 << 0) | UInt64(1 << 1) | UInt64(1 << 3) | UInt64(1 << 4)
    if reasons & exact_reasons != 0:
        tier[] = selected_tier
        mode[] = 0
        max_inline_bytes[] = UINT64_MAX
        if available_has_value == 1:
            max_rehydrate_tokens[] = available_context_tokens
        else:
            max_rehydrate_tokens[] = UINT64_MAX
        reason_bits[] = reasons
        return 0

    var selected_mode: Int64 = 0
    var inline_budget: UInt64 = 0
    var rehydrate_budget: UInt64 = 0
    if selected_tier == 0:
        selected_mode = 0
        inline_budget = UINT64_MAX
        if available_has_value == 1:
            rehydrate_budget = available_context_tokens
        else:
            rehydrate_budget = UINT64_MAX
        reasons = reasons | UInt64(1 << 6)
    elif selected_tier == 1:
        selected_mode = 1
        inline_budget = 32768
        rehydrate_budget = 12000
        reasons = reasons | UInt64(1 << 7)
    elif selected_tier == 2:
        selected_mode = 2
        inline_budget = 8192
        rehydrate_budget = 4000
        reasons = reasons | UInt64(1 << 8)
    else:
        selected_mode = 3
        inline_budget = 1024
        rehydrate_budget = 1000
        reasons = reasons | UInt64(1 << 9)

    var recent_decision = smart_context_recent_rewrite_decision(
        safe_rewrites,
        fallback_rewrites,
        saved_tokens,
    )
    if selected_tier == 1 and recent_decision == 1:
        reasons = reasons | UInt64(1 << 5)
        if missing_rehydrate_refs == 0:
            inline_budget = 65536
    if missing_rehydrate_refs == 1:
        selected_mode = 2
        if inline_budget > 8192:
            inline_budget = 8192
        if rehydrate_budget > 4000:
            rehydrate_budget = 4000
    if recent_decision == 1:
        if selected_tier == 1:
            if inline_budget < 65536:
                inline_budget = smart_context_calibration_saturating_mul(inline_budget, 2)
                if inline_budget > 65536:
                    inline_budget = 65536
        else:
            inline_budget = smart_context_budget_scale_ceil(inline_budget, 5, 4)
        rehydrate_budget = smart_context_budget_scale_ceil(rehydrate_budget, 5, 4)
    elif recent_decision == 2:
        if inline_budget > 256:
            inline_budget = smart_context_budget_scale_floor(inline_budget, 9, 10)
            if inline_budget < 256:
                inline_budget = 256
        if rehydrate_budget > 1:
            rehydrate_budget = smart_context_budget_scale_floor(rehydrate_budget, 9, 10)
            if rehydrate_budget < 1:
                rehydrate_budget = 1
    if available_has_value == 1 and rehydrate_budget > available_context_tokens:
        rehydrate_budget = available_context_tokens

    tier[] = selected_tier
    mode[] = selected_mode
    max_inline_bytes[] = inline_budget
    max_rehydrate_tokens[] = rehydrate_budget
    reason_bits[] = reasons
    return 0


@export("prodex_smart_context_budget_adjustment_v1")
def prodex_smart_context_budget_adjustment_v1(
    tier: Int64,
    mode: Int64,
    max_inline_bytes: UInt64,
    max_rehydrate_tokens: UInt64,
    decision: Int64,
    available_context_tokens: UInt64,
    available_has_value: Int64,
    adjusted_inline_bytes: Pointer[mut=True, UInt64, _],
    adjusted_rehydrate_tokens: Pointer[mut=True, UInt64, _],
) abi("C") -> Int64:
    if tier < 0 or tier > 3 or mode < 0 or mode > 3 or decision < 0 or decision > 2:
        return 1
    if available_has_value < 0 or available_has_value > 1:
        return 1
    var inline_budget = max_inline_bytes
    var rehydrate_budget = max_rehydrate_tokens
    if mode != 0:
        if decision == 1:
            if inline_budget != 0 and inline_budget != UINT64_MAX:
                if tier == 1:
                    if inline_budget < 65536:
                        inline_budget = smart_context_calibration_saturating_mul(inline_budget, 2)
                        if inline_budget > 65536:
                            inline_budget = 65536
                else:
                    inline_budget = smart_context_budget_scale_ceil(inline_budget, 5, 4)
            rehydrate_budget = smart_context_budget_scale_ceil(rehydrate_budget, 5, 4)
        elif decision == 2:
            if inline_budget > 256:
                inline_budget = smart_context_budget_scale_floor(inline_budget, 9, 10)
                if inline_budget < 256:
                    inline_budget = 256
            if rehydrate_budget > 1:
                rehydrate_budget = smart_context_budget_scale_floor(rehydrate_budget, 9, 10)
                if rehydrate_budget < 1:
                    rehydrate_budget = 1
        if available_has_value == 1 and rehydrate_budget > available_context_tokens:
            rehydrate_budget = available_context_tokens

    adjusted_inline_bytes[] = inline_budget
    adjusted_rehydrate_tokens[] = rehydrate_budget
    return 0


def smart_context_telemetry_safe_saved(
    body_bytes_before: UInt64,
    body_bytes_after: UInt64,
    tokens_before: UInt64,
    tokens_after: UInt64,
    token_count_source: Int64,
    safe: Int64,
    quality_risk: Int64,
) -> Bool:
    return safe == 1 and token_count_source == 1 and quality_risk == 0 and tokens_after < tokens_before and body_bytes_after < body_bytes_before


def smart_context_telemetry_ratio(
    body_bytes_before: UInt64, body_bytes_after: UInt64
) -> UInt64:
    if body_bytes_before == 0:
        return 100
    var scaled = smart_context_calibration_saturating_mul(body_bytes_after, 100)
    return scaled / body_bytes_before


@export("prodex_smart_context_rewrite_telemetry_decision_v1")
def prodex_smart_context_rewrite_telemetry_decision_v1(
    body_bytes_before_address: UInt,
    body_bytes_after_address: UInt,
    tokens_before_address: UInt,
    tokens_after_address: UInt,
    token_count_source_address: UInt,
    safe_address: UInt,
    fallback_address: UInt,
    quality_risk_address: UInt,
    recent_safe_rewrites: UInt64,
    recent_fallback_rewrites: UInt64,
    recent_saved_tokens: UInt64,
    decision: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > 64:
        return 1
    if count > 0 and (body_bytes_before_address == 0 or body_bytes_after_address == 0 or tokens_before_address == 0 or tokens_after_address == 0 or token_count_source_address == 0 or safe_address == 0 or fallback_address == 0 or quality_risk_address == 0):
        return 1
    var telemetry_count = count
    if telemetry_count > 4:
        telemetry_count = 4
    var body_bytes_before = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(body_bytes_before_address)
    )
    var body_bytes_after = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(body_bytes_after_address)
    )
    var tokens_before = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(tokens_before_address)
    )
    var tokens_after = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(tokens_after_address)
    )
    var token_count_source = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(token_count_source_address)
    )
    var safe = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(safe_address)
    )
    var fallback = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fallback_address)
    )
    var quality_risk = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(quality_risk_address)
    )
    var has_unsafe = False
    var saved_tokens: UInt64 = 0
    var ratio_total: UInt64 = 0
    for index in range(telemetry_count):
        if token_count_source[unsafe_offset=index] < 0 or token_count_source[unsafe_offset=index] > 1 or safe[unsafe_offset=index] < 0 or safe[unsafe_offset=index] > 1 or fallback[unsafe_offset=index] < 0 or fallback[unsafe_offset=index] > 1 or quality_risk[unsafe_offset=index] < 0 or quality_risk[unsafe_offset=index] > 1:
            return 1
        if fallback[unsafe_offset=index] == 1 or not smart_context_telemetry_safe_saved(
            body_bytes_before[unsafe_offset=index],
            body_bytes_after[unsafe_offset=index],
            tokens_before[unsafe_offset=index],
            tokens_after[unsafe_offset=index],
            token_count_source[unsafe_offset=index],
            safe[unsafe_offset=index],
            quality_risk[unsafe_offset=index],
        ):
            has_unsafe = True
        var saved = UInt64(0)
        if tokens_before[unsafe_offset=index] > tokens_after[unsafe_offset=index]:
            saved = tokens_before[unsafe_offset=index] - tokens_after[unsafe_offset=index]
        saved_tokens = smart_context_calibration_saturating_add(saved_tokens, saved)
        ratio_total = smart_context_calibration_saturating_add(
            ratio_total,
            smart_context_telemetry_ratio(
                body_bytes_before[unsafe_offset=index],
                body_bytes_after[unsafe_offset=index],
            ),
        )

    var selected_decision = smart_context_recent_rewrite_decision(
        recent_safe_rewrites,
        recent_fallback_rewrites,
        recent_saved_tokens,
    )
    if telemetry_count == 0:
        decision[] = selected_decision
        return 0
    if has_unsafe or telemetry_count < 2:
        decision[] = 2
        if telemetry_count < 2 and not has_unsafe:
            decision[] = selected_decision
        return 0
    var average_ratio = ratio_total / UInt64(telemetry_count)
    var required_saved = smart_context_calibration_saturating_mul(UInt64(telemetry_count), 256)
    if saved_tokens >= required_saved and average_ratio <= 70:
        selected_decision = 1
    elif saved_tokens < required_saved or average_ratio >= 85:
        selected_decision = 2
    else:
        selected_decision = 0
    decision[] = selected_decision
    return 0
