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
comptime OPTIONAL_TOOL_HEX_MAX_INPUT: Int64 = 4_611_686_018_427_387_903


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


def optional_tool_hex_digit(value: UInt8) -> UInt8:
    if value < 10:
        return value + 48
    return value + 87


comptime OPTIONAL_TOOL_AWARENESS_ABI_VERSION: Int64 = 1
comptime OPTIONAL_TOOL_AWARENESS_OK: Int64 = 0
comptime OPTIONAL_TOOL_AWARENESS_INVALID: Int64 = 1
comptime OPTIONAL_TOOL_AWARENESS_CAPACITY: Int64 = 3
comptime OPTIONAL_TOOL_AWARENESS_ABI: Int64 = 4
comptime OPTIONAL_TOOL_AWARENESS_TEXT_COUNT: Int64 = 4

@fieldwise_init
struct OptionalToolAwarenessWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def optional_tool_awareness_text(
    address: UInt, index: Int64
) -> ProdexRichStringView:
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(address))
    return values[unsafe_offset=index].copy()


def optional_tool_awareness_put_byte(
    writer: Pointer[mut=True, OptionalToolAwarenessWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def optional_tool_awareness_put_literal(
    writer: Pointer[mut=True, OptionalToolAwarenessWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not optional_tool_awareness_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def optional_tool_awareness_put_view(
    writer: Pointer[mut=True, OptionalToolAwarenessWriter, _],
    value: ProdexRichStringView,
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not optional_tool_awareness_put_byte(
            writer, source[unsafe_offset=index]
        ):
            return False
    return True


def optional_tool_awareness_put_available(
    writer: Pointer[mut=True, OptionalToolAwarenessWriter, _],
    availability_mask: Int64,
    bit: Int64,
    value: ProdexRichStringView,
) -> Bool:
    if availability_mask & bit != 0:
        return (
            optional_tool_awareness_put_literal(writer, StringSlice("yes ("))
            and optional_tool_awareness_put_view(writer, value)
            and optional_tool_awareness_put_byte(writer, UInt8(41))
        )
    return optional_tool_awareness_put_literal(writer, StringSlice("no"))


@export("prodex_optional_tool_awareness_v1")
def prodex_optional_tool_awareness_v1(
    abi_version: Int64,
    availability_mask: Int64,
    presidio_enabled: Int64,
    text_address: UInt,
    text_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_AWARENESS_ABI_VERSION:
        return OPTIONAL_TOOL_AWARENESS_ABI
    if (
        availability_mask < 0
        or availability_mask > 15
        or (presidio_enabled != 0 and presidio_enabled != 1)
        or text_count != OPTIONAL_TOOL_AWARENESS_TEXT_COUNT
        or text_address == 0
        or output_capacity < 0
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return OPTIONAL_TOOL_AWARENESS_INVALID
    for index in range(text_count):
        var value = optional_tool_awareness_text(text_address, index)
        if not rich_view_valid(value, Int64(value.len)):
            return OPTIONAL_TOOL_AWARENESS_INVALID

    var writer = OptionalToolAwarenessWriter(
        Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        ),
        output_capacity,
        0,
    )
    var rtk = optional_tool_awareness_text(text_address, 0)
    var codebase_memory = optional_tool_awareness_text(text_address, 1)
    var playwright = optional_tool_awareness_text(text_address, 2)
    var ponytail = optional_tool_awareness_text(text_address, 3)
    var ok = (
        optional_tool_awareness_put_literal(
            Pointer(to=writer),
            StringSlice("# Prodex Optional Tools\n\nProdex resolved optional tools for this temporary launch overlay.\n\n- Use visible `rtk <cmd>` for noisy shell output when RTK is available.\n- When Codebase Memory MCP is available, use it first for architecture, call-chain, impact,\n  and structural code search; run `index_repository` first when the workspace is not indexed.\n- Use Playwright MCP for browser work when available.\n- Follow Ponytail when its plugin is active.\n- Treat Presidio as enabled only when the session status says so.\n\nMissing tools are not active. Do not claim they were used.\n")
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer), StringSlice("\n## Available Now\n\n- rtk: ")
        )
        and optional_tool_awareness_put_available(
            Pointer(to=writer), availability_mask, 1, rtk
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer), StringSlice("\n- codebase-memory-mcp: ")
        )
        and optional_tool_awareness_put_available(
            Pointer(to=writer), availability_mask, 2, codebase_memory
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer), StringSlice("\n- playwright-mcp: ")
        )
        and optional_tool_awareness_put_available(
            Pointer(to=writer), availability_mask, 4, playwright
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer), StringSlice("\n- ponytail plugin: ")
        )
        and optional_tool_awareness_put_available(
            Pointer(to=writer), availability_mask, 8, ponytail
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer), StringSlice("\n- presidio: ")
        )
        and optional_tool_awareness_put_literal(
            Pointer(to=writer),
            StringSlice("enabled\n") if presidio_enabled == 1 else StringSlice("disabled\n"),
        )
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return OPTIONAL_TOOL_AWARENESS_OK if ok else OPTIONAL_TOOL_AWARENESS_CAPACITY


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


@export("prodex_optional_tool_hex_encode_v1")
def prodex_optional_tool_hex_encode_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOLS_POLICY_ABI_VERSION:
        return -2
    if (
        input_length < 0
        or input_length > OPTIONAL_TOOL_HEX_MAX_INPUT
        or output_capacity < 0
    ):
        return -2
    var output_length = input_length * 2
    if output_capacity < output_length:
        return -3
    if input_length == 0:
        return 0
    if input_address == 0 or output_address == 0:
        return -2

    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(input_length):
        var value = source[unsafe_offset=index]
        output[unsafe_offset=index * 2] = optional_tool_hex_digit(value >> 4)
        output[unsafe_offset=index * 2 + 1] = optional_tool_hex_digit(value & UInt8(15))
    return output_length


# Deterministic optional-tool config-shape preflight before any host mutation.
# Rust extracts TOML section kinds; only this owner decides selected section
# precedence and which wrong shape is a user-visible configuration error.
comptime OPTIONAL_TOOL_CONFIG_SHAPE_ABI_VERSION: Int64 = 1
comptime OPTIONAL_TOOL_CONFIG_SHAPE_FIELD_COUNT: Int64 = 8
comptime OPTIONAL_TOOL_CONFIG_PONYTAIL: Int64 = 1
comptime OPTIONAL_TOOL_CONFIG_MEMORY: Int64 = 2
comptime OPTIONAL_TOOL_CONFIG_PLAYWRIGHT: Int64 = 4


@export("prodex_optional_tool_config_shape_v1")
def prodex_optional_tool_config_shape_v1(
    abi_version: Int64,
    selected_mask: Int64,
    shape_address: UInt,
    shape_count: Int64,
    issue_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_CONFIG_SHAPE_ABI_VERSION:
        return 4
    if (
        selected_mask < 0 or selected_mask > 7
        or shape_address == 0
        or shape_count != OPTIONAL_TOOL_CONFIG_SHAPE_FIELD_COUNT
        or issue_address == 0
    ):
        return 1
    var shape = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(shape_address)
    )
    # Zero is absent, one is a table, two is a present non-table. Validate
    # all bits prior to emitting any output, even when a section is unselected.
    for index in range(OPTIONAL_TOOL_CONFIG_SHAPE_FIELD_COUNT):
        var value = shape[unsafe_offset=index]
        if value < 0 or value > 2:
            return 1
    var issue: Int64 = 0
    if (selected_mask & OPTIONAL_TOOL_CONFIG_PONYTAIL) != 0:
        for index in range(5):
            if shape[unsafe_offset=index] == 2:
                issue = Int64(index + 1)
                break
    if (
        issue == 0
        and (selected_mask & (OPTIONAL_TOOL_CONFIG_MEMORY | OPTIONAL_TOOL_CONFIG_PLAYWRIGHT)) != 0
    ):
        if shape[unsafe_offset=5] == 2:
            issue = 6
        elif shape[unsafe_offset=5] == 1:
            if (
                (selected_mask & OPTIONAL_TOOL_CONFIG_MEMORY) != 0
                and shape[unsafe_offset=6] == 2
            ):
                issue = 7
            elif (
                (selected_mask & OPTIONAL_TOOL_CONFIG_PLAYWRIGHT) != 0
                and shape[unsafe_offset=7] == 2
            ):
                issue = 8
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(issue_address)
    )[] = issue
    return 0


# Required MCP config equality is a security gate, not an overwrite policy:
# inherited command and argument vectors must exactly match the managed
# command plan, with enabled absent/true only. Rust supplies TOML facts.
@export("prodex_optional_tool_required_mcp_contract_v1")
def prodex_optional_tool_required_mcp_contract_v1(
    abi_version: Int64,
    enabled_tag: Int64,
    actual_command_address: UInt,
    actual_command_length: Int64,
    expected_command_address: UInt,
    expected_command_length: Int64,
    actual_args_address: UInt,
    actual_args_count: Int64,
    expected_args_address: UInt,
    expected_args_count: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOLS_POLICY_ABI_VERSION:
        return 4
    if (
        enabled_tag < 0 or enabled_tag > 3
        or actual_command_length < -1 or expected_command_length < 0
        or actual_args_count < -1 or expected_args_count < 0
        or (actual_command_length > 0 and actual_command_address == 0)
        or (expected_command_length > 0 and expected_command_address == 0)
        or (actual_args_count > 0 and actual_args_address == 0)
        or (expected_args_count > 0 and expected_args_address == 0)
        or output_address == 0
    ):
        return 1
    var actual_command = optional_tool_view(
        actual_command_address,
        max(actual_command_length, Int64(0)),
    )
    var expected_command = optional_tool_view(
        expected_command_address,
        expected_command_length,
    )
    if (
        not rich_view_valid(actual_command, 0x7FFFFFFFFFFFFFFF)
        or not rich_view_valid(expected_command, 0x7FFFFFFFFFFFFFFF)
    ):
        return 1
    var actual_args = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(actual_args_address))
    var expected_args = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(expected_args_address))
    for index in range(actual_args_count):
        if not rich_view_valid(actual_args[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    for index in range(expected_args_count):
        if not rich_view_valid(expected_args[unsafe_offset=index], 0x7FFFFFFFFFFFFFFF):
            return 1
    var issue: Int64 = 0
    if enabled_tag == 2:
        issue = 1
    elif enabled_tag == 3:
        issue = 2
    elif actual_command_length < 0 or actual_args_count != expected_args_count:
        issue = 3
    elif not optional_tool_views_equal(actual_command, expected_command):
        issue = 3
    else:
        for index in range(actual_args_count):
            if not optional_tool_views_equal(
                actual_args[unsafe_offset=index],
                expected_args[unsafe_offset=index],
            ):
                issue = 3
                break
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = issue
    return 0
