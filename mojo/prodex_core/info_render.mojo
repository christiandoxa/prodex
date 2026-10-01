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
    var seconds = max(info_signed(signed_address, 1, 0), Int64(0))
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
        or operation > INFO_RENDER_POOL_REMAINING
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
    else:
        required_signed = 1
        required_unsigned = 1
        required_text = 1
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
    else:
        ok = info_render_pool_remaining(
            Pointer(to=writer),
            signed_address,
            unsigned_address,
            text_address,
            presence,
        )

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return INFO_RENDER_OK if ok else INFO_RENDER_CAPACITY
