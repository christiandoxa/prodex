from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_unicode_space,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView
from std.memory import Pointer

comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_MODEL_REGISTRY_INVALID: Int64 = -2
comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_MISMATCH: Int64 = -4
comptime SMART_CONTEXT_MODEL_REGISTRY_MAX_BYTES: Int64 = 128
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_INPUT_BYTES: Int64 = 4 * 1024
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_OUTPUT_BYTES: Int64 = 128
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_OK: Int64 = 0
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_NO_MATCH: Int64 = 1
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_CAPACITY: Int64 = 3
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_OK: Int64 = 0
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH: Int64 = 1
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_INVALID: Int64 = -2
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_ABI_MISMATCH: Int64 = -4
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_CAPACITY: Int64 = 3
comptime SMART_CONTEXT_MODEL_PREFIX_SCAN_MAX_BYTES: Int64 = 4 * 1024


def model_registry_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def model_registry_equals[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    if view.len != UInt(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(Int64(view.len)):
        if model_registry_ascii_lower(source[unsafe_offset=index]) != wanted[unsafe_offset=index]:
            return False
    return True


def model_registry_prefix[literal: StaticString](
    view: ProdexRichStringView,
) -> Bool:
    if view.len < UInt(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var wanted = literal.unsafe_ptr()
    for index in range(Int64(literal.byte_length())):
        if model_registry_ascii_lower(source[unsafe_offset=index]) != wanted[unsafe_offset=index]:
            return False
    return True


def model_registry_is_control(codepoint: Int64) -> Bool:
    return codepoint <= 0x1F or codepoint >= 0x7F and codepoint <= 0x9F


def model_registry_literal_at[literal: StaticString](
    address: UInt, start: Int64, end: Int64
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if start < 0 or start + literal_length > end:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var wanted = literal.unsafe_ptr()
    for index in range(literal_length):
        if source[unsafe_offset=start + index] != wanted[unsafe_offset=index]:
            return False
    return True


def model_registry_model_prefix_start(address: UInt, length: Int64) -> Int64:
    var key_length = Int64(StringSlice("\"model\"").byte_length())
    var key_start: Int64 = -1
    var cursor: Int64 = 0
    while cursor + key_length <= length:
        if model_registry_literal_at["\"model\""](address, cursor, length):
            key_start = cursor
            break
        cursor += 1
    if key_start < 0:
        return -1

    cursor = key_start + key_length
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    while cursor < length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    if cursor >= length or source[unsafe_offset=cursor] != 58:
        return -1
    cursor += 1
    while cursor < length:
        var width = rich_codepoint_width(source[unsafe_offset=cursor])
        if not rich_unicode_space(rich_codepoint(source, cursor, width)):
            break
        cursor += width
    if cursor >= length or source[unsafe_offset=cursor] != 34:
        return -1
    return cursor + 1


def model_registry_copy_json_model_prefix(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, UInt8, MutUntrackedOrigin],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, MutUntrackedOrigin],
) -> Int64:
    var cursor = model_registry_model_prefix_start(address, length)
    if cursor < 0:
        return SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var escaped = False
    var model_length: Int64 = 0
    while cursor < length:
        var value = source[unsafe_offset=cursor]
        if escaped:
            var width = rich_codepoint_width(value)
            if model_length + width > SMART_CONTEXT_MODEL_PREFIX_SCAN_MAX_BYTES:
                return SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH
            if model_length + width > output_capacity:
                return SMART_CONTEXT_MODEL_PREFIX_SCAN_CAPACITY
            for offset in range(width):
                output[unsafe_offset=model_length + offset] = source[
                    unsafe_offset=cursor + offset
                ]
            model_length += width
            cursor += width
            escaped = False
            continue
        if value == 92:
            escaped = True
            cursor += 1
            continue
        if value == 34:
            written[] = model_length
            return SMART_CONTEXT_MODEL_PREFIX_SCAN_OK

        var width = rich_codepoint_width(value)
        if model_registry_is_control(rich_codepoint(source, cursor, width)):
            return SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH
        if model_length + width > SMART_CONTEXT_MODEL_PREFIX_SCAN_MAX_BYTES:
            return SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH
        if model_length + width > output_capacity:
            return SMART_CONTEXT_MODEL_PREFIX_SCAN_CAPACITY
        for offset in range(width):
            output[unsafe_offset=model_length + offset] = source[
                unsafe_offset=cursor + offset
            ]
        model_length += width
        cursor += width
    return SMART_CONTEXT_MODEL_PREFIX_SCAN_NO_MATCH


@export("prodex_smart_context_model_name_normalize_v1")
def prodex_smart_context_model_name_normalize_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION:
        return SMART_CONTEXT_MODEL_REGISTRY_ABI_MISMATCH
    if (
        address == 0
        or output_address == 0
        or written_address == 0
        or length < 0
        or output_capacity < 1
    ):
        return SMART_CONTEXT_MODEL_REGISTRY_INVALID
    if length > SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_INPUT_BYTES:
        return SMART_CONTEXT_MODEL_NAME_NORMALIZE_NO_MATCH

    var input = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(input, SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_INPUT_BYTES):
        return SMART_CONTEXT_MODEL_REGISTRY_INVALID
    var bounds = rich_trim_bounds(input)
    var normalized_length = bounds[1] - bounds[0]
    if (
        normalized_length <= 0
        or normalized_length > SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_OUTPUT_BYTES
    ):
        return SMART_CONTEXT_MODEL_NAME_NORMALIZE_NO_MATCH

    var source = rich_view_ptr(input)
    var index = bounds[0]
    while index < bounds[1]:
        var width = rich_codepoint_width(source[unsafe_offset=index])
        if model_registry_is_control(rich_codepoint(source, index, width)):
            return SMART_CONTEXT_MODEL_NAME_NORMALIZE_NO_MATCH
        index += width

    if normalized_length > output_capacity:
        return SMART_CONTEXT_MODEL_NAME_NORMALIZE_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for offset in range(normalized_length):
        output[unsafe_offset=offset] = source[unsafe_offset=bounds[0] + offset]
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = normalized_length
    return SMART_CONTEXT_MODEL_NAME_NORMALIZE_OK


@export("prodex_smart_context_model_prefix_scan_v1")
def prodex_smart_context_model_prefix_scan_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION:
        return SMART_CONTEXT_MODEL_PREFIX_SCAN_ABI_MISMATCH
    if (
        address == 0
        or output_address == 0
        or written_address == 0
        or length <= 0
        or length > SMART_CONTEXT_MODEL_PREFIX_SCAN_MAX_BYTES
        or output_capacity <= 0
    ):
        return SMART_CONTEXT_MODEL_PREFIX_SCAN_INVALID
    var input = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(input, SMART_CONTEXT_MODEL_PREFIX_SCAN_MAX_BYTES):
        return SMART_CONTEXT_MODEL_PREFIX_SCAN_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    return model_registry_copy_json_model_prefix(
        address, length, output, output_capacity, written
    )


def model_registry_context_window(view: ProdexRichStringView) -> Int64:
    if model_registry_equals["unsloth/qwen3.5-35b-a3b"](view):
        return 16_384
    if (
        model_registry_equals["gpt-5.6-luna"](view)
        or model_registry_equals["gpt-6-astra"](view)
        or model_registry_equals["gpt-6.1-sol"](view)
        or model_registry_equals["gpt-6-sol"](view)
        or model_registry_equals["gpt-6-luna"](view)
    ):
        return 872_000
    if (
        model_registry_equals["gpt-5.3-codex"](view)
        or model_registry_equals["gpt-5.4"](view)
        or model_registry_equals["gpt-5.5"](view)
    ):
        return 1_000_000
    if model_registry_equals["gpt-5.1-codex"](view):
        return 200_000
    if model_registry_equals["claude-sonnet-4-6"](view):
        return 1_000_000
    if model_registry_equals["deepseek-v4-pro"](view):
        return 128_000
    if model_registry_prefix["claude-"](view):
        return 200_000
    if model_registry_prefix["gemini-"](view):
        return 1_048_576
    if model_registry_prefix["deepseek-"](view):
        return 128_000
    return 0


@export("prodex_smart_context_model_context_window_v1")
def prodex_smart_context_model_context_window_v1(
    abi_version: Int64,
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if abi_version != SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION:
        return SMART_CONTEXT_MODEL_REGISTRY_ABI_MISMATCH
    if length < 0 or length > SMART_CONTEXT_MODEL_REGISTRY_MAX_BYTES:
        return SMART_CONTEXT_MODEL_REGISTRY_INVALID
    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, SMART_CONTEXT_MODEL_REGISTRY_MAX_BYTES):
        return SMART_CONTEXT_MODEL_REGISTRY_INVALID
    return model_registry_context_window(view)
