from std.memory.alloc import alloc, dealloc, Layout
from optional_tool_diagnostics import prodex_optional_tool_diagnostic_v1


def check(group: Int64, issue: Int64, name: StringSlice, expected: StringSlice) raises:
    var storage = alloc(Layout[UInt8](count=1024))
    var result_storage = alloc(Layout[Int64](count=1))
    var output = storage.unsafe_ptr()
    var written = result_storage.unsafe_ptr()
    var name_address = UInt(Int(name.unsafe_ptr()))
    var target = UInt(Int(output))
    var result = UInt(Int(written))
    assert prodex_optional_tool_diagnostic_v1(1, group, issue, name_address, Int64(name.byte_length()), target, 1024, result) == 0
    assert written[] == Int64(expected.byte_length())
    for index in range(written[]):
        assert output[unsafe_offset=index] == expected.unsafe_ptr()[unsafe_offset=index]
    for index in range(1024):
        output[unsafe_offset=index] = 73
    written[] = 83
    assert prodex_optional_tool_diagnostic_v1(2, group, issue, name_address, Int64(name.byte_length()), target, 1024, result) == 4
    assert prodex_optional_tool_diagnostic_v1(1, group, issue, name_address, Int64(name.byte_length()), target, 1, result) == 3
    assert prodex_optional_tool_diagnostic_v1(1, -1, issue, name_address, 0, target, 1024, result) == 1
    assert prodex_optional_tool_diagnostic_v1(1, group, 0, name_address, 0, target, 1024, result) == 1
    assert prodex_optional_tool_diagnostic_v1(1, group, issue, name_address, -1, target, 1024, result) == 1
    for index in range(1024):
        assert output[unsafe_offset=index] == 73
    assert written[] == 83
    dealloc(storage^)
    dealloc(result_storage^)


def main() raises:
    check(0, 1, "", "configuration entry `features` must be a TOML table")
    check(0, 2, "", "configuration entry `marketplaces` must be a TOML table")
    check(0, 3, "", "configuration entry `plugins` must be a TOML table")
    check(0, 4, "", "configuration entry `ponytail` must be a TOML table")
    check(0, 5, "", "configuration entry `ponytail@ponytail` must be a TOML table")
    check(0, 6, "", "mcp_servers must be a TOML table")
    check(0, 7, "", "mcp_servers.codebase-memory-mcp must be a TOML table")
    check(0, 8, "", "mcp_servers.playwright must be a TOML table")
    check(1, 1, "東京", "required MCP server `mcp_servers.東京` is disabled (`enabled = false`); refusing to override user configuration")
    check(1, 2, "", "required MCP server `mcp_servers.` cannot be safely verified: `enabled` must be a boolean")
    check(1, 3, "fixture", "required MCP server `mcp_servers.fixture` cannot be safely verified: inherited configuration is custom or incomplete; refusing to replace user configuration")
