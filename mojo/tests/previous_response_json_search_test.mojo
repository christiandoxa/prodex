# Direct ABI coverage for bounded previous-response JSON classification.
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import JSON_ARRAY, JSON_OBJECT, ParsedJsonNode, pj_literal
from rich_types import ProdexRichStringView
from rich_fallback import prodex_mojo_previous_response_json_search_v1


def main() raises:
    var node_storage = alloc(Layout[ParsedJsonNode](count=3))
    var word_storage = alloc(Layout[UInt64](count=54))
    var output_storage = alloc(Layout[Int64](count=3))
    var nodes = node_storage.unsafe_ptr()
    var words = word_storage.unsafe_ptr()
    var output = output_storage.unsafe_ptr()
    var empty = ProdexRichStringView(0, 0)
    nodes[unsafe_offset=0] = ParsedJsonNode(
        JSON_ARRAY, 1, -1, -1, empty.copy(), empty.copy(), 0, 0
    )
    nodes[unsafe_offset=1] = ParsedJsonNode(
        JSON_OBJECT, -1, 2, 0, empty.copy(), empty.copy(), 0, 0
    )
    nodes[unsafe_offset=2] = ParsedJsonNode(
        JSON_OBJECT, -1, -1, 0, empty.copy(), empty.copy(), 0, 0
    )
    for index in range(54):
        words[unsafe_offset=index] = 0
    var code = StringSlice("previous_response_not_found")
    var detail = StringSlice("first-雪")
    var later = StringSlice("later")
    var first = 18
    var second = 36
    words[unsafe_offset=first + 3] = 1
    words[unsafe_offset=first + 4] = UInt64(UInt(Int(code.unsafe_ptr())))
    words[unsafe_offset=first + 5] = UInt64(code.byte_length())
    words[unsafe_offset=first + 12] = 1
    words[unsafe_offset=first + 13] = UInt64(UInt(Int(detail.unsafe_ptr())))
    words[unsafe_offset=first + 14] = UInt64(detail.byte_length())
    words[unsafe_offset=second + 3] = 1
    words[unsafe_offset=second + 4] = UInt64(UInt(Int(code.unsafe_ptr())))
    words[unsafe_offset=second + 5] = UInt64(code.byte_length())
    words[unsafe_offset=second + 9] = 1
    words[unsafe_offset=second + 10] = UInt64(UInt(Int(later.unsafe_ptr())))
    words[unsafe_offset=second + 11] = UInt64(later.byte_length())
    var source = UInt(Int(nodes))
    var fields = UInt(Int(words))
    var result = UInt(Int(output))
    assert prodex_mojo_previous_response_json_search_v1(
        1, 0, source, 3, fields, result,
    ) == 0
    assert output[unsafe_offset=0] == 1
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == 4
    words[unsafe_offset=first + 12] = 0
    words[unsafe_offset=first + 13] = 0
    words[unsafe_offset=first + 14] = 0
    assert prodex_mojo_previous_response_json_search_v1(
        1, 0, source, 3, fields, result,
    ) == 0
    assert output[unsafe_offset=0] == -2
    assert output[unsafe_offset=1] == 1
    assert output[unsafe_offset=2] == -1
    output[unsafe_offset=0] = 91
    output[unsafe_offset=1] = 92
    output[unsafe_offset=2] = 93
    assert prodex_mojo_previous_response_json_search_v1(
        2, 0, source, 3, fields, result,
    ) == 4
    assert prodex_mojo_previous_response_json_search_v1(
        1, 2, source, 3, fields, result,
    ) == 1
    assert output[unsafe_offset=0] == 91
    assert output[unsafe_offset=1] == 92
    assert output[unsafe_offset=2] == 93
    words[unsafe_offset=53] = 2
    assert prodex_mojo_previous_response_json_search_v1(
        1, 0, source, 3, fields, result,
    ) == 1
    assert output[unsafe_offset=0] == 91
    assert output[unsafe_offset=1] == 92
    assert output[unsafe_offset=2] == 93
    dealloc(output_storage^)
    dealloc(word_storage^)
    dealloc(node_storage^)
