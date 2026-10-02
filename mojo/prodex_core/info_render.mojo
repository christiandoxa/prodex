from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

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
        or operation > INFO_RENDER_RUNTIME_QUOTA_HINT
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
    if operation == INFO_RENDER_RELATIVE_DURATION:
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

    var writer = InfoRenderWriter(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        output_capacity,
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
    else:
        ok = info_render_token_usage(
            Pointer(to=writer), unsigned_address, text_address, text_count
        )

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return INFO_RENDER_OK if ok else INFO_RENDER_CAPACITY
