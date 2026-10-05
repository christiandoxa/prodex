from std.memory import Pointer

from rich_text import (
    rich_trim_bounds,
    rich_utf8_continuation,
    rich_view_matches_literal,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime GEMINI_COMPACT_SNIPPET_ABI_VERSION: Int64 = 1
comptime GEMINI_COMPACT_SNIPPET_FIELD_COUNT: Int64 = 12
comptime GEMINI_COMPACT_SNIPPET_STATUS_OK: Int64 = 0
comptime GEMINI_COMPACT_SNIPPET_STATUS_INVALID: Int64 = 1
comptime GEMINI_COMPACT_SNIPPET_STATUS_UTF8: Int64 = 2
comptime GEMINI_COMPACT_SNIPPET_STATUS_CAPACITY: Int64 = 3
comptime GEMINI_COMPACT_SNIPPET_STATUS_ABI: Int64 = 4

comptime GEMINI_COMPACT_ITEM_TYPE: Int64 = 0
comptime GEMINI_COMPACT_ROLE: Int64 = 1
comptime GEMINI_COMPACT_NAME: Int64 = 2
comptime GEMINI_COMPACT_CALL_ID: Int64 = 3
comptime GEMINI_COMPACT_CONTENT: Int64 = 4
comptime GEMINI_COMPACT_TEXT: Int64 = 5
comptime GEMINI_COMPACT_ARGUMENTS: Int64 = 6
comptime GEMINI_COMPACT_INPUT: Int64 = 7
comptime GEMINI_COMPACT_OUTPUT: Int64 = 8
comptime GEMINI_COMPACT_ACTION: Int64 = 9
comptime GEMINI_COMPACT_SUMMARY: Int64 = 10
comptime GEMINI_COMPACT_GENERIC: Int64 = 11

comptime GEMINI_COMPACT_PRESENT_ITEM_TYPE: Int64 = 1 << 0
comptime GEMINI_COMPACT_PRESENT_ROLE: Int64 = 1 << 1
comptime GEMINI_COMPACT_PRESENT_NAME: Int64 = 1 << 2
comptime GEMINI_COMPACT_PRESENT_CALL_ID: Int64 = 1 << 3
comptime GEMINI_COMPACT_PRESENT_CONTENT: Int64 = 1 << 4


def gemini_compact_present(mask: Int64, bit: Int64) -> Bool:
    return (mask & bit) != 0


def gemini_compact_trimmed_empty(view: ProdexRichStringView) -> Bool:
    var bounds = rich_trim_bounds(view)
    return bounds[0] == bounds[1]


def gemini_compact_append_range(
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    copy_capacity: Int64,
    logical_limit: Int64,
    copied: Pointer[mut=True, Int64, _],
    logical: Pointer[mut=True, Int64, _],
):
    if length <= 0 or logical[] >= logical_limit:
        return
    var available = logical_limit - logical[]
    var take = length if length <= available else available
    var copy_available = copy_capacity - copied[]
    var copy_count = take if take <= copy_available else copy_available
    for index in range(copy_count):
        output[unsafe_offset=copied[] + index] = source[unsafe_offset=index]
    copied[] += copy_count
    logical[] += take
    if take < length:
        logical[] = logical_limit


def gemini_compact_append_view(
    view: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    copy_capacity: Int64,
    logical_limit: Int64,
    copied: Pointer[mut=True, Int64, _],
    logical: Pointer[mut=True, Int64, _],
):
    if view.len == 0:
        return
    gemini_compact_append_range(
        rich_view_ptr(view),
        Int64(view.len),
        output,
        copy_capacity,
        logical_limit,
        copied,
        logical,
    )


def gemini_compact_append_literal(
    literal: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    copy_capacity: Int64,
    logical_limit: Int64,
    copied: Pointer[mut=True, Int64, _],
    logical: Pointer[mut=True, Int64, _],
):
    gemini_compact_append_range(
        literal.unsafe_ptr(),
        Int64(literal.byte_length()),
        output,
        copy_capacity,
        logical_limit,
        copied,
        logical,
    )


def gemini_compact_append_optional_or_literal(
    view: ProdexRichStringView,
    present: Bool,
    fallback: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    copy_capacity: Int64,
    logical_limit: Int64,
    copied: Pointer[mut=True, Int64, _],
    logical: Pointer[mut=True, Int64, _],
):
    if present:
        gemini_compact_append_view(
            view, output, copy_capacity, logical_limit, copied, logical
        )
    else:
        gemini_compact_append_literal(
            fallback, output, copy_capacity, logical_limit, copied, logical
        )


def gemini_compact_finalize(
    output: Pointer[mut=True, UInt8, _],
    maximum: Int64,
    logical: Int64,
    written: Pointer[mut=True, Int64, _],
):
    if logical <= maximum:
        written[] = logical
        return
    var suffix = StringSlice("\n[truncated]")
    var suffix_length = Int64(suffix.byte_length())
    if maximum <= suffix_length:
        var suffix_ptr = suffix.unsafe_ptr()
        for index in range(maximum):
            output[unsafe_offset=index] = suffix_ptr[unsafe_offset=index]
        written[] = maximum
        return
    var end = maximum - suffix_length
    while end > 0 and rich_utf8_continuation(output[unsafe_offset=end]):
        end -= 1
    var suffix_ptr = suffix.unsafe_ptr()
    for index in range(suffix_length):
        output[unsafe_offset=end + index] = suffix_ptr[unsafe_offset=index]
    written[] = end + suffix_length


def gemini_compact_format(
    views: Pointer[mut=False, ProdexRichStringView, _],
    presence: Int64,
    maximum: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    emitted: Pointer[mut=True, Int64, _],
) -> Int64:
    var copy_capacity = maximum + 4
    if copy_capacity < 4 or output_capacity < copy_capacity:
        return GEMINI_COMPACT_SNIPPET_STATUS_CAPACITY
    var logical_limit = maximum + 1
    var copied: Int64 = 0
    var logical: Int64 = 0
    emitted[] = 1

    var item_type = views[unsafe_offset=GEMINI_COMPACT_ITEM_TYPE].copy()
    var role = views[unsafe_offset=GEMINI_COMPACT_ROLE].copy()
    var name = views[unsafe_offset=GEMINI_COMPACT_NAME].copy()
    var call_id = views[unsafe_offset=GEMINI_COMPACT_CALL_ID].copy()
    var content = views[unsafe_offset=GEMINI_COMPACT_CONTENT].copy()
    var text = views[unsafe_offset=GEMINI_COMPACT_TEXT].copy()
    var arguments = views[unsafe_offset=GEMINI_COMPACT_ARGUMENTS].copy()
    var input = views[unsafe_offset=GEMINI_COMPACT_INPUT].copy()
    var tool_output = views[unsafe_offset=GEMINI_COMPACT_OUTPUT].copy()
    var action = views[unsafe_offset=GEMINI_COMPACT_ACTION].copy()
    var summary = views[unsafe_offset=GEMINI_COMPACT_SUMMARY].copy()
    var generic = views[unsafe_offset=GEMINI_COMPACT_GENERIC].copy()

    if rich_view_matches_literal["message"](item_type, False):
        var selected_text = content.copy() if gemini_compact_present(
            presence, GEMINI_COMPACT_PRESENT_CONTENT
        ) else text.copy()
        gemini_compact_append_optional_or_literal(
            role,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_ROLE),
            StringSlice("unknown"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        if gemini_compact_trimmed_empty(selected_text):
            gemini_compact_append_literal(
                StringSlice(" message with no text content"),
                output,
                copy_capacity,
                logical_limit,
                Pointer(to=copied),
                Pointer(to=logical),
            )
        else:
            gemini_compact_append_literal(
                StringSlice(" message: "),
                output,
                copy_capacity,
                logical_limit,
                Pointer(to=copied),
                Pointer(to=logical),
            )
            gemini_compact_append_view(
                selected_text,
                output,
                copy_capacity,
                logical_limit,
                Pointer(to=copied),
                Pointer(to=logical),
            )
    elif rich_view_matches_literal["function_call"](item_type, False):
        gemini_compact_append_literal(
            StringSlice("tool call "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            name,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_NAME),
            StringSlice("function"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice(" ("),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            call_id,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_CALL_ID),
            StringSlice("unknown"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice("): "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            arguments,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    elif rich_view_matches_literal["custom_tool_call"](item_type, False):
        gemini_compact_append_literal(
            StringSlice("custom tool call "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            name,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_NAME),
            StringSlice("custom_tool"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice(" ("),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            call_id,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_CALL_ID),
            StringSlice("unknown"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice("): "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            input,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    elif rich_view_matches_literal["function_call_output"](
        item_type, False
    ) or rich_view_matches_literal["custom_tool_call_output"](item_type, False):
        gemini_compact_append_literal(
            StringSlice("tool output "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            call_id,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_CALL_ID),
            StringSlice("unknown"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice(": "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            tool_output,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    elif rich_view_matches_literal["local_shell_call"](item_type, False):
        gemini_compact_append_literal(
            StringSlice("local shell call "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_optional_or_literal(
            call_id,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_CALL_ID),
            StringSlice("unknown"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice(": "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            action,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    elif rich_view_matches_literal["web_search_call"](item_type, False):
        gemini_compact_append_literal(
            StringSlice("web search: "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            action,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    elif rich_view_matches_literal["reasoning"](item_type, False):
        if gemini_compact_trimmed_empty(summary):
            emitted[] = 0
            written[] = 0
            return GEMINI_COMPACT_SNIPPET_STATUS_OK
        gemini_compact_append_literal(
            StringSlice("reasoning summary: "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            summary,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
    else:
        if gemini_compact_trimmed_empty(generic):
            emitted[] = 0
            written[] = 0
            return GEMINI_COMPACT_SNIPPET_STATUS_OK
        gemini_compact_append_optional_or_literal(
            item_type,
            gemini_compact_present(presence, GEMINI_COMPACT_PRESENT_ITEM_TYPE),
            StringSlice("item"),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_literal(
            StringSlice(": "),
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )
        gemini_compact_append_view(
            generic,
            output,
            copy_capacity,
            logical_limit,
            Pointer(to=copied),
            Pointer(to=logical),
        )

    gemini_compact_finalize(output, maximum, logical, written)
    return GEMINI_COMPACT_SNIPPET_STATUS_OK


@export("prodex_mojo_gemini_compact_snippet_v1")
def prodex_mojo_gemini_compact_snippet_v1(
    abi_version: Int64,
    views_address: UInt,
    view_count: Int64,
    presence: Int64,
    maximum: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    emitted_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_COMPACT_SNIPPET_ABI_VERSION:
        return GEMINI_COMPACT_SNIPPET_STATUS_ABI
    if (
        views_address == 0
        or view_count != GEMINI_COMPACT_SNIPPET_FIELD_COUNT
        or maximum < 0
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or emitted_address == 0
    ):
        return GEMINI_COMPACT_SNIPPET_STATUS_INVALID
    var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(views_address)
    )
    for index in range(view_count):
        if not rich_view_valid(
            views[unsafe_offset=index].copy(), 0x7FFFFFFFFFFFFFFF
        ):
            return GEMINI_COMPACT_SNIPPET_STATUS_UTF8
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var emitted = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(emitted_address)
    )
    return gemini_compact_format(
        views, presence, maximum, output, output_capacity, written, emitted
    )


comptime GEMINI_COMPACT_TRUNCATE_TAIL: Int64 = 1
comptime GEMINI_COMPACT_TRUNCATE_EDGES: Int64 = 2


def gemini_compact_copy_bytes(
    source: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    for index in range(start, end):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1


def gemini_compact_copy_literal_prefix(
    literal: StringSlice,
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    var source = literal.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1


def gemini_compact_truncate_tail(
    source: Pointer[mut=False, UInt8, _],
    input_length: Int64,
    maximum: Int64,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    written[] = 0
    if input_length <= maximum:
        gemini_compact_copy_bytes(source, 0, input_length, output, written)
        return
    var suffix = StringSlice("\n[truncated]")
    var suffix_length = Int64(suffix.byte_length())
    if maximum <= suffix_length:
        gemini_compact_copy_literal_prefix(suffix, maximum, output, written)
        return
    var end = maximum - suffix_length
    while end > 0 and rich_utf8_continuation(source[unsafe_offset=end]):
        end -= 1
    gemini_compact_copy_bytes(source, 0, end, output, written)
    gemini_compact_copy_literal_prefix(suffix, suffix_length, output, written)


def gemini_compact_truncate_edges(
    source: Pointer[mut=False, UInt8, _],
    input_length: Int64,
    maximum: Int64,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
):
    written[] = 0
    if input_length <= maximum:
        gemini_compact_copy_bytes(source, 0, input_length, output, written)
        return
    var separator = StringSlice("\n[... middle truncated ...]\n")
    var separator_length = Int64(separator.byte_length())
    if maximum <= separator_length:
        gemini_compact_copy_literal_prefix(separator, maximum, output, written)
        return
    var retained = maximum - separator_length
    var head_bytes = retained // 3
    var tail_bytes = retained - head_bytes
    var head_end = head_bytes if head_bytes <= input_length else input_length
    while head_end > 0 and rich_utf8_continuation(
        source[unsafe_offset=head_end]
    ):
        head_end -= 1
    var tail_start = (
        input_length - tail_bytes if tail_bytes <= input_length else 0
    )
    while tail_start < input_length and rich_utf8_continuation(
        source[unsafe_offset=tail_start]
    ):
        tail_start += 1
    gemini_compact_copy_bytes(source, 0, head_end, output, written)
    gemini_compact_copy_literal_prefix(
        separator, separator_length, output, written
    )
    gemini_compact_copy_bytes(source, tail_start, input_length, output, written)


@export("prodex_mojo_gemini_compact_truncate_v1")
def prodex_mojo_gemini_compact_truncate_v1(
    abi_version: Int64,
    mode: Int64,
    input_address: UInt,
    input_length: Int64,
    maximum: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != GEMINI_COMPACT_SNIPPET_ABI_VERSION:
        return GEMINI_COMPACT_SNIPPET_STATUS_ABI
    if (
        input_length < 0
        or maximum < 0
        or (input_length > 0 and input_address == 0)
        or output_address == 0
        or output_capacity < maximum
        or written_address == 0
    ):
        return GEMINI_COMPACT_SNIPPET_STATUS_INVALID
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, 0x7FFFFFFFFFFFFFFF):
        return GEMINI_COMPACT_SNIPPET_STATUS_UTF8
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    if mode == GEMINI_COMPACT_TRUNCATE_TAIL:
        gemini_compact_truncate_tail(
            source, input_length, maximum, output, written
        )
        return GEMINI_COMPACT_SNIPPET_STATUS_OK
    if mode == GEMINI_COMPACT_TRUNCATE_EDGES:
        gemini_compact_truncate_edges(
            source, input_length, maximum, output, written
        )
        return GEMINI_COMPACT_SNIPPET_STATUS_OK
    return GEMINI_COMPACT_SNIPPET_STATUS_INVALID
