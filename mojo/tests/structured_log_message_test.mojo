from std.memory import Pointer
from std.memory.alloc import alloc, dealloc, Layout
from rich_types import ProdexRichStringView
from parsed_json import pj_literal
from log_semantics import prodex_mojo_structured_log_message_v1, prodex_mojo_structured_log_message_field_policy_v1, prodex_mojo_structured_log_field_policy_v1


def verify(event: StringSlice, fields: UInt, count: Int64, expected: StringSlice,
    output: Pointer[mut=True, UInt8, _], written: Pointer[mut=True, Int64, _]):
    var address = UInt(Int(event.unsafe_ptr()))
    var length = Int64(event.byte_length())
    var target = UInt(Int(output))
    var meta = UInt(Int(written))
    assert prodex_mojo_structured_log_message_v1(1, address, length, fields, count, 1, 0, 0, meta) == 0
    assert written[] == Int64(expected.byte_length())
    assert prodex_mojo_structured_log_message_v1(1, address, length, fields, count, 0, target, 128, meta) == 0
    for index in range(Int64(expected.byte_length())):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]


def main() raises:
    var os = alloc(Layout[UInt8](count=128))
    var ws = alloc(Layout[Int64](count=5))
    var fs = alloc(Layout[ProdexRichStringView](count=6))
    var output = os.unsafe_ptr()
    var written = ws.unsafe_ptr()
    var fields = fs.unsafe_ptr()
    fields[unsafe_offset=0] = pj_literal(StringSlice("clé"))
    fields[unsafe_offset=1] = pj_literal(StringSlice("\"one two\""))
    fields[unsafe_offset=2] = pj_literal(StringSlice("clé"))
    fields[unsafe_offset=3] = pj_literal(StringSlice("two"))
    fields[unsafe_offset=4] = pj_literal(StringSlice("empty"))
    fields[unsafe_offset=5] = pj_literal(StringSlice("\"\""))
    var table = UInt(Int(fields))
    verify("", table, 3, "clé=\"one two\" clé=two empty=\"\"", output, written)
    verify("évent ", table, 1, "évent  clé=\"one two\"", output, written)
    verify("", 0, 0, "", output, written)
    verify(" ", 0, 0, " ", output, written)
    var target = UInt(Int(output))
    var meta = UInt(Int(written))
    output[unsafe_offset=0] = 73
    written[] = 83
    assert prodex_mojo_structured_log_message_v1(2, 0, 0, 0, 0, 0, target, 128, meta) == 4
    assert prodex_mojo_structured_log_message_v1(1, 0, 0, table, 1, 0, target, 1, meta) == 2
    assert prodex_mojo_structured_log_message_v1(1, 0, 0, table, 1, 2, target, 128, meta) == 1
    fields[unsafe_offset=1] = ProdexRichStringView(fields[unsafe_offset=0].ptr + 2, 1)
    assert prodex_mojo_structured_log_message_v1(1, 0, 0, table, 1, 0, target, 128, meta) == 1
    assert output[unsafe_offset=0] == 73 and written[] == 83
    assert prodex_mojo_structured_log_field_policy_v1(1, 0, 0, 0, 0, meta) == 0
    assert written[] == 0
    assert prodex_mojo_structured_log_message_field_policy_v1(1, 0, 0, 0, 0, meta) == 0
    assert written[] == 1
    var bad = StringSlice("bad key")
    assert prodex_mojo_structured_log_message_field_policy_v1(1, UInt(Int(bad.unsafe_ptr())), 7, 0, 0, meta) == 0
    assert written[] == 1
    dealloc(os^)
    dealloc(ws^)
    dealloc(fs^)
