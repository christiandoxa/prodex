from std.memory import Pointer

from rich_text import rich_view_valid
from rich_types import ProdexRichStringView


comptime SESSION_REPAIR_ABI_VERSION: Int64 = 1
comptime SESSION_REPAIR_OK: Int64 = 0
comptime SESSION_REPAIR_INVALID: Int64 = 1
comptime SESSION_REPAIR_HEADER_WORDS: Int64 = 5
comptime SESSION_REPAIR_INT64_MAX: Int64 = 9223372036854775807


@fieldwise_init
struct SessionRepairLine(Copyable):
    """One Rust-observed session line fact consumed by the repair planner."""

    var text: ProdexRichStringView
    var blank: Int64
    var valid_json: Int64
    var starts_resume_metadata: Int64
    var matches_selector: Int64
    var starts_codex_metadata: Int64


def session_repair_valid_flag(value: Int64) -> Bool:
    return value == 0 or value == 1


def session_repair_line_valid(line: SessionRepairLine) -> Bool:
    return (
        session_repair_valid_flag(line.blank)
        and session_repair_valid_flag(line.valid_json)
        and session_repair_valid_flag(line.starts_resume_metadata)
        and session_repair_valid_flag(line.matches_selector)
        and session_repair_valid_flag(line.starts_codex_metadata)
        and rich_view_valid(line.text, 0x7FFFFFFFFFFFFFFF)
    )


@export("prodex_session_repair_plan_v1")
def prodex_session_repair_plan_v1(
    abi_version: Int64,
    selector_address: UInt,
    selector_length: Int64,
    lines_address: UInt,
    lines_count: Int64,
    synthesize_missing: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPAIR_ABI_VERSION
        or selector_length <= 0
        or selector_address == 0
        or lines_count < 0
        or lines_count > SESSION_REPAIR_INT64_MAX - SESSION_REPAIR_HEADER_WORDS
        or (lines_count > 0 and lines_address == 0)
        or (synthesize_missing != 0 and synthesize_missing != 1)
        or output_address == 0
        or output_capacity < SESSION_REPAIR_HEADER_WORDS + lines_count
    ):
        return SESSION_REPAIR_INVALID

    var selector = ProdexRichStringView(selector_address, UInt(selector_length))
    if not rich_view_valid(selector, 0x7FFFFFFFFFFFFFFF):
        return SESSION_REPAIR_INVALID

    var lines = Pointer[mut=False, SessionRepairLine, ImmUntrackedOrigin](
        unsafe_from_address=Int(lines_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(SESSION_REPAIR_HEADER_WORDS + lines_count):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = -1
    output[unsafe_offset=3] = 0
    output[unsafe_offset=4] = 0

    var first_content: Int64 = -1
    var first_matching: Int64 = 0
    var first_codex: Int64 = 0
    var unreadable: Int64 = 0
    var metadata_index: Int64 = -1

    for index in range(lines_count):
        var line = lines[unsafe_offset=index].copy()
        if not session_repair_line_valid(line):
            return SESSION_REPAIR_INVALID
        if line.blank == 0 and first_content < 0:
            first_content = index
            first_matching = Int64(
                line.starts_resume_metadata == 1 and line.matches_selector == 1
            )
            first_codex = Int64(
                first_matching == 1 and line.starts_codex_metadata == 1
            )
        if line.blank == 0 and line.valid_json == 0:
            unreadable = 1
        if (
            index > first_content
            and metadata_index < 0
            and line.starts_codex_metadata == 1
            and line.matches_selector == 1
        ):
            metadata_index = index
        output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 1

    output[unsafe_offset=2] = first_content
    output[unsafe_offset=4] = first_matching

    # A complete, clean rollout already has the only metadata prefix it needs.
    if first_codex == 1 and unreadable == 0:
        for index in range(lines_count):
            output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 0
        return SESSION_REPAIR_OK
    if first_content < 0:
        for index in range(lines_count):
            output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 0
        return SESSION_REPAIR_OK

    var selected = first_content
    var use_synthetic: Int64 = 0
    if first_codex == 0:
        if metadata_index >= 0:
            selected = metadata_index
        elif synthesize_missing == 1:
            selected = -1
            use_synthetic = 1
        else:
            for index in range(lines_count):
                output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 0
            return SESSION_REPAIR_OK

    output[unsafe_offset=0] = 1
    output[unsafe_offset=1] = selected
    output[unsafe_offset=3] = use_synthetic
    for index in range(lines_count):
        var line = lines[unsafe_offset=index].copy()
        if (
            line.blank == 1
            or line.valid_json == 0
            or line.starts_resume_metadata == 1 and line.matches_selector == 1
        ):
            output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 0
        else:
            output[unsafe_offset=SESSION_REPAIR_HEADER_WORDS + index] = 1
    return SESSION_REPAIR_OK
