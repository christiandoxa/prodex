from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime PROFILE_UI_ABI_VERSION: Int64 = 1
comptime PROFILE_UI_INVALID: Int64 = 1
comptime PROFILE_UI_MAX_INT64: Int64 = 9_223_372_036_854_775_807

# Output color classes consumed by the host renderer.
comptime PROFILE_UI_COLOR_RESET: Int64 = 0
comptime PROFILE_UI_COLOR_RED: Int64 = 1
comptime PROFILE_UI_COLOR_GREEN: Int64 = 2
comptime PROFILE_UI_COLOR_CYAN: Int64 = 3


def profile_ascii_lower(byte: UInt8) -> UInt8:
    if byte >= 65 and byte <= 90:
        return byte + 32
    return byte


def profile_ascii_contains(
    value: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var ptr = rich_view_ptr(value)
    var literal_ptr = literal.unsafe_ptr()
    var literal_len = Int64(literal.byte_length())
    if literal_len == 0:
        return True
    if value.len < UInt(literal_len):
        return False
    for start in range(Int64(value.len) - literal_len + 1):
        var matched = True
        for offset in range(literal_len):
            if profile_ascii_lower(ptr[unsafe_offset=start + offset]) != profile_ascii_lower(
                literal_ptr[unsafe_offset=offset]
            ):
                matched = False
                break
        if matched:
            return True
    return False


def profile_ascii_equal(
    value: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var ptr = rich_view_ptr(value)
    var literal_ptr = literal.unsafe_ptr()
    var literal_len = Int64(literal.byte_length())
    if value.len != UInt(literal_len):
        return False
    for index in range(literal_len):
        if profile_ascii_lower(ptr[unsafe_offset=index]) != profile_ascii_lower(
            literal_ptr[unsafe_offset=index]
        ):
            return False
    return True


def profile_ascii_exact(
    value: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var ptr = rich_view_ptr(value)
    var literal_ptr = literal.unsafe_ptr()
    var literal_len = Int64(literal.byte_length())
    if value.len != UInt(literal_len):
        return False
    for index in range(literal_len):
        if ptr[unsafe_offset=index] != literal_ptr[unsafe_offset=index]:
            return False
    return True


def profile_value_color(
    label: ProdexRichStringView, value: ProdexRichStringView
) -> Int64:
    if (
        profile_ascii_contains(value, StringSlice("no active"))
        or profile_ascii_contains(value, StringSlice("missing"))
        or profile_ascii_contains(value, StringSlice("error"))
    ):
        return PROFILE_UI_COLOR_RED
    if (
        profile_ascii_contains(value, StringSlice("active"))
        or profile_ascii_equal(value, StringSlice("yes"))
        or profile_ascii_exact(label, StringSlice("Active"))
    ):
        return PROFILE_UI_COLOR_GREEN
    if (
        profile_ascii_exact(label, StringSlice("Provider"))
        or profile_ascii_exact(label, StringSlice("Auth"))
        or profile_ascii_exact(label, StringSlice("Runtime route"))
        or profile_ascii_exact(label, StringSlice("Identity"))
    ):
        return PROFILE_UI_COLOR_CYAN
    return PROFILE_UI_COLOR_RESET


@export("prodex_profile_ui_numeric_v1")
def prodex_profile_ui_numeric_v1(
    abi_version: Int64,
    operation: Int64,
    input0: Int64,
    input1: Int64,
    label_address: UInt,
    label_length: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROFILE_UI_ABI_VERSION
        or output_address == 0
        or input0 < 0
        or input1 < 0
        or label_length < 0
        or value_length < 0
        or (label_length > 0 and label_address == 0)
        or (value_length > 0 and value_address == 0)
    ):
        return PROFILE_UI_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if operation == 0:  # normalize terminal height
        output[unsafe_offset=0] = input0 if input0 != 0 else 24
    elif operation == 1:  # scroll body height
        output[unsafe_offset=0] = max(input0 - 6, 1)
    elif operation == 2:  # scroll maximum offset
        var body = max(input1, 1)
        output[unsafe_offset=0] = max(input0 - body, 0)
    elif operation == 3:  # profile value color class
        var label = ProdexRichStringView(label_address, UInt(label_length))
        var value = ProdexRichStringView(value_address, UInt(value_length))
        if not rich_view_valid(label, 0x7FFFFFFFFFFFFFFF) or not rich_view_valid(
            value, 0x7FFFFFFFFFFFFFFF
        ):
            return PROFILE_UI_INVALID
        output[unsafe_offset=0] = profile_value_color(label, value)
    elif operation == 4:  # bounded inline TUI height
        # An unbounded row count must not overflow before terminal clipping.
        var rows = PROFILE_UI_MAX_INT64 if input0 > PROFILE_UI_MAX_INT64 - 4 else input0 + 4
        rows = max(rows, 4)
        var terminal_height = input1 if input1 != 0 else 24
        output[unsafe_offset=0] = max(min(rows, terminal_height), 1)
    else:
        return PROFILE_UI_INVALID
    return 0
