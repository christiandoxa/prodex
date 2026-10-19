# Buffered provider-error SSE framing; streaming transport retains its separate wire contract.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_trim_bounds, rich_view_prefix, rich_view_valid


def sse_buffered_line_span(source: ProdexRichStringView, start: Int64, end: Int64) -> Tuple[Int64, Int64]:
    var line = ProdexRichStringView(source.ptr + UInt(start), UInt(end - start))
    var bounds = rich_trim_bounds(line)
    var trimmed = ProdexRichStringView(line.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))
    if not rich_view_prefix["data:"](trimmed, False):
        return (-1, -1)
    var payload = ProdexRichStringView(trimmed.ptr + 5, trimmed.len - 5)
    var body = rich_trim_bounds(payload)
    var offset = start + bounds[0] + 5
    return (offset + body[0], offset + body[1])


def sse_buffered_collect(source: ProdexRichStringView, output: Pointer[mut=True, UInt64, MutUntrackedOrigin], write: Bool) -> Int64:
    var bytes = Pointer[mut=False, UInt8, ImmUntrackedOrigin](unsafe_from_address=Int(source.ptr))
    var length = Int64(source.len)
    var count: Int64 = 0
    var start: Int64 = 0
    while start < length:
        var end = start
        while end < length and bytes[unsafe_offset=end] != 10:
            end += 1
        var span = sse_buffered_line_span(source, start, end)
        if span[0] >= 0:
            if write:
                output[unsafe_offset=count * 2] = UInt64(span[0])
                output[unsafe_offset=count * 2 + 1] = UInt64(span[1])
            count += 1
        if end == length:
            break
        start = end + 1
    return count


@export("prodex_sse_buffered_data_spans_v1")
def prodex_sse_buffered_data_spans_v1(version: Int64, address: UInt, length: Int64, output_address: UInt, capacity: Int64, required_address: UInt) abi("C") -> Int64:
    if version != 1:
        return 4
    if length < 0 or capacity < 0 or required_address == 0 or required_address % 8 != 0 or (capacity > 0 and (output_address == 0 or output_address % 8 != 0)):
        return 1
    var source = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(source, length):
        return 1
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var count = sse_buffered_collect(source, output, False)
    var required = Pointer[mut=True, UInt64, MutUntrackedOrigin](unsafe_from_address=Int(required_address))
    required[] = UInt64(count)
    if capacity < count:
        return 3
    _ = sse_buffered_collect(source, output, True)
    return 0
