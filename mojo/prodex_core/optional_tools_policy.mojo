from std.memory import Pointer
from core_file_policy import core_file_view_suffix

from rich_text import (
    rich_view_prefix,
    rich_codepoint, rich_codepoint_width, rich_trim_bounds,
    rich_unicode_space, rich_view_ptr, rich_view_valid,
)
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


def optional_tool_class(value: ProdexRichStringView) -> Int64:
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
    return optional_tool_class(optional_tool_view(address, length))


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

# Rust owns the semver parser, Mojo owns the exact first-candidate lexical
# traversal used for optional tool probe output, including Unicode whitespace
# and ASCII token punctuation trimming. Output [start,end,next_cursor].
comptime OPTIONAL_TOOL_PROBE_TOKEN_ABI: Int64 = 1
comptime OPTIONAL_TOOL_PROBE_TOKEN_MAX_BYTES: Int64 = 1_048_576


def optional_tool_probe_token_char_allowed(codepoint: Int64) -> Bool:
    return (
        (codepoint >= 48 and codepoint <= 57)
        or (codepoint >= 65 and codepoint <= 90)
        or (codepoint >= 97 and codepoint <= 122)
        or codepoint == 43 or codepoint == 45 or codepoint == 46
    )


@export("prodex_optional_tool_probe_token_v1")
def prodex_optional_tool_probe_token_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    cursor: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_PROBE_TOKEN_ABI:
        return 4
    if (
        input_length < 0 or input_length > OPTIONAL_TOOL_PROBE_TOKEN_MAX_BYTES
        or cursor < 0 or cursor > input_length
        or (input_length > 0 and input_address == 0)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var view = ProdexRichStringView(input_address, UInt(input_length))
    if not rich_view_valid(view, OPTIONAL_TOOL_PROBE_TOKEN_MAX_BYTES):
        return 1
    var bytes = rich_view_ptr(view)
    var index = cursor
    while index < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        if not rich_unicode_space(codepoint):
            break
        index += width

    var token_start = index
    while index < input_length:
        var width = rich_codepoint_width(bytes[unsafe_offset=index])
        var codepoint = rich_codepoint(bytes, index, width)
        if rich_unicode_space(codepoint):
            break
        index += width
    var token_end = index
    # Rust char::is_ascii_alphanumeric is strictly ASCII. Non-ASCII characters
    # inside a token remain intact; only boundary punctuation is trimmed.
    var start = token_start
    var end = token_start
    var saw_allowed = False
    var probe = token_start
    while probe < token_end:
        var width = rich_codepoint_width(bytes[unsafe_offset=probe])
        var codepoint = rich_codepoint(bytes, probe, width)
        probe += width
        if optional_tool_probe_token_char_allowed(codepoint):
            if not saw_allowed:
                start = probe - width
                saw_allowed = True
            end = probe
    if not saw_allowed:
        start = token_end
        end = token_end
    while start < end and bytes[unsafe_offset=start] == 118:
        start += 1

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if token_start == input_length:
        output[unsafe_offset=0] = -1
        output[unsafe_offset=1] = -1
        output[unsafe_offset=2] = input_length
    else:
        output[unsafe_offset=0] = start
        output[unsafe_offset=1] = end
        output[unsafe_offset=2] = token_end
    return 0

# Overlay home may only share/localize SQLite rollout state files that match
# the exact Codex state_ prefix and a supported canonical suffix.
comptime OPTIONAL_TOOL_OVERLAY_STATE_ABI_VERSION: Int64 = 1
comptime OPTIONAL_TOOL_OVERLAY_STATE_NAME_MAX: Int64 = 1_048_576


@export("prodex_optional_tool_overlay_rollout_name_v1")
def prodex_optional_tool_overlay_rollout_name_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_OVERLAY_STATE_ABI_VERSION:
        return 4
    if (
        name_length < 0 or name_length > OPTIONAL_TOOL_OVERLAY_STATE_NAME_MAX
        or not optional_tool_valid(name_address, name_length)
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var name = ProdexRichStringView(name_address, UInt(name_length))
    var matched = rich_view_prefix["state_"](name, False) and (
        core_file_view_suffix[".sqlite"](name)
        or core_file_view_suffix[".sqlite-shm"](name)
        or core_file_view_suffix[".sqlite-wal"](name)
    )
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )[] = Int64(matched)
    return 0

# Legacy Caveman instruction cleanup uses SHA-256 digests acquired by Rust.
# Compiled Mojo alone compares the one known retired instruction digest and
# selects input-order paragraphs to retain, without touching TOML or files.
comptime OPTIONAL_TOOL_LEGACY_INSTRUCTIONS_ABI: Int64 = 1
comptime OPTIONAL_TOOL_LEGACY_INSTRUCTIONS_MAX_PARAGRAPHS: Int64 = 65_536


def optional_tool_legacy_digest_matches(
    digests: Pointer[mut=False, UInt8, _], paragraph_index: Int64
) -> Bool:
    var expected = StringSlice(
        "a07e8d5167e454f7637eb0e35230a84987c537ae98d6f795d246338b652810f1"
    ).unsafe_ptr()
    for offset in range(Int64(32)):
        var byte = digests[unsafe_offset=paragraph_index * 32 + offset]
        if (
            optional_tool_hex_digit(byte >> 4) != expected[unsafe_offset=offset * 2]
            or optional_tool_hex_digit(byte & 15) != expected[unsafe_offset=offset * 2 + 1]
        ):
            return False
    return True


@export("prodex_optional_tool_legacy_instruction_paragraphs_v1")
def prodex_optional_tool_legacy_instruction_paragraphs_v1(
    abi_version: Int64,
    digests_address: UInt,
    count: Int64,
    indices_address: UInt,
    capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_LEGACY_INSTRUCTIONS_ABI:
        return 4
    if (
        count < 0 or count > OPTIONAL_TOOL_LEGACY_INSTRUCTIONS_MAX_PARAGRAPHS
        or capacity < 0
        or (count > 0 and (
            digests_address == 0
            or indices_address == 0 or indices_address % 8 != 0
        ))
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    if capacity < count:
        return 3
    var digests = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(digests_address)
    )
    var indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(indices_address)
    )
    var selected: Int64 = 0
    for index in range(count):
        if not optional_tool_legacy_digest_matches(digests, index):
            indices[unsafe_offset=selected] = index
            selected += 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = selected
    return 0

# AGENTS.md / override.md marked-block stripping. The byte-level UTF-8
# operation matches Rust's repeated find(begin), find(end after begin),
# replace_range(..., "") and final unmatched-end detection. Original text
# is cloned into caller-owned output, then edits are applied in place.
comptime OPTIONAL_TOOL_MARKED_BLOCK_ABI_VERSION: Int64 = 1
comptime OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES: Int64 = 1_048_576


def optional_tool_marked_block_find(
    value: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    length: Int64,
    pattern: Pointer[mut=False, UInt8, ImmUntrackedOrigin],
    pattern_length: Int64,
    offset: Int64,
) -> Int64:
    if offset > length or pattern_length > length - offset:
        return -1
    var position = offset
    while position <= length - pattern_length:
        var matched = True
        for index in range(pattern_length):
            if value[unsafe_offset=position + index] != pattern[unsafe_offset=index]:
                matched = False
                break
        if matched:
            return position
        position += 1
    return -1


@export("prodex_optional_tool_marked_block_strip_v1")
def prodex_optional_tool_marked_block_strip_v1(
    abi_version: Int64,
    text_address: UInt,
    text_length: Int64,
    begin_address: UInt,
    begin_length: Int64,
    end_address: UInt,
    end_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_MARKED_BLOCK_ABI_VERSION:
        return 4
    if (
        text_length < 0 or text_length > OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES
        or begin_length <= 0 or begin_length > OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES
        or end_length <= 0 or end_length > OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES
        or output_capacity < text_length
        or (text_length > 0 and (text_address == 0 or output_address == 0))
        or begin_address == 0 or end_address == 0
        or written_address == 0 or written_address % 8 != 0
    ):
        return 1
    var text = ProdexRichStringView(text_address, UInt(text_length))
    var begin = ProdexRichStringView(begin_address, UInt(begin_length))
    var end = ProdexRichStringView(end_address, UInt(end_length))
    if (
        not rich_view_valid(text, OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES)
        or not rich_view_valid(begin, OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES)
        or not rich_view_valid(end, OPTIONAL_TOOL_MARKED_BLOCK_MAX_BYTES)
    ):
        return 1

    var source = rich_view_ptr(text)
    var begin_bytes = rich_view_ptr(begin)
    var end_bytes = rich_view_ptr(end)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var length = text_length
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]

    while True:
        var start = optional_tool_marked_block_find(
            output, length, begin_bytes, begin_length, 0
        )
        if start == -1:
            break
        var end_start = optional_tool_marked_block_find(
            output, length, end_bytes, end_length, start + begin_length
        )
        if end_start == -1:
            return 2
        var stop = end_start + end_length
        var removed = stop - start
        for index in range(stop, length):
            output[unsafe_offset=index - removed] = output[unsafe_offset=index]
        length -= removed

    # An unmatched trailing end marker is an error even without a begin.
    if optional_tool_marked_block_find(
        output, length, end_bytes, end_length, 0
    ) >= 0:
        return 3
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )[] = length
    return 0

# Canonical managed AGENTS reference update: full Rust str::lines behavior,
# Unicode trim and exact reference first-occurrence detection are in Mojo.
# The host constructs the @path reference, reads/writes files only.
comptime OPTIONAL_TOOL_AGENTS_REFERENCE_ABI: Int64 = 1
comptime OPTIONAL_TOOL_AGENTS_REFERENCE_MAX_BYTES: Int64 = 1_048_576


@export("prodex_optional_tool_agents_reference_update_v1")
def prodex_optional_tool_agents_reference_update_v1(
    abi_version: Int64,
    text_address: UInt,
    text_length: Int64,
    reference_address: UInt,
    reference_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_AGENTS_REFERENCE_ABI:
        return 4
    if (
        text_length < 0 or text_length > OPTIONAL_TOOL_AGENTS_REFERENCE_MAX_BYTES
        or reference_length <= 0 or reference_length > OPTIONAL_TOOL_AGENTS_REFERENCE_MAX_BYTES
        or (text_length > 0 and text_address == 0)
        or reference_address == 0
        or output_capacity < text_length + reference_length + 3
        or output_address == 0 or result_address == 0 or result_address % 8 != 0
    ):
        return 1
    var contents = ProdexRichStringView(text_address, UInt(text_length))
    var reference = ProdexRichStringView(reference_address, UInt(reference_length))
    if (
        not rich_view_valid(contents, OPTIONAL_TOOL_AGENTS_REFERENCE_MAX_BYTES)
        or not rich_view_valid(reference, OPTIONAL_TOOL_AGENTS_REFERENCE_MAX_BYTES)
    ):
        return 1

    var source = rich_view_ptr(contents)
    var reference_bytes = rich_view_ptr(reference)
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var source_pos: Int64 = 0
    var written: Int64 = 0
    var kept_count: Int64 = 0
    var reference_seen = False
    # Rust str::lines splits on LF, omits the final empty tail and removes
    # optional CR at the end of each line.
    while source_pos < text_length:
        var line_end = source_pos
        while line_end < text_length and source[unsafe_offset=line_end] != 10:
            line_end += 1
        var next_pos = line_end + 1
        if line_end > source_pos and source[unsafe_offset=line_end - 1] == 13:
            line_end -= 1

        var line = ProdexRichStringView(
            text_address + UInt(source_pos), UInt(line_end - source_pos)
        )
        var bounds = rich_trim_bounds(line)
        var normalized = ProdexRichStringView(
            text_address + UInt(source_pos + bounds[0]),
            UInt(bounds[1] - bounds[0]),
        )
        if optional_tool_views_equal(normalized, reference):
            reference_seen = True
        else:
            if kept_count > 0:
                output[unsafe_offset=written] = 10
                written += 1
            for index in range(source_pos, line_end):
                output[unsafe_offset=written] = source[unsafe_offset=index]
                written += 1
            kept_count += 1
        source_pos = next_pos

    var trimmed = rich_trim_bounds(
        ProdexRichStringView(output_address, UInt(written))
    )
    written = trimmed[1] if trimmed[1] > trimmed[0] else Int64(0)
    if written > 0:
        output[unsafe_offset=written] = 10
        output[unsafe_offset=written + 1] = 10
        written += 2
    for index in range(reference_length):
        output[unsafe_offset=written] = reference_bytes[unsafe_offset=index]
        written += 1
    output[unsafe_offset=written] = 10
    written += 1

    # Preserve Rust's reference_seen && updated == contents idempotence:
    # absent original reference causes an explicit write even if equal.
    var should_write = not reference_seen or written != text_length
    if reference_seen and written == text_length:
        for index in range(written):
            if output[unsafe_offset=index] != source[unsafe_offset=index]:
                should_write = True
                break
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = written
    result[unsafe_offset=1] = Int64(should_write)
    return 0

# Managed AGENTS block text projection: the host supplies the prior contents,
# compiled-stripped contents, block delimiters and user block. Mojo chooses
# Unicode trim rules, paragraph separators, removal/no-op and exact output.
comptime OPTIONAL_TOOL_AGENTS_BLOCK_RENDER_ABI: Int64 = 1
comptime OPTIONAL_TOOL_AGENTS_BLOCK_MAX_TEXT: Int64 = 1_048_576
comptime OPTIONAL_TOOL_AGENTS_BLOCK_VIEW_COUNT: Int64 = 5


@export("prodex_optional_tool_agents_block_render_v1")
def prodex_optional_tool_agents_block_render_v1(
    abi_version: Int64,
    operation: Int64,
    views_address: UInt,
    view_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != OPTIONAL_TOOL_AGENTS_BLOCK_RENDER_ABI:
        return 4
    if (
        operation != 0 and operation != 1
        or view_count != OPTIONAL_TOOL_AGENTS_BLOCK_VIEW_COUNT
        or views_address == 0 or views_address % 8 != 0
        or output_address == 0 or output_capacity < 0
        or result_address == 0 or result_address % 8 != 0
    ):
        return 1
    var views = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(views_address)
    )
    for index in range(OPTIONAL_TOOL_AGENTS_BLOCK_VIEW_COUNT):
        if not rich_view_valid(
            views[unsafe_offset=index], OPTIONAL_TOOL_AGENTS_BLOCK_MAX_TEXT
        ):
            return 1
    var original = views[unsafe_offset=0].copy()
    var cleaned = views[unsafe_offset=1].copy()
    var begin = views[unsafe_offset=2].copy()
    var end = views[unsafe_offset=3].copy()
    var block = views[unsafe_offset=4].copy()
    var required = Int64(cleaned.len)
    if operation == 0:
        required += Int64(begin.len) + Int64(end.len) + Int64(block.len) + 8
    if required > output_capacity:
        return 3
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    if operation == 1 and optional_tool_views_equal(original, cleaned):
        result[unsafe_offset=0] = 0
        result[unsafe_offset=1] = 0
        return 0

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written: Int64 = 0
    var bounds = rich_trim_bounds(cleaned)
    var content_end = bounds[1] if bounds[1] > bounds[0] else Int64(0)
    if operation == 1:
        var source = rich_view_ptr(cleaned)
        for index in range(content_end):
            output[unsafe_offset=written] = source[unsafe_offset=index]
            written += 1
    else:
        if content_end > 0:
            var source = rich_view_ptr(cleaned)
            for index in range(content_end):
                output[unsafe_offset=written] = source[unsafe_offset=index]
                written += 1
            output[unsafe_offset=written] = 10
            output[unsafe_offset=written + 1] = 10
            written += 2
        var begin_bytes = rich_view_ptr(begin)
        for index in range(Int64(begin.len)):
            output[unsafe_offset=written] = begin_bytes[unsafe_offset=index]
            written += 1
        output[unsafe_offset=written] = 10
        written += 1
        var body = rich_trim_bounds(block)
        var block_bytes = rich_view_ptr(block)
        for index in range(body[0], body[1]):
            output[unsafe_offset=written] = block_bytes[unsafe_offset=index]
            written += 1
        output[unsafe_offset=written] = 10
        written += 1
        var end_bytes = rich_view_ptr(end)
        for index in range(Int64(end.len)):
            output[unsafe_offset=written] = end_bytes[unsafe_offset=index]
            written += 1
        output[unsafe_offset=written] = 10
        written += 1
    result[unsafe_offset=0] = written
    result[unsafe_offset=1] = 1
    return 0
