# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_alias_test.mojo`.
from std.collections import Array
from std.memory import Pointer
from parsed_json import pj_literal, pj_equal
from rich_types import ProdexRichStringView
from json_sink import JsonSink
from super_expose_exec import super_expose_write_optional_alias_plan, prodex_mojo_super_expose_optional_alias_plan_v1


def plan_matches(alias: StringSlice, expected: StringSlice) -> Bool:
    var output = Array[UInt8, 1024](fill=0)
    var buffer = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(Pointer(to=output[0])))
    var sink = JsonSink(buffer, 1024, 0, False, False)
    super_expose_write_optional_alias_plan(Pointer(to=sink), pj_literal(alias))
    return not sink.failed and pj_equal(ProdexRichStringView(UInt(Int(Pointer(to=output[0]))), UInt(sink.written)), pj_literal(expected))


def main() raises:
    assert plan_matches(StringSlice(" RTK "), StringSlice('{"tool":"rtk","program":{"kind":"tool_path","missing_path_message":"validated optional tool rtk has no executable path"},"prefix":[]}'))
    assert plan_matches(StringSlice("playwright"), StringSlice('{"tool":"playwright-mcp","program":{"kind":"prodex"},"prefix":["super","--no-sub-agent","--no-presidio","--tool","playwright"]}'))
    assert plan_matches(StringSlice("PLAYWRIGHT"), StringSlice('{"tool":"playwright-mcp","program":{"kind":"tool_path","missing_path_message":"validated Playwright MCP has no npx executable path"},"prefix":["--no-install","@playwright/mcp"]}'))
    assert plan_matches(StringSlice("unknown"), StringSlice("null"))
    assert prodex_mojo_super_expose_optional_alias_plan_v1(2, 0, 0, 0, 0, 0) == 4
