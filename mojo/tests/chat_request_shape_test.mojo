from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING, JSON_NULL, JSON_FALSE, JSON_TRUE, JSON_NUMBER
from openai_chat_request import prodex_mojo_openai_chat_request_v1 as request_policy


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def node(kind: Int64, child: Int64, sibling: Int64, parent: Int64, key: StringSlice, text: StringSlice = "") -> ParsedJsonNode:
    return ParsedJsonNode(kind, child, sibling, parent, view(key), view(text), 0, 0)


def check(nodes: Pointer[mut=True, ParsedJsonNode, _], expected: StringSlice = "", provider: StringSlice = "Gemini", count: Int64 = 5):
    var scratch_storage = alloc(Layout[ProdexRichStringView](count=5))
    var output_storage = alloc(Layout[UInt8](count=128))
    var metadata_storage = alloc(Layout[Int64](count=2))
    var label = view(provider)
    var source = UInt64(Int(nodes))
    var scratch = UInt64(Int(scratch_storage.unsafe_ptr()))
    var target = UInt64(Int(output_storage.unsafe_ptr()))
    var metadata = metadata_storage.unsafe_ptr()
    var meta = UInt64(Int(metadata))
    assert request_policy(1, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 0
    var length = Int64(expected.byte_length())
    assert metadata[unsafe_offset=0] == Int64(1 if length > 0 else 0)
    assert metadata[unsafe_offset=1] == length
    assert request_policy(1, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 0, target, 128, meta) == 0
    var output = output_storage.unsafe_ptr()
    for index in range(length):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    if length > 0:
        output[unsafe_offset=0] = 91
        metadata[unsafe_offset=0] = 71
        metadata[unsafe_offset=1] = 72
        assert request_policy(1, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 0, target, length - 1, meta) == 3
        assert output[unsafe_offset=0] == 91
        assert metadata[unsafe_offset=0] == 71 and metadata[unsafe_offset=1] == 72
    metadata[unsafe_offset=0] = 71
    metadata[unsafe_offset=1] = 72
    assert request_policy(2, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 4
    assert request_policy(1, 2, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 1
    assert request_policy(1, 1, 1, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 1
    assert request_policy(1, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count - 1, 1, 0, 0, meta) == 1
    assert request_policy(1, 1, 0, 0, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 1
    assert request_policy(1, 1, 0, source + 1, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta) == 1
    assert request_policy(1, 1, 0, source, count, UInt64(label.ptr), Int64(label.len), scratch, count, 1, 0, 0, meta + 1) == 1
    assert metadata[unsafe_offset=0] == 71 and metadata[unsafe_offset=1] == 72
    dealloc(scratch_storage^)
    dealloc(output_storage^)
    dealloc(metadata_storage^)


def main() raises:
    var storage = alloc(Layout[ParsedJsonNode](count=5))
    var nodes = storage.unsafe_ptr()
    nodes[unsafe_offset=0] = node(JSON_OBJECT, 1, -1, -1, "")
    nodes[unsafe_offset=1] = node(JSON_STRING, -1, 2, 0, "model", " 東京🧭 ")
    nodes[unsafe_offset=2] = node(JSON_TRUE, -1, 3, 0, "stream")
    nodes[unsafe_offset=3] = node(JSON_STRING, -1, 4, 0, "instructions")
    nodes[unsafe_offset=4] = node(JSON_STRING, -1, -1, 0, "previous_response_id")
    check(nodes)
    nodes[unsafe_offset=2].kind = JSON_FALSE
    check(nodes)
    nodes[unsafe_offset=1].text = view(" \t\n\u0085\u00a0\u2003\u3000")
    check(nodes, "EGemini model must not be empty")
    nodes[unsafe_offset=1].text = view("\u200b")
    check(nodes)
    nodes[unsafe_offset=1].kind = JSON_NULL
    nodes[unsafe_offset=2].kind = JSON_NULL
    nodes[unsafe_offset=3].kind = JSON_NUMBER
    nodes[unsafe_offset=4].kind = JSON_NULL
    check(nodes, "EGemini model must be a string")
    nodes[unsafe_offset=1].kind = JSON_STRING
    check(nodes, "EGemini stream must be a boolean")
    nodes[unsafe_offset=2].kind = JSON_TRUE
    check(nodes, "EGemini instructions must be a string")
    nodes[unsafe_offset=3].kind = JSON_STRING
    check(nodes, "EGemini previous_response_id must be a string")
    nodes[unsafe_offset=4].kind = JSON_STRING
    nodes[unsafe_offset=0].first_child = -1
    check(nodes)
    nodes[unsafe_offset=0].kind = JSON_NULL
    check(nodes, "EGemini request body must be a JSON object", "Gemini", 1)
    check(nodes, "E 東京\n\0  request body must be a JSON object", " 東京\n\0 ", 1)
    dealloc(storage^)
