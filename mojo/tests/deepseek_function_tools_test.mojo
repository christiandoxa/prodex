# Run with `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/deepseek_function_tools_test.mojo`.
from parsed_json import pj_literal
from deepseek_function_tools import function_name_valid, generic_wire, canonical_tools_equal, prodex_mojo_deepseek_function_tools_scratch_v1, prodex_mojo_deepseek_function_tools_v1


def main() raises:
    assert function_name_valid(pj_literal(StringSlice("Lookup_09-test")), 64)
    assert not function_name_valid(pj_literal(StringSlice("")), 64)
    assert not function_name_valid(pj_literal(StringSlice("bad.name")), 64)
    assert not function_name_valid(pj_literal(StringSlice("héllo")), 64)
    assert not function_name_valid(pj_literal(StringSlice("abcd")), 3)
    assert generic_wire(pj_literal(StringSlice('{"function":{"parameters":{"additionalProperties":true}}}')))
    assert not generic_wire(pj_literal(StringSlice('{"function":{"parameters":{"additionalProperties":true,"required":null}}}')))
    assert canonical_tools_equal(pj_literal(StringSlice('{"enum":[-0.0]}')), pj_literal(StringSlice('{"enum":[0.0]}')))
    assert not canonical_tools_equal(pj_literal(StringSlice('{"enum":[0]}')), pj_literal(StringSlice('{"enum":[0.0]}')))
    assert not canonical_tools_equal(pj_literal(StringSlice('{"description":"-0.0"}')), pj_literal(StringSlice('{"description":"0.0"}')))
    assert prodex_mojo_deepseek_function_tools_scratch_v1(2, 0, 0, 0, 0, 0, 0, 0) == 4
    assert prodex_mojo_deepseek_function_tools_v1(2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0) == 4
