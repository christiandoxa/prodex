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
