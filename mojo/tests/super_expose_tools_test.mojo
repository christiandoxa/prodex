# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_tools_test.mojo`.
from parsed_json import pj_literal, pj_equal
from super_expose import super_expose_tool, super_expose_tool_is_allowed, SUPER_EXPOSE_MODE_FULL, SUPER_EXPOSE_MODE_EXEC
from super_expose_tools import tool_definition, TOOL_READ_ONLY, TOOL_DESTRUCTIVE, TOOL_OPEN_WORLD, prodex_mojo_super_expose_tool_catalog_v1
from super_expose_tools import optional_invocation, prodex_mojo_super_expose_optional_presentation_v1
from optional_tools_policy import optional_tool_class


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
    var rtk = optional_tool_class(pj_literal(StringSlice("rtk")))
    assert pj_equal(optional_invocation(rtk), pj_literal(StringSlice("program optional:rtk; use it for noisy shell output")))
    assert optional_tool_class(pj_literal(StringSlice("unknown"))) == -1
    assert prodex_mojo_super_expose_optional_presentation_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0) == 4
