from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView
from smart_context_symbol_parser import (
    SYMBOL_ABI_MISMATCH,
    SYMBOL_ABI_VERSION,
    SYMBOL_CAPACITY,
    SYMBOL_INVALID,
    SYMBOL_MAX_INPUT_BYTES,
    SYMBOL_MAX_NAME_BYTES,
    SYMBOL_NONE,
    SYMBOL_OK,
    symbol_line_address,
    symbol_line_length,
    symbol_name_length,
    symbol_parse_line,
    symbol_write_name,
)


@export("prodex_smart_context_symbol_classify_v1")
def prodex_smart_context_symbol_classify_v1(
    abi_version: Int64,
    line_spans_address: UInt,
    line_count: Int64,
    start_index: Int64,
    output_address: UInt,
    output_capacity: Int64,
    metadata_address: UInt,
) abi("C") -> Int64:
    if abi_version != SYMBOL_ABI_VERSION:
        return SYMBOL_ABI_MISMATCH
    if (
        line_count < 0
        or line_count > SYMBOL_MAX_INPUT_BYTES + 1
        or (line_count > 0 and line_spans_address == 0)
        or start_index < 0
        or start_index > line_count
        or output_capacity < SYMBOL_MAX_NAME_BYTES
        or output_address == 0
        or metadata_address == 0
    ):
        return SYMBOL_INVALID
    var lines = Pointer[mut=False, UInt64, ImmUntrackedOrigin](
        unsafe_from_address=Int(line_spans_address)
    )
    var metadata = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(metadata_address)
    )
    metadata[unsafe_offset=0] = SYMBOL_NONE
    metadata[unsafe_offset=1] = 0
    metadata[unsafe_offset=2] = 0
    metadata[unsafe_offset=3] = 0
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(start_index, line_count):
        var address = symbol_line_address(lines, index)
        var length = symbol_line_length(lines, index)
        if (
            length < 0
            or length > SYMBOL_MAX_INPUT_BYTES
            or (length > 0 and address == 0)
        ):
            return SYMBOL_INVALID
        if not rich_view_valid(
            ProdexRichStringView(address, UInt(length)), SYMBOL_MAX_INPUT_BYTES
        ):
            return SYMBOL_INVALID
        var parsed = symbol_parse_line(lines, index)
        if not parsed[0]:
            continue
        var name_length = symbol_name_length(parsed[1], parsed[2], parsed[4])
        if name_length <= 0 or name_length > SYMBOL_MAX_NAME_BYTES:
            return SYMBOL_INVALID
        if name_length > output_capacity:
            return SYMBOL_CAPACITY
        var written = symbol_write_name(
            address, parsed[1], parsed[2], parsed[4], output
        )
        if written != name_length:
            return SYMBOL_INVALID
        metadata[unsafe_offset=0] = UInt64(index)
        metadata[unsafe_offset=1] = parsed[3]
        metadata[unsafe_offset=2] = parsed[5]
        metadata[unsafe_offset=3] = UInt64(written)
        return SYMBOL_OK
    return SYMBOL_OK
