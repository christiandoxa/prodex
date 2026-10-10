from rich_text import (
    rich_codepoint,
    rich_codepoint_width,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView

comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_MODEL_REGISTRY_INVALID: Int64 = -2
comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_MISMATCH: Int64 = -4
comptime SMART_CONTEXT_MODEL_REGISTRY_MAX_BYTES: Int64 = 128
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_INPUT_BYTES: Int64 = 4 * 1024
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_MAX_OUTPUT_BYTES: Int64 = 128
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_OK: Int64 = 0
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_NO_MATCH: Int64 = 1
comptime SMART_CONTEXT_MODEL_NAME_NORMALIZE_CAPACITY: Int64 = 3


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
