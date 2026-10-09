from std.memory import Pointer
from std.math import isfinite

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from runtime_math import INT64_MAX, INT64_MIN, UINT64_MAX

comptime INFO_RENDER_ABI_VERSION: Int64 = 1
comptime INFO_RENDER_OK: Int64 = 0
comptime INFO_RENDER_INVALID: Int64 = 1
comptime INFO_RENDER_CAPACITY: Int64 = 3
comptime INFO_RENDER_ABI: Int64 = 4

comptime INFO_RENDER_RELATIVE_DURATION: Int64 = 0
comptime INFO_RENDER_QUOTA_DATA: Int64 = 1
comptime INFO_RENDER_RUNTIME_POLICY: Int64 = 2
comptime INFO_RENDER_RUNTIME_LOGS: Int64 = 3
comptime INFO_RENDER_TUNING_WORKERS: Int64 = 4
comptime INFO_RENDER_TUNING_BUDGETS: Int64 = 5
comptime INFO_RENDER_TUNING_TRANSPORT: Int64 = 6
comptime INFO_RENDER_POOL_REMAINING: Int64 = 7
comptime INFO_RENDER_PROCESS_SUMMARY: Int64 = 8
comptime INFO_RENDER_LOAD_SUMMARY: Int64 = 9
comptime INFO_RENDER_TOKEN_USAGE: Int64 = 10
comptime INFO_RENDER_RUNTIME_LAUNCH_SELECTION: Int64 = 11
comptime INFO_RENDER_RUNTIME_LAUNCH_WARNING: Int64 = 12
comptime INFO_RENDER_RUNTIME_PROVIDER_DIRECT: Int64 = 13
comptime INFO_RENDER_RUNTIME_QUOTA_HINT: Int64 = 14
comptime INFO_RENDER_HUMAN_BYTES: Int64 = 15
comptime INFO_RENDER_HUMAN_COUNT: Int64 = 16
comptime INFO_RENDER_TOKEN_EFFICIENCY: Int64 = 17
comptime INFO_RENDER_MEMORY_PERCENT: Int64 = 18
comptime INFO_RENDER_TEXT_SPARKLINE: Int64 = 19
comptime INFO_RENDER_STATUS_PROFILE_FILTER: Int64 = 20
comptime INFO_RENDER_STATUS_TOKEN_PLAN: Int64 = 21
comptime INFO_RENDER_STATUS_RUNTIME_PROFILE: Int64 = 22
comptime INFO_RENDER_STATUS_RUNWAY: Int64 = 23
comptime INFO_RENDER_STATUS_FIELDS: Int64 = 24
comptime INFO_RENDER_STATUS_RESOURCE_METRICS: Int64 = 25
comptime INFO_RENDER_STATUS_RESOURCE_HISTORY: Int64 = 26
comptime INFO_RENDER_STATUS_QUOTA_GAUGE: Int64 = 27
comptime INFO_RENDER_DOCTOR_VALUE_COLOR: Int64 = 28
comptime INFO_RENDER_DOCTOR_VIEWPORT: Int64 = 29
comptime INFO_STATUS_HISTORY_LIMIT: Int64 = 64
comptime INFO_STATUS_FIELD_COUNT: Int64 = 14

comptime INFO_RUNTIME_LAUNCH_STATUS_NONE: UInt64 = 0
comptime INFO_RUNTIME_LAUNCH_STATUS_READY: UInt64 = 1
comptime INFO_RUNTIME_LAUNCH_STATUS_BLOCKED: UInt64 = 2
comptime INFO_RUNTIME_LAUNCH_STATUS_PROBE_FAILED: UInt64 = 3

@fieldwise_init
struct InfoRenderWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def info_put_byte(
    writer: Pointer[mut=True, InfoRenderWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def info_put_literal(
    writer: Pointer[mut=True, InfoRenderWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not info_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def info_put_view(
    writer: Pointer[mut=True, InfoRenderWriter, _], value: ProdexRichStringView
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not info_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def info_put_u64(
    writer: Pointer[mut=True, InfoRenderWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return info_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not info_put_byte(writer, UInt8(remaining / divisor) + UInt8(48)):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def info_put_i64(
    writer: Pointer[mut=True, InfoRenderWriter, _], value: Int64
) -> Bool:
    if value >= 0:
        return info_put_u64(writer, UInt64(value))
    if not info_put_byte(writer, UInt8(45)):
        return False
    var magnitude = (
        UInt64(9_223_372_036_854_775_808)
        if value == -9_223_372_036_854_775_808
        else UInt64(-value)
    )
    return info_put_u64(writer, magnitude)


def info_unsigned(
    address: UInt, count: Int64, index: Int64
) -> UInt64:
    var values = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return values[unsafe_offset=index]


def info_signed(
    address: UInt, count: Int64, index: Int64
) -> Int64:
    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return values[unsafe_offset=index]


def info_text(
    address: UInt, count: Int64, index: Int64
) -> ProdexRichStringView:
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    return values[unsafe_offset=index].copy()


def info_validate_texts(address: UInt, count: Int64) -> Bool:
    if count < 0 or (count > 0 and address == 0):
        return False
    if count == 0:
        return True
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    for index in range(count):
        var value = values[unsafe_offset=index].copy()
        if not rich_view_valid(value, Int64(value.len)):
            return False
    return True


def info_status_compare_views(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Int64:
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    var common = min(Int64(left.len), Int64(right.len))
    for index in range(common):
        var left_byte = left_ptr[unsafe_offset=index]
        var right_byte = right_ptr[unsafe_offset=index]
        if left_byte < right_byte:
            return -1
        if left_byte > right_byte:
            return 1
    if left.len < right.len:
        return -1
    if left.len > right.len:
        return 1
    return 0


def info_status_event_compare(
    left: Int64,
    right: Int64,
    unsigned_address: UInt,
    text_address: UInt,
) -> Int64:
    var left_timestamp = info_text(text_address, 0, left * 2)
    var right_timestamp = info_text(text_address, 0, right * 2)
    var compared = info_status_compare_views(left_timestamp, right_timestamp)
    if compared != 0:
        return compared

    var left_request_present = info_unsigned(unsigned_address, 0, left * 4)
    var right_request_present = info_unsigned(unsigned_address, 0, right * 4)
    if left_request_present < right_request_present:
        return -1
    if left_request_present > right_request_present:
        return 1
    if left_request_present == 1:
        var left_request = info_unsigned(unsigned_address, 0, left * 4 + 1)
        var right_request = info_unsigned(unsigned_address, 0, right * 4 + 1)
        if left_request < right_request:
            return -1
        if left_request > right_request:
            return 1

    var left_profile = info_text(text_address, 0, left * 2 + 1)
    var right_profile = info_text(text_address, 0, right * 2 + 1)
    compared = info_status_compare_views(left_profile, right_profile)
    if compared != 0:
        return compared
    if left < right:
        return -1
    if left > right:
        return 1
    return 0


def info_status_put_field_label(
    writer: Pointer[mut=True, InfoRenderWriter, _], label: StringSlice
) -> Bool:
    return (
        info_put_u64(writer, UInt64(label.byte_length()))
        and info_put_byte(writer, UInt8(58))
        and info_put_literal(writer, label)
    )


def info_status_begin_field_value(
    writer: Pointer[mut=True, InfoRenderWriter, _]
) -> Int64:
    var prefix = writer[].written
    for _ in range(20):
        if not info_put_byte(writer, UInt8(48)):
            return -1
    if not info_put_byte(writer, UInt8(58)):
        return -1
    return prefix


def info_status_finish_field_value(
    writer: Pointer[mut=True, InfoRenderWriter, _], prefix: Int64
) -> Bool:
    var length = UInt64(writer[].written - prefix - 21)
    for index in range(20):
        var position = prefix + 19 - Int64(index)
        writer[].output[unsafe_offset=position] = UInt8(length % UInt64(10)) + UInt8(48)
        length //= UInt64(10)
    return length == 0


def info_render_relative_duration(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
) -> Bool:
    return info_render_relative_duration_value(
        writer, info_signed(signed_address, 1, 0)
    )


def info_render_quota_data(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var compatible = info_unsigned(unsigned_address, 4, 0)
    if compatible == 0:
        return info_put_literal(writer, StringSlice("No quota-compatible profiles"))
    return (
        info_put_u64(writer, compatible)
        and info_put_literal(writer, StringSlice(" quota-compatible profile(s): live="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 4, 1))
        and info_put_literal(writer, StringSlice(", snapshot="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 4, 2))
        and info_put_literal(writer, StringSlice(", unavailable="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 4, 3))
    )


def info_render_runtime_policy(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    if presence & UInt64(3) != UInt64(3):
        return info_put_literal(writer, StringSlice("disabled"))
    return (
        info_put_view(writer, info_text(text_address, 1, 0))
        and info_put_literal(writer, StringSlice(" (v"))
        and info_put_u64(writer, info_unsigned(unsigned_address, 1, 0))
        and info_put_byte(writer, UInt8(41))
    )


def info_render_runtime_logs(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    text_address: UInt,
) -> Bool:
    return (
        info_put_view(writer, info_text(text_address, 2, 0))
        and info_put_literal(writer, StringSlice(" ("))
        and info_put_view(writer, info_text(text_address, 2, 1))
        and info_put_byte(writer, UInt8(41))
    )


def info_render_tuning_workers(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    values: UInt,
) -> Bool:
    return (
        info_put_literal(writer, StringSlice("workers proxy="))
        and info_put_u64(writer, info_unsigned(values, 16, 0))
        and info_put_literal(writer, StringSlice(", long-lived="))
        and info_put_u64(writer, info_unsigned(values, 16, 1))
        and info_put_literal(writer, StringSlice(", async="))
        and info_put_u64(writer, info_unsigned(values, 16, 2))
        and info_put_literal(writer, StringSlice(", probe-refresh="))
        and info_put_u64(writer, info_unsigned(values, 16, 3))
        and info_put_literal(writer, StringSlice("; active="))
        and info_put_u64(writer, info_unsigned(values, 16, 4))
        and info_put_literal(writer, StringSlice(", queue="))
        and info_put_u64(writer, info_unsigned(values, 16, 5))
        and info_put_literal(writer, StringSlice("; lanes responses="))
        and info_put_u64(writer, info_unsigned(values, 16, 6))
        and info_put_literal(writer, StringSlice(", compact="))
        and info_put_u64(writer, info_unsigned(values, 16, 7))
        and info_put_literal(writer, StringSlice(", websocket="))
        and info_put_u64(writer, info_unsigned(values, 16, 8))
        and info_put_literal(writer, StringSlice(", standard="))
        and info_put_u64(writer, info_unsigned(values, 16, 9))
        and info_put_literal(writer, StringSlice("; ws-connect workers="))
        and info_put_u64(writer, info_unsigned(values, 16, 10))
        and info_put_literal(writer, StringSlice(", queue="))
        and info_put_u64(writer, info_unsigned(values, 16, 11))
        and info_put_literal(writer, StringSlice(", overflow="))
        and info_put_u64(writer, info_unsigned(values, 16, 12))
        and info_put_literal(writer, StringSlice("; ws-dns workers="))
        and info_put_u64(writer, info_unsigned(values, 16, 13))
        and info_put_literal(writer, StringSlice(", queue="))
        and info_put_u64(writer, info_unsigned(values, 16, 14))
        and info_put_literal(writer, StringSlice(", overflow="))
        and info_put_u64(writer, info_unsigned(values, 16, 15))
    )


def info_render_tuning_budgets(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    values: UInt,
) -> Bool:
    return (
        info_put_literal(writer, StringSlice("precommit="))
        and info_put_u64(writer, info_unsigned(values, 10, 0))
        and info_put_literal(writer, StringSlice("x/"))
        and info_put_u64(writer, info_unsigned(values, 10, 1))
        and info_put_literal(writer, StringSlice("ms, pressure-precommit="))
        and info_put_u64(writer, info_unsigned(values, 10, 2))
        and info_put_literal(writer, StringSlice("x/"))
        and info_put_u64(writer, info_unsigned(values, 10, 3))
        and info_put_literal(writer, StringSlice("ms, continuation="))
        and info_put_u64(writer, info_unsigned(values, 10, 4))
        and info_put_literal(writer, StringSlice("x/"))
        and info_put_u64(writer, info_unsigned(values, 10, 5))
        and info_put_literal(writer, StringSlice("ms; admission="))
        and info_put_u64(writer, info_unsigned(values, 10, 6))
        and info_put_literal(writer, StringSlice("ms, pressure-admission="))
        and info_put_u64(writer, info_unsigned(values, 10, 7))
        and info_put_literal(writer, StringSlice("ms, long-lived="))
        and info_put_u64(writer, info_unsigned(values, 10, 8))
        and info_put_literal(writer, StringSlice("ms, pressure-long-lived="))
        and info_put_u64(writer, info_unsigned(values, 10, 9))
        and info_put_literal(writer, StringSlice("ms"))
    )


def info_render_tuning_transport(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    values: UInt,
) -> Bool:
    return (
        info_put_literal(writer, StringSlice("http-connect="))
        and info_put_u64(writer, info_unsigned(values, 9, 0))
        and info_put_literal(writer, StringSlice("ms, stream-idle="))
        and info_put_u64(writer, info_unsigned(values, 9, 1))
        and info_put_literal(writer, StringSlice("ms, sse-lookahead="))
        and info_put_u64(writer, info_unsigned(values, 9, 2))
        and info_put_literal(writer, StringSlice("ms; ws-connect="))
        and info_put_u64(writer, info_unsigned(values, 9, 3))
        and info_put_literal(writer, StringSlice("ms, ws-progress="))
        and info_put_u64(writer, info_unsigned(values, 9, 4))
        and info_put_literal(writer, StringSlice("ms, ws-happy="))
        and info_put_u64(writer, info_unsigned(values, 9, 5))
        and info_put_literal(writer, StringSlice("ms, ws-stale-reuse="))
        and info_put_u64(writer, info_unsigned(values, 9, 6))
        and info_put_literal(writer, StringSlice("ms; inflight soft/hard="))
        and info_put_u64(writer, info_unsigned(values, 9, 7))
        and info_put_byte(writer, UInt8(47))
        and info_put_u64(writer, info_unsigned(values, 9, 8))
    )


def info_render_process_summary(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    text_address: UInt,
    text_count: Int64,
) -> Bool:
    var total = info_unsigned(unsigned_address, 3, 0)
    if total == 0:
        return info_put_literal(writer, StringSlice("No"))
    var runtime = info_unsigned(unsigned_address, 3, 1)
    var max_visible = info_unsigned(unsigned_address, 3, 2)
    if not (
        info_put_literal(writer, StringSlice("Yes ("))
        and info_put_u64(writer, total)
        and info_put_literal(writer, StringSlice(" total, "))
        and info_put_u64(writer, runtime)
        and info_put_literal(writer, StringSlice(" runtime; processes: "))
    ):
        return False
    for index in range(text_count):
        if index > 0 and not info_put_literal(writer, StringSlice(", ")):
            return False
        if not info_put_view(writer, info_text(text_address, text_count, index)):
            return False
    var remaining = total - min(total, max_visible)
    if remaining > 0:
        if not (
            info_put_literal(writer, StringSlice(" (+"))
            and info_put_u64(writer, remaining)
            and info_put_literal(writer, StringSlice(" more)"))
        ):
            return False
    return info_put_byte(writer, UInt8(41))


def info_saturating_sub_i64(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < -9_223_372_036_854_775_808 + right:
        return -9_223_372_036_854_775_808
    if right < 0 and left > 9_223_372_036_854_775_807 + right:
        return 9_223_372_036_854_775_807
    return left - right


def info_render_load_summary(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
    unsigned_address: UInt,
    presence: UInt64,
) -> Bool:
    var log_count = info_unsigned(unsigned_address, 4, 0)
    var inflight = info_unsigned(unsigned_address, 4, 1)
    var selections = info_unsigned(unsigned_address, 4, 2)
    var runtime_processes = info_unsigned(unsigned_address, 4, 3)
    if runtime_processes == 0:
        return info_put_literal(writer, StringSlice("No active prodex runtime detected"))
    if log_count == 0:
        return info_put_literal(
            writer,
            StringSlice("Runtime process detected, but no matching runtime log was found"),
        )
    if selections == 0:
        return (
            info_put_u64(writer, log_count)
            and info_put_literal(
                writer,
                StringSlice(
                    " active runtime log(s); no selection activity observed in the sampled window; inflight units "
                ),
            )
            and info_put_u64(writer, inflight)
        )
    if selections == 1:
        return (
            info_put_literal(writer, StringSlice("1 selection event observed in the sampled window; inflight units "))
            and info_put_u64(writer, inflight)
            and info_put_literal(writer, StringSlice("; "))
            and info_put_u64(writer, log_count)
            and info_put_literal(writer, StringSlice(" active runtime log(s)"))
        )

    if not (
        info_put_u64(writer, selections)
        and info_put_literal(writer, StringSlice(" selection event(s) over "))
    ):
        return False
    if presence & UInt64(1) != 0:
        var span = info_saturating_sub_i64(
            info_signed(signed_address, 3, 1),
            info_signed(signed_address, 3, 0),
        )
        if not info_render_relative_duration_value(writer, span):
            return False
    else:
        var window_minutes = info_signed(signed_address, 3, 2) // 60
        if not (
            info_put_i64(writer, window_minutes)
            and info_put_byte(writer, UInt8(109))
        ):
            return False
    return (
        info_put_literal(writer, StringSlice("; inflight units "))
        and info_put_u64(writer, inflight)
        and info_put_literal(writer, StringSlice("; "))
        and info_put_u64(writer, log_count)
        and info_put_literal(writer, StringSlice(" active runtime log(s)"))
    )


def info_render_relative_duration_value(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    seconds_value: Int64,
) -> Bool:
    var seconds = max(seconds_value, Int64(0))
    if seconds == 0:
        return info_put_literal(writer, StringSlice("now"))
    var days = seconds // 86_400
    var hours = (seconds % 86_400) // 3_600
    var minutes = (seconds % 3_600) // 60
    if days > 0:
        if not info_put_i64(writer, days) or not info_put_byte(writer, UInt8(100)):
            return False
        if hours > 0:
            return (
                info_put_byte(writer, UInt8(32))
                and info_put_i64(writer, hours)
                and info_put_byte(writer, UInt8(104))
            )
        return True
    if hours > 0:
        if not info_put_i64(writer, hours) or not info_put_byte(writer, UInt8(104)):
            return False
        if minutes > 0:
            return (
                info_put_byte(writer, UInt8(32))
                and info_put_i64(writer, minutes)
                and info_put_byte(writer, UInt8(109))
            )
        return True
    if minutes > 0:
        return info_put_i64(writer, minutes) and info_put_byte(writer, UInt8(109))
    return info_put_literal(writer, StringSlice("<1m"))


def info_render_token_usage(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    text_address: UInt,
    text_count: Int64,
) -> Bool:
    var events = info_unsigned(unsigned_address, 6, 0)
    var logs = info_unsigned(unsigned_address, 6, 1)
    if events == 0:
        return (
            info_put_literal(writer, StringSlice("No token_usage events found in "))
            and info_put_u64(writer, logs)
            and info_put_literal(writer, StringSlice(" recent runtime log(s)"))
        )
    if not (
        info_put_u64(writer, events)
        and info_put_literal(writer, StringSlice(" event(s), logs="))
        and info_put_u64(writer, logs)
        and info_put_literal(writer, StringSlice(": input="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 6, 2))
        and info_put_literal(writer, StringSlice(", cached_input="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 6, 3))
        and info_put_literal(writer, StringSlice(", output="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 6, 4))
        and info_put_literal(writer, StringSlice(", reasoning="))
        and info_put_u64(writer, info_unsigned(unsigned_address, 6, 5))
    ):
        return False
    if text_count == 0:
        return True
    if not info_put_literal(writer, StringSlice("; by profile: ")):
        return False
    for index in range(text_count):
        if index > 0 and not info_put_literal(writer, StringSlice("; ")):
            return False
        var base = Int64(6) + index * Int64(4)
        if not (
            info_put_view(writer, info_text(text_address, text_count, index))
            and info_put_byte(writer, UInt8(58))
            and info_put_u64(writer, info_unsigned(unsigned_address, 6 + text_count * 4, base))
            and info_put_literal(writer, StringSlice(" in/"))
            and info_put_u64(writer, info_unsigned(unsigned_address, 6 + text_count * 4, base + 1))
            and info_put_literal(writer, StringSlice(" cached/"))
            and info_put_u64(writer, info_unsigned(unsigned_address, 6 + text_count * 4, base + 2))
            and info_put_literal(writer, StringSlice(" out/"))
            and info_put_u64(writer, info_unsigned(unsigned_address, 6 + text_count * 4, base + 3))
            and info_put_literal(writer, StringSlice(" reasoning"))
        ):
            return False
    return True


def info_render_pool_remaining(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var profiles = info_unsigned(unsigned_address, 1, 0)
    if profiles == 0:
        return info_put_literal(writer, StringSlice("Unavailable"))
    if not (
        info_put_i64(writer, info_signed(signed_address, 1, 0))
        and info_put_literal(writer, StringSlice("% across "))
        and info_put_u64(writer, profiles)
        and info_put_literal(writer, StringSlice(" profile(s)"))
    ):
        return False
    if presence & UInt64(1) != 0:
        return (
            info_put_literal(writer, StringSlice("; earliest reset "))
            and info_put_view(writer, info_text(text_address, 1, 0))
        )
    return True


def info_render_runtime_launch_selection(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    text_address: UInt,
    status: UInt64,
) -> Bool:
    var initial_profile = info_text(text_address, 4, 0)
    var candidate_name = info_text(text_address, 4, 1)
    var quota_summary = info_text(text_address, 4, 2)
    if status == INFO_RUNTIME_LAUNCH_STATUS_BLOCKED:
        return (
            info_put_literal(writer, StringSlice("Auto-rotating to profile '"))
            and info_put_view(writer, candidate_name)
            and info_put_literal(
                writer,
                StringSlice("' using quota-pressure scoring ("),
            )
            and info_put_view(writer, quota_summary)
            and info_put_literal(writer, StringSlice(")."))
        )
    if status == INFO_RUNTIME_LAUNCH_STATUS_READY:
        return (
            info_put_literal(writer, StringSlice("Auto-selecting profile '"))
            and info_put_view(writer, candidate_name)
            and info_put_literal(writer, StringSlice("' over active profile '"))
            and info_put_view(writer, initial_profile)
            and info_put_literal(
                writer,
                StringSlice("' using quota-pressure scoring ("),
            )
            and info_put_view(writer, quota_summary)
            and info_put_literal(writer, StringSlice(")."))
        )
    if status == INFO_RUNTIME_LAUNCH_STATUS_PROBE_FAILED:
        return (
            info_put_literal(writer, StringSlice("Using ready profile '"))
            and info_put_view(writer, candidate_name)
            and info_put_literal(
                writer,
                StringSlice("' after quota preflight failed ("),
            )
            and info_put_view(writer, quota_summary)
            and info_put_byte(writer, UInt8(41))
        )
    return (
        info_put_literal(writer, StringSlice("Using profile '"))
        and info_put_view(writer, candidate_name)
        and info_put_literal(writer, StringSlice("' ("))
        and info_put_view(writer, quota_summary)
        and info_put_byte(writer, UInt8(41))
    )


def info_render_runtime_launch_warning(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    text_address: UInt,
    status: UInt64,
) -> Bool:
    if status == INFO_RUNTIME_LAUNCH_STATUS_BLOCKED:
        return (
            info_put_literal(writer, StringSlice("Quota preflight blocked profile '"))
            and info_put_view(writer, info_text(text_address, 4, 0))
            and info_put_literal(writer, StringSlice("': "))
            and info_put_view(writer, info_text(text_address, 4, 3))
        )
    if status == INFO_RUNTIME_LAUNCH_STATUS_PROBE_FAILED:
        return (
            info_put_literal(
                writer,
                StringSlice("Warning: quota preflight failed for '"),
            )
            and info_put_view(writer, info_text(text_address, 4, 0))
            and info_put_literal(writer, StringSlice("': "))
            and info_put_view(writer, info_text(text_address, 4, 3))
        )
    return True


def info_render_runtime_provider_direct(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    text_address: UInt,
) -> Bool:
    return (
        info_put_literal(writer, StringSlice("Detected model_provider '"))
        and info_put_view(writer, info_text(text_address, 2, 0))
        and info_put_literal(writer, StringSlice("' from "))
        and info_put_view(writer, info_text(text_address, 2, 1))
        and info_put_literal(
            writer,
            StringSlice(
                ". Launching directly without prodex quota preflight or auto-rotate proxy."
            ),
        )
    )



def info_put_one_decimal(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    tenths: UInt64,
) -> Bool:
    return (
        info_put_u64(writer, tenths // UInt64(10))
        and info_put_byte(writer, UInt8(46))
        and info_put_byte(writer, UInt8(tenths % UInt64(10)) + UInt8(48))
    )


def info_round_nonnegative_tenths(value: Float64) -> UInt64:
    if value <= 0.0:
        return UInt64(0)
    return UInt64(value * 10.0 + 0.5)


def info_render_human_bytes(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var bytes = info_unsigned(unsigned_address, 1, 0)
    if bytes < UInt64(1024):
        return info_put_u64(writer, bytes) and info_put_literal(writer, StringSlice(" B"))
    var value = Float64(bytes)
    var unit: Int64 = 0
    while value >= 1024.0 and unit < 4:
        value /= 1024.0
        unit += 1
    if not info_put_one_decimal(writer, info_round_nonnegative_tenths(value)):
        return False
    if unit == 1:
        return info_put_literal(writer, StringSlice(" KiB"))
    if unit == 2:
        return info_put_literal(writer, StringSlice(" MiB"))
    if unit == 3:
        return info_put_literal(writer, StringSlice(" GiB"))
    return info_put_literal(writer, StringSlice(" TiB"))


def info_render_human_count(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var value = info_unsigned(unsigned_address, 1, 0)
    var divisor = UInt64(0)
    var suffix = StringSlice("")
    if value >= UInt64(1_000_000_000):
        divisor = UInt64(1_000_000_000)
        suffix = StringSlice("B")
    elif value >= UInt64(1_000_000):
        divisor = UInt64(1_000_000)
        suffix = StringSlice("M")
    elif value >= UInt64(1_000):
        divisor = UInt64(1_000)
        suffix = StringSlice("K")
    else:
        return info_put_u64(writer, value)
    var scaled = Float64(value) / Float64(divisor)
    return (
        info_put_one_decimal(writer, info_round_nonnegative_tenths(scaled))
        and info_put_literal(writer, suffix)
    )


def info_render_token_efficiency(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var input = info_unsigned(unsigned_address, 3, 0)
    var cached = info_unsigned(unsigned_address, 3, 1)
    var output = info_unsigned(unsigned_address, 3, 2)
    var cache_tenths = UInt64(0)
    if input != 0:
        cache_tenths = info_round_nonnegative_tenths(
            Float64(cached) / Float64(input) * 100.0
        )
    var total = UInt64(18_446_744_073_709_551_615)
    if input <= total - output:
        total = input + output
    var output_tenths = UInt64(0)
    if total != 0:
        output_tenths = info_round_nonnegative_tenths(
            Float64(output) / Float64(total) * 100.0
        )
    return (
        info_put_literal(writer, StringSlice("cache hit "))
        and info_put_one_decimal(writer, cache_tenths)
        and info_put_literal(writer, StringSlice("% · output share "))
        and info_put_one_decimal(writer, output_tenths)
        and info_put_byte(writer, UInt8(37))
    )


def info_render_memory_percent(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var resident = info_unsigned(unsigned_address, 2, 0)
    var total = info_unsigned(unsigned_address, 2, 1)
    var tenths = UInt64(0)
    if total != 0:
        tenths = info_round_nonnegative_tenths(
            Float64(resident) / Float64(total) * 100.0
        )
    return info_put_one_decimal(writer, tenths) and info_put_byte(writer, UInt8(37))



def info_render_text_sparkline(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    unsigned_count: Int64,
) -> Bool:
    if unsigned_count <= 0:
        return info_put_byte(writer, UInt8(45))
    var maximum = UInt64(0)
    for index in range(unsigned_count):
        var value = info_unsigned(unsigned_address, unsigned_count, index)
        if value > maximum:
            maximum = value
    if maximum == 0:
        return info_put_byte(writer, UInt8(45))
    for index in range(unsigned_count):
        var value = info_unsigned(unsigned_address, unsigned_count, index)
        var scaled = UInt128(value) * UInt128(7) // UInt128(maximum)
        if scaled == UInt128(0):
            if not info_put_literal(writer, StringSlice("▁")): return False
        elif scaled == UInt128(1):
            if not info_put_literal(writer, StringSlice("▂")): return False
        elif scaled == UInt128(2):
            if not info_put_literal(writer, StringSlice("▃")): return False
        elif scaled == UInt128(3):
            if not info_put_literal(writer, StringSlice("▄")): return False
        elif scaled == UInt128(4):
            if not info_put_literal(writer, StringSlice("▅")): return False
        elif scaled == UInt128(5):
            if not info_put_literal(writer, StringSlice("▆")): return False
        elif scaled == UInt128(6):
            if not info_put_literal(writer, StringSlice("▇")): return False
        else:
            if not info_put_literal(writer, StringSlice("█")): return False
    return True


def info_status_write_field_text(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    label: StringSlice,
    value: ProdexRichStringView,
) -> Bool:
    if not info_status_put_field_label(writer, label):
        return False
    var prefix = info_status_begin_field_value(writer)
    if prefix < 0 or not info_put_view(writer, value):
        return False
    return info_status_finish_field_value(writer, prefix)


def info_render_status_field(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    field: Int64,
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var label = StringSlice("")
    if field == 0:
        label = StringSlice("Profile")
    elif field == 1:
        return info_status_write_field_text(
            writer, StringSlice("5h quota"), info_text(text_address, 11, 2)
        )
    elif field == 2:
        return info_status_write_field_text(
            writer, StringSlice("5h runway"), info_text(text_address, 11, 3)
        )
    elif field == 3:
        return info_status_write_field_text(
            writer, StringSlice("Weekly quota"), info_text(text_address, 11, 4)
        )
    elif field == 4:
        return info_status_write_field_text(
            writer, StringSlice("Weekly runway"), info_text(text_address, 11, 5)
        )
    elif field == 5:
        return info_status_write_field_text(
            writer, StringSlice("Token usage"), info_text(text_address, 11, 6)
        )
    elif field == 6:
        label = StringSlice("Token efficiency")
    elif field == 7:
        label = StringSlice("Token history")
    elif field == 8:
        label = StringSlice("Processes")
    elif field == 9:
        label = StringSlice("Memory")
    elif field == 10:
        label = StringSlice("Network")
    elif field == 11:
        label = StringSlice("Disk I/O")
    elif field == 12:
        return info_status_write_field_text(
            writer, StringSlice("Recent load"), info_text(text_address, 11, 9)
        )
    elif field == 13:
        return info_status_write_field_text(
            writer, StringSlice("Updated"), info_text(text_address, 11, 10)
        )
    else:
        return False

    if not info_status_put_field_label(writer, label):
        return False
    var prefix = info_status_begin_field_value(writer)
    if prefix < 0:
        return False

    if field == 0:
        if not (
            info_put_literal(writer, StringSlice("runtime="))
            and info_put_view(writer, info_text(text_address, 11, 0))
            and info_put_literal(writer, StringSlice(", configured="))
            and info_put_view(writer, info_text(text_address, 11, 1))
            and info_put_literal(writer, StringSlice(", pool="))
            and info_put_u64(writer, info_unsigned(unsigned_address, 0, 0))
            and info_put_literal(writer, StringSlice(", quota-compatible="))
            and info_put_u64(writer, info_unsigned(unsigned_address, 0, 1))
            and info_put_literal(writer, StringSlice(", unavailable="))
            and info_put_u64(writer, info_unsigned(unsigned_address, 0, 2))
        ):
            return False
    elif field == 6:
        if not info_render_token_efficiency(
            writer, unsigned_address + UInt(3 * 8)
        ):
            return False
    elif field == 7:
        if not info_put_view(writer, info_text(text_address, 12, 11)):
            return False
        if not info_put_byte(writer, UInt8(32)):
            return False
        if (presence & UInt64(4)) != 0:
            if not info_put_view(writer, info_text(text_address, 11, 7)):
                return False
        elif not info_put_byte(writer, UInt8(45)):
            return False
        if not info_put_literal(writer, StringSlice(" → ")):
            return False
        if (presence & UInt64(8)) != 0:
            if not info_put_view(writer, info_text(text_address, 11, 8)):
                return False
        elif not info_put_byte(writer, UInt8(45)):
            return False
    elif field == 8:
        if (presence & UInt64(1)) == 0:
            if not info_put_literal(writer, StringSlice("unavailable")):
                return False
        else:
            if not (
                info_put_u64(writer, info_unsigned(unsigned_address, 0, 8))
                and info_put_literal(writer, StringSlice(" total, "))
                and info_put_u64(writer, info_unsigned(unsigned_address, 0, 9))
                and info_put_literal(writer, StringSlice(" runtime; CPU "))
            ):
                return False
            if (presence & UInt64(2)) != 0:
                var cpu_values = Pointer[
                    mut=False, Float64, ImmUntrackedOrigin
                ](unsafe_from_address=Int(unsigned_address + UInt(17 * 8)))
                if not info_put_one_decimal(
                    writer, info_round_nonnegative_tenths(cpu_values[unsafe_offset=0])
                ) or not info_put_byte(writer, UInt8(37)):
                    return False
            elif not info_put_literal(writer, StringSlice("warming up")):
                return False
    elif field == 9:
        if (presence & UInt64(1)) == 0:
            if not info_put_literal(writer, StringSlice("unavailable")):
                return False
        else:
            if not (
                info_render_human_bytes(writer, unsigned_address + UInt(6 * 8))
                and info_put_literal(writer, StringSlice(" ("))
                and info_render_memory_percent(writer, unsigned_address + UInt(6 * 8))
                and info_put_literal(writer, StringSlice(" host)"))
            ):
                return False
    elif field == 10:
        if (presence & UInt64(1)) == 0:
            if not info_put_literal(writer, StringSlice("unavailable")):
                return False
        elif not (
            info_put_u64(writer, info_unsigned(unsigned_address, 0, 10))
            and info_put_literal(writer, StringSlice(" sockets; RX queue "))
            and info_render_human_bytes(writer, unsigned_address + UInt(11 * 8))
            and info_put_literal(writer, StringSlice(", TX queue "))
            and info_render_human_bytes(writer, unsigned_address + UInt(12 * 8))
        ):
            return False
    elif field == 11:
        if (presence & UInt64(1)) == 0:
            if not info_put_literal(writer, StringSlice("unavailable")):
                return False
        elif not (
            info_put_literal(writer, StringSlice("read "))
            and info_render_human_bytes(writer, unsigned_address + UInt(13 * 8))
            and info_put_literal(writer, StringSlice(" total ("))
            and info_render_human_bytes(writer, unsigned_address + UInt(15 * 8))
            and info_put_literal(writer, StringSlice("/s), write "))
            and info_render_human_bytes(writer, unsigned_address + UInt(14 * 8))
            and info_put_literal(writer, StringSlice(" total ("))
            and info_render_human_bytes(writer, unsigned_address + UInt(16 * 8))
            and info_put_literal(writer, StringSlice("/s)"))
        ):
            return False
    return info_status_finish_field_value(writer, prefix)


def info_render_status_fields(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    for field in range(INFO_STATUS_FIELD_COUNT):
        if not info_render_status_field(
            writer, field, unsigned_address, text_address, presence
        ):
            return False
    return True


def info_status_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def info_render_status_runway(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var profiles = info_unsigned(unsigned_address, 2, 0)
    var current_remaining = info_signed(signed_address, 5, 0)
    var has_reset = (presence & UInt64(1)) != 0
    var has_estimate = (presence & UInt64(2)) != 0
    if profiles == 0:
        return info_put_literal(writer, StringSlice("Unavailable"))
    if current_remaining <= 0:
        return info_put_literal(writer, StringSlice("Exhausted"))
    if not has_estimate:
        return info_put_literal(
            writer,
            StringSlice(
                "Unavailable (no recent quota decay observed in active runtime logs)"
            ),
        )

    var estimate_values = Pointer[
        mut=False, Float64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(unsigned_address + UInt(16)))
    var burn_per_hour = estimate_values[unsafe_offset=0]
    if not isfinite(burn_per_hour) or burn_per_hour < 0.0:
        return False
    var exhaust_at = info_signed(signed_address, 5, 2)
    var now = info_signed(signed_address, 5, 4)
    var exhaust_in = info_status_saturating_sub(exhaust_at, now)
    var observed = info_signed(signed_address, 5, 3)
    var observed_profiles = info_unsigned(unsigned_address, 3, 1)
    var burn_tenths = info_round_nonnegative_tenths(burn_per_hour)
    if has_reset and info_signed(signed_address, 5, 1) <= exhaust_at:
        return (
            info_put_literal(writer, StringSlice("Earliest reset "))
            and info_put_view(writer, info_text(text_address, 2, 0))
            and info_put_literal(
                writer,
                StringSlice(" arrives before the no-reset runway (~"),
            )
            and info_render_relative_duration_value(writer, exhaust_in)
            and info_put_literal(writer, StringSlice(" at "))
            and info_put_one_decimal(writer, burn_tenths)
            and info_put_literal(
                writer,
                StringSlice(" aggregated-%/h, "),
            )
            and info_put_u64(writer, observed_profiles)
            and info_put_literal(writer, StringSlice(" profile(s), observed over "))
            and info_render_relative_duration_value(writer, observed)
            and info_put_byte(writer, UInt8(41))
        )

    return (
        info_put_view(writer, info_text(text_address, 2, 1))
        and info_put_literal(writer, StringSlice(" (~"))
        and info_render_relative_duration_value(writer, exhaust_in)
        and info_put_literal(writer, StringSlice(") at "))
        and info_put_one_decimal(writer, burn_tenths)
        and info_put_literal(writer, StringSlice(" aggregated-%/h from "))
        and info_put_u64(writer, observed_profiles)
        and info_put_literal(writer, StringSlice(" profile(s), observed over "))
        and info_render_relative_duration_value(writer, observed)
        and info_put_literal(writer, StringSlice(", no-reset estimate"))
    )


def info_render_status_profile_filter(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    unsigned_count: Int64,
) -> Bool:
    var first = True
    for index in range(unsigned_count):
        var eligible = info_unsigned(unsigned_address, unsigned_count, index)
        if eligible != 0 and eligible != 1:
            return False
        if eligible == 1:
            if not first and not info_put_byte(writer, UInt8(44)):
                return False
            if not info_put_u64(writer, UInt64(index)):
                return False
            first = False
    return True


def info_render_status_token_plan(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
    text_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    event_count: Int64,
) -> Bool:
    # ponytail: O(events * 64) bounded history selection; use merge sort if cap grows.
    var scratch_bytes = INFO_STATUS_HISTORY_LIMIT * 8
    if output_capacity < scratch_bytes:
        return False
    var scratch = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address + UInt(output_capacity - scratch_bytes))
    )
    var first_index: Int64 = 0
    var latest_index: Int64 = 0
    var kept: Int64 = 0
    for candidate in range(event_count):
        if candidate == 0:
            first_index = candidate
            latest_index = candidate
        else:
            if info_status_event_compare(
                candidate, first_index, unsigned_address, text_address
            ) < 0:
                first_index = candidate
            if info_status_event_compare(
                candidate, latest_index, unsigned_address, text_address
            ) >= 0:
                latest_index = candidate

        var position = kept
        while position > 0 and info_status_event_compare(
            candidate,
            Int64(scratch[unsafe_offset=position - 1]),
            unsigned_address,
            text_address,
        ) < 0:
            position -= 1

        if kept < INFO_STATUS_HISTORY_LIMIT:
            var shift = kept
            while shift > position:
                scratch[unsafe_offset=shift] = scratch[unsafe_offset=shift - 1]
                shift -= 1
            scratch[unsafe_offset=position] = UInt64(candidate)
            kept += 1
        elif position == INFO_STATUS_HISTORY_LIMIT:
            for shift in range(INFO_STATUS_HISTORY_LIMIT - 1):
                scratch[unsafe_offset=shift] = scratch[unsafe_offset=shift + 1]
            scratch[unsafe_offset=INFO_STATUS_HISTORY_LIMIT - 1] = UInt64(candidate)
        elif position > 0:
            # ponytail: bounded insertion keeps the latest 64 events; sort cost is capped.
            for shift in range(INFO_STATUS_HISTORY_LIMIT - 1):
                scratch[unsafe_offset=shift] = scratch[unsafe_offset=shift + 1]
            var insert = position - 1
            var shift = INFO_STATUS_HISTORY_LIMIT - 1
            while shift > insert:
                scratch[unsafe_offset=shift] = scratch[unsafe_offset=shift - 1]
                shift -= 1
            scratch[unsafe_offset=insert] = UInt64(candidate)

    if event_count == 0:
        if not info_put_literal(writer, StringSlice("-;-")):
            return False
    elif not (
        info_put_u64(writer, UInt64(first_index))
        and info_put_byte(writer, UInt8(59))
        and info_put_u64(writer, UInt64(latest_index))
    ):
        return False
    if not (
        info_put_byte(writer, UInt8(59))
        and info_put_u64(writer, UInt64(kept))
        and info_put_byte(writer, UInt8(59))
    ):
        return False

    for index in range(kept):
        if index > 0 and not info_put_byte(writer, UInt8(44)):
            return False
        var event_index = Int64(scratch[unsafe_offset=index])
        var input_tokens = info_unsigned(unsigned_address, 0, event_index * 4 + 2)
        var output_tokens = info_unsigned(unsigned_address, 0, event_index * 4 + 3)
        var total = UINT64_MAX
        if input_tokens <= UINT64_MAX - output_tokens:
            total = input_tokens + output_tokens
        if not info_put_u64(writer, total):
            return False
    return True


def info_render_status_runtime_profile(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
    signed_count: Int64,
    unsigned_address: UInt,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
) -> Bool:
    if info_unsigned(unsigned_address, 1, 0) == 0:
        return info_put_byte(writer, UInt8(48))
    var latest: Int64 = -1
    for index in range(signed_count):
        if latest < 0 or info_signed(signed_address, signed_count, index) >= info_signed(
            signed_address, signed_count, latest
        ):
            latest = index
    if latest >= 0:
        return info_put_u64(writer, UInt64(latest + 2))
    if (presence & UInt64(1)) != 0:
        return info_put_byte(writer, UInt8(49))
    return info_put_byte(writer, UInt8(48))


def info_status_u64_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right


def info_render_status_resource_metrics(
    unsigned_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
) -> Int64:
    if output_capacity < 32:
        return INFO_RENDER_CAPACITY
    if (output_address % UInt(8)) != 0:
        return INFO_RENDER_INVALID
    var input = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(unsigned_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var cpu_output = Pointer[mut=True, Float64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address + UInt(8))
    )
    output[unsafe_offset=0] = UInt64(0)
    cpu_output[unsafe_offset=0] = 0.0
    output[unsafe_offset=2] = UInt64(0)
    output[unsafe_offset=3] = UInt64(0)

    var previous_present = input[unsafe_offset=0]
    var previous_available = input[unsafe_offset=1]
    var current_available = input[unsafe_offset=2]
    if previous_present > 1 or previous_available > 1 or current_available > 1:
        return INFO_RENDER_INVALID
    if previous_present == 0 and previous_available != 0:
        return INFO_RENDER_INVALID
    if previous_present == 1 and previous_available == 1 and current_available == 1:
        var elapsed_values = Pointer[mut=False, Float64, ImmUntrackedOrigin](
            unsafe_from_address=Int(unsigned_address + UInt(11 * 8))
        )
        var seconds = elapsed_values[unsafe_offset=0]
        if not isfinite(seconds) or seconds < 0.0:
            return INFO_RENDER_INVALID
        if seconds < 0.001:
            seconds = 0.001

        var previous_process = input[unsafe_offset=3]
        var previous_system = input[unsafe_offset=4]
        var previous_read = input[unsafe_offset=5]
        var previous_write = input[unsafe_offset=6]
        var process_delta = UInt64(0)
        var system_delta = UInt64(0)
        var read_delta = UInt64(0)
        var write_delta = UInt64(0)
        if input[unsafe_offset=7] > previous_process:
            process_delta = input[unsafe_offset=7] - previous_process
        if input[unsafe_offset=8] > previous_system:
            system_delta = input[unsafe_offset=8] - previous_system
        if input[unsafe_offset=9] > previous_read:
            read_delta = input[unsafe_offset=9] - previous_read
        if input[unsafe_offset=10] > previous_write:
            write_delta = input[unsafe_offset=10] - previous_write

        if system_delta > 0:
            var cpu = Float64(process_delta) / Float64(system_delta) * 100.0
            if cpu < 0.0:
                cpu = 0.0
            if cpu > 100.0:
                cpu = 100.0
            output[unsafe_offset=0] = UInt64(1)
            cpu_output[unsafe_offset=0] = cpu

        var read_rate = Float64(read_delta) / seconds
        var write_rate = Float64(write_delta) / seconds
        if not isfinite(read_rate) or not isfinite(write_rate):
            return INFO_RENDER_INVALID
        output[unsafe_offset=2] = (
            UINT64_MAX if read_rate >= Float64(UINT64_MAX) else UInt64(read_rate)
        )
        output[unsafe_offset=3] = (
            UINT64_MAX if write_rate >= Float64(UINT64_MAX) else UInt64(write_rate)
        )
    return INFO_RENDER_OK


def info_render_status_resource_history(
    unsigned_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    presence: UInt64,
) -> Int64:
    if output_capacity < 32:
        return INFO_RENDER_CAPACITY
    if (output_address % UInt(8)) != 0:
        return INFO_RENDER_INVALID
    var input = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(unsigned_address)
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var cpu: UInt64 = 0
    if presence == 1:
        var cpu_values = Pointer[mut=False, Float64, ImmUntrackedOrigin](
            unsafe_from_address=Int(unsigned_address)
        )
        var percent = cpu_values[unsafe_offset=0]
        if not isfinite(percent) or percent < 0.0 or percent > 100.0:
            return INFO_RENDER_INVALID
        cpu = UInt64(percent + 0.5)
    output[unsafe_offset=0] = cpu
    output[unsafe_offset=1] = input[unsafe_offset=1]
    output[unsafe_offset=2] = info_status_u64_saturating_add(
        input[unsafe_offset=2], input[unsafe_offset=3]
    )
    output[unsafe_offset=3] = info_status_u64_saturating_add(
        input[unsafe_offset=4], input[unsafe_offset=5]
    )
    return INFO_RENDER_OK


def info_render_status_quota_gauge(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    signed_address: UInt,
    unsigned_address: UInt,
    text_address: UInt,
    presence: UInt64,
) -> Bool:
    var profiles = info_unsigned(unsigned_address, 1, 0)
    var remaining = info_signed(signed_address, 3, 0)
    var average = 0.0
    if profiles > 0:
        average = Float64(remaining) / Float64(profiles)
        if average < 0.0:
            average = 0.0
        if average > 100.0:
            average = 100.0
    var ratio = UInt64(average + 0.5)
    var band: UInt64 = 2
    if average <= 10.0:
        band = 0
    elif average <= 25.0:
        band = 1
    if not (
        info_put_u64(writer, band)
        and info_put_byte(writer, UInt8(59))
        and info_put_u64(writer, ratio)
        and info_put_byte(writer, UInt8(59))
    ):
        return False
    if profiles == 0:
        return info_put_literal(writer, StringSlice("quota unavailable"))

    if not (
        info_put_u64(writer, ratio)
        and info_put_literal(writer, StringSlice("% avg · pool "))
        and info_put_i64(writer, remaining)
        and info_put_literal(writer, StringSlice("% · "))
    ):
        return False
    if (presence & UInt64(1)) == 0:
        return info_put_literal(writer, StringSlice("reset unknown"))
    if not (
        info_put_literal(writer, StringSlice("reset in "))
        and info_render_relative_duration_value(
            writer,
            info_status_saturating_sub(
                info_signed(signed_address, 3, 1),
                info_signed(signed_address, 3, 2),
            ),
        )
        and info_put_literal(writer, StringSlice(" ("))
        and info_put_view(writer, info_text(text_address, 1, 0))
        and info_put_byte(writer, UInt8(41))
    ):
        return False
    return True


def info_render_runtime_quota_hint(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    text_address: UInt,
) -> Bool:
    return (
        info_put_literal(writer, StringSlice("Inspect with `prodex quota --profile "))
        and info_put_view(writer, info_text(text_address, 1, 0))
        and info_put_literal(
            writer,
            StringSlice(
                "` or bypass with `prodex run --skip-quota-check`."
            ),
        )
    )


def info_doctor_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def info_doctor_contains(
    view: ProdexRichStringView, needle: StringSlice, lowercase: Bool
) -> Bool:
    var needle_length = Int64(needle.byte_length())
    if needle_length == 0:
        return True
    if view.len < UInt(needle_length):
        return False
    var source = rich_view_ptr(view)
    var wanted = needle.unsafe_ptr()
    var last_start = Int64(view.len) - needle_length
    for start in range(last_start + 1):
        var matches = True
        for offset in range(needle_length):
            var left = source[unsafe_offset=start + offset]
            var right = wanted[unsafe_offset=offset]
            if lowercase:
                left = info_doctor_ascii_lower(left)
                right = info_doctor_ascii_lower(right)
            if left != right:
                matches = False
                break
        if matches:
            return True
    return False


def info_render_doctor_value_color(
    writer: Pointer[mut=True, InfoRenderWriter, _], text_address: UInt
) -> Bool:
    var label = info_text(text_address, 2, 0)
    var value = info_text(text_address, 2, 1)
    if (
        info_doctor_contains(value, StringSlice("error"), True)
        or info_doctor_contains(value, StringSlice("missing"), True)
        or info_doctor_contains(value, StringSlice("blocked"), True)
        or info_doctor_contains(value, StringSlice("warning"), True)
        or info_doctor_contains(value, StringSlice("orphan"), True)
        or info_doctor_contains(value, StringSlice("critical"), True)
        or info_doctor_contains(value, StringSlice("thin"), True)
        or info_doctor_contains(value, StringSlice("degraded"), True)
    ):
        return info_put_u64(writer, UInt64(1))
    if (
        info_doctor_contains(value, StringSlice("ready"), True)
        or info_doctor_contains(value, StringSlice("yes"), True)
        or info_doctor_contains(value, StringSlice("exists"), True)
    ):
        return info_put_u64(writer, UInt64(2))
    if (
        info_doctor_contains(label, StringSlice("Runtime"), False)
        or info_doctor_contains(label, StringSlice("Quota"), False)
        or info_doctor_contains(label, StringSlice("Main"), False)
    ):
        return info_put_u64(writer, UInt64(3))
    return info_put_u64(writer, UInt64(0))


def info_render_doctor_viewport(
    writer: Pointer[mut=True, InfoRenderWriter, _],
    unsigned_address: UInt,
) -> Bool:
    var max_rows = info_unsigned(unsigned_address, 0, 0)
    var line_count = info_unsigned(unsigned_address, 0, 1)
    var visible = Int64(line_count)
    var hidden: UInt64 = 0
    var critical: Int64 = -1
    if line_count > max_rows:
        if max_rows == 0:
            visible = 0
            hidden = line_count
        else:
            visible = Int64(max_rows - 1)
            hidden = line_count - UInt64(visible)
            for index in range(visible, Int64(line_count)):
                if info_unsigned(unsigned_address, 0, 2 + index) == 1:
                    critical = index
                    break
    return (
        info_put_i64(writer, visible)
        and info_put_byte(writer, UInt8(44))
        and info_put_u64(writer, hidden)
        and info_put_byte(writer, UInt8(44))
        and info_put_i64(writer, critical)
    )


@export("prodex_terminal_info_render_v1")
def prodex_terminal_info_render_v1(
    abi_version: Int64,
    operation: Int64,
    signed_address: UInt,
    signed_count: Int64,
    unsigned_address: UInt,
    unsigned_count: Int64,
    text_address: UInt,
    text_count: Int64,
    presence: UInt64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != INFO_RENDER_ABI_VERSION
        or operation < INFO_RENDER_RELATIVE_DURATION
        or operation > INFO_RENDER_DOCTOR_VIEWPORT
        or signed_count < 0
        or unsigned_count < 0
        or text_count < 0
        or (signed_count > 0 and signed_address == 0)
        or (unsigned_count > 0 and unsigned_address == 0)
        or not info_validate_texts(text_address, text_count)
        or output_capacity < 0
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return INFO_RENDER_INVALID

    var required_signed: Int64 = 0
    var required_unsigned: Int64 = 0
    var required_text: Int64 = 0
    if operation == INFO_RENDER_DOCTOR_VALUE_COLOR:
        if signed_count != 0 or unsigned_count != 0 or text_count != 2 or presence != 0:
            return INFO_RENDER_INVALID
        required_text = 2
    elif operation == INFO_RENDER_DOCTOR_VIEWPORT:
        if signed_count != 0 or text_count != 0 or presence != 0 or unsigned_count < 2:
            return INFO_RENDER_INVALID
        var line_count = info_unsigned(unsigned_address, unsigned_count, 1)
        if (
            line_count > UInt64(INT64_MAX)
            or UInt64(unsigned_count - 2) != line_count
        ):
            return INFO_RENDER_INVALID
        var index: Int64 = 2
        while index < unsigned_count:
            if info_unsigned(unsigned_address, unsigned_count, index) > 1:
                return INFO_RENDER_INVALID
            index += 1
        required_unsigned = 2
    elif operation == INFO_RENDER_STATUS_PROFILE_FILTER:
        if signed_count != 0 or text_count != 0:
            return INFO_RENDER_INVALID
        for index in range(unsigned_count):
            if info_unsigned(unsigned_address, unsigned_count, index) > 1:
                return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_TOKEN_PLAN:
        if signed_count != 0 or text_count % 2 != 0 or text_count > INT64_MAX / 2:
            return INFO_RENDER_INVALID
        if unsigned_count != text_count * 2:
            return INFO_RENDER_INVALID
        for row in range(text_count // 2):
            var request_present = info_unsigned(unsigned_address, unsigned_count, row * 4)
            var request_value = info_unsigned(unsigned_address, unsigned_count, row * 4 + 1)
            if request_present > 1 or (request_present == 0 and request_value != 0):
                return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_RUNTIME_PROFILE:
        if (
            unsigned_count != 1
            or text_count < 2
            or signed_count != text_count - 2
            or presence > 1
            or info_unsigned(unsigned_address, unsigned_count, 0) > 1
        ):
            return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_RUNWAY:
        required_signed = 5
        required_unsigned = 3
        required_text = 2
        if signed_count != 5 or unsigned_count != 3 or text_count != 2 or presence > 3:
            return INFO_RENDER_INVALID
        if (presence & UInt64(1)) != 0 and info_text(text_address, text_count, 0).len == 0:
            return INFO_RENDER_INVALID
        if (presence & UInt64(2)) != 0:
            var burn_values = Pointer[
                mut=False, Float64, ImmUntrackedOrigin
            ](unsafe_from_address=Int(unsigned_address + UInt(16)))
            if not isfinite(burn_values[unsafe_offset=0]) or burn_values[unsafe_offset=0] <= 0.0:
                return INFO_RENDER_INVALID
            if info_text(text_address, text_count, 1).len == 0:
                return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_FIELDS:
        required_unsigned = 18
        required_text = 12
        if text_count != 12 or presence > 15 or unsigned_count != 18:
            return INFO_RENDER_INVALID
        if info_unsigned(unsigned_address, unsigned_count, 1) > info_unsigned(
            unsigned_address, unsigned_count, 0
        ) or info_unsigned(unsigned_address, unsigned_count, 2) > info_unsigned(
            unsigned_address, unsigned_count, 1
        ):
            return INFO_RENDER_INVALID
        if (presence & UInt64(2)) != 0:
            if (presence & UInt64(1)) == 0:
                return INFO_RENDER_INVALID
            var cpu_values = Pointer[
                mut=False, Float64, ImmUntrackedOrigin
            ](unsafe_from_address=Int(unsigned_address + UInt(17 * 8)))
            var cpu_percent = cpu_values[unsafe_offset=0]
            if not isfinite(cpu_percent) or cpu_percent < 0.0 or cpu_percent > 100.0:
                return INFO_RENDER_INVALID
        if (presence & UInt64(1)) != 0:
            if info_unsigned(unsigned_address, unsigned_count, 9) > info_unsigned(
                unsigned_address, unsigned_count, 8
            ):
                return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_RESOURCE_METRICS:
        required_unsigned = 12
        if signed_count != 0 or unsigned_count != 12 or text_count != 0 or presence != 0:
            return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_RESOURCE_HISTORY:
        required_unsigned = 6
        if signed_count != 0 or unsigned_count != 6 or text_count != 0 or presence > 1:
            return INFO_RENDER_INVALID
        if presence == 1:
            var cpu_values = Pointer[mut=False, Float64, ImmUntrackedOrigin](
                unsafe_from_address=Int(unsigned_address)
            )
            var percent = cpu_values[unsafe_offset=0]
            if not isfinite(percent) or percent < 0.0 or percent > 100.0:
                return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_STATUS_QUOTA_GAUGE:
        required_signed = 3
        required_unsigned = 1
        required_text = 1
        if signed_count != 3 or unsigned_count != 1 or text_count != 1 or presence > 1:
            return INFO_RENDER_INVALID
        if (presence & UInt64(1)) != 0 and info_text(text_address, text_count, 0).len == 0:
            return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_RELATIVE_DURATION:
        required_signed = 1
    elif operation == INFO_RENDER_QUOTA_DATA:
        required_unsigned = 4
    elif operation == INFO_RENDER_RUNTIME_POLICY:
        required_unsigned = 1
        required_text = 1
    elif operation == INFO_RENDER_RUNTIME_LOGS:
        required_text = 2
    elif operation == INFO_RENDER_TUNING_WORKERS:
        required_unsigned = 16
    elif operation == INFO_RENDER_TUNING_BUDGETS:
        required_unsigned = 10
    elif operation == INFO_RENDER_TUNING_TRANSPORT:
        required_unsigned = 9
    elif operation == INFO_RENDER_POOL_REMAINING:
        required_signed = 1
        required_unsigned = 1
        required_text = 1
    elif operation == INFO_RENDER_PROCESS_SUMMARY:
        required_unsigned = 3
    elif operation == INFO_RENDER_LOAD_SUMMARY:
        required_signed = 3
        required_unsigned = 4
    elif (
        operation == INFO_RENDER_RUNTIME_LAUNCH_SELECTION
        or operation == INFO_RENDER_RUNTIME_LAUNCH_WARNING
    ):
        required_text = 4
        if presence > INFO_RUNTIME_LAUNCH_STATUS_PROBE_FAILED:
            return INFO_RENDER_INVALID
    elif operation == INFO_RENDER_RUNTIME_PROVIDER_DIRECT:
        required_text = 2
    elif operation == INFO_RENDER_RUNTIME_QUOTA_HINT:
        required_text = 1
    elif operation == INFO_RENDER_HUMAN_BYTES or operation == INFO_RENDER_HUMAN_COUNT:
        required_unsigned = 1
    elif operation == INFO_RENDER_TOKEN_EFFICIENCY:
        required_unsigned = 3
    elif operation == INFO_RENDER_MEMORY_PERCENT:
        required_unsigned = 2
    elif operation == INFO_RENDER_TEXT_SPARKLINE:
        required_unsigned = 0
    else:
        required_unsigned = 6
        if text_count > 4 or unsigned_count < 6 + text_count * 4:
            return INFO_RENDER_INVALID
    if (
        signed_count < required_signed
        or unsigned_count < required_unsigned
        or text_count < required_text
    ):
        return INFO_RENDER_INVALID

    if operation == INFO_RENDER_STATUS_RESOURCE_METRICS or operation == INFO_RENDER_STATUS_RESOURCE_HISTORY:
        if output_capacity < 32 or (output_address % UInt(8)) != 0:
            return INFO_RENDER_INVALID
        var result = INFO_RENDER_INVALID
        if operation == INFO_RENDER_STATUS_RESOURCE_METRICS:
            result = info_render_status_resource_metrics(
                unsigned_address, output_address, output_capacity
            )
        else:
            result = info_render_status_resource_history(
                unsigned_address, output_address, output_capacity, presence
            )
        if result != INFO_RENDER_OK:
            return result
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = 32
        return INFO_RENDER_OK

    var writer_capacity = output_capacity
    if operation == INFO_RENDER_STATUS_TOKEN_PLAN:
        if output_capacity < INFO_STATUS_HISTORY_LIMIT * 8:
            return INFO_RENDER_INVALID
        writer_capacity -= INFO_STATUS_HISTORY_LIMIT * 8
        if (output_address % UInt(8)) != 0 or (output_capacity % 8) != 0:
            return INFO_RENDER_INVALID
    var writer = InfoRenderWriter(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        writer_capacity,
        0,
    )
    var ok = False
    if operation == INFO_RENDER_RELATIVE_DURATION:
        ok = info_render_relative_duration(Pointer(to=writer), signed_address)
    elif operation == INFO_RENDER_QUOTA_DATA:
        ok = info_render_quota_data(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_RUNTIME_POLICY:
        ok = info_render_runtime_policy(
            Pointer(to=writer), unsigned_address, text_address, presence
        )
    elif operation == INFO_RENDER_RUNTIME_LOGS:
        ok = info_render_runtime_logs(Pointer(to=writer), text_address)
    elif operation == INFO_RENDER_TUNING_WORKERS:
        ok = info_render_tuning_workers(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_TUNING_BUDGETS:
        ok = info_render_tuning_budgets(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_TUNING_TRANSPORT:
        ok = info_render_tuning_transport(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_POOL_REMAINING:
        ok = info_render_pool_remaining(
            Pointer(to=writer),
            signed_address,
            unsigned_address,
            text_address,
            presence,
        )
    elif operation == INFO_RENDER_PROCESS_SUMMARY:
        ok = info_render_process_summary(
            Pointer(to=writer), unsigned_address, text_address, text_count
        )
    elif operation == INFO_RENDER_LOAD_SUMMARY:
        ok = info_render_load_summary(
            Pointer(to=writer), signed_address, unsigned_address, presence
        )
    elif operation == INFO_RENDER_RUNTIME_LAUNCH_SELECTION:
        ok = info_render_runtime_launch_selection(
            Pointer(to=writer), text_address, presence
        )
    elif operation == INFO_RENDER_RUNTIME_LAUNCH_WARNING:
        ok = info_render_runtime_launch_warning(
            Pointer(to=writer), text_address, presence
        )
    elif operation == INFO_RENDER_RUNTIME_PROVIDER_DIRECT:
        ok = info_render_runtime_provider_direct(Pointer(to=writer), text_address)
    elif operation == INFO_RENDER_RUNTIME_QUOTA_HINT:
        ok = info_render_runtime_quota_hint(Pointer(to=writer), text_address)
    elif operation == INFO_RENDER_HUMAN_BYTES:
        ok = info_render_human_bytes(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_HUMAN_COUNT:
        ok = info_render_human_count(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_TOKEN_EFFICIENCY:
        ok = info_render_token_efficiency(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_MEMORY_PERCENT:
        ok = info_render_memory_percent(Pointer(to=writer), unsigned_address)
    elif operation == INFO_RENDER_TEXT_SPARKLINE:
        ok = info_render_text_sparkline(
            Pointer(to=writer), unsigned_address, unsigned_count
        )
    elif operation == INFO_RENDER_STATUS_PROFILE_FILTER:
        ok = info_render_status_profile_filter(
            Pointer(to=writer), unsigned_address, unsigned_count
        )
    elif operation == INFO_RENDER_STATUS_TOKEN_PLAN:
        ok = info_render_status_token_plan(
            Pointer(to=writer),
            unsigned_address,
            text_address,
            output_address,
            output_capacity,
            text_count // 2,
        )
    elif operation == INFO_RENDER_STATUS_RUNTIME_PROFILE:
        ok = info_render_status_runtime_profile(
            Pointer(to=writer),
            signed_address,
            signed_count,
            unsigned_address,
            text_address,
            text_count,
            presence,
        )
    elif operation == INFO_RENDER_STATUS_RUNWAY:
        ok = info_render_status_runway(
            Pointer(to=writer), signed_address, unsigned_address, text_address, presence
        )
    elif operation == INFO_RENDER_STATUS_FIELDS:
        ok = info_render_status_fields(
            Pointer(to=writer), unsigned_address, text_address, presence
        )
    elif operation == INFO_RENDER_STATUS_QUOTA_GAUGE:
        ok = info_render_status_quota_gauge(
            Pointer(to=writer), signed_address, unsigned_address, text_address, presence
        )
    elif operation == INFO_RENDER_DOCTOR_VALUE_COLOR:
        ok = info_render_doctor_value_color(Pointer(to=writer), text_address)
    elif operation == INFO_RENDER_DOCTOR_VIEWPORT:
        ok = info_render_doctor_viewport(Pointer(to=writer), unsigned_address)
    else:
        ok = info_render_token_usage(
            Pointer(to=writer), unsigned_address, text_address, text_count
        )

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return INFO_RENDER_OK if ok else INFO_RENDER_CAPACITY
