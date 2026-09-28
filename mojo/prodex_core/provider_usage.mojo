from std.memory import Pointer

from json_view import (
    deepseek_json_object_member,
    deepseek_json_skip_ws,
    deepseek_json_value_end,
)
from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime PROVIDER_USAGE_ABI_VERSION: Int64 = 1
comptime PROVIDER_USAGE_OK: Int64 = 0
comptime PROVIDER_USAGE_INVALID: Int64 = 1

def usage_root_bounds(view: ProdexRichStringView) -> Tuple[Int64, Int64]:
    var start = deepseek_json_skip_ws(view, 0, Int64(view.len))
    var end = deepseek_json_value_end(view, start, Int64(view.len), 0)
    if end < 0 or deepseek_json_skip_ws(view, end, Int64(view.len)) != Int64(view.len):
        return (-1, -1)
    return (start, end)

def usage_parse_u64(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    if start < 0 or end <= start:
        return (False, UInt64(0))
    var source = rich_view_ptr(view)
    var value = UInt64(0)
    for index in range(start, end):
        var byte = source[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return (False, UInt64(0))
        var digit = UInt64(0)
        if byte == 49:
            digit = UInt64(1)
        elif byte == 50:
            digit = UInt64(2)
        elif byte == 51:
            digit = UInt64(3)
        elif byte == 52:
            digit = UInt64(4)
        elif byte == 53:
            digit = UInt64(5)
        elif byte == 54:
            digit = UInt64(6)
        elif byte == 55:
            digit = UInt64(7)
        elif byte == 56:
            digit = UInt64(8)
        elif byte == 57:
            digit = UInt64(9)
        if value > UInt64(1844674407370955161) or (
            value == UInt64(1844674407370955161) and digit > UInt64(5)
        ):
            return (False, UInt64(0))
        value = value * UInt64(10) + digit
    return (True, value)

def usage_member_u64(
    view: ProdexRichStringView,
    object_start: Int64,
    object_end: Int64,
    key: StringSlice,
) -> Tuple[Bool, UInt64]:
    var bounds = deepseek_json_object_member(view, object_start, object_end, key)
    return usage_parse_u64(view, bounds[0], bounds[1])

def usage_selected_object(
    view: ProdexRichStringView, root_start: Int64, root_end: Int64
) -> Tuple[Int64, Int64]:
    var usage = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("usage")
    )
    if usage[0] >= 0:
        return (usage[0], usage[1])
    var response = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("response")
    )
    if response[0] >= 0:
        usage = deepseek_json_object_member(
            view, response[0], response[1], StringSlice("usage")
        )
        if usage[0] >= 0:
            return (usage[0], usage[1])
    var message = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("message")
    )
    if message[0] >= 0:
        usage = deepseek_json_object_member(
            view, message[0], message[1], StringSlice("usage")
        )
        if usage[0] >= 0:
            return (usage[0], usage[1])
    return (root_start, root_end)
def usage_input_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(
        view, start, end, StringSlice("input_tokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("prompt_tokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("promptTokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("inputTokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("cache_creation_input_tokens")
    )
    return value

def usage_output_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(
        view, start, end, StringSlice("output_tokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("completion_tokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("completionTokens")
    )
    if value[0]:
        return value
    value = usage_member_u64(
        view, start, end, StringSlice("outputTokens")
    )
    return value

def usage_total_tokens(
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Tuple[Bool, UInt64]:
    var value = usage_member_u64(
        view, start, end, StringSlice("total_tokens")
    )
    if value[0]:
        return value
    return usage_member_u64(
        view, start, end, StringSlice("totalTokens")
    )

def usage_flag(value: Bool) -> UInt64:
    return UInt64(1) if value else UInt64(0)

@export("prodex_provider_usage_extract_v1")
def prodex_provider_usage_extract_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or length < 0
        or (length > 0 and address == 0)
        or output_address == 0
    ):
        return PROVIDER_USAGE_INVALID

    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, length):
        return PROVIDER_USAGE_INVALID

    var root = usage_root_bounds(view)
    if root[0] < 0:
        return PROVIDER_USAGE_INVALID

    var selected = usage_selected_object(view, root[0], root[1])
    var input = usage_input_tokens(view, selected[0], selected[1])
    var output = usage_output_tokens(view, selected[0], selected[1])
    var total = usage_total_tokens(view, selected[0], selected[1])

    var metadata = deepseek_json_object_member(
        view, root[0], root[1], StringSlice("usageMetadata")
    )
    if not input[0] and metadata[0] >= 0:
        input = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("promptTokenCount")
        )
    if not output[0] and metadata[0] >= 0:
        output = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("candidatesTokenCount")
        )
    if not total[0] and metadata[0] >= 0:
        total = usage_member_u64(
            view, metadata[0], metadata[1], StringSlice("totalTokenCount")
        )

    var out = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    out[0] = usage_flag(input[0])
    out[1] = input[1]
    out[2] = usage_flag(output[0])
    out[3] = output[1]
    out[4] = usage_flag(total[0])
    out[5] = total[1]
    return PROVIDER_USAGE_OK
def usage_saturating_mul(left: UInt64, right: UInt64) -> UInt64:
    if left == 0 or right == 0:
        return UInt64(0)
    if left > UInt64(18446744073709551615) // right:
        return UInt64(18446744073709551615)
    return left * right

def usage_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if UInt64(18446744073709551615) - left < right:
        return UInt64(18446744073709551615)
    return left + right

@export("prodex_provider_usage_cost_v1")
def prodex_provider_usage_cost_v1(
    abi_version: Int64,
    input_present: Int64,
    input_tokens: UInt64,
    input_rate_present: Int64,
    input_rate: UInt64,
    output_present: Int64,
    output_tokens: UInt64,
    output_rate_present: Int64,
    output_rate: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or result_address == 0
        or (input_present != 0 and input_present != 1)
        or (input_rate_present != 0 and input_rate_present != 1)
        or (output_present != 0 and output_present != 1)
        or (output_rate_present != 0 and output_rate_present != 1)
    ):
        return PROVIDER_USAGE_INVALID

    var known = False
    var total = UInt64(0)
    if input_present == 1 and input_rate_present == 1:
        total = usage_saturating_add(
            total,
            usage_saturating_mul(input_tokens, input_rate) // UInt64(1_000_000),
        )
        known = True
    if output_present == 1 and output_rate_present == 1:
        total = usage_saturating_add(
            total,
            usage_saturating_mul(output_tokens, output_rate) // UInt64(1_000_000),
        )
        known = True

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = usage_flag(known)
    result[1] = total
    return PROVIDER_USAGE_OK

@export("prodex_provider_usage_merged_total_v1")
def prodex_provider_usage_merged_total_v1(
    abi_version: Int64,
    total_present: Int64,
    total_tokens: UInt64,
    input_present: Int64,
    input_tokens: UInt64,
    output_present: Int64,
    output_tokens: UInt64,
    result_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PROVIDER_USAGE_ABI_VERSION
        or result_address == 0
        or (total_present != 0 and total_present != 1)
        or (input_present != 0 and input_present != 1)
        or (output_present != 0 and output_present != 1)
    ):
        return PROVIDER_USAGE_INVALID

    var known = False
    var total = UInt64(0)
    if total_present == 1:
        total = total_tokens
        known = True
    elif input_present == 1:
        total = input_tokens
        if output_present == 1:
            total = usage_saturating_add(total, output_tokens)
        known = True

    var result = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[0] = usage_flag(known)
    result[1] = total
    return PROVIDER_USAGE_OK
