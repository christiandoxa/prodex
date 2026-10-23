# Optional-tool version selection and minimum compatibility; Rust probes processes.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr, rich_view_valid, rich_codepoint, rich_codepoint_width, rich_unicode_space
from update_notice_policy import update_notice_parse_release_version, update_notice_release_version_compare

comptime OPTIONAL_TOOL_PROBE_MAX_BYTES: Int64 = 1_048_576


def optional_tool_probe_token_char_allowed(codepoint: Int64) -> Bool:
    return (
        (codepoint >= 48 and codepoint <= 57)
        or (codepoint >= 65 and codepoint <= 90)
        or (codepoint >= 97 and codepoint <= 122)
        or codepoint == 43 or codepoint == 45 or codepoint == 46
    )


def optional_tool_probe_token_bounds(
    view: ProdexRichStringView, cursor: Int64,
) -> Tuple[Int64, Int64, Int64]:
    var input_length = Int64(view.len)
    var bytes = rich_view_ptr(view)
    var index = cursor
    while index < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        if not rich_unicode_space(codepoint):
            break
        index += width

    var token_start = index
    while index < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        if rich_unicode_space(codepoint):
            break
        index += width
    var token_end = index
    # Rust char::is_ascii_alphanumeric is strictly ASCII. Non-ASCII characters
    # inside a token remain intact; only boundary punctuation is trimmed.
    var start = token_start
    var end = token_start
    var saw_allowed = False
    var probe = token_start
    while probe < token_end:
        var width = rich_codepoint_width(bytes[unsafe_offset=probe])
        var codepoint = rich_codepoint(bytes, probe, width)
        probe += width
        if optional_tool_probe_token_char_allowed(codepoint):
            if not saw_allowed:
                start = probe - width
                saw_allowed = True
            end = probe
    if not saw_allowed:
        start = token_end
        end = token_end
    while start < end and bytes[unsafe_offset=start] == 118:
        start += 1

    if token_start == input_length:
        return (Int64(-1), Int64(-1), input_length)
    return (start, end, token_end)


@export("prodex_optional_tool_probe_version_v1")
def prodex_optional_tool_probe_version_v1(
    abi_version: Int64, input_address: UInt, input_length: Int64,
    minimum_address: UInt, minimum_length: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (input_length < 0 or minimum_length < 0 or output_address == 0 or output_address % 8 != 0):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    var minimum = ProdexRichStringView(minimum_address, UInt(minimum_length))
    if not rich_view_valid(view, OPTIONAL_TOOL_PROBE_MAX_BYTES) or not rich_view_valid(minimum, OPTIONAL_TOOL_PROBE_MAX_BYTES):
        return 1
    var cursor: Int64 = 0
    var selected_start: Int64 = 0
    var selected_end: Int64 = 0
    var tag: Int64 = 0
    while cursor < input_length:
        var token = optional_tool_probe_token_bounds(view, cursor)
        if token[0] < 0:
            break
        var candidate = ProdexRichStringView(view.ptr + UInt(token[0]), UInt(token[1] - token[0]))
        if update_notice_parse_release_version(candidate).valid:
            var ordering = update_notice_release_version_compare(candidate, minimum, True)
            if ordering == -2:
                return 1
            tag = Int64(1) if ordering < 0 else Int64(2)
            selected_start = token[0]
            selected_end = token[1]
            break
        cursor = token[2]
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    output[unsafe_offset=0] = tag
    output[unsafe_offset=1] = selected_start
    output[unsafe_offset=2] = selected_end
    return 0
