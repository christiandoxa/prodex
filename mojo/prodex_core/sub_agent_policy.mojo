from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SUB_AGENT_POLICY_ABI_VERSION: Int64 = 1
comptime SUB_AGENT_POLICY_OK: Int64 = 0
comptime SUB_AGENT_POLICY_INVALID: Int64 = 1
comptime SUB_AGENT_POLICY_ABI: Int64 = 4

comptime OP_CONCURRENCY_PARSE: Int64 = 0
comptime OP_CONCURRENCY_VALIDATE: Int64 = 1
comptime OP_REASONING_EFFORT: Int64 = 2
comptime OP_MODEL_NONEMPTY: Int64 = 3
comptime OP_PROVIDER_URL_POLICY: Int64 = 4
comptime OP_CHILD_SPEC_SCALAR_POLICY: Int64 = 5

def sub_agent_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))

def sub_agent_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value

def sub_agent_range_equals[
    literal: StaticString
](
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    case_insensitive: Bool,
) -> Bool:
    var n = Int64(literal.byte_length())
    if start < 0 or end - start != n:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(n):
        var actual = source[unsafe_offset=start + index]
        var expected = wanted[unsafe_offset=index]
        if case_insensitive:
            actual = sub_agent_ascii_lower(actual)
        if actual != expected:
            return False
    return True

def sub_agent_digit(value: UInt8) -> Int64:
    if value == 48:
        return 0
    if value == 49:
        return 1
    if value == 50:
        return 2
    if value == 51:
        return 3
    if value == 52:
        return 4
    if value == 53:
        return 5
    if value == 54:
        return 6
    if value == 55:
        return 7
    if value == 56:
        return 8
    if value == 57:
        return 9
    return -1

def sub_agent_concurrency_source(value: Int64) -> Int64:
    if value == 4 or value == 8 or value == 16 or value == 32:
        return 1
    return 2

def sub_agent_parse_concurrency(
    view: ProdexRichStringView,
    result: Pointer[mut=True, Int64, _],
):
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    if sub_agent_range_equals["default"](view, start, end, False):
        result[0] = 0
        result[1] = 4
        result[2] = 0
        return
    if start >= end:
        result[0] = 1
        return

    var source = rich_view_ptr(view)
    var parsed: Int64 = 0
    var overflow = False
    for index in range(start, end):
        var digit = sub_agent_digit(source[unsafe_offset=index])
        if digit < 0:
            result[0] = 1
            return
        if not overflow:
            if parsed > 6553 or (parsed == 6553 and digit > 5):
                overflow = True
            else:
                parsed = parsed * 10 + digit
    if overflow:
        result[0] = 2
        return
    if parsed < 1 or parsed > 64:
        result[0] = 3
        result[1] = parsed
        return

    result[0] = 0
    result[1] = parsed
    result[2] = sub_agent_concurrency_source(parsed)

def sub_agent_reasoning_effort(
    view: ProdexRichStringView,
    result: Pointer[mut=True, Int64, _],
):
    var bounds = rich_trim_bounds(view)
    var start = bounds[0]
    var end = bounds[1]
    result[0] = 0
    if sub_agent_range_equals["none"](view, start, end, True):
        result[1] = 0
    elif sub_agent_range_equals["minimal"](view, start, end, True):
        result[1] = 1
    elif sub_agent_range_equals["low"](view, start, end, True):
        result[1] = 2
    elif sub_agent_range_equals["medium"](view, start, end, True):
        result[1] = 3
    elif sub_agent_range_equals["high"](view, start, end, True):
        result[1] = 4
    elif sub_agent_range_equals["xhigh"](view, start, end, True):
        result[1] = 5
    elif sub_agent_range_equals["max"](view, start, end, True):
        result[1] = 6
    elif sub_agent_range_equals["ultra"](view, start, end, True):
        result[1] = 7
    else:
        result[0] = 1
        result[1] = -1

@export("prodex_sub_agent_policy_v1")
def prodex_sub_agent_policy_v1(
    abi_version: Int64,
    operation: Int64,
    address: UInt,
    length: Int64,
    scalar: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != SUB_AGENT_POLICY_ABI_VERSION:
        return SUB_AGENT_POLICY_ABI
    if (
        operation < OP_CONCURRENCY_PARSE
        or operation > OP_CHILD_SPEC_SCALAR_POLICY
        or length < 0
        or (length > 0 and address == 0)
        or result_address == 0
    ):
        return SUB_AGENT_POLICY_INVALID

    var view = sub_agent_view(address, length)
    if not rich_view_valid(view, length):
        return SUB_AGENT_POLICY_INVALID
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = 0
    result[1] = 0
    result[2] = 0

    if operation == OP_CONCURRENCY_PARSE:
        sub_agent_parse_concurrency(view, result)
    elif operation == OP_CONCURRENCY_VALIDATE:
        result[0] = Int64(scalar < 1 or scalar > 64) * 3
        result[1] = scalar
    elif operation == OP_REASONING_EFFORT:
        sub_agent_reasoning_effort(view, result)
    elif operation == OP_MODEL_NONEMPTY:
        var bounds = rich_trim_bounds(view)
        result[1] = Int64(bounds[1] > bounds[0])
    elif operation == OP_PROVIDER_URL_POLICY:
        var provider_is_local = (scalar & 1) == 1
        var url_present = (scalar & 2) == 2
        if provider_is_local and not url_present:
            result[0] = 1
        elif not provider_is_local and url_present:
            result[0] = 2
    else:
        if not sub_agent_range_equals["PRODEX_SUB_AGENT"](
            view, 0, Int64(view.len), False
        ):
            result[0] = 1
        elif scalar < 1 or scalar > 65536:
            result[0] = 2
    return SUB_AGENT_POLICY_OK
