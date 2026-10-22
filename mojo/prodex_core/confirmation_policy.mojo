from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from runtime_math import runtime_i64_saturating_sub

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


@export("prodex_manual_redeem_reset_v1")
def prodex_manual_redeem_reset_v1(
    version: Int64, primary_present: Int64, primary_reset: Int64,
    weekly_present: Int64, weekly_reset: Int64, now: Int64, near_seconds: Int64,
) abi("C") -> Int64:
    if version != 1:
        return -2
    if primary_present < 0 or primary_present > 1 or weekly_present < 0 or weekly_present > 1:
        return -1
    if primary_present == 0 and primary_reset != 0 or weekly_present == 0 and weekly_reset != 0:
        return -1
    var selected: Int64 = 0
    if primary_present == 1 and runtime_i64_saturating_sub(primary_reset, now) <= near_seconds:
        selected = 1
    if weekly_present == 1 and runtime_i64_saturating_sub(weekly_reset, now) <= near_seconds:
        if selected == 0 or weekly_reset < primary_reset:
            selected = 2
    return selected
