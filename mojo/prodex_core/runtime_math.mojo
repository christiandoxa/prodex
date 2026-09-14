comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808


def runtime_quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    return left + right


def runtime_quota_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > INT64_MAX / right:
        return INT64_MAX
    return left * right


def runtime_quota_scale_pressure(pressure: Int64, scale_bps: Int64) -> Int64:
    if pressure == INT64_MAX:
        return INT64_MAX
    var scale = scale_bps
    if scale < 0:
        scale = 0
    if pressure < 0:
        return pressure
    if scale == 0 or pressure == 0:
        return 0
    return runtime_quota_saturating_mul(pressure, scale) / 10_000


def runtime_precommit_budget_plan(
    continuation: Int64,
    pressure_mode: Int64,
    standard_attempt_limit: Int64,
    standard_budget_ms: Int64,
    continuation_attempt_limit: Int64,
    continuation_budget_ms: Int64,
    pressure_attempt_limit: Int64,
    pressure_budget_ms: Int64,
    profile_count: Int64,
    attempts_per_profile: Int64,
    attempt_limit_out: Pointer[mut=True, Int64, _],
    budget_ms_out: Pointer[mut=True, Int64, _],
) -> Int64:
    if (
        continuation < 0
        or continuation > 1
        or pressure_mode < 0
        or pressure_mode > 1
        or standard_attempt_limit <= 0
        or standard_budget_ms < 0
        or continuation_attempt_limit <= 0
        or continuation_budget_ms < 0
        or pressure_attempt_limit <= 0
        or pressure_budget_ms < 0
        or profile_count < 0
        or attempts_per_profile <= 0
    ):
        return 1

    var base_attempt_limit = standard_attempt_limit
    var base_budget_ms = standard_budget_ms
    if continuation == 1:
        base_attempt_limit = continuation_attempt_limit
        base_budget_ms = continuation_budget_ms
    elif pressure_mode == 1:
        base_attempt_limit = pressure_attempt_limit
        base_budget_ms = pressure_budget_ms

    var required_profile_attempts = runtime_quota_saturating_mul(
        max(profile_count, 1), attempts_per_profile
    )
    var attempt_limit = max(base_attempt_limit, required_profile_attempts)
    var scaled_budget_ms = runtime_quota_saturating_mul(
        base_budget_ms, attempt_limit
    )
    scaled_budget_ms = runtime_quota_saturating_add(
        scaled_budget_ms, base_attempt_limit - 1
    ) / base_attempt_limit
    attempt_limit_out[unsafe_offset=0] = attempt_limit
    budget_ms_out[unsafe_offset=0] = scaled_budget_ms
    return 0
