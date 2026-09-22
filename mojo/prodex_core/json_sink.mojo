# A capacity-measured writer: the first call counts bytes without a heap or
# output access; the second call writes into the exact Rust-owned allocation.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_ptr
from parsed_json import ParsedJson

@fieldwise_init
struct JsonSink(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64
    var measuring: Bool
    var failed: Bool


def js_byte(sink: Pointer[mut=True, JsonSink, _], value: UInt8):
    if sink[].failed:
        return
    if sink[].written == 0x7FFFFFFFFFFFFFFF:
        sink[].failed = True
        return
    if not sink[].measuring:
        if sink[].written >= sink[].capacity:
            sink[].failed = True
            return
        sink[].output[unsafe_offset=sink[].written] = value
    sink[].written += 1


def js_view(sink: Pointer[mut=True, JsonSink, _], view: ProdexRichStringView):
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        js_byte(sink, ptr[unsafe_offset=index])


def js_literal(sink: Pointer[mut=True, JsonSink, _], text: StringSlice):
    var ptr = text.unsafe_ptr()
    for index in range(Int64(text.byte_length())):
        js_byte(sink, ptr[unsafe_offset=index])


def js_escaped(sink: Pointer[mut=True, JsonSink, _], view: ProdexRichStringView):
    var ptr = rich_view_ptr(view)
    for index in range(Int64(view.len)):
        var byte = ptr[unsafe_offset=index]
        if byte == 34 or byte == 92:
            js_byte(sink, 92)
            js_byte(sink, byte)
        elif byte == 8:
            js_literal(sink, StringSlice("\\b"))
        elif byte == 9:
            js_literal(sink, StringSlice("\\t"))
        elif byte == 10:
            js_literal(sink, StringSlice("\\n"))
        elif byte == 12:
            js_literal(sink, StringSlice("\\f"))
        elif byte == 13:
            js_literal(sink, StringSlice("\\r"))
        elif byte < 32:
            js_literal(sink, StringSlice("\\u00"))
            var high = byte >> 4
            var low = byte & 15
            js_byte(sink, high + UInt8(48 if high < 10 else 87))
            js_byte(sink, low + UInt8(48 if low < 10 else 87))
        else:
            js_byte(sink, byte)


def js_string(sink: Pointer[mut=True, JsonSink, _], view: ProdexRichStringView):
    js_byte(sink, 34)
    js_escaped(sink, view)
    js_byte(sink, 34)


def js_raw_view(tree: ParsedJson, index: Int64) -> ProdexRichStringView:
    var node = tree.nodes[unsafe_offset=index].copy()
    return ProdexRichStringView(tree.raw.ptr + UInt(node.raw_start), UInt(node.raw_length))


def js_raw(sink: Pointer[mut=True, JsonSink, _], tree: ParsedJson, index: Int64):
    js_view(sink, js_raw_view(tree, index))
