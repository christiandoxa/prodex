from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime GEMINI_CODE_ASSIST_POLICY_ABI_VERSION: Int64 = 1
comptime GEMINI_CODE_ASSIST_POLICY_OK: Int64 = 0
comptime GEMINI_CODE_ASSIST_POLICY_INVALID: Int64 = 1
comptime GEMINI_CODE_ASSIST_POLICY_CAPACITY: Int64 = 3
comptime GEMINI_CODE_ASSIST_POLICY_ABI: Int64 = 4


def code_assist_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def code_assist_range_equals_exact[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if start < 0 or end < start or end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if source[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True


def code_assist_range_ends_with_exact[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if end - start < length:
        return False
    return code_assist_range_equals_exact[literal](view, end - length, end)


def code_assist_range_contains_folded[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    var length = Int64(literal.byte_length())
    if length == 0:
        return True
    if end - start < length:
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for candidate in range(start, end - length + 1):
        var matched = True
        for index in range(length):
            if code_assist_ascii_lower(source[unsafe_offset=candidate + index]) != wanted[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return True
    return False


def code_assist_write_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    var length = Int64(literal.byte_length())
    if written[] < 0 or length > capacity - written[]:
        return False
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


def code_assist_write_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    lowercase_ascii: Bool,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len) or end - start > capacity - written[]:
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        var value = source[unsafe_offset=index]
        if lowercase_ascii:
            value = code_assist_ascii_lower(value)
        output[unsafe_offset=written[]] = value
        written[] += 1
    return True


@export("prodex_gemini_code_assist_endpoint_v1")
def prodex_gemini_code_assist_endpoint_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    present_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_CODE_ASSIST_POLICY_ABI_VERSION:
        return GEMINI_CODE_ASSIST_POLICY_ABI
    if (
        input_length < 0
        or output_capacity < 0
        or written_address == 0
        or present_address == 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return GEMINI_CODE_ASSIST_POLICY_INVALID

    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, input_length):
        return GEMINI_CODE_ASSIST_POLICY_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(present_address)
    )
    written[] = 0
    present[] = 0

    var bounds = rich_trim_bounds(view)
    var end = bounds[1]
    var source = rich_view_ptr(view)
    while end > bounds[0] and source[unsafe_offset=end - 1] == 47:
        end -= 1
    if end <= bounds[0]:
        return GEMINI_CODE_ASSIST_POLICY_OK
    if not code_assist_write_range(
        view,
        bounds[0],
        end,
        output,
        output_capacity,
        written,
        False,
    ):
        return GEMINI_CODE_ASSIST_POLICY_CAPACITY
    present[] = 1
    return GEMINI_CODE_ASSIST_POLICY_OK


@export("prodex_gemini_code_assist_tier_label_v1")
def prodex_gemini_code_assist_tier_label_v1(
    abi_version: Int64,
    id_address: UInt,
    id_length: Int64,
    id_present: Int64,
    name_address: UInt,
    name_length: Int64,
    name_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    present_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_CODE_ASSIST_POLICY_ABI_VERSION:
        return GEMINI_CODE_ASSIST_POLICY_ABI
    if (
        id_length < 0
        or name_length < 0
        or id_present < 0
        or id_present > 1
        or name_present < 0
        or name_present > 1
        or output_capacity < 0
        or written_address == 0
        or present_address == 0
        or (id_length > 0 and id_address == 0)
        or (name_length > 0 and name_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return GEMINI_CODE_ASSIST_POLICY_INVALID

    var id = ProdexRichStringView(id_address, UInt(id_length))
    var name = ProdexRichStringView(name_address, UInt(name_length))
    if not rich_view_valid(id, id_length) or not rich_view_valid(name, name_length):
        return GEMINI_CODE_ASSIST_POLICY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(present_address)
    )
    written[] = 0
    present[] = 0

    if id_present == 1:
        var bounds = rich_trim_bounds(id)
        if bounds[1] > bounds[0]:
            var label = StringSlice("")
            if code_assist_range_equals_exact["free-tier"](id, bounds[0], bounds[1]):
                label = StringSlice("free")
            elif code_assist_range_equals_exact["legacy-tier"](id, bounds[0], bounds[1]):
                label = StringSlice("legacy")
            elif code_assist_range_equals_exact["standard-tier"](id, bounds[0], bounds[1]):
                label = StringSlice("standard")
            elif code_assist_range_equals_exact["g1-pro-tier"](id, bounds[0], bounds[1]):
                label = StringSlice("pro")
            elif code_assist_range_equals_exact["g1-ultra-tier"](id, bounds[0], bounds[1]):
                label = StringSlice("ultra")
            if label.byte_length() > 0:
                if not code_assist_write_literal(output, output_capacity, written, label):
                    return GEMINI_CODE_ASSIST_POLICY_CAPACITY
            else:
                var end = bounds[1]
                if code_assist_range_ends_with_exact["-tier"](id, bounds[0], end):
                    end -= 5
                if not code_assist_write_range(
                    id,
                    bounds[0],
                    end,
                    output,
                    output_capacity,
                    written,
                    True,
                ):
                    return GEMINI_CODE_ASSIST_POLICY_CAPACITY
            present[] = 1
            return GEMINI_CODE_ASSIST_POLICY_OK

    if name_present == 1:
        var bounds = rich_trim_bounds(name)
        if bounds[1] <= bounds[0]:
            return GEMINI_CODE_ASSIST_POLICY_OK
        var label = StringSlice("")
        if code_assist_range_contains_folded["google one ai ultra"](
            name, bounds[0], bounds[1]
        ):
            label = StringSlice("ultra")
        elif code_assist_range_contains_folded["google one ai pro"](
            name, bounds[0], bounds[1]
        ):
            label = StringSlice("pro")
        elif code_assist_range_contains_folded["standard"](
            name, bounds[0], bounds[1]
        ):
            label = StringSlice("standard")
        elif code_assist_range_contains_folded["free"](
            name, bounds[0], bounds[1]
        ):
            label = StringSlice("free")
        if label.byte_length() > 0:
            if not code_assist_write_literal(output, output_capacity, written, label):
                return GEMINI_CODE_ASSIST_POLICY_CAPACITY
        elif not code_assist_write_range(
            name,
            bounds[0],
            bounds[1],
            output,
            output_capacity,
            written,
            False,
        ):
            return GEMINI_CODE_ASSIST_POLICY_CAPACITY
        present[] = 1
    return GEMINI_CODE_ASSIST_POLICY_OK
