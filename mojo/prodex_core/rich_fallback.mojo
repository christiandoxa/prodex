from std.memory import Pointer

from rich_text import (
    rich_copy_range,
    rich_hash_slice,
    rich_required_hash_capacity,
    rich_slice_equal_folded,
    rich_trim_bounds,
    rich_view_matches_literal,
    rich_view_prefix,
    rich_view_valid,
)
from rich_types import (
    ProdexRichFallbackRecord,
    ProdexRichFallbackResult,
    ProdexRichSlice,
    ProdexRichStringView,
    rich_view_ptr,
)


comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime RICH_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime RICH_MAX_FALLBACK_MODELS: Int64 = 2_048
comptime RICH_MAX_FALLBACK_INPUTS: Int64 = 256
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2
comptime RICH_STATUS_CAPACITY: Int64 = 3
comptime RICH_STATUS_ABI: Int64 = 4


def fallback_add_bytes(
    ptr: Pointer[mut=False, UInt8, _],
    length: Int64,
    source_kind: Int64,
    input_index: Int64,
    output_records: Pointer[mut=True, ProdexRichFallbackRecord, _],
    record_count: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Bool:
    if length == 0 or record_count[] >= hash_capacity / 2:
        return length == 0
    var slice = rich_copy_range(ptr, 0, length, output, output_capacity, written, False)
    if slice.len < 0:
        return False
    var hash = rich_hash_slice(output, slice)
    var slot = Int64(hash % UInt64(hash_capacity))
    for _ in range(hash_capacity):
        var existing = hash_slots[unsafe_offset=slot]
        if existing < 0:
            hash_slots[unsafe_offset=slot] = record_count[]
            output_records[unsafe_offset=record_count[]].model = slice.copy()
            output_records[unsafe_offset=record_count[]].source_kind = source_kind
            output_records[unsafe_offset=record_count[]].input_index = input_index
            record_count[] += 1
            return True
        if rich_slice_equal_folded(output, slice, output_records[unsafe_offset=existing].model):
            written[] = slice.offset
            return True
        slot += 1
        if slot == hash_capacity:
            slot = 0
    return False


def fallback_add_literal[literal: StaticString](
    source_kind: Int64,
    output_records: Pointer[mut=True, ProdexRichFallbackRecord, _],
    record_count: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Bool:
    return fallback_add_bytes(literal.unsafe_ptr(), Int64(literal.byte_length()), source_kind, -1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)


def fallback_add_view_range(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output_records: Pointer[mut=True, ProdexRichFallbackRecord, _],
    record_count: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Bool:
    return fallback_add_bytes((rich_view_ptr(view) + start).as_imm(), end - start, 2, -1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)


def fallback_combo(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    output_records: Pointer[mut=True, ProdexRichFallbackRecord, _],
    record_count: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Bool:
    var ptr = rich_view_ptr(view)
    var component_start = start
    var index = start
    while index <= end:
        if index == end or ptr[unsafe_offset=index] == 44 or ptr[unsafe_offset=index] == 59 or ptr[unsafe_offset=index] == 124 or ptr[unsafe_offset=index] == 62:
            var component = ProdexRichStringView(view.ptr + UInt(component_start), UInt(index - component_start))
            var bounds = rich_trim_bounds(component)
            if bounds[1] > bounds[0] and not fallback_add_view_range(component, bounds[0], bounds[1], output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity):
                return False
            component_start = index + 1
        index += 1
    return True


def combo_has_component(view: ProdexRichStringView, start: Int64, end: Int64) -> Bool:
    var ptr = rich_view_ptr(view)
    var component_start = start
    var index = start
    while index <= end:
        if index == end or ptr[unsafe_offset=index] == 44 or ptr[unsafe_offset=index] == 59 or ptr[unsafe_offset=index] == 124 or ptr[unsafe_offset=index] == 62:
            var cursor = component_start
            while cursor < index and (ptr[unsafe_offset=cursor] == 32 or ptr[unsafe_offset=cursor] == 9):
                cursor += 1
            var component_end = index
            while component_end > cursor and (ptr[unsafe_offset=component_end - 1] == 32 or ptr[unsafe_offset=component_end - 1] == 9):
                component_end -= 1
            if component_end > cursor:
                return True
            component_start = index + 1
        index += 1
    return False


def fallback_add_chain(
    provider: ProdexRichStringView,
    model: ProdexRichStringView,
    output_records: Pointer[mut=True, ProdexRichFallbackRecord, _],
    record_count: Pointer[mut=True, Int64, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    hash_slots: Pointer[mut=True, Int64, _],
    hash_capacity: Int64,
) -> Bool:
    var bounds = rich_trim_bounds(model)
    var trimmed_model = ProdexRichStringView(model.ptr + UInt(bounds[0]), UInt(bounds[1] - bounds[0]))
    if bounds[1] >= bounds[0] + 6 and rich_view_prefix["combo:"](trimmed_model, False) and combo_has_component(model, bounds[0] + 6, bounds[1]):
        return fallback_combo(model, bounds[0] + 6, bounds[1], output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    var empty_alias = rich_view_matches_literal[""](trimmed_model, True)
    var auto_alias = rich_view_matches_literal["auto"](trimmed_model, True)
    var default_alias = rich_view_matches_literal["default"](trimmed_model, True)
    var is_alias = empty_alias or auto_alias or default_alias
    if rich_view_matches_literal["anthropic"](provider, True):
        if is_alias:
            return fallback_add_literal["claude-sonnet-4-6"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["claude-opus-4-8"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["claude-haiku-4-5"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["opus"](trimmed_model, True) or rich_view_matches_literal["best"](trimmed_model, True):
            return fallback_add_literal["claude-opus-4-8"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["claude-sonnet-4-6"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["sonnet"](trimmed_model, True) or rich_view_matches_literal["pro"](trimmed_model, True):
            return fallback_add_literal["claude-sonnet-4-6"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["claude-opus-4-8"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["haiku"](trimmed_model, True) or rich_view_matches_literal["flash"](trimmed_model, True):
            return fallback_add_literal["claude-haiku-4-5"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["claude-sonnet-4-6"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    elif rich_view_matches_literal["copilot"](provider, True):
        if is_alias or rich_view_matches_literal["codex"](trimmed_model, True) or rich_view_matches_literal["pro"](trimmed_model, True):
            return fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-4o"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gpt-5.5"](trimmed_model, True):
            return fallback_add_literal["gpt-5.5"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-4o"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gpt-5.4"](trimmed_model, True):
            return fallback_add_literal["gpt-5.4"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-4o"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gpt-5.3-codex"](trimmed_model, True):
            return fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-4o"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["claude"](trimmed_model, True) or rich_view_matches_literal["sonnet"](trimmed_model, True):
            return fallback_add_literal["claude-sonnet-4-6"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini"](trimmed_model, True):
            return fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.3-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gpt-5.1-codex"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    elif rich_view_matches_literal["deepseek"](provider, True):
        if empty_alias or auto_alias or rich_view_matches_literal["pro"](trimmed_model, True):
            return fallback_add_literal["deepseek-v4-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["deepseek-v4-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["flash"](trimmed_model, True):
            return fallback_add_literal["deepseek-v4-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["deepseek-v4-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    elif rich_view_matches_literal["gemini"](provider, True):
        if empty_alias or auto_alias or rich_view_matches_literal["auto-gemini-3"](trimmed_model, True):
            return fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["chat-compression-default"](trimmed_model, True):
            return fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["auto-gemini-2.5"](trimmed_model, True):
            return fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3.1-pro-preview-customtools"](trimmed_model, True):
            return fallback_add_literal["gemini-3.1-pro-preview-customtools"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3.1-pro-preview"](trimmed_model, True):
            return fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3-pro-preview"](trimmed_model, True):
            return fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3.5-flash"](trimmed_model, True):
            return fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3-flash-preview"](trimmed_model, True):
            return fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3-flash"](trimmed_model, True):
            return fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["pro"](trimmed_model, True):
            return fallback_add_literal["gemini-3-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.1-pro-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-pro"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["flash"](trimmed_model, True):
            return fallback_add_literal["gemini-3-flash-preview"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-3-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["flash-lite"](trimmed_model, True):
            return fallback_add_literal["gemini-3.1-flash-lite"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash-lite"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
        if rich_view_matches_literal["gemini-3.1-flash-lite"](trimmed_model, True):
            return fallback_add_literal["gemini-3.1-flash-lite"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash-lite"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity) and fallback_add_literal["gemini-2.5-flash"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    elif rich_view_matches_literal["kiro"](provider, True):
        if is_alias or rich_view_matches_literal["claude"](trimmed_model, True) or rich_view_matches_literal["sonnet"](trimmed_model, True):
            return fallback_add_literal["auto"](1, output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)
    if bounds[1] == bounds[0]:
        return True
    return fallback_add_view_range(model, bounds[0], bounds[1], output_records, record_count, output, output_capacity, written, hash_slots, hash_capacity)


@export("prodex_mojo_rich_model_fallback_plan_v1")
def prodex_mojo_rich_model_fallback_plan_v1(
    abi_version: Int64,
    provider_address: UInt,
    models_address: UInt,
    model_count: Int64,
    output_records_address: UInt,
    record_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    hash_slots_address: UInt,
    hash_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[
        mut=True, ProdexRichFallbackResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].records_written = 0
    result_ptr[].required_records = 0
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].issue_kind = 0
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result_ptr[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if provider_address == 0 or model_count < 0 or model_count > RICH_MAX_FALLBACK_INPUTS or record_capacity < 1 or record_capacity > RICH_MAX_FALLBACK_MODELS or output_capacity < 1 or output_records_address == 0 or output_address == 0 or hash_slots_address == 0 or models_address == 0 and model_count > 0:
        return RICH_STATUS_INVALID
    var provider_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(provider_address))
    var provider = provider_ptr[].copy()
    if not rich_view_valid(provider, RICH_MAX_IDENTIFIER_BYTES):
        return RICH_STATUS_UTF8
    var models = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(models_address))
    for index in range(model_count):
        if not rich_view_valid(models[unsafe_offset=index], RICH_MAX_IDENTIFIER_BYTES):
            return RICH_STATUS_UTF8
    var output_records = Pointer[
        mut=True, ProdexRichFallbackRecord, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_records_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var hash_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hash_slots_address)
    )
    var required_hash = rich_required_hash_capacity(record_capacity)
    if hash_capacity != required_hash:
        return RICH_STATUS_CAPACITY
    for index in range(hash_capacity):
        hash_slots[unsafe_offset=index] = -1
    var written: Int64 = 0
    var records: Int64 = 0
    for index in range(model_count):
        if not fallback_add_chain(
            provider,
            models[unsafe_offset=index],
            output_records,
            Pointer(to=records),
            output,
            output_capacity,
            Pointer(to=written),
            hash_slots,
            hash_capacity,
        ):
            result_ptr[].required_records = records + 1
            result_ptr[].required_output = written + 256
            return RICH_STATUS_CAPACITY
    result_ptr[].records_written = records
    result_ptr[].required_records = records
    result_ptr[].output_written = written
    result_ptr[].required_output = written
    return RICH_STATUS_OK


@export("prodex_mojo_rich_model_fallback_v2")
def prodex_mojo_rich_model_fallback_v2(
    abi_version: Int64,
    provider_address: UInt,
    model_address: UInt,
    output_records_address: UInt,
    record_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    hash_slots_address: UInt,
    hash_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[mut=True, ProdexRichFallbackResult, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].records_written = 0
    result_ptr[].required_records = 0
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].issue_kind = 0
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result_ptr[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if provider_address == 0 or model_address == 0:
        return RICH_STATUS_INVALID
    var provider_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(provider_address))
    var model_ptr = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(model_address))
    var provider = provider_ptr[].copy()
    var model = model_ptr[].copy()
    if record_capacity < 0 or record_capacity > RICH_MAX_FALLBACK_MODELS or not rich_view_valid(provider, RICH_MAX_IDENTIFIER_BYTES) or not rich_view_valid(model, RICH_MAX_IDENTIFIER_BYTES):
        return RICH_STATUS_INVALID
    if output_records_address == 0 or output_address == 0 or hash_slots_address == 0:
        return RICH_STATUS_INVALID
    var output_records = Pointer[mut=True, ProdexRichFallbackRecord, MutUntrackedOrigin](
        unsafe_from_address=Int(output_records_address)
    )
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var hash_slots = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(hash_slots_address)
    )
    var required_hash = rich_required_hash_capacity(record_capacity)
    if hash_capacity != required_hash:
        return RICH_STATUS_CAPACITY
    for index in range(hash_capacity):
        hash_slots[unsafe_offset=index] = -1
    var written: Int64 = 0
    var records: Int64 = 0
    if not fallback_add_chain(provider, model, output_records, Pointer(to=records), output, output_capacity, Pointer(to=written), hash_slots, hash_capacity):
        result_ptr[].required_output = written + 256
        return RICH_STATUS_CAPACITY
    result_ptr[].records_written = records
    result_ptr[].required_records = records
    result_ptr[].output_written = written
    result_ptr[].required_output = written
    return RICH_STATUS_OK
