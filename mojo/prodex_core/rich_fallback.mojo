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


comptime RUNTIME_ERROR_MODE_HTTP: Int64 = 0
comptime RUNTIME_ERROR_MODE_STREAM: Int64 = 1
comptime RUNTIME_ERROR_MODE_JSON_QUOTA: Int64 = 2
comptime RUNTIME_ERROR_MODE_JSON_RATE: Int64 = 3
comptime RUNTIME_ERROR_MODE_JSON_PROFILE: Int64 = 4
comptime RUNTIME_ERROR_MODE_JSON_OVERLOAD: Int64 = 5
comptime RUNTIME_ERROR_MODE_TEXT_QUOTA: Int64 = 6
comptime RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA: Int64 = 7
comptime RUNTIME_ERROR_MODE_TEXT_RATE: Int64 = 8
comptime RUNTIME_ERROR_MODE_TEXT_PROFILE: Int64 = 9
comptime RUNTIME_ERROR_MODE_TEXT_OVERLOAD: Int64 = 10
comptime RUNTIME_ERROR_MODE_TEXT_WORKSPACE: Int64 = 11
comptime RUNTIME_ERROR_MODE_CODE_QUOTA: Int64 = 12
comptime RUNTIME_ERROR_MODE_CODE_RATE: Int64 = 13
comptime RUNTIME_ERROR_MODE_CODE_OVERLOAD: Int64 = 14

comptime RUNTIME_ERROR_CLASS_OTHER: Int64 = 0
comptime RUNTIME_ERROR_CLASS_QUOTA: Int64 = 1
comptime RUNTIME_ERROR_CLASS_RATE: Int64 = 2
comptime RUNTIME_ERROR_CLASS_PROFILE: Int64 = 3
comptime RUNTIME_ERROR_CLASS_OVERLOAD: Int64 = 4
comptime RUNTIME_ERROR_CLASS_TRANSIENT: Int64 = 5
comptime RUNTIME_ERROR_ACTION_PASS: Int64 = 0
comptime RUNTIME_ERROR_ACTION_ROTATE: Int64 = 1
comptime RUNTIME_ERROR_ACTION_RETRY: Int64 = 2
comptime RUNTIME_ERROR_PHASE_PRECOMMIT: Int64 = 0
comptime RUNTIME_ERROR_PHASE_COMMITTED: Int64 = 1
comptime RUNTIME_ERROR_MAX_BYTES: Int64 = 65_536
comptime RUNTIME_ERROR_MAX_DEPTH: Int64 = 32


def runtime_error_none() -> ProdexRichFallbackRecord:
    return ProdexRichFallbackRecord(ProdexRichSlice(0, 0), RUNTIME_ERROR_CLASS_OTHER, 0)


def runtime_error_invalid() -> ProdexRichFallbackRecord:
    return ProdexRichFallbackRecord(ProdexRichSlice(-1, -1), RUNTIME_ERROR_CLASS_OTHER, -1)


def runtime_error_match(
    class_tag: Int64, message_start: Int64, message_end: Int64
) -> ProdexRichFallbackRecord:
    return ProdexRichFallbackRecord(
        ProdexRichSlice(message_start, message_end - message_start), class_tag, 1
    )


def runtime_error_is_match(record: ProdexRichFallbackRecord) -> Bool:
    return record.input_index == 1 and record.source_kind != RUNTIME_ERROR_CLASS_OTHER


def runtime_error_is_invalid(record: ProdexRichFallbackRecord) -> Bool:
    return record.input_index < 0


def runtime_error_ascii_space(value: UInt8) -> Bool:
    return value == 32 or value == 9 or value == 10 or value == 13


def runtime_error_skip_space(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var index = start
    while index < end and runtime_error_ascii_space(ptr[unsafe_offset=index]):
        index += 1
    return index


def runtime_error_trim_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    var index = end
    while index > start and runtime_error_ascii_space(ptr[unsafe_offset=index - 1]):
        index -= 1
    return index


def runtime_error_range_matches(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
    folded: Bool,
) -> Bool:
    if start < 0 or end < start or end - start != Int64(literal.byte_length()):
        return False
    for offset in range(end - start):
        var left = ptr[unsafe_offset=start + offset]
        var right = literal.unsafe_ptr()[unsafe_offset=offset]
        if folded and left >= 65 and left <= 90:
            left += 32
        if left != right:
            return False
    return True


def runtime_error_contains(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    var needle = Int64(literal.byte_length())
    if needle == 0:
        return True
    if start < 0 or end < start or needle > end - start:
        return False
    for offset in range(end - start - needle + 1):
        var matched = True
        for inner in range(needle):
            var left = ptr[unsafe_offset=start + offset + inner]
            var right = literal.unsafe_ptr()[unsafe_offset=inner]
            if left >= 65 and left <= 90:
                left += 32
            if left != right:
                matched = False
                break
        if matched:
            return True
    return False


def runtime_error_string_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if start >= end or ptr[unsafe_offset=start] != 34:
        return -1
    var index = start + 1
    while index < end:
        var value = ptr[unsafe_offset=index]
        if value == 92:
            if index + 1 >= end:
                return -1
            index += 2
        elif value == 34:
            return index
        elif value < 32:
            return -1
        else:
            index += 1
    return -1


def runtime_error_primitive_end(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Int64:
    if runtime_error_range_matches(ptr, start, min(start + 4, end), StringSlice("true"), False) or runtime_error_range_matches(ptr, start, min(start + 5, end), StringSlice("false"), False) or runtime_error_range_matches(ptr, start, min(start + 4, end), StringSlice("null"), False):
        if ptr[unsafe_offset=start] == 116:
            return start + 4
        if ptr[unsafe_offset=start] == 102:
            return start + 5
        return start + 4
    var index = start
    if index < end and ptr[unsafe_offset=index] == 45:
        index += 1
    var digits = index
    while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
        index += 1
    if index == digits:
        return -1
    if index < end and ptr[unsafe_offset=index] == 46:
        index += 1
        var fraction = index
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            index += 1
        if index == fraction:
            return -1
    if index < end and (ptr[unsafe_offset=index] == 101 or ptr[unsafe_offset=index] == 69):
        index += 1
        if index < end and (ptr[unsafe_offset=index] == 43 or ptr[unsafe_offset=index] == 45):
            index += 1
        var exponent = index
        while index < end and ptr[unsafe_offset=index] >= 48 and ptr[unsafe_offset=index] <= 57:
            index += 1
        if index == exponent:
            return -1
    return index


def runtime_error_code_class(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    mode: Int64,
) -> Int64:
    var quota = runtime_error_range_matches(ptr, start, end, StringSlice("insufficient_quota"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("credit_balance_exhausted"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("organization_spend_limit_exceeded"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("project_spend_limit_exceeded"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("quota_exhausted"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("quota_exceeded"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("resource_exhausted"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("usage_limit_reached"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("usage_not_included"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("workspace_member_credits_depleted"), True)
    var rate = runtime_error_range_matches(ptr, start, end, StringSlice("rate_limit_exceeded"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("rate_limit_exceeded_error"), True) or runtime_error_range_matches(ptr, start, end, StringSlice("slow_down"), True)
    var profile = runtime_error_range_matches(ptr, start, end, StringSlice("deactivated_workspace"), True)
    var overload = runtime_error_range_matches(ptr, start, end, StringSlice("server_is_overloaded"), True)
    if mode == RUNTIME_ERROR_MODE_CODE_QUOTA:
        if quota:
            return RUNTIME_ERROR_CLASS_QUOTA
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_CODE_RATE:
        if rate:
            return RUNTIME_ERROR_CLASS_RATE
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_CODE_OVERLOAD:
        if overload:
            return RUNTIME_ERROR_CLASS_OVERLOAD
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_JSON_QUOTA:
        if quota:
            return RUNTIME_ERROR_CLASS_QUOTA
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_JSON_RATE:
        if rate:
            return RUNTIME_ERROR_CLASS_RATE
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_JSON_PROFILE:
        if profile:
            return RUNTIME_ERROR_CLASS_PROFILE
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_JSON_OVERLOAD:
        if overload:
            return RUNTIME_ERROR_CLASS_OVERLOAD
        return RUNTIME_ERROR_CLASS_OTHER
    if mode == RUNTIME_ERROR_MODE_HTTP or mode == RUNTIME_ERROR_MODE_STREAM:
        if quota:
            return RUNTIME_ERROR_CLASS_QUOTA
        if rate:
            return RUNTIME_ERROR_CLASS_RATE
        if profile:
            return RUNTIME_ERROR_CLASS_PROFILE
        if overload:
            return RUNTIME_ERROR_CLASS_OVERLOAD
    return RUNTIME_ERROR_CLASS_OTHER


def runtime_error_json_direct_record(
    mode: Int64,
    status: Int64,
    quota: Bool,
    rate: Bool,
    profile: Bool,
    overload: Bool,
    error_quota: Bool,
    message_start: Int64,
    message_end: Int64,
    detail_start: Int64,
    detail_end: Int64,
    error_start: Int64,
    error_end: Int64,
    ptr: Pointer[mut=False, UInt8, _],
) -> ProdexRichFallbackRecord:
    var selected_start = message_start
    var selected_end = message_end
    if selected_start < 0:
        selected_start = detail_start
        selected_end = detail_end
    if selected_start < 0:
        selected_start = error_start
        selected_end = error_end

    if mode == RUNTIME_ERROR_MODE_HTTP:
        if status == 402 or status == 403:
            if profile:
                return runtime_error_match(RUNTIME_ERROR_CLASS_PROFILE, selected_start, selected_end)
            if quota or error_quota or selected_start >= 0 and (
                runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you've hit your usage limit"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you have hit your usage limit"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("the usage limit has been reached"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("usage limit has been reached"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("usage limit")) and (
                    runtime_error_contains(ptr, selected_start, selected_end, StringSlice("try again at"))
                    or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("request to your admin"))
                    or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("more access now"))
                )
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace_member_credits_depleted"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace is out of credits"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("out of credits")) and runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace owner")) and runtime_error_contains(ptr, selected_start, selected_end, StringSlice("refill"))
            ):
                return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, selected_start, selected_end)
        elif status == 429:
            if rate:
                return runtime_error_match(RUNTIME_ERROR_CLASS_RATE, selected_start, selected_end)
            if quota:
                return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, selected_start, selected_end)
            if selected_start >= 0 and (
                runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you've hit your usage limit"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you have hit your usage limit"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you hit your usage limit"))
            ):
                return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, selected_start, selected_end)
        elif status == 500 or status == 502 or status == 503 or status == 504 or status == 529:
            if overload:
                return runtime_error_match(RUNTIME_ERROR_CLASS_OVERLOAD, selected_start, selected_end)
        return runtime_error_none()

    if mode == RUNTIME_ERROR_MODE_STREAM:
        if profile:
            return runtime_error_match(RUNTIME_ERROR_CLASS_PROFILE, selected_start, selected_end)
        if overload:
            return runtime_error_match(RUNTIME_ERROR_CLASS_OVERLOAD, selected_start, selected_end)
        if quota:
            return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, selected_start, selected_end)
        if rate:
            return runtime_error_match(RUNTIME_ERROR_CLASS_RATE, selected_start, selected_end)
        return runtime_error_none()

    if mode == RUNTIME_ERROR_MODE_JSON_QUOTA:
        if quota or error_quota or selected_start >= 0 and (
            runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you've hit your usage limit"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("you have hit your usage limit"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("the usage limit has been reached"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("usage limit has been reached"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("usage limit")) and (
                runtime_error_contains(ptr, selected_start, selected_end, StringSlice("try again at"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("request to your admin"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("more access now"))
            )
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace_member_credits_depleted"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace is out of credits"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("out of credits")) and runtime_error_contains(ptr, selected_start, selected_end, StringSlice("workspace owner")) and runtime_error_contains(ptr, selected_start, selected_end, StringSlice("refill"))
        ):
            return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, selected_start, selected_end)
    elif mode == RUNTIME_ERROR_MODE_JSON_RATE and rate:
        return runtime_error_match(RUNTIME_ERROR_CLASS_RATE, selected_start, selected_end)
    elif mode == RUNTIME_ERROR_MODE_JSON_PROFILE and profile:
        return runtime_error_match(RUNTIME_ERROR_CLASS_PROFILE, selected_start, selected_end)
    elif mode == RUNTIME_ERROR_MODE_JSON_OVERLOAD:
        if overload or selected_start >= 0 and (
            runtime_error_contains(ptr, selected_start, selected_end, StringSlice("selected model is at capacity"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("model is at capacity")) and (
                runtime_error_contains(ptr, selected_start, selected_end, StringSlice("try a different model"))
                or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("please try again"))
            )
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("backend under high demand"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("experiencing high demand"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("server is overloaded"))
            or runtime_error_contains(ptr, selected_start, selected_end, StringSlice("currently overloaded"))
        ):
            return runtime_error_match(RUNTIME_ERROR_CLASS_OVERLOAD, selected_start, selected_end)
    return runtime_error_none()


def runtime_error_json_scan_value(
    view: ProdexRichStringView,
    cursor: Pointer[mut=True, Int64, _],
    end: Int64,
    mode: Int64,
    status: Int64,
    depth: Int64,
) -> ProdexRichFallbackRecord:
    if depth > RUNTIME_ERROR_MAX_DEPTH:
        return runtime_error_invalid()
    var ptr = rich_view_ptr(view)
    var start = runtime_error_skip_space(ptr, cursor[], end)
    if start >= end:
        return runtime_error_invalid()
    var value = ptr[unsafe_offset=start]
    if value == 123:
        return runtime_error_json_scan_object(view, cursor, end, mode, status, depth)
    if value == 91:
        cursor[] = start + 1
        var nested = runtime_error_none()
        while True:
            var index = runtime_error_skip_space(ptr, cursor[], end)
            if index >= end:
                return runtime_error_invalid()
            if ptr[unsafe_offset=index] == 93:
                cursor[] = index + 1
                return nested^
            cursor[] = index
            var item = runtime_error_json_scan_value(view, cursor, end, mode, status, depth + 1)
            if runtime_error_is_invalid(item):
                return item^
            if runtime_error_is_match(item) and not runtime_error_is_match(nested):
                nested = item^
            index = runtime_error_skip_space(ptr, cursor[], end)
            if index < end and ptr[unsafe_offset=index] == 44:
                cursor[] = index + 1
                continue
            if index < end and ptr[unsafe_offset=index] == 93:
                cursor[] = index + 1
                return nested^
            return runtime_error_invalid()
    if value == 34:
        var string_end = runtime_error_string_end(ptr, start, end)
        if string_end < 0:
            return runtime_error_invalid()
        cursor[] = string_end + 1
        return runtime_error_none()
    var primitive_end = runtime_error_primitive_end(ptr, start, end)
    if primitive_end < 0:
        return runtime_error_invalid()
    cursor[] = primitive_end
    return runtime_error_none()


def runtime_error_json_scan_object(
    view: ProdexRichStringView,
    cursor: Pointer[mut=True, Int64, _],
    end: Int64,
    mode: Int64,
    status: Int64,
    depth: Int64,
) -> ProdexRichFallbackRecord:
    var ptr = rich_view_ptr(view)
    var index = runtime_error_skip_space(ptr, cursor[], end)
    if index >= end or ptr[unsafe_offset=index] != 123:
        return runtime_error_invalid()
    index += 1
    var nested = runtime_error_none()
    var quota = False
    var rate = False
    var profile = False
    var overload = False
    var error_quota = False
    var message_start: Int64 = -1
    var message_end: Int64 = -1
    var detail_start: Int64 = -1
    var detail_end: Int64 = -1
    var error_start: Int64 = -1
    var error_end: Int64 = -1
    while True:
        index = runtime_error_skip_space(ptr, index, end)
        if index >= end:
            return runtime_error_invalid()
        if ptr[unsafe_offset=index] == 125:
            index += 1
            break
        var key_start = index + 1
        var key_end = runtime_error_string_end(ptr, index, end)
        if key_end < 0:
            return runtime_error_invalid()
        index = runtime_error_skip_space(ptr, key_end + 1, end)
        if index >= end or ptr[unsafe_offset=index] != 58:
            return runtime_error_invalid()
        index = runtime_error_skip_space(ptr, index + 1, end)
        if index >= end:
            return runtime_error_invalid()
        var value_start = index
        if ptr[unsafe_offset=index] == 34:
            var value_end = runtime_error_string_end(ptr, index, end)
            if value_end < 0:
                return runtime_error_invalid()
            var code_field = runtime_error_range_matches(ptr, key_start, key_end, StringSlice("code"), False) or runtime_error_range_matches(ptr, key_start, key_end, StringSlice("type"), False) or runtime_error_range_matches(ptr, key_start, key_end, StringSlice("status"), False) or runtime_error_range_matches(ptr, key_start, key_end, StringSlice("reason"), False)
            if code_field:
                var class_tag = runtime_error_code_class(ptr, value_start + 1, value_end, mode)
                if class_tag == RUNTIME_ERROR_CLASS_QUOTA:
                    quota = True
                elif class_tag == RUNTIME_ERROR_CLASS_RATE:
                    rate = True
                elif class_tag == RUNTIME_ERROR_CLASS_PROFILE:
                    profile = True
                elif class_tag == RUNTIME_ERROR_CLASS_OVERLOAD:
                    overload = True
            if runtime_error_range_matches(ptr, key_start, key_end, StringSlice("message"), False):
                message_start = value_start + 1
                message_end = value_end
            elif runtime_error_range_matches(ptr, key_start, key_end, StringSlice("detail"), False):
                detail_start = value_start + 1
                detail_end = value_end
            elif runtime_error_range_matches(ptr, key_start, key_end, StringSlice("error"), False):
                error_start = value_start + 1
                error_end = value_end
                if mode == RUNTIME_ERROR_MODE_HTTP and (status == 402 or status == 403):
                    error_quota = runtime_error_code_class(ptr, value_start + 1, value_end, RUNTIME_ERROR_MODE_CODE_QUOTA) == RUNTIME_ERROR_CLASS_QUOTA
                elif mode == RUNTIME_ERROR_MODE_JSON_QUOTA:
                    error_quota = runtime_error_code_class(ptr, value_start + 1, value_end, RUNTIME_ERROR_MODE_CODE_QUOTA) == RUNTIME_ERROR_CLASS_QUOTA
            index = value_end + 1
        else:
            cursor[] = index
            var child = runtime_error_json_scan_value(view, cursor, end, mode, status, depth + 1)
            if runtime_error_is_invalid(child):
                return child^
            if runtime_error_is_match(child) and not runtime_error_is_match(nested):
                nested = child^
            index = cursor[]
        index = runtime_error_skip_space(ptr, index, end)
        if index < end and ptr[unsafe_offset=index] == 44:
            index += 1
            continue
        if index < end and ptr[unsafe_offset=index] == 125:
            index += 1
            break
        return runtime_error_invalid()
    cursor[] = index
    var direct = runtime_error_json_direct_record(
        mode,
        status,
        quota,
        rate,
        profile,
        overload,
        error_quota,
        message_start,
        message_end,
        detail_start,
        detail_end,
        error_start,
        error_end,
        ptr,
    )
    if runtime_error_is_match(direct):
        return direct^
    return nested^


def runtime_error_usage_text(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return runtime_error_contains(ptr, start, end, StringSlice("you've hit your usage limit")) or runtime_error_contains(ptr, start, end, StringSlice("you have hit your usage limit")) or runtime_error_contains(ptr, start, end, StringSlice("the usage limit has been reached")) or runtime_error_contains(ptr, start, end, StringSlice("usage limit has been reached")) or runtime_error_contains(ptr, start, end, StringSlice("usage limit")) and (
        runtime_error_contains(ptr, start, end, StringSlice("try again at"))
        or runtime_error_contains(ptr, start, end, StringSlice("request to your admin"))
        or runtime_error_contains(ptr, start, end, StringSlice("more access now"))
    )


def runtime_error_workspace_text(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return runtime_error_contains(ptr, start, end, StringSlice("workspace_member_credits_depleted")) or runtime_error_contains(ptr, start, end, StringSlice("workspace is out of credits")) or runtime_error_contains(ptr, start, end, StringSlice("out of credits")) and runtime_error_contains(ptr, start, end, StringSlice("workspace owner")) and runtime_error_contains(ptr, start, end, StringSlice("refill"))


def runtime_error_overload_text(
    ptr: Pointer[mut=False, UInt8, _], start: Int64, end: Int64
) -> Bool:
    return runtime_error_contains(ptr, start, end, StringSlice("selected model is at capacity")) or runtime_error_contains(ptr, start, end, StringSlice("model is at capacity")) and (
        runtime_error_contains(ptr, start, end, StringSlice("try a different model"))
        or runtime_error_contains(ptr, start, end, StringSlice("please try again"))
    ) or runtime_error_contains(ptr, start, end, StringSlice("backend under high demand")) or runtime_error_contains(ptr, start, end, StringSlice("experiencing high demand")) or runtime_error_contains(ptr, start, end, StringSlice("server is overloaded")) or runtime_error_contains(ptr, start, end, StringSlice("currently overloaded"))


def runtime_error_text_record(
    view: ProdexRichStringView, mode: Int64
) -> ProdexRichFallbackRecord:
    var ptr = rich_view_ptr(view)
    var start = runtime_error_skip_space(ptr, 0, Int64(view.len))
    var end = runtime_error_trim_end(ptr, start, Int64(view.len))
    if start >= end:
        return runtime_error_none()
    if mode == RUNTIME_ERROR_MODE_TEXT_QUOTA:
        if runtime_error_code_class(ptr, start, end, RUNTIME_ERROR_MODE_CODE_QUOTA) != RUNTIME_ERROR_CLASS_OTHER or runtime_error_usage_text(ptr, start, end) or runtime_error_workspace_text(ptr, start, end):
            return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, start, end)
    elif mode == RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA:
        if runtime_error_contains(ptr, start, end, StringSlice("you've hit your usage limit")) or runtime_error_contains(ptr, start, end, StringSlice("you have hit your usage limit")) or runtime_error_contains(ptr, start, end, StringSlice("you hit your usage limit")):
            return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, start, end)
    elif mode == RUNTIME_ERROR_MODE_TEXT_RATE:
        if runtime_error_contains(ptr, start, end, StringSlice("rate_limit_exceeded")) or runtime_error_contains(ptr, start, end, StringSlice("rate_limit_exceeded_error")):
            return runtime_error_match(RUNTIME_ERROR_CLASS_RATE, start, end)
    elif mode == RUNTIME_ERROR_MODE_TEXT_PROFILE:
        if runtime_error_contains(ptr, start, end, StringSlice("deactivated_workspace")):
            return runtime_error_match(RUNTIME_ERROR_CLASS_PROFILE, start, end)
    elif mode == RUNTIME_ERROR_MODE_TEXT_OVERLOAD:
        if runtime_error_overload_text(ptr, start, end):
            return runtime_error_match(RUNTIME_ERROR_CLASS_OVERLOAD, start, end)
    elif mode == RUNTIME_ERROR_MODE_TEXT_WORKSPACE:
        if runtime_error_workspace_text(ptr, start, end):
            return runtime_error_match(RUNTIME_ERROR_CLASS_QUOTA, start, end)
    elif mode == RUNTIME_ERROR_MODE_CODE_QUOTA or mode == RUNTIME_ERROR_MODE_CODE_RATE or mode == RUNTIME_ERROR_MODE_CODE_OVERLOAD:
        var class_tag = runtime_error_code_class(ptr, start, end, mode)
        if class_tag != RUNTIME_ERROR_CLASS_OTHER:
            return runtime_error_match(class_tag, start, end)
    return runtime_error_none()


def runtime_error_scan_json(
    view: ProdexRichStringView, start: Int64, end: Int64, mode: Int64, status: Int64
) -> ProdexRichFallbackRecord:
    var cursor_value = start
    var cursor = Pointer(to=cursor_value)
    var record = runtime_error_json_scan_value(view, cursor, end, mode, status, 0)
    if runtime_error_is_invalid(record):
        return record^
    var ptr = rich_view_ptr(view)
    if runtime_error_skip_space(ptr, cursor[], end) != end:
        return runtime_error_invalid()
    return record^


def runtime_error_scan_sse(
    view: ProdexRichStringView, mode: Int64, status: Int64
) -> ProdexRichFallbackRecord:
    var ptr = rich_view_ptr(view)
    var line_start: Int64 = 0
    var length = Int64(view.len)
    while line_start <= length:
        var line_end = line_start
        while line_end < length and ptr[unsafe_offset=line_end] != 10:
            line_end += 1
        var trimmed_start = runtime_error_skip_space(ptr, line_start, line_end)
        var trimmed_end = runtime_error_trim_end(ptr, trimmed_start, line_end)
        if trimmed_end >= trimmed_start + 5 and runtime_error_range_matches(ptr, trimmed_start, trimmed_start + 5, StringSlice("data:"), False):
            var payload_start = runtime_error_skip_space(ptr, trimmed_start + 5, trimmed_end)
            if payload_start < trimmed_end:
                var record = runtime_error_scan_json(view, payload_start, trimmed_end, mode, status)
                if runtime_error_is_match(record):
                    return record^
        if line_end >= length:
            break
        line_start = line_end + 1
    return runtime_error_none()


def runtime_error_is_transient_status(status: Int64) -> Bool:
    return status == 500 or status == 502 or status == 503 or status == 504 or status == 529


def runtime_error_scan_body(
    view: ProdexRichStringView, mode: Int64, status: Int64
) -> ProdexRichFallbackRecord:
    if mode == RUNTIME_ERROR_MODE_TEXT_QUOTA or mode == RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA or mode == RUNTIME_ERROR_MODE_TEXT_RATE or mode == RUNTIME_ERROR_MODE_TEXT_PROFILE or mode == RUNTIME_ERROR_MODE_TEXT_OVERLOAD or mode == RUNTIME_ERROR_MODE_TEXT_WORKSPACE or mode == RUNTIME_ERROR_MODE_CODE_QUOTA or mode == RUNTIME_ERROR_MODE_CODE_RATE or mode == RUNTIME_ERROR_MODE_CODE_OVERLOAD:
        return runtime_error_text_record(view, mode)

    var ptr = rich_view_ptr(view)
    var start = runtime_error_skip_space(ptr, 0, Int64(view.len))
    var end = runtime_error_trim_end(ptr, start, Int64(view.len))
    if start < end and (ptr[unsafe_offset=start] == 123 or ptr[unsafe_offset=start] == 91):
        var json_record = runtime_error_scan_json(view, start, end, mode, status)
        if runtime_error_is_invalid(json_record):
            json_record = runtime_error_none()
        elif runtime_error_is_match(json_record):
            return json_record^
        else:
            if mode == RUNTIME_ERROR_MODE_HTTP and runtime_error_is_transient_status(status):
                return runtime_error_match(RUNTIME_ERROR_CLASS_TRANSIENT, start, end)
            return json_record^
        if mode == RUNTIME_ERROR_MODE_HTTP and start < end and runtime_error_is_transient_status(status):
            return runtime_error_match(RUNTIME_ERROR_CLASS_TRANSIENT, start, end)
    if mode == RUNTIME_ERROR_MODE_HTTP or mode == RUNTIME_ERROR_MODE_STREAM:
        var sse_record = runtime_error_scan_sse(view, mode, status)
        if runtime_error_is_match(sse_record):
            return sse_record^
    if mode != RUNTIME_ERROR_MODE_HTTP:
        return runtime_error_none()
    if status == 402 or status == 403:
        var profile_record = runtime_error_text_record(view, RUNTIME_ERROR_MODE_TEXT_PROFILE)
        if runtime_error_is_match(profile_record):
            return profile_record^
        return runtime_error_text_record(view, RUNTIME_ERROR_MODE_TEXT_QUOTA)
    if status == 429:
        return runtime_error_text_record(view, RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA)
    if runtime_error_is_transient_status(status):
        var overload_record = runtime_error_text_record(view, RUNTIME_ERROR_MODE_TEXT_OVERLOAD)
        if runtime_error_is_match(overload_record):
            return overload_record^
        if start < end:
            return runtime_error_match(RUNTIME_ERROR_CLASS_TRANSIENT, start, end)
        return runtime_error_match(RUNTIME_ERROR_CLASS_TRANSIENT, -1, -1)
    return runtime_error_none()


def runtime_error_action(class_tag: Int64, phase: Int64) -> Int64:
    if phase == RUNTIME_ERROR_PHASE_COMMITTED or class_tag == RUNTIME_ERROR_CLASS_OTHER:
        return RUNTIME_ERROR_ACTION_PASS
    if class_tag == RUNTIME_ERROR_CLASS_QUOTA or class_tag == RUNTIME_ERROR_CLASS_PROFILE:
        return RUNTIME_ERROR_ACTION_ROTATE
    if class_tag == RUNTIME_ERROR_CLASS_RATE or class_tag == RUNTIME_ERROR_CLASS_OVERLOAD or class_tag == RUNTIME_ERROR_CLASS_TRANSIENT:
        return RUNTIME_ERROR_ACTION_RETRY
    return RUNTIME_ERROR_ACTION_PASS


def runtime_error_write_literal(
    literal: StringSlice,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    return rich_copy_range(
        literal.unsafe_ptr(), 0, Int64(literal.byte_length()), output, output_capacity, written, False
    )


def runtime_error_write_message(
    record: ProdexRichFallbackRecord,
    status: Int64,
    body: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    if record.model.len > 0:
        var source = rich_view_ptr(body)
        var copied = rich_copy_range(
            source,
            record.model.offset,
            record.model.offset + record.model.len,
            output,
            output_capacity,
            written,
            False,
        )
        if copied.len >= 0:
            return copied^
    if record.source_kind == RUNTIME_ERROR_CLASS_QUOTA:
        return runtime_error_write_literal(StringSlice("Upstream Codex account quota was exhausted."), output, output_capacity, written)
    if record.source_kind == RUNTIME_ERROR_CLASS_RATE:
        return runtime_error_write_literal(StringSlice("Upstream Codex profile is temporarily rate limited."), output, output_capacity, written)
    if record.source_kind == RUNTIME_ERROR_CLASS_PROFILE:
        return runtime_error_write_literal(StringSlice("Upstream Codex workspace is deactivated for this profile."), output, output_capacity, written)
    if record.source_kind == RUNTIME_ERROR_CLASS_OVERLOAD:
        return runtime_error_write_literal(StringSlice("Upstream Codex backend is currently overloaded."), output, output_capacity, written)
    if status == 500:
        return runtime_error_write_literal(StringSlice("Upstream Codex backend is currently experiencing high demand."), output, output_capacity, written)
    if status == 502:
        return runtime_error_write_literal(StringSlice("Upstream Codex backend returned transient HTTP 502."), output, output_capacity, written)
    if status == 503:
        return runtime_error_write_literal(StringSlice("Upstream Codex backend returned transient HTTP 503."), output, output_capacity, written)
    if status == 504:
        return runtime_error_write_literal(StringSlice("Upstream Codex backend returned transient HTTP 504."), output, output_capacity, written)
    return runtime_error_write_literal(StringSlice("Upstream Codex backend returned transient HTTP 529."), output, output_capacity, written)


@export("prodex_mojo_rich_runtime_error_policy_v1")
def prodex_mojo_rich_runtime_error_policy_v1(
    abi_version: Int64,
    operation: Int64,
    status: Int64,
    phase: Int64,
    body_address: UInt,
    body_len: Int64,
    output_records_address: UInt,
    record_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
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
    if abi_version != PRODEX_RICH_ABI_VERSION or operation < RUNTIME_ERROR_MODE_HTTP or operation > RUNTIME_ERROR_MODE_CODE_OVERLOAD or (phase != RUNTIME_ERROR_PHASE_PRECOMMIT and phase != RUNTIME_ERROR_PHASE_COMMITTED) or status < 0 or body_len < 0 or body_len > RUNTIME_ERROR_MAX_BYTES or record_capacity < 1 or output_capacity < 1 or output_records_address == 0 or output_address == 0 or body_address == 0 and body_len > 0:
        return RICH_STATUS_INVALID
    var body = ProdexRichStringView(body_address, UInt(body_len))
    if not rich_view_valid(body, RUNTIME_ERROR_MAX_BYTES):
        return RICH_STATUS_UTF8
    var output_records = Pointer[
        mut=True, ProdexRichFallbackRecord, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_records_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var record = runtime_error_scan_body(body, operation, status)
    if not runtime_error_is_match(record):
        return RICH_STATUS_OK
    var written: Int64 = 0
    var message = runtime_error_write_message(record, status, body, output, output_capacity, Pointer(to=written))
    if message.len < 0:
        result_ptr[].required_records = 1
        result_ptr[].required_output = written + 256
        return RICH_STATUS_CAPACITY
    output_records[unsafe_offset=0].model = message^
    output_records[unsafe_offset=0].source_kind = record.source_kind
    output_records[unsafe_offset=0].input_index = runtime_error_action(record.source_kind, phase)
    result_ptr[].records_written = 1
    result_ptr[].required_records = 1
    result_ptr[].output_written = written
    result_ptr[].required_output = written
    return RICH_STATUS_OK



comptime RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS: Int64 = 0
comptime RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS: Int64 = 1
comptime RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS: Int64 = 2
comptime RUNTIME_RETRY_AFTER_CAP_MILLIS: UInt128 = 300_000
comptime RUNTIME_RETRY_AFTER_U64_MAX: UInt128 = 18_446_744_073_709_551_615
comptime RUNTIME_RETRY_AFTER_U128_MAX: UInt128 = 340_282_366_920_938_463_463_374_607_431_768_211_455
comptime RUNTIME_RETRY_AFTER_U128_SECONDS_QUOTIENT: UInt128 = 340_282_366_920_938_463_463_374_607_431_768_211
comptime RUNTIME_RETRY_AFTER_U128_SECONDS_REMAINDER: UInt128 = 455


def runtime_retry_after_parse_whole(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt128, _],
) -> Bool:
    if start >= end:
        return False
    var value: UInt128 = 0
    for index in range(start, end):
        var byte = ptr[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt128(byte - 48)
        if value > (RUNTIME_RETRY_AFTER_U128_MAX - digit) // UInt128(10):
            return False
        value = value * UInt128(10) + digit
    output[] = value
    return True


def runtime_retry_after_fraction_millis(
    ptr: Pointer[mut=False, UInt8, _],
    start: Int64,
    end: Int64,
    output: Pointer[mut=True, UInt128, _],
    any_nonzero: Pointer[mut=True, Bool, _],
) -> Bool:
    var millis: UInt128 = 0
    var digits: Int64 = 0
    var rounded = False
    for index in range(start, end):
        var byte = ptr[unsafe_offset=index]
        if byte < 48 or byte > 57:
            return False
        var digit = UInt128(byte - 48)
        if digit != 0:
            any_nonzero[] = True
        if digits < 3:
            millis = millis * UInt128(10) + digit
        elif digit != 0:
            rounded = True
        digits += 1
    while digits < 3:
        millis *= UInt128(10)
        digits += 1
    if rounded:
        millis += UInt128(1)
    output[] = millis
    return True


@export("prodex_mojo_rich_retry_after_millis_v1")
def prodex_mojo_rich_retry_after_millis_v1(
    abi_version: Int64,
    mode: Int64,
    number_address: UInt,
    number_length: Int64,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return -2
    if (
        mode < RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS
        or mode > RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS
        or number_length <= 0
        or number_address == 0
    ):
        return -2

    var ptr = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(number_address)
    )
    var dot: Int64 = -1
    for index in range(number_length):
        var byte = ptr[unsafe_offset=index]
        if byte == 46:
            if dot >= 0:
                return -1
            dot = index
        elif byte < 48 or byte > 57:
            return -1

    if mode == RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS and dot >= 0:
        return -1

    var whole_end = number_length
    if dot >= 0:
        whole_end = dot
    var whole: UInt128 = 0
    if not runtime_retry_after_parse_whole(ptr, 0, whole_end, Pointer(to=whole)):
        return -1

    if mode == RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS:
        if whole == 0 or whole > RUNTIME_RETRY_AFTER_U64_MAX:
            return -1
        var millis = whole * UInt128(1000)
        if millis > RUNTIME_RETRY_AFTER_CAP_MILLIS:
            millis = RUNTIME_RETRY_AFTER_CAP_MILLIS
        return Int64(millis)

    var fraction_start = number_length
    if dot >= 0:
        fraction_start = dot + 1
    var fraction_millis: UInt128 = 0
    var fraction_nonzero = False
    if not runtime_retry_after_fraction_millis(
        ptr,
        fraction_start,
        number_length,
        Pointer(to=fraction_millis),
        Pointer(to=fraction_nonzero),
    ):
        return -1

    var millis: UInt128 = 0
    if mode == RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS:
        var increment = UInt128(1) if fraction_nonzero else UInt128(0)
        if whole == RUNTIME_RETRY_AFTER_U128_MAX and increment != 0:
            return -1
        millis = whole + increment
    else:
        if whole > RUNTIME_RETRY_AFTER_U128_SECONDS_QUOTIENT:
            return -1
        if (
            whole == RUNTIME_RETRY_AFTER_U128_SECONDS_QUOTIENT
            and fraction_millis > RUNTIME_RETRY_AFTER_U128_SECONDS_REMAINDER
        ):
            return -1
        millis = whole * UInt128(1000) + fraction_millis

    if millis == 0:
        return -1
    if millis > RUNTIME_RETRY_AFTER_CAP_MILLIS:
        millis = RUNTIME_RETRY_AFTER_CAP_MILLIS
    return Int64(millis)

comptime PREVIOUS_RESPONSE_ROUTE_RESPONSES: Int64 = 0
comptime PREVIOUS_RESPONSE_ROUTE_WEBSOCKET: Int64 = 1
comptime PREVIOUS_RESPONSE_SHAPE_NONE: Int64 = -1
comptime PREVIOUS_RESPONSE_SHAPE_EMPTY_INPUT: Int64 = 1
comptime PREVIOUS_RESPONSE_SHAPE_SESSION_REPLAY: Int64 = 2
comptime PREVIOUS_RESPONSE_RETRY_NONE: Int64 = 0
comptime PREVIOUS_RESPONSE_RETRY_TURN_STATE: Int64 = 1
comptime PREVIOUS_RESPONSE_RETRY_LOCKED_AFFINITY: Int64 = 2
comptime PREVIOUS_RESPONSE_CHAIN_NONE: Int64 = 0
comptime PREVIOUS_RESPONSE_CHAIN_RESPONSES: Int64 = 1
comptime PREVIOUS_RESPONSE_CHAIN_WEBSOCKET_LOCKED: Int64 = 2
comptime PREVIOUS_RESPONSE_STALE_NOT_APPLICABLE: Int64 = 0
comptime PREVIOUS_RESPONSE_STALE_RETRY_TURN_STATE: Int64 = 1
comptime PREVIOUS_RESPONSE_STALE_FAIL_CLOSED: Int64 = 2
comptime PREVIOUS_RESPONSE_OBSERVABILITY_NONE: Int64 = 0
comptime PREVIOUS_RESPONSE_OBSERVABILITY_BLOCKED: Int64 = 1
comptime PREVIOUS_RESPONSE_OBSERVABILITY_NONREPLAYABLE: Int64 = 2


def previous_response_bool(value: Int64) -> Bool:
    return value == 1


def previous_response_retry_delay_ms(retry_index: Int64) -> Int64:
    if retry_index == 0:
        return 75
    if retry_index == 1:
        return 200
    if retry_index == 2:
        return 500
    return -1


@export("prodex_runtime_previous_response_plan_v1")
def prodex_runtime_previous_response_plan_v1(
    route: Int64,
    previous_response_present: Int64,
    has_turn_state_retry: Int64,
    request_requires_previous_response_affinity: Int64,
    trusted_previous_response_affinity: Int64,
    request_turn_state_present: Int64,
    previous_response_fresh_fallback_used: Int64,
    fresh_fallback_shape: Int64,
    retry_index: Int64,
    has_session_affinity: Int64,
    output: Pointer[mut=True, Int64, _],
) abi("C") -> Int64:
    if (
        route < PREVIOUS_RESPONSE_ROUTE_RESPONSES
        or route > PREVIOUS_RESPONSE_ROUTE_WEBSOCKET
        or previous_response_present < 0
        or previous_response_present > 1
        or has_turn_state_retry < 0
        or has_turn_state_retry > 1
        or request_requires_previous_response_affinity < 0
        or request_requires_previous_response_affinity > 1
        or trusted_previous_response_affinity < 0
        or trusted_previous_response_affinity > 1
        or request_turn_state_present < 0
        or request_turn_state_present > 1
        or previous_response_fresh_fallback_used < 0
        or previous_response_fresh_fallback_used > 1
        or fresh_fallback_shape < PREVIOUS_RESPONSE_SHAPE_NONE
        or fresh_fallback_shape > 3
        or retry_index < 0
        or has_session_affinity < 0
        or has_session_affinity > 1
    ):
        return RICH_STATUS_INVALID

    var previous_present = previous_response_bool(previous_response_present)
    var turn_state_retry = previous_response_bool(has_turn_state_retry)
    var request_turn_state = previous_response_bool(request_turn_state_present)
    var request_requires_affinity = previous_response_bool(
        request_requires_previous_response_affinity
    )
    var websocket_requires_affinity = (
        previous_response_bool(trusted_previous_response_affinity)
        and previous_present
        and not request_turn_state
    )
    var request_requires_locked_affinity = request_requires_affinity
    if route == PREVIOUS_RESPONSE_ROUTE_WEBSOCKET:
        request_requires_locked_affinity = (
            request_requires_affinity or websocket_requires_affinity
        )

    var locked_affinity_retry = (
        route == PREVIOUS_RESPONSE_ROUTE_WEBSOCKET
        and request_requires_affinity
        and not turn_state_retry
    )
    var retry_reason = PREVIOUS_RESPONSE_RETRY_NONE
    if turn_state_retry:
        retry_reason = PREVIOUS_RESPONSE_RETRY_TURN_STATE
    elif locked_affinity_retry:
        retry_reason = PREVIOUS_RESPONSE_RETRY_LOCKED_AFFINITY

    var chain_reason = PREVIOUS_RESPONSE_CHAIN_NONE
    if route == PREVIOUS_RESPONSE_ROUTE_RESPONSES and turn_state_retry:
        chain_reason = PREVIOUS_RESPONSE_CHAIN_RESPONSES
    elif route == PREVIOUS_RESPONSE_ROUTE_WEBSOCKET and locked_affinity_retry:
        chain_reason = PREVIOUS_RESPONSE_CHAIN_WEBSOCKET_LOCKED

    var stale_policy = PREVIOUS_RESPONSE_STALE_NOT_APPLICABLE
    if previous_present:
        if turn_state_retry:
            stale_policy = PREVIOUS_RESPONSE_STALE_RETRY_TURN_STATE
        else:
            stale_policy = PREVIOUS_RESPONSE_STALE_FAIL_CLOSED

    var has_previous_context = (
        previous_present
        or previous_response_bool(previous_response_fresh_fallback_used)
    )
    var fresh_fail_closed = (
        has_previous_context
        or request_requires_locked_affinity
        or fresh_fallback_shape != PREVIOUS_RESPONSE_SHAPE_NONE
    )
    var fresh_blocked_without_affinity = (
        fresh_fail_closed
        and not turn_state_retry
        and not request_requires_locked_affinity
    )
    var observability = PREVIOUS_RESPONSE_OBSERVABILITY_NONE
    if fresh_blocked_without_affinity:
        observability = PREVIOUS_RESPONSE_OBSERVABILITY_BLOCKED
        if fresh_fallback_shape == 3:
            observability = PREVIOUS_RESPONSE_OBSERVABILITY_NONREPLAYABLE

    var effective_shape = fresh_fallback_shape
    if (
        previous_response_bool(has_session_affinity)
        and fresh_fallback_shape == PREVIOUS_RESPONSE_SHAPE_EMPTY_INPUT
    ):
        effective_shape = PREVIOUS_RESPONSE_SHAPE_SESSION_REPLAY

    output[unsafe_offset=0] = retry_reason
    output[unsafe_offset=1] = previous_response_retry_delay_ms(retry_index)
    if retry_reason == PREVIOUS_RESPONSE_RETRY_NONE:
        output[unsafe_offset=1] = -1
    output[unsafe_offset=2] = chain_reason
    output[unsafe_offset=3] = Int64(request_requires_locked_affinity)
    output[unsafe_offset=4] = stale_policy
    output[unsafe_offset=5] = Int64(fresh_fail_closed)
    output[unsafe_offset=6] = Int64(fresh_blocked_without_affinity)
    output[unsafe_offset=7] = observability
    output[unsafe_offset=8] = effective_shape
    output[unsafe_offset=9] = Int64(websocket_requires_affinity)
    return RICH_STATUS_OK
