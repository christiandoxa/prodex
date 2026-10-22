# Decoded session record policy; Serde owns wire parsing and duplicate-key behavior.
from std.memory import Pointer
from parsed_json import (
    ParsedJson, ParsedJsonNode, JSON_ARRAY, JSON_OBJECT, JSON_STRING,
    pj_valid, pj_field, pj_kind, pj_child, pj_next, pj_is, pj_text, pj_equal,
)
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_ptr


def session_prompt_record_shape(
    record_type: Int64, payload_present: Bool, payload_object: Bool, payload_type: Bool,
) -> Bool:
    if not payload_present:
        return False
    if record_type == 1 or record_type == 2:
        return payload_object and payload_type
    if record_type == 3 or record_type == 4:
        return payload_object
    return True


def session_prompt_user_visible(
    record_type: Int64, payload_user: Bool, role_user: Bool,
    metadata_present: Bool, kinds_nonempty: Bool, kinds_all_user_text: Bool,
) -> Bool:
    if record_type == 1:
        return payload_user
    return record_type == 2 and payload_user and role_user and (
        not metadata_present or kinds_nonempty and kinds_all_user_text
    )


def session_prompt_exact_match(
    record_type: Int64, payload_user: Bool, role_user: Bool, visible: Bool, matches: Bool,
) -> Bool:
    return matches and payload_user and (
        record_type == 1 or record_type == 2 and role_user and visible
    )


def session_prompt_record_type(tree: ParsedJson) -> Int64:
    var record = pj_field(tree, 0, "type")
    if pj_is["event_msg"](tree, record):
        return 1
    if pj_is["response_item"](tree, record):
        return 2
    if pj_is["session_meta"](tree, record):
        return 3
    if pj_is["turn_context"](tree, record):
        return 4
    return 5 if pj_kind(tree, record) == JSON_STRING else 0


def session_prompt_metadata_visible(tree: ParsedJson, kinds: Int64) -> Bool:
    if pj_kind(tree, kinds) != JSON_ARRAY:
        return True
    var child = pj_child(tree, kinds)
    if child < 0:
        return False
    while child >= 0:
        if not pj_is["user.text"](tree, child):
            return False
        child = pj_next(tree, child)
    return True


def session_prompt_content_matches(
    tree: ParsedJson, payload: Int64, expected: ProdexRichStringView,
) -> Bool:
    # Compare the LF join directly without allocating another copy of the prompt.
    var content = pj_field(tree, payload, "content")
    if pj_kind(tree, content) != JSON_ARRAY:
        return False
    var child = pj_child(tree, content)
    if child < 0:
        return False
    var cursor: UInt = 0
    var expected_ptr = rich_view_ptr(expected)
    while child >= 0:
        var text = pj_field(tree, child, "text")
        # Presence precedes type: a non-string text must not use content instead.
        if text < 0:
            text = pj_field(tree, child, "content")
        if pj_kind(tree, text) != JSON_STRING:
            return False
        var part = pj_text(tree, text)
        if part.len > expected.len - cursor:
            return False
        if not pj_equal(part, ProdexRichStringView(expected.ptr + cursor, part.len)):
            return False
        cursor += part.len
        child = pj_next(tree, child)
        if child >= 0:
            if cursor == expected.len or expected_ptr[unsafe_offset=Int64(cursor)] != 10:
                return False
            cursor += 1
    return cursor == expected.len


def session_prompt_json_check(
    version: Int64, operation: Int64, nodes_address: UInt, node_count: Int64,
    expected_address: UInt, expected_length: Int64,
) -> Int64:
    if version != 1:
        return -2
    if (operation < 0 or operation > 3 or nodes_address == 0
        or node_count <= 0 or node_count > 1_048_576
        or expected_length < 0 or expected_length > 67_108_864
        or operation != 2 and expected_length != 0):
        return -1
    var expected = ProdexRichStringView(expected_address, UInt(expected_length))
    if not rich_view_valid(expected, 67_108_864):
        return -1
    var tree = ParsedJson(
        Pointer[mut=False, ParsedJsonNode, ImmUntrackedOrigin](unsafe_from_address=Int(nodes_address)),
        node_count, ProdexRichStringView(0, 0),
    )
    var decoded_bytes: UInt = 0
    for index in range(node_count):
        var node = tree.nodes[unsafe_offset=index].copy()
        if node.key.len > 67_108_864 - decoded_bytes:
            return -1
        decoded_bytes += node.key.len
        if node.text.len > 67_108_864 - decoded_bytes:
            return -1
        decoded_bytes += node.text.len
    if not pj_valid(tree):
        return -1
    var record_type = session_prompt_record_type(tree)
    var payload = 0 if operation == 3 else pj_field(tree, 0, "payload")
    var payload_type = pj_field(tree, payload, "type")
    if operation == 0:
        return Int64(session_prompt_record_shape(
            record_type, payload >= 0, pj_kind(tree, payload) == JSON_OBJECT,
            pj_kind(tree, payload_type) == JSON_STRING,
        ))
    var metadata = pj_field(tree, payload, "internal_chat_message_metadata_passthrough")
    var kinds = pj_field(tree, metadata, "content_item_kinds")
    var metadata_present = pj_kind(tree, kinds) == JSON_ARRAY
    var metadata_visible = session_prompt_metadata_visible(tree, kinds)
    var role_user = pj_is["user"](tree, pj_field(tree, payload, "role"))
    var payload_message = pj_is["message"](tree, payload_type)
    if operation == 3:
        # The log caller has already classified the record when metadata is absent.
        return Int64(not metadata_present or session_prompt_user_visible(
            2, payload_message, role_user, True, metadata_visible, metadata_visible,
        ))
    var message = pj_field(tree, payload, "message")
    var payload_user = (pj_is["user_message"](tree, payload_type)
        and pj_kind(tree, message) == JSON_STRING) if record_type == 1 else payload_message
    var visible = session_prompt_user_visible(
        record_type, payload_user, role_user, metadata_present, metadata_visible, metadata_visible,
    )
    if operation == 1:
        return Int64(visible)
    var matches = False
    if record_type == 1 and pj_kind(tree, message) == JSON_STRING:
        matches = pj_equal(pj_text(tree, message), expected)
    elif record_type == 2 and visible:
        matches = session_prompt_content_matches(tree, payload, expected)
    return Int64(session_prompt_exact_match(record_type, payload_user, role_user, visible, matches))
