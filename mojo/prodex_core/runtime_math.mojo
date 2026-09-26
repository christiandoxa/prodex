comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808
comptime UINT64_MAX: UInt64 = 18446744073709551615


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
    standard_attempt_limit: UInt64,
    standard_budget_ms: UInt64,
    continuation_attempt_limit: UInt64,
    continuation_budget_ms: UInt64,
    pressure_attempt_limit: UInt64,
    pressure_budget_ms: UInt64,
    profile_count: UInt64,
    attempts_per_profile: UInt64,
    attempt_limit_cap: UInt64,
    attempt_limit_out: Pointer[mut=True, UInt64, _],
    budget_ms_out: Pointer[mut=True, UInt64, _],
) -> Int64:
    if (
        continuation < 0
        or continuation > 1
        or pressure_mode < 0
        or pressure_mode > 1
        or standard_attempt_limit == 0
        or continuation_attempt_limit == 0
        or pressure_attempt_limit == 0
        or attempts_per_profile == 0
        or attempt_limit_cap == 0
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

    var required_profile_attempts = UInt128(max(profile_count, UInt64(1))) * UInt128(
        attempts_per_profile
    )
    if required_profile_attempts > UInt128(attempt_limit_cap):
        required_profile_attempts = UInt128(attempt_limit_cap)
    var attempt_limit = max(base_attempt_limit, UInt64(required_profile_attempts))
    if attempt_limit > attempt_limit_cap:
        attempt_limit = attempt_limit_cap

    var scaled_budget_ms = (
        UInt128(base_budget_ms) * UInt128(attempt_limit)
        + UInt128(base_attempt_limit - UInt64(1))
    ) / UInt128(base_attempt_limit)
    if scaled_budget_ms > UInt128(UINT64_MAX):
        scaled_budget_ms = UInt128(UINT64_MAX)
    attempt_limit_out[unsafe_offset=0] = attempt_limit
    budget_ms_out[unsafe_offset=0] = UInt64(scaled_budget_ms)
    return 0
