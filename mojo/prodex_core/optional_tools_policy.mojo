from std.memory import Pointer

from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime OPTIONAL_TOOLS_POLICY_ABI_VERSION: Int64 = 1

comptime OPTIONAL_TOOL_CAVEMAN: Int64 = 0
comptime OPTIONAL_TOOL_RTK: Int64 = 1
comptime OPTIONAL_TOOL_CODEBASE_MEMORY: Int64 = 2
comptime OPTIONAL_TOOL_PLAYWRIGHT: Int64 = 3
comptime OPTIONAL_TOOL_PONYTAIL: Int64 = 4
comptime OPTIONAL_TOOL_PRESIDIO: Int64 = 5

comptime OPTIONAL_TOOL_KIND_COMMAND: Int64 = 0
comptime OPTIONAL_TOOL_KIND_CODEX_PLUGIN: Int64 = 1
comptime OPTIONAL_TOOL_KIND_MCP_SERVER: Int64 = 2
comptime OPTIONAL_TOOL_KIND_SERVICE: Int64 = 3

comptime OPTIONAL_TOOL_CAP_CODEX: Int64 = 1
comptime OPTIONAL_TOOL_CAP_CLAUDE: Int64 = 2
comptime OPTIONAL_TOOL_CAP_SHELL_COMPRESSION: Int64 = 4
comptime OPTIONAL_TOOL_CAP_STRUCTURAL_NAVIGATION: Int64 = 8
comptime OPTIONAL_TOOL_CAP_BROWSER_AUTOMATION: Int64 = 16
comptime OPTIONAL_TOOL_CAP_SIMPLICITY_REVIEW: Int64 = 32
comptime OPTIONAL_TOOL_CAP_REDACTION: Int64 = 64
comptime OPTIONAL_TOOL_CAP_SHIFT: Int64 = 8
comptime OPTIONAL_TOOL_DEFAULT_SHIFT: Int64 = 16


def optional_tool_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def optional_tool_valid(address: UInt, length: Int64) -> Bool:
    if length < 0 or (length > 0 and address == 0):
        return False
    return rich_view_valid(optional_tool_view(address, length), length)


def optional_tool_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def optional_tool_range_equals[literal: StaticString](
    view: ProdexRichStringView, start: Int64, end: Int64
) -> Bool:
    if end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(end - start):
        if optional_tool_ascii_lower(source[unsafe_offset=start + index]) != wanted[unsafe_offset=index]:
            return False
    return True


def optional_tool_views_equal(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    if left.len == 0:
        return True
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if left_ptr[unsafe_offset=index] != right_ptr[unsafe_offset=index]:
            return False
    return True


@export("prodex_optional_tool_class_v1")
def prodex_optional_tool_class_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOLS_POLICY_ABI_VERSION:
        return -2
    if not optional_tool_valid(address, length):
        return -2
    var value = optional_tool_view(address, length)
    var bounds = rich_trim_bounds(value)
    if optional_tool_range_equals["caveman"](value, bounds[0], bounds[1]):
        return OPTIONAL_TOOL_CAVEMAN
    if optional_tool_range_equals["rtk"](value, bounds[0], bounds[1]):
        return OPTIONAL_TOOL_RTK
    if (
        optional_tool_range_equals["codebase-memory-mcp"](value, bounds[0], bounds[1])
        or optional_tool_range_equals["codebase-memory"](value, bounds[0], bounds[1])
        or optional_tool_range_equals["cbm"](value, bounds[0], bounds[1])
    ):
        return OPTIONAL_TOOL_CODEBASE_MEMORY
    if (
        optional_tool_range_equals["playwright"](value, bounds[0], bounds[1])
        or optional_tool_range_equals["playwright-mcp"](value, bounds[0], bounds[1])
    ):
        return OPTIONAL_TOOL_PLAYWRIGHT
    if optional_tool_range_equals["ponytail"](value, bounds[0], bounds[1]):
        return OPTIONAL_TOOL_PONYTAIL
    if optional_tool_range_equals["presidio"](value, bounds[0], bounds[1]):
        return OPTIONAL_TOOL_PRESIDIO
    return -1


@export("prodex_optional_tool_descriptor_policy_v1")
def prodex_optional_tool_descriptor_policy_v1(
    abi_version: Int64,
    tool_id: Int64,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOLS_POLICY_ABI_VERSION or tool_id < 0 or tool_id > 5:
        return -2
    var kind: Int64 = OPTIONAL_TOOL_KIND_COMMAND
    var capabilities: Int64 = 0
    var super_default: Int64 = 1
    if tool_id == OPTIONAL_TOOL_CAVEMAN:
        kind = OPTIONAL_TOOL_KIND_CODEX_PLUGIN
        capabilities = OPTIONAL_TOOL_CAP_CODEX | OPTIONAL_TOOL_CAP_CLAUDE
    elif tool_id == OPTIONAL_TOOL_RTK:
        kind = OPTIONAL_TOOL_KIND_COMMAND
        capabilities = OPTIONAL_TOOL_CAP_SHELL_COMPRESSION
    elif tool_id == OPTIONAL_TOOL_CODEBASE_MEMORY:
        kind = OPTIONAL_TOOL_KIND_MCP_SERVER
        capabilities = OPTIONAL_TOOL_CAP_STRUCTURAL_NAVIGATION
    elif tool_id == OPTIONAL_TOOL_PLAYWRIGHT:
        kind = OPTIONAL_TOOL_KIND_MCP_SERVER
        capabilities = OPTIONAL_TOOL_CAP_BROWSER_AUTOMATION
    elif tool_id == OPTIONAL_TOOL_PONYTAIL:
        kind = OPTIONAL_TOOL_KIND_CODEX_PLUGIN
        capabilities = OPTIONAL_TOOL_CAP_SIMPLICITY_REVIEW
    else:
        kind = OPTIONAL_TOOL_KIND_SERVICE
        capabilities = OPTIONAL_TOOL_CAP_REDACTION
        super_default = 0
    return kind | (capabilities << OPTIONAL_TOOL_CAP_SHIFT) | (super_default << OPTIONAL_TOOL_DEFAULT_SHIFT)


@export("prodex_optional_tool_manifest_tree_supported_v1")
def prodex_optional_tool_manifest_tree_supported_v1(
    abi_version: Int64,
    value_address: UInt,
    value_length: Int64,
    vetted_address: UInt,
    vetted_length: Int64,
    legacy_address: UInt,
    legacy_length: Int64,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOLS_POLICY_ABI_VERSION:
        return -2
    if (
        not optional_tool_valid(value_address, value_length)
        or not optional_tool_valid(vetted_address, vetted_length)
        or not optional_tool_valid(legacy_address, legacy_length)
    ):
        return -2
    var value = optional_tool_view(value_address, value_length)
    var vetted = optional_tool_view(vetted_address, vetted_length)
    var legacy = optional_tool_view(legacy_address, legacy_length)
    return Int64(
        optional_tool_views_equal(value, vetted)
        or optional_tool_views_equal(value, legacy)
    )
