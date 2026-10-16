from std.memory import Pointer
from rich_text import rich_codepoint, rich_codepoint_width, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr
from log_transcript_facts import PRODEX_TRANSCRIPT_ABI_VERSION, transcript_fact_view_valid, transcript_fact_control

comptime TRANSCRIPT_SAFE_TEXT_MAX_BYTES: Int64 = 65_536
comptime TRANSCRIPT_SAFE_TEXT_MARKER = StringSlice(" …[truncated]")


def transcript_safe_copy_literal(
    literal: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var source = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        output[unsafe_offset=written[]] = source[unsafe_offset=index]
        written[] += 1
    return True


@export("prodex_mojo_transcript_safe_text_v1")
def prodex_mojo_transcript_safe_text_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    facts_address: UInt,
    fact_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or input_length < 0
        or output_capacity < 0
        or output_address == 0
        or written_address == 0
        or (input_length > 0 and input_address == 0)
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not transcript_fact_view_valid(view, facts_address, fact_count):
        return 2
    var source = rich_view_ptr(view)
    var facts = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    var normalized_length: Int64 = 0
    var cursor: Int64 = 0
    var index: Int64 = 0
    while cursor < input_length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        normalized_length += (
            1
            if transcript_fact_control(facts, index)
            and codepoint != 10
            and codepoint != 13
            and codepoint != 9
            else width
        )
        cursor += width
        index += 1
    var marker_length = Int64(TRANSCRIPT_SAFE_TEXT_MARKER.byte_length())
    var truncated = normalized_length > TRANSCRIPT_SAFE_TEXT_MAX_BYTES
    var required = TRANSCRIPT_SAFE_TEXT_MAX_BYTES if truncated else normalized_length
    if output_capacity < required:
        return 3
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var budget = TRANSCRIPT_SAFE_TEXT_MAX_BYTES - 16
    cursor = 0
    index = 0
    while cursor < input_length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        var codepoint = rich_codepoint(source, cursor, width)
        var emitted_width = width
        var replacement = transcript_fact_control(facts, index) and (
            codepoint != 10 and codepoint != 13 and codepoint != 9
        )
        if replacement:
            emitted_width = 1
        if truncated and written[] + emitted_width > budget:
            break
        if replacement:
            output[unsafe_offset=written[]] = 32
            written[] += 1
        else:
            for offset in range(width):
                output[unsafe_offset=written[]] = source[unsafe_offset=cursor + offset]
                written[] += 1
        cursor += width
        index += 1
    if truncated:
        if written[] + marker_length > output_capacity:
            return 3
        _ = transcript_safe_copy_literal(TRANSCRIPT_SAFE_TEXT_MARKER, output, written)
    return 0


@export("prodex_mojo_transcript_join_user_parts_v1")
def prodex_mojo_transcript_join_user_parts_v1(
    abi_version: Int64,
    addresses_address: UInt,
    lengths_address: UInt,
    part_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != PRODEX_TRANSCRIPT_ABI_VERSION
        or part_count < 0
        or output_capacity < 0
        or output_address == 0
        or written_address == 0
        or (part_count > 0 and (addresses_address == 0 or lengths_address == 0))
    ):
        return 1
    var addresses = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(addresses_address)
    )
    var lengths = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(lengths_address)
    )
    var required: Int64 = 0
    for index in range(part_count):
        var length = lengths[unsafe_offset=index]
        if length < 0 or (length > 0 and addresses[unsafe_offset=index] == 0):
            return 1
        var view = ProdexRichStringView(
            UInt(addresses[unsafe_offset=index]), UInt(length)
        )
        if not rich_view_valid(view, 0x7FFF_FFFF_FFFF_FFFF):
            return 2
        if required > 0x7FFF_FFFF_FFFF_FFFF - length:
            return 1
        required += length
        if index > 0:
            if required == 0x7FFF_FFFF_FFFF_FFFF:
                return 1
            required += 1
    if output_capacity < required:
        return 3
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    for index in range(part_count):
        if index > 0:
            output[unsafe_offset=written[]] = 10
            written[] += 1
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(addresses[unsafe_offset=index])
        )
        var length = lengths[unsafe_offset=index]
        for offset in range(length):
            output[unsafe_offset=written[]] = source[unsafe_offset=offset]
            written[] += 1
    return 0
