from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_NULL, JSON_STRING
from log_transcript_context import prodex_mojo_log_transcript_context_v1


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def node(kind: Int64, parent: Int64, next: Int64, key: StringSlice) -> ParsedJsonNode:
    return ParsedJsonNode(kind, -1, next, parent, view(key), view("raw"), 0, 0)


def verify(nodes: Pointer[mut=True, ParsedJsonNode, _], values: Pointer[mut=True, ProdexRichStringView, _],
           output: Pointer[mut=True, UInt8, _], metadata: Pointer[mut=True, Int64, _], operation: Int64, expected: StringSlice):
    assert prodex_mojo_log_transcript_context_v1(1, operation, 0, UInt64(Int(nodes)), 4, 0, 0,
        UInt64(Int(values)), 8, 0, UInt64(Int(output)), 128, UInt64(Int(metadata))) == 0
    assert metadata[unsafe_offset=0] == 1
    assert metadata[unsafe_offset=1] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var ns = alloc(Layout[ParsedJsonNode](count=4))
    var vs = alloc(Layout[ProdexRichStringView](count=8))
    var os = alloc(Layout[UInt8](count=128))
    var ms = alloc(Layout[Int64](count=2))
    var nodes = ns.unsafe_ptr()
    var values = vs.unsafe_ptr()
    var output = os.unsafe_ptr()
    var metadata = ms.unsafe_ptr()
    nodes[unsafe_offset=0] = node(JSON_OBJECT, -1, -1, "")
    nodes[unsafe_offset=0].first_child = 1
    nodes[unsafe_offset=1] = node(JSON_NULL, 0, 2, "command")
    nodes[unsafe_offset=2] = node(JSON_STRING, 0, 3, "arguments")
    nodes[unsafe_offset=3] = node(JSON_STRING, 0, -1, "name")
    for index in range(8):
        values[unsafe_offset=index] = view("")
    values[unsafe_offset=4] = view("echo 東京")
    values[unsafe_offset=5] = view("visible")
    values[unsafe_offset=7] = view("sh")
    verify(nodes, values, output, metadata, 8, "[[\"tool-call:sh\",\"\"]]")
    nodes[unsafe_offset=1].key = view("other")
    verify(nodes, values, output, metadata, 8, "[[\"tool-call:sh\",\"echo 東京\"]]")
    nodes[unsafe_offset=3].kind = JSON_NULL
    verify(nodes, values, output, metadata, 8, "[[\"tool-call:shell\",\"echo 東京\"]]")
    nodes[unsafe_offset=2].key = view("stdout")
    nodes[unsafe_offset=1].key = view("output")
    verify(nodes, values, output, metadata, 9, "[[\"tool-output\",\"\"]]")
    nodes[unsafe_offset=1].key = view("other")
    verify(nodes, values, output, metadata, 9, "[[\"tool-output\",\"visible\"]]")
    output[unsafe_offset=0] = 61
    metadata[unsafe_offset=0] = 71
    assert prodex_mojo_log_transcript_context_v1(1, 9, 0, UInt64(Int(nodes)), 4, 0, 0,
        UInt64(Int(values)), 8, 0, UInt64(Int(output)), 1, UInt64(Int(metadata))) == 3
    assert prodex_mojo_log_transcript_context_v1(1, 10, 0, UInt64(Int(nodes)), 4, 0, 0,
        UInt64(Int(values)), 8, 0, UInt64(Int(output)), 128, UInt64(Int(metadata))) == 1
    assert prodex_mojo_log_transcript_context_v1(2, 8, 0, UInt64(Int(nodes)), 4, 0, 0,
        UInt64(Int(values)), 8, 0, UInt64(Int(output)), 128, UInt64(Int(metadata))) == 4
    assert output[unsafe_offset=0] == 61 and metadata[unsafe_offset=0] == 71
    dealloc(ns^)
    dealloc(vs^)
    dealloc(os^)
    dealloc(ms^)
