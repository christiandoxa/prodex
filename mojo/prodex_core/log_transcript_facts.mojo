from std.memory import Pointer
from rich_text import rich_codepoint, rich_codepoint_width, rich_utf8_continuation, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime PRODEX_TRANSCRIPT_ABI_VERSION: Int64 = 1


def transcript_fact_view_valid(
    view: ProdexRichStringView,
    facts_address: UInt,
    fact_count: Int64,
) -> Bool:
    if (
        fact_count < 0
        or (fact_count > 0 and facts_address == 0)
        or not rich_view_valid(view, 0x7FFF_FFFF_FFFF_FFFF)
    ):
        return False
    var facts = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    var source = rich_view_ptr(view)
    var cursor: Int64 = 0
    var index: Int64 = 0
    while cursor < Int64(view.len):
        if index >= fact_count:
            return False
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        var packed = facts[unsafe_offset=index]
        if (
            packed >> 24 != 0
            or (packed & UInt64(0x1F_FFFF)) != UInt64(codepoint)
        ):
            return False
        cursor += width
        index += 1
    return index == fact_count


def transcript_fact_whitespace(facts: Pointer[mut=False, UInt64, _], index: Int64) -> Bool:
    return ((facts[unsafe_offset=index] >> 21) & UInt64(1)) == UInt64(1)


def transcript_fact_control(facts: Pointer[mut=False, UInt64, _], index: Int64) -> Bool:
    return ((facts[unsafe_offset=index] >> 23) & UInt64(1)) == UInt64(1)


def transcript_fact_codepoint(
    facts: Pointer[mut=False, UInt64, _], index: Int64
) -> Int64:
    return Int64(facts[unsafe_offset=index] & UInt64(0x1F_FFFF))


def transcript_fact_trim_bounds(
    source: Pointer[mut=False, UInt8, _],
    input_length: Int64,
    facts: Pointer[mut=False, UInt64, _],
    fact_count: Int64,
) -> Tuple[Int64, Int64, Int64, Int64]:
    var start_byte: Int64 = 0
    var start_scalar: Int64 = 0
    while start_scalar < fact_count:
        if not transcript_fact_whitespace(facts, start_scalar):
            break
        var width = rich_codepoint_width(source[unsafe_offset=start_byte])
        start_byte += width
        start_scalar += 1

    var end_byte = input_length
    var end_scalar = fact_count
    while end_scalar > start_scalar:
        var probe = end_byte - 1
        while probe > start_byte and rich_utf8_continuation(source[unsafe_offset=probe]):
            probe -= 1
        if not transcript_fact_whitespace(facts, end_scalar - 1):
            break
        end_byte = probe
        end_scalar -= 1
    return (start_byte, end_byte, start_scalar, end_scalar)
