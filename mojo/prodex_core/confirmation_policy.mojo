from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime CONFIRMATION_POLICY_ABI_VERSION: Int64 = 1
comptime CONFIRMATION_POLICY_REDEEM: Int64 = 0
comptime CONFIRMATION_POLICY_YES_NO: Int64 = 1


def confirmation_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def confirmation_range_equals[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if confirmation_ascii_lower(source[unsafe_offset=start + index]) != wanted[unsafe_offset=index]:
            return False
    return True


@export("prodex_confirmation_policy_v1")
def prodex_confirmation_policy_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
    default_value: Int64,
) abi("C") -> Int64:
    if abi_version != CONFIRMATION_POLICY_ABI_VERSION:
        return -2
    if (
        operation < CONFIRMATION_POLICY_REDEEM
        or operation > CONFIRMATION_POLICY_YES_NO
        or length < 0
        or default_value < 0
        or default_value > 1
        or (length > 0 and address == 0)
    ):
        return -2

    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, length):
        return -2
    var bounds = rich_trim_bounds(view)
    if bounds[0] == bounds[1]:
        if operation == CONFIRMATION_POLICY_REDEEM:
            return 0
        return default_value

    if (
        confirmation_range_equals["y"](view, bounds[0], bounds[1])
        or confirmation_range_equals["yes"](view, bounds[0], bounds[1])
    ):
        return 1
    if (
        confirmation_range_equals["n"](view, bounds[0], bounds[1])
        or confirmation_range_equals["no"](view, bounds[0], bounds[1])
    ):
        return 0
    return -1
