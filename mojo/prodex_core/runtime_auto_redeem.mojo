from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr
from runtime_math import INT64_MAX, INT64_MIN


comptime RUNTIME_AUTO_REDEEM_FIELD_COUNT: Int64 = 6
comptime RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT: Int64 = 256
comptime RUNTIME_AUTO_REDEEM_MAX_PLAN_BYTES: Int64 = 4096
comptime RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS: Int64 = 300
comptime RUNTIME_AUTO_REDEEM_STATUS_OK: Int64 = 0
comptime RUNTIME_AUTO_REDEEM_STATUS_INVALID: Int64 = 1
comptime RUNTIME_AUTO_REDEEM_STATUS_TEXT: Int64 = 2
comptime RUNTIME_AUTO_REDEEM_STATUS_ABI: Int64 = 4


def runtime_auto_redeem_field(
    fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64
) -> Int64:
    return fields[
        unsafe_offset=(index * RUNTIME_AUTO_REDEEM_FIELD_COUNT) + field
    ]


def runtime_auto_redeem_plan_matches[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var pointer = rich_view_ptr(view)
    for index in range(bounds[0]):
        var value = pointer[unsafe_offset=index]
        if value >= 0x1C and value <= 0x1F:
            return False
    for index in range(bounds[1], Int64(view.len)):
        var value = pointer[unsafe_offset=index]
        if value >= 0x1C and value <= 0x1F:
            return False
    var expected = literal.unsafe_ptr()
    var expected_index: Int64 = 0
    var expected_length = Int64(literal.byte_length())
    for index in range(bounds[0], bounds[1]):
        var value = pointer[unsafe_offset=index]
        if value == 32 or value == 45 or value == 95:
            continue
        if expected_index >= expected_length:
            return False
        if value >= 65 and value <= 90:
            value += 32
        if value != expected[unsafe_offset=expected_index]:
            return False
        expected_index += 1
    return expected_index == expected_length


def runtime_auto_redeem_plan_priority(view: ProdexRichStringView) -> Int64:
    if runtime_auto_redeem_plan_matches["plus"](view):
        return 0
    if runtime_auto_redeem_plan_matches["free"](view) or runtime_auto_redeem_plan_matches[
        "basic"
    ](view):
        return 1
    if runtime_auto_redeem_plan_matches[
        "prolite"
    ](view) or runtime_auto_redeem_plan_matches["pro"](view) or runtime_auto_redeem_plan_matches[
        "pro5x"
    ](view) or runtime_auto_redeem_plan_matches["5x"](view) or runtime_auto_redeem_plan_matches[
        "pro20x"
    ](view) or runtime_auto_redeem_plan_matches["pro20"](view) or runtime_auto_redeem_plan_matches[
        "20x"
    ](view) or runtime_auto_redeem_plan_matches["ultra"](view) or runtime_auto_redeem_plan_matches[
        "max"
    ](view) or runtime_auto_redeem_plan_matches["team"](view) or runtime_auto_redeem_plan_matches[
        "business"
    ](view) or runtime_auto_redeem_plan_matches["enterprise"](view):
        return 3
    return 2


def runtime_auto_redeem_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def runtime_auto_redeem_saturating_neg(value: Int64) -> Int64:
    if value == INT64_MIN:
        return INT64_MAX
    return -value


def runtime_auto_redeem_candidate_eligible(
    fields: Pointer[mut=False, Int64, _], index: Int64, now: Int64
) -> Bool:
    if runtime_auto_redeem_field(fields, index, 0) <= 0:
        return False
    if runtime_auto_redeem_field(fields, index, 1) != 3:
        return False
    var reset_at = runtime_auto_redeem_field(fields, index, 2)
    if reset_at == INT64_MAX:
        return False
    return (
        runtime_auto_redeem_saturating_sub(reset_at, now)
        > RUNTIME_AUTO_REDEEM_NATURAL_RESET_GRACE_SECONDS
    )


def runtime_auto_redeem_candidate_less(
    plan_types: Pointer[mut=False, ProdexRichStringView, _],
    fields: Pointer[mut=False, Int64, _],
    left: Int64,
    right: Int64,
) -> Bool:
    var left_value = runtime_auto_redeem_plan_priority(plan_types[unsafe_offset=left])
    var right_value = runtime_auto_redeem_plan_priority(plan_types[unsafe_offset=right])
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_auto_redeem_saturating_neg(
        runtime_auto_redeem_field(fields, left, 2)
    )
    right_value = runtime_auto_redeem_saturating_neg(
        runtime_auto_redeem_field(fields, right, 2)
    )
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_auto_redeem_field(fields, left, 3)
    right_value = runtime_auto_redeem_field(fields, right, 3)
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_auto_redeem_field(fields, left, 4)
    right_value = runtime_auto_redeem_field(fields, right, 4)
    if left_value != right_value:
        return left_value < right_value

    left_value = runtime_auto_redeem_field(fields, left, 5)
    right_value = runtime_auto_redeem_field(fields, right, 5)
    if left_value != right_value:
        return left_value < right_value
    return left < right


@export("prodex_runtime_auto_redeem_plan_batch")
def prodex_runtime_auto_redeem_plan_batch(
    abi_version: Int64,
    plan_types_address: UInt,
    fields_address: UInt,
    selected_index_address: UInt,
    count: Int64,
    now: Int64,
) abi("C") -> Int64:
    if abi_version != 6:
        return RUNTIME_AUTO_REDEEM_STATUS_ABI
    if count < 0 or count > RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT:
        return RUNTIME_AUTO_REDEEM_STATUS_INVALID
    if count == 0:
        return RUNTIME_AUTO_REDEEM_STATUS_OK
    if plan_types_address == 0 or fields_address == 0 or selected_index_address == 0:
        return RUNTIME_AUTO_REDEEM_STATUS_INVALID

    var plan_types = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(plan_types_address))
    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var selected_index = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(selected_index_address)
    )

    var selected: Int64 = -1
    for index in range(count):
        var plan = plan_types[unsafe_offset=index].copy()
        if not rich_view_valid(plan, RUNTIME_AUTO_REDEEM_MAX_PLAN_BYTES):
            return RUNTIME_AUTO_REDEEM_STATUS_TEXT
        var weekly_status = runtime_auto_redeem_field(fields, index, 1)
        var inflight_count = runtime_auto_redeem_field(fields, index, 3)
        var health_sort_key = runtime_auto_redeem_field(fields, index, 4)
        var order_index = runtime_auto_redeem_field(fields, index, 5)
        if (
            weekly_status < 0
            or weekly_status > 4
            or inflight_count < 0
            or health_sort_key < 0
            or order_index < 0
        ):
            return RUNTIME_AUTO_REDEEM_STATUS_INVALID
        if not runtime_auto_redeem_candidate_eligible(fields, index, now):
            continue
        if selected < 0 or runtime_auto_redeem_candidate_less(
            plan_types, fields, index, selected
        ):
            selected = index
    selected_index[unsafe_offset=0] = selected
    return RUNTIME_AUTO_REDEEM_STATUS_OK
