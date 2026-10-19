# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_tools_test.mojo`.
from parsed_json import pj_literal, pj_equal
from super_expose import super_expose_tool, super_expose_tool_is_allowed, SUPER_EXPOSE_MODE_FULL, SUPER_EXPOSE_MODE_EXEC
from super_expose_tools import tool_definition, TOOL_READ_ONLY, TOOL_DESTRUCTIVE, TOOL_OPEN_WORLD, prodex_mojo_super_expose_tool_catalog_v1


def main() raises:
    var start = tool_definition(0)
    var execute = tool_definition(6)
    var prompt = tool_definition(7)
    assert pj_equal(start.name, pj_literal(StringSlice("prodex_super_start")))
    assert start.annotations == TOOL_DESTRUCTIVE | TOOL_OPEN_WORLD
    assert prompt.annotations == 0
    assert tool_definition(9).annotations == TOOL_READ_ONLY
    assert super_expose_tool_is_allowed(SUPER_EXPOSE_MODE_FULL, super_expose_tool(start.name))
    assert not super_expose_tool_is_allowed(SUPER_EXPOSE_MODE_EXEC, super_expose_tool(start.name))
    assert super_expose_tool_is_allowed(SUPER_EXPOSE_MODE_EXEC, super_expose_tool(execute.name))
    assert prodex_mojo_super_expose_tool_catalog_v1(2, 64, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0) == 4
