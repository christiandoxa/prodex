# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/gemini_tool_descriptions_test.mojo`.
from std.collections import Array
from std.memory import Pointer
from parsed_json import pj_literal, pj_equal
from gemini_tool_descriptions import description_kind, description_text, parameter_text, prodex_mojo_gemini_tool_descriptions_v1


def main() raises:
    var scratch = Array[UInt8, 512](fill=0)
    var address = UInt64(Int(Pointer(to=scratch[0])))
    assert description_kind(pj_literal(StringSlice(" Namespace.Read-File ")), address, 512) == 1
    assert description_kind(pj_literal(StringSlice("read")), address, 512) == 1
    assert description_kind(pj_literal(StringSlice("mcp__repo__read")), address, 512) == 0
    assert description_kind(pj_literal(StringSlice("mcp__repo__read_file")), address, 512) == 1
    assert description_kind(pj_literal(StringSlice("glob")), address, 512) == 2
    assert description_kind(pj_literal(StringSlice("search")), address, 512) == 3
    assert description_kind(pj_literal(StringSlice("bash")), address, 512) == 4
    assert description_kind(pj_literal(StringSlice("mcp__repo__bash")), address, 512) == 0
    assert description_kind(pj_literal(StringSlice("write")), address, 512) == 5
    assert description_kind(pj_literal(StringSlice("edit")), address, 512) == 6
    assert description_kind(pj_literal(StringSlice("unrelated")), address, 512) == 0
    assert description_kind(pj_literal(StringSlice("read_file")), address, 1) == -1
    assert pj_equal(parameter_text(1, pj_literal(StringSlice("end_line"))), pj_literal(StringSlice("1-based last line to read, inclusive.")))
    assert parameter_text(1, pj_literal(StringSlice("path"))).len == 0
    assert description_text(6).len > 0
    assert prodex_mojo_gemini_tool_descriptions_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0) == 4
