# Request Connection tokens share the response owner's HTTP token grammar.
from std.memory import Pointer
from response_forwarding import response_is_tchar
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView


def request_connection_token_rows(
    values: Pointer[mut=False, ProdexRichStringView, _], count: Int64,
    rows: Pointer[mut=True, Int64, _], measuring: Bool,
) -> Int64:
    var written: Int64 = 0
    for index in range(count):
        var view = values[unsafe_offset=index].copy()
        var source = rich_view_ptr(view)
        var cursor: Int64 = 0
        while cursor <= Int64(view.len):
            var end = cursor
            while end < Int64(view.len) and source[unsafe_offset=end] != 44:
                end += 1
            var bounds = rich_trim_bounds(ProdexRichStringView(view.ptr + UInt(cursor), UInt(end - cursor)))
            var start = cursor + bounds[0]
            var finish = cursor + bounds[1]
            var valid = start < finish
            for offset in range(start, finish):
                if not response_is_tchar(source[unsafe_offset=offset]):
                    valid = False
                    break
            if valid:
                if written == 0x7FFFFFFFFFFFFFFF // 3:
                    return -1
                if not measuring:
                    rows[unsafe_offset=written * 3] = index
                    rows[unsafe_offset=written * 3 + 1] = start
                    rows[unsafe_offset=written * 3 + 2] = finish
                written += 1
            if end == Int64(view.len):
                break
            cursor = end + 1
    return written


@export("prodex_runtime_request_connection_tokens_v1")
def prodex_runtime_request_connection_tokens_v1(
    abi_version: Int64, values_address: UInt, value_count: Int64,
    measuring: Int64, output_address: UInt, capacity: Int64, written_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        value_count < 0 or value_count > 0x7FFFFFFFFFFFFFFF // 16
        or (value_count > 0 and (values_address == 0 or values_address % 8 != 0))
        or measuring < 0 or measuring > 1 or capacity < 0
        or written_address == 0 or written_address % 8 != 0
        or (measuring == 1 and (output_address != 0 or capacity != 0))
        or (measuring == 0 and (output_address == 0 or output_address % 8 != 0))
    ):
        return 1
    var values = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(values_address))
    for index in range(value_count):
        if not rich_view_valid(values[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var rows = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var required = request_connection_token_rows(values, value_count, rows, True)
    if required < 0:
        return 3
    if measuring == 0:
        if capacity < required:
            return 3
        if request_connection_token_rows(values, value_count, rows, False) != required:
            return 3
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = required
    return 0
