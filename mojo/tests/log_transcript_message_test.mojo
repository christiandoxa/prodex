from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_ARRAY, JSON_STRING, JSON_NULL
from log_transcript_context import prodex_mojo_log_transcript_context_v1


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def node(kind: Int64, parent: Int64, child: Int64, next: Int64, key: StringSlice, text: StringSlice = "") -> ParsedJsonNode:
    return ParsedJsonNode(kind, child, next, parent, view(key), view(text), 0, 0)


def verify(nodes: Pointer[mut=True, ParsedJsonNode, _], values: Pointer[mut=True, ProdexRichStringView, _],
           output: Pointer[mut=True, UInt8, _], metadata: Pointer[mut=True, Int64, _], count: Int64, op: Int64, expected: StringSlice):
    assert prodex_mojo_log_transcript_context_v1(1, op, 0, UInt64(Int(nodes)), count, 0, 0,
        UInt64(Int(values)), count * 2, 0, UInt64(Int(output)), 128, UInt64(Int(metadata))) == 0
    assert metadata[unsafe_offset=0] == 1 and metadata[unsafe_offset=1] == Int64(expected.byte_length())
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var ns = alloc(Layout[ParsedJsonNode](count=14))
    var vs = alloc(Layout[ProdexRichStringView](count=28))
    var os = alloc(Layout[UInt8](count=128))
    var ms = alloc(Layout[Int64](count=2))
    var nodes = ns.unsafe_ptr()
    var values = vs.unsafe_ptr()
    var output = os.unsafe_ptr()
    var metadata = ms.unsafe_ptr()
    for index in range(28):
        values[unsafe_offset=index] = view("")
    nodes[unsafe_offset=0] = node(JSON_OBJECT, -1, 1, -1, "")
    nodes[unsafe_offset=1] = node(JSON_STRING, 0, -1, 2, "type", "message")
    nodes[unsafe_offset=2] = node(JSON_STRING, 0, -1, 3, "role", "user")
    nodes[unsafe_offset=3] = node(JSON_ARRAY, 0, 4, 10, "content")
    nodes[unsafe_offset=4] = node(JSON_OBJECT, 3, 5, 6, "")
    nodes[unsafe_offset=5] = node(JSON_STRING, 4, -1, -1, "text", "raw first")
    nodes[unsafe_offset=6] = node(JSON_OBJECT, 3, 7, 8, "")
    nodes[unsafe_offset=7] = node(JSON_STRING, 6, -1, -1, "content", "raw second")
    nodes[unsafe_offset=8] = node(JSON_OBJECT, 3, 9, -1, "")
    nodes[unsafe_offset=9] = node(JSON_NULL, 8, -1, 13, "text")
    nodes[unsafe_offset=10] = node(JSON_OBJECT, 0, 11, -1, "internal_chat_message_metadata_passthrough")
    nodes[unsafe_offset=11] = node(JSON_ARRAY, 10, 12, -1, "content_item_kinds")
    nodes[unsafe_offset=12] = node(JSON_STRING, 11, -1, -1, "", "user.text")
    nodes[unsafe_offset=13] = node(JSON_STRING, 8, -1, -1, "content", "blocked")
    values[unsafe_offset=11] = view("first")
    values[unsafe_offset=15] = view("second")
    values[unsafe_offset=27] = view("blocked")
    verify(nodes, values, output, metadata, 14, 13, "[[\"user\",\"first\\nsecond\"]]")
    nodes[unsafe_offset=12].text = view("internal")
    verify(nodes, values, output, metadata, 14, 13, "[]")
    nodes[unsafe_offset=2].text = view("assistant")
    verify(nodes, values, output, metadata, 14, 13, "[[\"assistant\",\"first\\nsecond\"]]")
    nodes[unsafe_offset=2].kind = JSON_NULL
    verify(nodes, values, output, metadata, 14, 13, "[[\"message\",\"first\\nsecond\"]]")
    nodes[unsafe_offset=5].kind = JSON_NULL
    verify(nodes, values, output, metadata, 14, 13, "[[\"message\",\"second\"]]")
    nodes[unsafe_offset=0] = node(JSON_OBJECT, -1, 1, -1, "")
    nodes[unsafe_offset=1] = node(JSON_STRING, 0, -1, 2, "message", "raw")
    nodes[unsafe_offset=2] = node(JSON_STRING, 0, -1, -1, "text", "raw")
    values[unsafe_offset=3] = view("event")
    values[unsafe_offset=5] = view("reason")
    verify(nodes, values, output, metadata, 3, 14, "[[\"user\",\"event\"]]")
    verify(nodes, values, output, metadata, 3, 15, "[[\"assistant\",\"event\"]]")
    verify(nodes, values, output, metadata, 3, 16, "[[\"reasoning\",\"reason\"]]")
    output[unsafe_offset=0] = 61
    metadata[unsafe_offset=0] = 71
    assert prodex_mojo_log_transcript_context_v1(1, 13, 0, UInt64(Int(nodes)), 3, 0, 0,
        UInt64(Int(values)), 6, 0, UInt64(Int(output)), 1, UInt64(Int(metadata))) == 3
    assert prodex_mojo_log_transcript_context_v1(1, 17, 0, UInt64(Int(nodes)), 3, 0, 0,
        UInt64(Int(values)), 6, 0, UInt64(Int(output)), 128, UInt64(Int(metadata))) == 1
    assert output[unsafe_offset=0] == 61 and metadata[unsafe_offset=0] == 71
    dealloc(ns^)
    dealloc(vs^)
    dealloc(os^)
    dealloc(ms^)
