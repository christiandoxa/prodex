# Canonical quota-watch cadence and cache-lifetime policy.
# Rust owns persistent state and clock acquisition, not timing decisions.

from std.memory import Pointer

comptime QUOTA_WATCH_ABI_VERSION: Int64 = 1
comptime QUOTA_WATCH_INVALID: Int64 = 1
comptime I64_MAX: Int64 = 9223372036854775807
comptime I64_MIN: Int64 = -9223372036854775807 - 1
comptime U64_MAX: UInt64 = 18446744073709551615

comptime WATCH_FAST_SECONDS: UInt64 = 10
comptime WATCH_IMMINENT_SECONDS: UInt64 = 5
comptime WATCH_STABLE_SECONDS: UInt64 = 45
comptime WATCH_IMMINENT_WINDOW: Int64 = 120
comptime WATCH_NEAR_WINDOW: Int64 = 900


def quota_watch_add_positive(base: Int64, delta: Int64) -> Int64:
    if base > I64_MAX - delta:
        return I64_MAX
    return base + delta


@export("prodex_quota_watch_refresh_v1")
def prodex_quota_watch_refresh_v1(
    abi_version: Int64,
    windows_address: UInt64,
    windows_count: Int64,
    watch: Int64,
    profile_count: UInt64,
    now: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        windows_count < 0
        or (windows_count > 0 and windows_address == 0)
        or (watch != 0 and watch != 1)
        or result_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var imminent = False
    var near = False
    if watch == 0 and windows_count > 0:
        var windows = Pointer[mut=False, Int64, ImmUntrackedOrigin](
            unsafe_from_address=Int(windows_address)
        )
        var imminent_until = quota_watch_add_positive(now, WATCH_IMMINENT_WINDOW)
        var near_until = quota_watch_add_positive(now, WATCH_NEAR_WINDOW)
        for index in range(windows_count):
            var reset = windows[unsafe_offset=index]
            if reset <= imminent_until:
                imminent = True
            if reset <= near_until:
                near = True

    var base = WATCH_STABLE_SECONDS
    if watch == 1 or near:
        base = WATCH_FAST_SECONDS
    if watch == 0 and imminent:
        base = WATCH_IMMINENT_SECONDS

    var proportional = U64_MAX
    if profile_count <= U64_MAX // 2:
        proportional = profile_count * 2
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[] = base if base >= proportional else proportional
    return 0


@export("prodex_quota_watch_cache_alive_until_v1")
def prodex_quota_watch_cache_alive_until_v1(
    abi_version: Int64,
    now: Int64,
    refresh_seconds: UInt64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if result_address == 0:
        return QUOTA_WATCH_INVALID
    var duration: Int64 = I64_MAX // 2
    if refresh_seconds <= UInt64(I64_MAX):
        duration = Int64(refresh_seconds)
    if duration < 1:
        duration = 1
    var alive_until = quota_watch_add_positive(
        quota_watch_add_positive(now, duration), Int64(WATCH_FAST_SECONDS)
    )
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[] = alive_until
    return 0


@export("prodex_quota_watch_cache_remaining_v1")
def prodex_quota_watch_cache_remaining_v1(
    abi_version: Int64,
    alive_until: Int64,
    now: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if result_address == 0:
        return QUOTA_WATCH_INVALID
    var remaining: Int64 = 1
    if now < 0 and alive_until > I64_MAX + now:
        remaining = I64_MAX
    elif now > 0 and alive_until < I64_MIN + now:
        remaining = I64_MIN
    else:
        remaining = alive_until - now
    if remaining < 1:
        remaining = 1
    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[] = UInt64(remaining)
    return 0


@export("prodex_quota_watch_cache_live_v1")
def prodex_quota_watch_cache_live_v1(
    abi_version: Int64,
    alive_until: Int64,
    now: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if result_address == 0:
        return QUOTA_WATCH_INVALID
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[] = 1 if alive_until >= now else 0
    return 0
