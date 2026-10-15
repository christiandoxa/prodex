from std.collections import Array
from std.memory import Pointer

from json_sink import JsonSink, js_literal, js_string
from session_report import (
    session_meta_repair_fields,
    session_raw_present,
    session_raw_root,
    session_report_raw_metadata,
)
from rich_text import rich_view_valid
from rich_types import ProdexRichStringView


comptime SESSION_REPAIR_ABI_VERSION: Int64 = 1
comptime SESSION_REPAIR_OK: Int64 = 0
comptime SESSION_REPAIR_INVALID: Int64 = 1
comptime SESSION_REPAIR_CAPACITY: Int64 = 3
comptime SESSION_REPAIR_HEADER_WORDS: Int64 = 5
comptime SESSION_REPAIR_INT64_MAX: Int64 = 9223372036854775807
comptime SESSION_SYNTHETIC_OUTPUT_WORDS: Int64 = 13
comptime SESSION_SYNTHETIC_SOURCE_NONE: Int64 = 0
comptime SESSION_SYNTHETIC_SOURCE_LINE: Int64 = 1
comptime SESSION_SYNTHETIC_SOURCE_PATH: Int64 = 2
comptime SESSION_SYNTHETIC_SOURCE_SELECTOR: Int64 = 3


@fieldwise_init
struct SessionSyntheticMetadataInput(Copyable):
    """Decoded values supplied by the host for synthetic metadata encoding."""

    var session_id: ProdexRichStringView
    var timestamp: ProdexRichStringView
    var cwd: ProdexRichStringView
    var model_provider: ProdexRichStringView
    var model_provider_present: Int64
    var cli_version: ProdexRichStringView


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


def session_repair_write_span(
    output: Pointer[mut=True, Int64, _],
    offset: Int64,
    line_index: Int64,
    start: Int64,
    end: Int64,
):
    output[unsafe_offset=offset] = line_index
    output[unsafe_offset=offset + 1] = start
    output[unsafe_offset=offset + 2] = end


def session_repair_scan_synthetic_fields(
    line: SessionRepairLine,
    line_index: Int64,
    output: Pointer[mut=True, Int64, _],
):
    var root = session_raw_root(line.text)
    if not session_raw_present(root):
        return
    var metadata = Array[Int64, 28](fill=-1)
    session_report_raw_metadata(line.text, root, metadata.unsafe_ptr())
    session_meta_repair_fields(line.text, root, metadata.unsafe_ptr())

    if (
        output[unsafe_offset=1] < 0
        and line.matches_selector == 1
        and metadata[1] >= 0
        and metadata[2] > metadata[1]
    ):
        output[unsafe_offset=0] = SESSION_SYNTHETIC_SOURCE_LINE
        session_repair_write_span(output, 1, line_index, metadata[1], metadata[2])

    if output[unsafe_offset=4] < 0 and metadata[22] >= 0 and metadata[23] > metadata[22]:
        session_repair_write_span(output, 4, line_index, metadata[22], metadata[23])
    if output[unsafe_offset=7] < 0 and metadata[24] >= 0 and metadata[25] > metadata[24]:
        session_repair_write_span(output, 7, line_index, metadata[24], metadata[25])
    if output[unsafe_offset=10] < 0 and metadata[26] >= 0 and metadata[27] > metadata[26]:
        session_repair_write_span(output, 10, line_index, metadata[26], metadata[27])


@export("prodex_session_repair_synthetic_plan_v1")
def prodex_session_repair_synthetic_plan_v1(
    abi_version: Int64,
    selector_address: UInt,
    selector_length: Int64,
    lines_address: UInt,
    lines_count: Int64,
    path_id_present: Int64,
    selector_is_full: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPAIR_ABI_VERSION
        or selector_length <= 0
        or selector_address == 0
        or lines_count < 0
        or lines_count > SESSION_REPAIR_INT64_MAX
        or (lines_count > 0 and lines_address == 0)
        or (path_id_present != 0 and path_id_present != 1)
        or (selector_is_full != 0 and selector_is_full != 1)
        or output_address == 0
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
    for index in range(SESSION_SYNTHETIC_OUTPUT_WORDS):
        output[unsafe_offset=index] = -1
    output[unsafe_offset=0] = SESSION_SYNTHETIC_SOURCE_NONE

    for index in range(lines_count):
        var line = lines[unsafe_offset=index].copy()
        if not session_repair_line_valid(line):
            return SESSION_REPAIR_INVALID
        session_repair_scan_synthetic_fields(line, index, output)

    if output[unsafe_offset=0] == SESSION_SYNTHETIC_SOURCE_NONE:
        if path_id_present == 1:
            output[unsafe_offset=0] = SESSION_SYNTHETIC_SOURCE_PATH
        elif selector_is_full == 1:
            output[unsafe_offset=0] = SESSION_SYNTHETIC_SOURCE_SELECTOR
    return SESSION_REPAIR_OK


def session_synthetic_input_valid(input: SessionSyntheticMetadataInput) -> Bool:
    return (
        (input.model_provider_present == 0 or input.model_provider_present == 1)
        and rich_view_valid(input.session_id, 0x7FFFFFFFFFFFFFFF)
        and rich_view_valid(input.timestamp, 0x7FFFFFFFFFFFFFFF)
        and rich_view_valid(input.cwd, 0x7FFFFFFFFFFFFFFF)
        and rich_view_valid(input.model_provider, 0x7FFFFFFFFFFFFFFF)
        and rich_view_valid(input.cli_version, 0x7FFFFFFFFFFFFFFF)
    )


def session_synthetic_write_json(
    input: SessionSyntheticMetadataInput,
    sink: Pointer[mut=True, JsonSink, _],
):
    # serde_json::Map uses sorted keys in the Rust compatibility path.
    js_literal(sink, StringSlice("{\"payload\":{\"cli_version\":"))
    js_string(sink, input.cli_version)
    js_literal(sink, StringSlice(",\"cwd\":"))
    js_string(sink, input.cwd)
    js_literal(sink, StringSlice(",\"id\":"))
    js_string(sink, input.session_id)
    if input.model_provider_present == 1:
        js_literal(sink, StringSlice(",\"model_provider\":"))
        js_string(sink, input.model_provider)
    js_literal(sink, StringSlice(",\"originator\":\"prodex-repair\",\"session_id\":"))
    js_string(sink, input.session_id)
    js_literal(sink, StringSlice(",\"source\":\"cli\",\"timestamp\":"))
    js_string(sink, input.timestamp)
    js_literal(sink, StringSlice("},\"timestamp\":"))
    js_string(sink, input.timestamp)
    js_literal(sink, StringSlice(",\"type\":\"session_meta\"}"))


@export("prodex_session_repair_synthetic_json_v1")
def prodex_session_repair_synthetic_json_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if (
        abi_version != SESSION_REPAIR_ABI_VERSION
        or input_address == 0
        or output_capacity < 0
        or (output_capacity > 0 and output_address == 0)
        or written_address == 0
    ):
        return SESSION_REPAIR_INVALID

    var input = Pointer[mut=False, SessionSyntheticMetadataInput, ImmUntrackedOrigin](
        unsafe_from_address=Int(input_address)
    )
    var input_value = input[].copy()
    if not session_synthetic_input_valid(input_value):
        return SESSION_REPAIR_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var sink = JsonSink(
        output,
        output_capacity,
        0,
        output_capacity == 0,
        False,
    )
    session_synthetic_write_json(input_value, Pointer(to=sink))
    if sink.failed:
        return SESSION_REPAIR_CAPACITY
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[unsafe_offset=0] = sink.written
    return SESSION_REPAIR_OK


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
