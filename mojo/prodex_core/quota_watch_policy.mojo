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
comptime WATCH_LIVE_SECONDS: UInt64 = 5
comptime WATCH_MAX_ROWS: Int64 = 1_048_576

comptime WATCH_ACTION_UP: Int64 = 0
comptime WATCH_ACTION_DOWN: Int64 = 1
comptime WATCH_ACTION_SORT: Int64 = 2
comptime WATCH_ACTION_FILTER: Int64 = 3
comptime WATCH_ACTION_UPDATE: Int64 = 4
comptime WATCH_ACTION_QUIT: Int64 = 5

comptime WATCH_OUTCOME_CONTINUE: Int64 = 0
comptime WATCH_OUTCOME_SORT: Int64 = 1
comptime WATCH_OUTCOME_FILTER: Int64 = 2
comptime WATCH_OUTCOME_UPDATE: Int64 = 3
comptime WATCH_OUTCOME_QUIT: Int64 = 4

comptime WATCH_FILTER_ALL: Int64 = 0
comptime WATCH_FILTER_OPENAI: Int64 = 1
comptime WATCH_FILTER_GEMINI: Int64 = 2
comptime WATCH_FILTER_ANTHROPIC: Int64 = 3
comptime WATCH_FILTER_COPILOT: Int64 = 4
comptime WATCH_FILTER_KIRO: Int64 = 5
comptime WATCH_FILTER_DEEPSEEK: Int64 = 6
comptime WATCH_FILTER_LOCAL: Int64 = 7
comptime WATCH_FILTER_AGY: Int64 = 8

comptime WATCH_SNAPSHOT_OPENAI: Int64 = 0
comptime WATCH_SNAPSHOT_GEMINI: Int64 = 1
comptime WATCH_SNAPSHOT_COPILOT: Int64 = 2
comptime WATCH_SNAPSHOT_EXTERNAL: Int64 = 3
comptime WATCH_SNAPSHOT_NONE: Int64 = 4

comptime WATCH_RESET_NORMAL: Int64 = 0
comptime WATCH_RESET_ERROR: Int64 = 1
comptime WATCH_RESET_CREDITS: Int64 = 2

comptime WATCH_SNAPSHOT_USE_NEXT: Int64 = 0
comptime WATCH_SNAPSHOT_KEEP_PREVIOUS: Int64 = 1
comptime WATCH_SNAPSHOT_MERGE_REPORTS: Int64 = 2


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


def quota_watch_read_i64(address: UInt64, offset: Int64) -> Int64:
    var source = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return source[unsafe_offset=offset]


def quota_watch_write_i64(address: UInt64, offset: Int64, value: Int64):
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    output[unsafe_offset=offset] = value


def quota_watch_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def quota_watch_text_contains(
    address: UInt64,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    if length <= 0 or address == 0:
        return False
    var expected_length = Int64(expected.byte_length())
    if expected_length <= 0 or expected_length > length:
        return expected_length == 0
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = expected.unsafe_ptr()
    for start in range(length - expected_length + 1):
        var matched = True
        for offset in range(expected_length):
            if quota_watch_ascii_lower(source[unsafe_offset=start + offset]) != quota_watch_ascii_lower(
                target[unsafe_offset=offset]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def quota_watch_text_starts_at(
    address: UInt64,
    length: Int64,
    start: Int64,
    expected: StringSlice,
) -> Bool:
    if start < 0 or start > length:
        return False
    var expected_length = Int64(expected.byte_length())
    if expected_length > length - start:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = expected.unsafe_ptr()
    for offset in range(expected_length):
        if quota_watch_ascii_lower(source[unsafe_offset=start + offset]) != quota_watch_ascii_lower(
            target[unsafe_offset=offset]
        ):
            return False
    return True


def quota_watch_text_equal(
    address: UInt64,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    var expected_length = Int64(expected.byte_length())
    if length != expected_length or (length > 0 and address == 0):
        return False
    return quota_watch_text_starts_at(address, length, 0, expected)


def quota_watch_text_starts_at_exact(
    address: UInt64,
    length: Int64,
    start: Int64,
    expected: StringSlice,
) -> Bool:
    if start < 0 or start > length:
        return False
    var expected_length = Int64(expected.byte_length())
    if expected_length > length - start:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var target = expected.unsafe_ptr()
    for offset in range(expected_length):
        if source[unsafe_offset=start + offset] != target[unsafe_offset=offset]:
            return False
    return True


def quota_watch_text_contains_exact(
    address: UInt64,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    var expected_length = Int64(expected.byte_length())
    if expected_length <= 0 or expected_length > length or address == 0:
        return False
    for start in range(length - expected_length + 1):
        if quota_watch_text_starts_at_exact(address, length, start, expected):
            return True
    return False


@export("prodex_quota_watch_live_refresh_v1")
def prodex_quota_watch_live_refresh_v1(
    abi_version: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if result_address == 0:
        return QUOTA_WATCH_INVALID
    quota_watch_write_i64(result_address, 0, Int64(WATCH_LIVE_SECONDS))
    return 0


@export("prodex_quota_watch_cache_eligible_v1")
def prodex_quota_watch_cache_eligible_v1(
    abi_version: Int64,
    detail: Int64,
    auth_filter: Int64,
    provider_filter: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        (detail != 0 and detail != 1)
        or auth_filter < 0
        or auth_filter > 3
        or provider_filter < WATCH_FILTER_ALL
        or provider_filter > WATCH_FILTER_AGY
        or result_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var eligible = detail == 1 and auth_filter == 0 and (
        provider_filter == WATCH_FILTER_ALL or provider_filter == WATCH_FILTER_OPENAI
    )
    quota_watch_write_i64(result_address, 0, Int64(1) if eligible else Int64(0))
    return 0


@export("prodex_quota_watch_filter_next_v1")
def prodex_quota_watch_filter_next_v1(
    abi_version: Int64,
    filter_kind: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if filter_kind < WATCH_FILTER_ALL or filter_kind > WATCH_FILTER_AGY or result_address == 0:
        return QUOTA_WATCH_INVALID
    quota_watch_write_i64(result_address, 0, (filter_kind + 1) % (WATCH_FILTER_AGY + 1))
    return 0


def quota_watch_visible_for_start(
    rows_address: UInt64,
    rows_count: Int64,
    max_lines: Int64,
    start: Int64,
) -> Int64:
    if max_lines < 0:
        return rows_count - start
    var shown: Int64 = 0
    var remaining = max_lines - 1
    if remaining < 0:
        remaining = 0
    for index in range(start, rows_count):
        var row_lines = quota_watch_read_i64(rows_address, index)
        var row_cost = row_lines + (Int64(1) if shown > 0 else Int64(0))
        if row_cost > remaining:
            break
        remaining -= row_cost
        shown += 1
    return shown


@export("prodex_quota_watch_window_v1")
def prodex_quota_watch_window_v1(
    abi_version: Int64,
    rows_address: UInt64,
    rows_count: Int64,
    max_lines: Int64,
    requested_start: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        rows_count < 0
        or rows_count > WATCH_MAX_ROWS
        or (rows_count > 0 and rows_address == 0)
        or max_lines < -1
        or requested_start < 0
        or output_address == 0
    ):
        return QUOTA_WATCH_INVALID
    for index in range(rows_count):
        if quota_watch_read_i64(rows_address, index) < 0:
            return QUOTA_WATCH_INVALID

    var start = requested_start
    if start > rows_count:
        start = rows_count
    var shown = quota_watch_visible_for_start(rows_address, rows_count, max_lines, start)
    var max_scroll: Int64 = 0
    if rows_count > 0:
        max_scroll = rows_count - 1
        for candidate in range(rows_count):
            var candidate_shown = quota_watch_visible_for_start(
                rows_address, rows_count, max_lines, candidate
            )
            if candidate + candidate_shown >= rows_count:
                max_scroll = candidate
                break
    var end = start + shown
    if end > rows_count:
        end = rows_count
    quota_watch_write_i64(output_address, 0, start)
    quota_watch_write_i64(output_address, 1, shown)
    quota_watch_write_i64(output_address, 2, start)
    quota_watch_write_i64(output_address, 3, rows_count - end)
    quota_watch_write_i64(output_address, 4, max_scroll)
    return 0


@export("prodex_quota_watch_viewport_v1")
def prodex_quota_watch_viewport_v1(
    abi_version: Int64,
    terminal_height: Int64,
    overview_fields: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if terminal_height < 0 or overview_fields < 0 or output_address == 0:
        return QUOTA_WATCH_INVALID
    var body_inner = terminal_height - 4
    if body_inner < 0:
        body_inner = 0
    var overview_height = overview_fields
    if overview_height > body_inner:
        overview_height = body_inner
    var table_lines = body_inner - overview_height - 1
    if table_lines < 1:
        table_lines = 1
    quota_watch_write_i64(output_address, 0, overview_height)
    quota_watch_write_i64(output_address, 1, table_lines)
    return 0


@export("prodex_quota_watch_available_lines_v1")
def prodex_quota_watch_available_lines_v1(
    abi_version: Int64,
    terminal_height: Int64,
    reserved_lines: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if terminal_height < 0 or reserved_lines < 0 or result_address == 0:
        return QUOTA_WATCH_INVALID
    var available = terminal_height - reserved_lines
    if available < 0:
        available = 0
    quota_watch_write_i64(result_address, 0, available)
    return 0


@export("prodex_quota_watch_scroll_kind_v1")
def prodex_quota_watch_scroll_kind_v1(
    abi_version: Int64,
    total_profiles: Int64,
    shown_profiles: Int64,
    hidden_before: Int64,
    hidden_after: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        total_profiles < 0
        or shown_profiles < 0
        or hidden_before < 0
        or hidden_after < 0
        or result_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var kind: Int64 = 0
    if total_profiles > 0 and (hidden_before > 0 or hidden_after > 0):
        kind = 2 if shown_profiles > 0 else 1
    quota_watch_write_i64(result_address, 0, kind)
    return 0


@export("prodex_quota_watch_filter_matches_v1")
def prodex_quota_watch_filter_matches_v1(
    abi_version: Int64,
    filter_kind: Int64,
    snapshot_kind: Int64,
    auth_address: UInt64,
    auth_length: Int64,
    provider_address: UInt64,
    provider_length: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        filter_kind < WATCH_FILTER_ALL
        or filter_kind > WATCH_FILTER_AGY
        or snapshot_kind < WATCH_SNAPSHOT_OPENAI
        or snapshot_kind > WATCH_SNAPSHOT_NONE
        or auth_length < 0
        or provider_length < 0
        or (auth_length > 0 and auth_address == 0)
        or (provider_length > 0 and provider_address == 0)
        or result_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var matches = filter_kind == WATCH_FILTER_ALL
    if not matches and filter_kind == WATCH_FILTER_OPENAI:
        matches = snapshot_kind == WATCH_SNAPSHOT_OPENAI or quota_watch_text_equal(
            auth_address, auth_length, StringSlice("chatgpt")
        )
    elif not matches and filter_kind == WATCH_FILTER_GEMINI:
        matches = snapshot_kind == WATCH_SNAPSHOT_GEMINI or quota_watch_text_equal(
            auth_address, auth_length, StringSlice("gemini")
        )
    elif not matches and filter_kind == WATCH_FILTER_ANTHROPIC:
        matches = quota_watch_text_equal(auth_address, auth_length, StringSlice("anthropic"))
    elif not matches and filter_kind == WATCH_FILTER_COPILOT:
        matches = snapshot_kind == WATCH_SNAPSHOT_COPILOT or quota_watch_text_equal(
            auth_address, auth_length, StringSlice("copilot")
        )
    elif not matches and filter_kind == WATCH_FILTER_KIRO:
        matches = quota_watch_text_equal(auth_address, auth_length, StringSlice("kiro"))
    elif not matches and filter_kind == WATCH_FILTER_DEEPSEEK:
        matches = quota_watch_text_equal(auth_address, auth_length, StringSlice("deepseek-key"))
        if not matches and snapshot_kind == WATCH_SNAPSHOT_EXTERNAL:
            matches = quota_watch_text_equal(
                provider_address, provider_length, StringSlice("deepseek")
            )
    elif not matches and filter_kind == WATCH_FILTER_LOCAL:
        matches = quota_watch_text_equal(auth_address, auth_length, StringSlice("local"))
        if not matches and snapshot_kind == WATCH_SNAPSHOT_EXTERNAL:
            matches = quota_watch_text_equal(
                provider_address, provider_length, StringSlice("local openai-compatible")
            )
    quota_watch_write_i64(result_address, 0, Int64(1) if matches else Int64(0))
    return 0


@export("prodex_quota_watch_key_action_v1")
def prodex_quota_watch_key_action_v1(
    abi_version: Int64,
    key_kind: Int64,
    char_code: Int64,
    control: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_WATCH_ABI_VERSION
        or key_kind < 0
        or key_kind > 4
        or char_code < 0
        or control < 0
        or control > 1
        or result_address == 0
    ):
        return 4 if abi_version != QUOTA_WATCH_ABI_VERSION else QUOTA_WATCH_INVALID
    var action: Int64 = -1
    if key_kind == 1 or (key_kind == 2 and (char_code == 113 or char_code == 81)):
        action = WATCH_ACTION_QUIT
    elif key_kind == 2 and control == 1 and (char_code == 99 or char_code == 122):
        action = WATCH_ACTION_QUIT
    elif key_kind == 3:
        action = WATCH_ACTION_DOWN
    elif key_kind == 4:
        action = WATCH_ACTION_UP
    elif key_kind == 2 and char_code == 106:
        action = WATCH_ACTION_DOWN
    elif key_kind == 2 and char_code == 107:
        action = WATCH_ACTION_UP
    elif key_kind == 2 and char_code == 115:
        action = WATCH_ACTION_SORT
    elif key_kind == 2 and char_code == 102:
        action = WATCH_ACTION_FILTER
    elif key_kind == 2 and (char_code == 117 or char_code == 85):
        action = WATCH_ACTION_UPDATE
    quota_watch_write_i64(result_address, 0, action)
    return 0


@export("prodex_quota_watch_action_v1")
def prodex_quota_watch_action_v1(
    abi_version: Int64,
    action: Int64,
    scroll_offset: Int64,
    max_scroll_offset: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        action < WATCH_ACTION_UP
        or action > WATCH_ACTION_QUIT
        or scroll_offset < 0
        or max_scroll_offset < 0
        or output_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var outcome: Int64 = WATCH_OUTCOME_CONTINUE
    var next_offset = scroll_offset
    if action == WATCH_ACTION_UP:
        if next_offset > 0:
            next_offset -= 1
    elif action == WATCH_ACTION_DOWN:
        if next_offset < max_scroll_offset:
            next_offset += 1
    elif action == WATCH_ACTION_SORT:
        outcome = WATCH_OUTCOME_SORT
    elif action == WATCH_ACTION_FILTER:
        outcome = WATCH_OUTCOME_FILTER
    elif action == WATCH_ACTION_UPDATE:
        outcome = WATCH_OUTCOME_UPDATE
    elif action == WATCH_ACTION_QUIT:
        outcome = WATCH_OUTCOME_QUIT
    quota_watch_write_i64(output_address, 0, outcome)
    quota_watch_write_i64(output_address, 1, next_offset)
    return 0


@export("prodex_quota_watch_reset_kind_v1")
def prodex_quota_watch_reset_kind_v1(
    abi_version: Int64,
    address: UInt64,
    length: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if length < 0 or (length > 0 and address == 0) or result_address == 0:
        return QUOTA_WATCH_INVALID
    var start: Int64 = 0
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while start < length and (
        source[unsafe_offset=start] == 9
        or source[unsafe_offset=start] == 10
        or source[unsafe_offset=start] == 13
        or source[unsafe_offset=start] == 32
    ):
        start += 1
    var kind = WATCH_RESET_NORMAL
    if quota_watch_text_starts_at_exact(address, length, start, StringSlice("error:")):
        kind = WATCH_RESET_ERROR
    elif quota_watch_text_starts_at_exact(address, length, start, StringSlice("resets: ")) and quota_watch_text_contains_exact(
        address, length, StringSlice("; reset credits:")
    ):
        kind = WATCH_RESET_CREDITS
    quota_watch_write_i64(result_address, 0, kind)
    return 0


@export("prodex_quota_watch_merge_v1")
def prodex_quota_watch_merge_v1(
    abi_version: Int64,
    previous_kind: Int64,
    next_kind: Int64,
    output_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        previous_kind < 0
        or previous_kind > 3
        or next_kind < 0
        or next_kind > 3
        or output_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var plan = WATCH_SNAPSHOT_USE_NEXT
    if previous_kind == 1 and next_kind == 1:
        plan = WATCH_SNAPSHOT_MERGE_REPORTS
    elif previous_kind == 1 and next_kind == 3:
        plan = WATCH_SNAPSHOT_KEEP_PREVIOUS
    quota_watch_write_i64(output_address, 0, plan)
    return 0


@export("prodex_quota_watch_preserve_v1")
def prodex_quota_watch_preserve_v1(
    abi_version: Int64,
    previous_success: Int64,
    current_success: Int64,
    current_auth_error: Int64,
    same_profile: Int64,
    same_auth: Int64,
    result_address: UInt64,
) abi("C") -> Int64:
    if abi_version != QUOTA_WATCH_ABI_VERSION:
        return 4
    if (
        previous_success < 0
        or previous_success > 1
        or current_success < 0
        or current_success > 1
        or current_auth_error < 0
        or current_auth_error > 1
        or same_profile < 0
        or same_profile > 1
        or same_auth < 0
        or same_auth > 1
        or result_address == 0
    ):
        return QUOTA_WATCH_INVALID
    var preserve = previous_success == 1 and current_success == 0 and current_auth_error == 0
    preserve = preserve and same_profile == 1 and same_auth == 1
    quota_watch_write_i64(result_address, 0, Int64(1) if preserve else Int64(0))
    return 0
