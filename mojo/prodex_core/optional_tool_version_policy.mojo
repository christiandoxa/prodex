# Exact tool versions, shared-daemon coordination, and stable directory ranking.
# Filesystem inspection and child-process acquisition remain with Rust.
from std.memory import Pointer
from rich_types import ProdexRichStringView
from rich_text import rich_view_valid, rich_view_prefix, rich_view_matches_literal
from update_notice_policy import update_notice_parse_exact_release_version, update_notice_release_version_compare

comptime OPTIONAL_TOOL_VERSION_MAX_BYTES: Int64 = 1_048_576


def optional_tool_exact_version_at_least(
    candidate: ProdexRichStringView, minimum: ProdexRichStringView,
) -> Bool:
    return (
        update_notice_parse_exact_release_version(candidate).valid
        and update_notice_parse_exact_release_version(minimum).valid
        and update_notice_release_version_compare(candidate, minimum, True) >= 0
    )


@export("prodex_optional_tool_version_policy_v1")
def prodex_optional_tool_version_policy_v1(
    abi_version: Int64, operation: Int64, success: Int64,
    input_address: UInt, input_length: Int64,
    minimum_address: UInt, minimum_length: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if (
        operation < 0 or operation > 3 or success < 0 or success > 1
        or operation != 2 and success != 0
        or operation == 2 and minimum_length != 0
        or input_length < 0 or minimum_length < 0
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var text = ProdexRichStringView(input_address, UInt(input_length))
    var minimum = ProdexRichStringView(minimum_address, UInt(minimum_length))
    if not rich_view_valid(text, OPTIONAL_TOOL_VERSION_MAX_BYTES) or not rich_view_valid(minimum, OPTIONAL_TOOL_VERSION_MAX_BYTES):
        return 1
    var accepted = False
    if operation == 0:
        if not update_notice_parse_exact_release_version(minimum).valid:
            return 1
        accepted = optional_tool_exact_version_at_least(text, minimum)
    elif operation == 1:
        if rich_view_prefix["codebase-memory-mcp "](text, False):
            var prefix_length = UInt(StringSlice("codebase-memory-mcp ").byte_length())
            var version = ProdexRichStringView(text.ptr + prefix_length, text.len - prefix_length)
            accepted = rich_view_matches_literal["dev"](version, False) or optional_tool_exact_version_at_least(version, minimum)
    elif operation == 2:
        accepted = (success == 1 and rich_view_prefix["daemon: active"](text, False)) or rich_view_prefix["daemon: not running"](text, False)
    else:
        var candidate = update_notice_parse_exact_release_version(text)
        var result: Int64 = 0
        if candidate.valid:
            if not update_notice_parse_exact_release_version(minimum).valid:
                return 1
            result = Int64(2) if not candidate.has_pre and optional_tool_exact_version_at_least(text, minimum) else Int64(1)
        Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))[] = result
        return 0
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))[] = Int64(accepted)
    return 0


@export("prodex_optional_tool_newest_stable_version_v1")
def prodex_optional_tool_newest_stable_version_v1(
    abi_version: Int64, names_address: UInt, count: Int64, output_address: UInt,
) abi("C") -> Int64:
    if abi_version != 1:
        return 4
    if count < 0 or count > 1_048_576 or (count > 0 and names_address == 0) or names_address % 8 != 0 or output_address == 0 or output_address % 8 != 0:
        return 1
    var names = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](unsafe_from_address=Int(names_address))
    var total: UInt = 0
    for index in range(count):
        var name = names[unsafe_offset=index].copy()
        if not rich_view_valid(name, OPTIONAL_TOOL_VERSION_MAX_BYTES):
            return 1
        total += name.len
        if total > 67_108_864:
            return 1
    var selected: Int64 = -1
    for index in range(count):
        var candidate = names[unsafe_offset=index].copy()
        var version = update_notice_parse_exact_release_version(candidate)
        if not version.valid or version.has_pre:
            continue
        # Preserve discovery order when equal versions refer to different entries.
        if selected < 0 or update_notice_release_version_compare(candidate, names[unsafe_offset=selected], True) > 0:
            selected = index
    Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(output_address))[] = selected
    return 0
