from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING, JSON_ARRAY, JSON_NULL
from rich_types import ProdexRichStringView
from session_report import prodex_session_prompt_write_json_v1 as session_prompt_json_check


def view(text: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(text.unsafe_ptr())), UInt(text.byte_length()))


def node(kind: Int64, child: Int64, sibling: Int64, parent: Int64,
         key: StringSlice, text: StringSlice) -> ParsedJsonNode:
    return ParsedJsonNode(kind, child, sibling, parent, view(key), view(text), 0, 0)


def check(nodes: Pointer[mut=True, ParsedJsonNode, _], count: Int64,
          operation: Int64, expected: StringSlice = "") -> Int64:
    var value = view(expected)
    return session_prompt_json_check(1, operation, UInt(Int(nodes)), count, value.ptr, Int64(value.len))


def main() raises:
    var storage = alloc(Layout[ParsedJsonNode](count=14))
    var nodes = storage.unsafe_ptr()
    nodes[unsafe_offset=0] = node(JSON_OBJECT, 1, -1, -1, "", "")
    nodes[unsafe_offset=1] = node(JSON_STRING, -1, 2, 0, "type", "event_msg")
    nodes[unsafe_offset=2] = node(JSON_OBJECT, 3, -1, 0, "payload", "")
    nodes[unsafe_offset=3] = node(JSON_STRING, -1, 4, 2, "type", "user_message")
    nodes[unsafe_offset=4] = node(JSON_STRING, -1, -1, 2, "message", "  東京\n\0🧭 ")
    assert check(nodes, 5, 0) == 1
    assert check(nodes, 5, 1) == 1
    assert check(nodes, 5, 2, "  東京\n\0🧭 ") == 1
    assert check(nodes, 5, 2, "東京\n\0🧭") == 0
    nodes[unsafe_offset=4].kind = JSON_NULL
    assert check(nodes, 5, 1) == 0
    assert check(nodes, 5, 2, "") == 0
    nodes[unsafe_offset=1].text = view("unknown")
    nodes[unsafe_offset=2] = node(JSON_NULL, -1, -1, 0, "payload", "")
    assert check(nodes, 3, 0) == 1
    nodes[unsafe_offset=1].text = view("session_meta")
    assert check(nodes, 3, 0) == 0
    nodes[unsafe_offset=2].kind = JSON_OBJECT
    assert check(nodes, 3, 0) == 1

    nodes[unsafe_offset=1].text = view("response_item")
    nodes[unsafe_offset=2].first_child = 3
    nodes[unsafe_offset=3] = node(JSON_STRING, -1, 4, 2, "type", "message")
    nodes[unsafe_offset=4] = node(JSON_STRING, -1, 5, 2, "role", "user")
    nodes[unsafe_offset=5] = node(JSON_ARRAY, 6, 10, 2, "content", "")
    nodes[unsafe_offset=6] = node(JSON_OBJECT, 7, 8, 5, "", "")
    nodes[unsafe_offset=7] = node(JSON_STRING, -1, -1, 6, "text", "first ")
    nodes[unsafe_offset=8] = node(JSON_OBJECT, 9, -1, 5, "", "")
    nodes[unsafe_offset=9] = node(JSON_STRING, -1, -1, 8, "content", "\n東京🧭")
    nodes[unsafe_offset=10] = node(JSON_OBJECT, 11, -1, 2, "internal_chat_message_metadata_passthrough", "")
    nodes[unsafe_offset=11] = node(JSON_ARRAY, 12, -1, 10, "content_item_kinds", "")
    nodes[unsafe_offset=12] = node(JSON_STRING, -1, -1, 11, "", "user.text")
    assert check(nodes, 13, 0) == 1
    assert check(nodes, 13, 1) == 1
    assert check(nodes, 13, 2, "first \n\n東京🧭") == 1
    assert check(nodes, 13, 2, "first \n東京🧭") == 0
    nodes[unsafe_offset=7].text = view("")
    nodes[unsafe_offset=9].text = view("")
    assert check(nodes, 13, 2, "\n") == 1
    assert check(nodes, 13, 2, "") == 0
    nodes[unsafe_offset=7].kind = JSON_NULL
    nodes[unsafe_offset=7].next_sibling = 13
    nodes[unsafe_offset=13] = node(JSON_STRING, -1, -1, 6, "content", "fallback")
    assert check(nodes, 14, 2, "fallback\n") == 0
    nodes[unsafe_offset=7].kind = JSON_STRING
    nodes[unsafe_offset=7].next_sibling = -1
    nodes[unsafe_offset=12].text = view("internal")
    assert check(nodes, 14, 1) == 0
    nodes[unsafe_offset=12].text = view("user.text")
    nodes[unsafe_offset=11].first_child = -1
    assert check(nodes, 12, 1) == 0
    nodes[unsafe_offset=11].kind = JSON_NULL
    assert check(nodes, 12, 1) == 1
    nodes[unsafe_offset=4].text = view("assistant")
    assert check(nodes, 12, 1) == 0

    # Log payload visibility has an intentional absent-metadata fast path.
    nodes[unsafe_offset=0] = node(JSON_OBJECT, 1, -1, -1, "", "")
    nodes[unsafe_offset=1] = node(JSON_STRING, -1, -1, 0, "role", "assistant")
    assert check(nodes, 2, 3) == 1
    assert check(nodes, 0, 3) == -1
    assert check(nodes, 2, 4) == -1
    nodes[unsafe_offset=1].next_sibling = 1
    assert check(nodes, 2, 3) == -1
    assert session_prompt_json_check(2, 0, 0, 0, 0, 0) == -2
    assert session_prompt_json_check(1, 0, 0, 1, 0, 0) == -1
    assert session_prompt_json_check(1, 2, UInt(Int(nodes)), 2, 0, 1) == -1
    dealloc(storage^)
