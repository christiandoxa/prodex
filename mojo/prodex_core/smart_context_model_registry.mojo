from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_VERSION: Int64 = 1
comptime SMART_CONTEXT_MODEL_REGISTRY_INVALID: Int64 = -2
comptime SMART_CONTEXT_MODEL_REGISTRY_ABI_MISMATCH: Int64 = -4
comptime SMART_CONTEXT_MODEL_REGISTRY_MAX_BYTES: Int64 = 128


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
