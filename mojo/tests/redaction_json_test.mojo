# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/redaction_json_test.mojo`.
from std.collections import List
from std.memory import Pointer
from parsed_json import ParsedJsonNode, JSON_OBJECT, JSON_STRING
from rich_types import ProdexRichStringView
from redaction_json import prodex_redaction_json_content_plan_v1


def view(value: StringSlice) -> ProdexRichStringView:
    return ProdexRichStringView(UInt(Int(value.unsafe_ptr())), UInt(value.byte_length()))


def test_opaque_root_wrappers_keep_the_same_inspection_and_replacement_slot() raises:
    var nodes = List[ParsedJsonNode]()
    var empty = view(StringSlice(""))
    nodes.append(ParsedJsonNode(JSON_OBJECT, 1, -1, -1, empty.copy(), empty.copy(), 0, 0))
    nodes.append(ParsedJsonNode(JSON_OBJECT, 2, 4, 0, view(StringSlice("a")), empty.copy(), 0, 0))
    nodes.append(ParsedJsonNode(JSON_OBJECT, 3, -1, 1, view(StringSlice("tools")), empty.copy(), 0, 0))
    nodes.append(ParsedJsonNode(JSON_STRING, -1, -1, 2, view(StringSlice("content")), view(StringSlice("not inspected")), 0, 0))
    nodes.append(ParsedJsonNode(JSON_STRING, -1, -1, 0, view(StringSlice("input")), view(StringSlice("user@example.com")), 0, 0))
    var scratch = List[UInt64](length=128, fill=0)
    var output = List[UInt8](length=512, fill=71)
    var metadata = List[Int64](length=2, fill=71)
    assert prodex_redaction_json_content_plan_v1(
        1, 0, 0, UInt64(Int(nodes.unsafe_ptr())), 5, 0, 0,
        UInt64(Int(scratch.unsafe_ptr())), 64, 0,
        UInt64(Int(output.unsafe_ptr())), 512, UInt64(Int(metadata.unsafe_ptr())),
    ) == 0
    assert metadata[0] == 1
    var result = String(unsafe_from_utf8=Span[UInt8](
        unsafe_ptr=output.unsafe_ptr(), length=Int(metadata[1])
    ))
    assert result == '{"slots":[{"node":4,"path":"$.input","sensitive_kind":null}],"coverage":"partial","error":0}'
    metadata[0] = 71
    metadata[1] = 71
    var short_status = prodex_redaction_json_content_plan_v1(
        1, 0, 0, UInt64(Int(nodes.unsafe_ptr())), 5, 0, 0,
        UInt64(Int(scratch.unsafe_ptr())), 64, 0,
        UInt64(Int(output.unsafe_ptr())), 1, UInt64(Int(metadata.unsafe_ptr())),
    )
    assert nodes[4].kind == JSON_STRING
    assert scratch[0] != 0
    assert output[0] == 123
    assert short_status == 3
    assert metadata[0] == 71 and metadata[1] == 71
    assert prodex_redaction_json_content_plan_v1(
        2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, UInt64(Int(metadata.unsafe_ptr())),
    ) == 4
    assert metadata[0] == 71 and metadata[1] == 71


def main() raises:
    test_opaque_root_wrappers_keep_the_same_inspection_and_replacement_slot()
