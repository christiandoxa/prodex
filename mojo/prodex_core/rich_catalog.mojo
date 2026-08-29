from std.memory import Pointer
from rich_text import (
    rich_copy_range,
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import (
    ProdexRichCatalogPlanChoice,
    ProdexRichCatalogPlanModel,
    ProdexRichCatalogPlanResult,
    ProdexRichCatalogReasoningResult,
    ProdexRichSlice,
    ProdexRichStringView,
)
comptime PRODEX_RICH_ABI_VERSION: Int64 = 6
comptime CATALOG_MAX_MODELS: Int64 = 1_024
comptime CATALOG_MAX_INPUT_MODELS: Int64 = 65_536
comptime CATALOG_MAX_IDENTIFIER_BYTES: Int64 = 4_096
comptime CATALOG_MAX_QUERY_BYTES: Int64 = 65_536
comptime CATALOG_MAX_CHOICES: Int64 = 1_024
comptime RICH_STATUS_OK: Int64 = 0
comptime RICH_STATUS_INVALID: Int64 = 1
comptime RICH_STATUS_UTF8: Int64 = 2
comptime RICH_STATUS_CAPACITY: Int64 = 3
comptime RICH_STATUS_ABI: Int64 = 4
comptime RICH_ISSUE_EFFORT: Int64 = 5

comptime CHOICE_PROVIDER_DEFAULT: Int64 = 0
comptime CHOICE_CATALOG: Int64 = 1
comptime CHOICE_CONFIGURED: Int64 = 2
comptime CHOICE_CURRENT: Int64 = 3
comptime CHOICE_CUSTOM: Int64 = 4
comptime CATALOG_PLAN_ROLE_MAIN: Int64 = 0
comptime CATALOG_PLAN_ROLE_SUB_AGENT: Int64 = 1
comptime CATALOG_PLAN_FLAG_SUPPORTED: Int64 = 1
comptime CATALOG_PLAN_FLAG_HIDDEN: Int64 = 2
comptime CATALOG_PLAN_FLAG_NOT_LISTED: Int64 = 4
comptime CATALOG_PLAN_FLAG_MASK: Int64 = 7
comptime CATALOG_PLAN_MAX_QUERY_BYTES: Int64 = 65_536


def catalog_byte_equal_folded(left: UInt8, right: UInt8) -> Bool:
    var left_value = left
    var right_value = right
    if left_value >= 65 and left_value <= 90:
        left_value += 32
    if right_value >= 65 and right_value <= 90:
        right_value += 32
    return left_value == right_value
def catalog_view_equal_range(
    left: ProdexRichStringView,
    left_start: Int64,
    left_end: Int64,
    right: ProdexRichStringView,
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    if left_end - left_start != right_end - right_start:
        return False
    if left_end == left_start:
        return True
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(left_end - left_start):
        if not catalog_byte_equal_folded(
            left_ptr[unsafe_offset=left_start + index],
            right_ptr[unsafe_offset=right_start + index],
        ):
            return False
    return True


def catalog_view_equal_full(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    return catalog_view_equal_range(
        left, 0, Int64(left.len), right, 0, Int64(right.len)
    )
def catalog_valid_views(
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    model_count: Int64,
    aliases: Pointer[mut=False, ProdexRichStringView, _],
    alias_models: Pointer[mut=False, Int64, _],
    alias_count: Int64,
) -> Bool:
    if model_count < 0 or model_count > CATALOG_MAX_MODELS:
        return False
    if alias_count < 0 or alias_count > CATALOG_MAX_INPUT_MODELS:
        return False
    for model_index in range(model_count):
        var model = model_ids[unsafe_offset=model_index].copy()
        if not rich_view_valid(model, CATALOG_MAX_IDENTIFIER_BYTES) or model.len == 0:
            return False
    for alias_index in range(alias_count):
        var alias_view = aliases[unsafe_offset=alias_index].copy()
        var model_index = alias_models[unsafe_offset=alias_index]
        if not rich_view_valid(alias_view, CATALOG_MAX_IDENTIFIER_BYTES):
            return False
        if model_index < 0 or model_index >= model_count:
            return False
    return True


# ponytail: the catalog is currently small (<=1,024 models); linear identity scans keep the ABI arena-only. Add per-provider hash indexes if catalogs grow materially.
def catalog_find(
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    model_count: Int64,
    aliases: Pointer[mut=False, ProdexRichStringView, _],
    alias_models: Pointer[mut=False, Int64, _],
    alias_count: Int64,
    query: ProdexRichStringView,
) -> Int64:
    var bounds = rich_trim_bounds(query)
    if bounds[1] <= bounds[0]:
        return -1
    for model_index in range(model_count):
        if catalog_view_equal_range(
            query,
            bounds[0],
            bounds[1],
            model_ids[unsafe_offset=model_index],
            0,
            Int64(model_ids[unsafe_offset=model_index].len),
        ):
            return model_index
        for alias_index in range(alias_count):
            if alias_models[unsafe_offset=alias_index] == model_index and catalog_view_equal_range(
                query,
                bounds[0],
                bounds[1],
                aliases[unsafe_offset=alias_index],
                0,
                Int64(aliases[unsafe_offset=alias_index].len),
            ):
                return model_index
    return -1


def catalog_choice_view(
    kind: Int64,
    index: Int64,
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    configured: Pointer[mut=False, ProdexRichStringView, _],
    current: ProdexRichStringView,
) -> ProdexRichStringView:
    if kind == CHOICE_CATALOG:
        return model_ids[unsafe_offset=index].copy()
    if kind == CHOICE_CONFIGURED:
        return configured[unsafe_offset=index].copy()
    return current.copy()


def catalog_choice_seen(
    kind: Int64,
    index: Int64,
    output_kinds: Pointer[mut=True, Int64, _],
    output_indices: Pointer[mut=True, Int64, _],
    output_count: Int64,
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    configured: Pointer[mut=False, ProdexRichStringView, _],
    current: ProdexRichStringView,
) -> Bool:
    var candidate = catalog_choice_view(kind, index, model_ids, configured, current)
    for position in range(output_count):
        var existing_kind = output_kinds[unsafe_offset=position]
        if existing_kind == CHOICE_PROVIDER_DEFAULT or existing_kind == CHOICE_CUSTOM:
            continue
        var existing = catalog_choice_view(
            existing_kind,
            output_indices[unsafe_offset=position],
            model_ids,
            configured,
            current,
        )
        if catalog_view_equal_full(candidate, existing):
            return True
    return False


def catalog_choices_equal(
    left_kind: Int64,
    left_index: Int64,
    right_kind: Int64,
    right_index: Int64,
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    configured: Pointer[mut=False, ProdexRichStringView, _],
    current: ProdexRichStringView,
) -> Bool:
    return catalog_view_equal_full(
        catalog_choice_view(left_kind, left_index, model_ids, configured, current),
        catalog_choice_view(right_kind, right_index, model_ids, configured, current),
    )


def catalog_write_choice(
    kind: Int64,
    index: Int64,
    output_kinds: Pointer[mut=True, Int64, _],
    output_indices: Pointer[mut=True, Int64, _],
    output_count: Pointer[mut=True, Int64, _],
    output_capacity: Int64,
) -> Bool:
    if output_count[] >= output_capacity:
        return False
    output_kinds[unsafe_offset=output_count[]] = kind
    output_indices[unsafe_offset=output_count[]] = index
    output_count[] += 1
    return True


def catalog_prepare(
    model_ids_address: UInt,
    model_count: Int64,
    aliases_address: UInt,
    alias_models_address: UInt,
    alias_count: Int64,
) -> Bool:
    if model_count < 0 or model_count > CATALOG_MAX_MODELS or alias_count < 0 or alias_count > CATALOG_MAX_INPUT_MODELS:
        return False
    if model_count == 0:
        return alias_count == 0
    if model_ids_address == 0:
        return False
    if alias_count > 0 and (aliases_address == 0 or alias_models_address == 0):
        return False
    return True


def catalog_effort_equal(
    left: ProdexRichStringView,
    left_start: Int64,
    left_end: Int64,
    right: ProdexRichStringView,
    right_start: Int64,
    right_end: Int64,
) -> Bool:
    return catalog_view_equal_range(
        left, left_start, left_end, right, right_start, right_end
    )


def catalog_effort_output_equal(
    output: Pointer[mut=False, UInt8, _],
    stored: ProdexRichSlice,
    candidate: ProdexRichStringView,
    candidate_start: Int64,
    candidate_end: Int64,
) -> Bool:
    if stored.len != candidate_end - candidate_start:
        return False
    var candidate_ptr = rich_view_ptr(candidate)
    for index in range(stored.len):
        if not catalog_byte_equal_folded(
            output[unsafe_offset=stored.offset + index],
            candidate_ptr[unsafe_offset=candidate_start + index],
        ):
            return False
    return True


@export("prodex_mojo_rich_catalog_reasoning_v1")
def prodex_mojo_rich_catalog_reasoning_v1(
    abi_version: Int64,
    model_ids_address: UInt,
    model_count: Int64,
    aliases_address: UInt,
    alias_models_address: UInt,
    alias_count: Int64,
    efforts_address: UInt,
    effort_models_address: UInt,
    effort_count: Int64,
    defaults_address: UInt,
    requested_model_address: UInt,
    requested_present: Int64,
    fallback_model_address: UInt,
    fallback_present: Int64,
    requested_effort_address: UInt,
    effort_present: Int64,
    output_efforts_address: UInt,
    effort_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[
        mut=True, ProdexRichCatalogReasoningResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].model_index = -1
    result_ptr[].efforts_written = 0
    result_ptr[].selected_effort = ProdexRichSlice(-1, 0)
    result_ptr[].default_effort = ProdexRichSlice(-1, 0)
    result_ptr[].output_written = 0
    result_ptr[].issue_kind = 0
    result_ptr[].issue_index = -1
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result_ptr[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if (
        requested_present < 0
        or requested_present > 1
        or fallback_present < 0
        or fallback_present > 1
        or effort_present < 0
        or effort_present > 1
        or effort_count < 0
        or effort_count > CATALOG_MAX_INPUT_MODELS
        or effort_capacity < 0
        or output_capacity < 0
        or not catalog_prepare(
            model_ids_address,
            model_count,
            aliases_address,
            alias_models_address,
            alias_count,
        )
    ):
        return RICH_STATUS_INVALID
    if model_count > 0 and defaults_address == 0:
        return RICH_STATUS_INVALID
    if effort_count > 0 and (efforts_address == 0 or effort_models_address == 0):
        return RICH_STATUS_INVALID
    if effort_capacity > 0 and output_efforts_address == 0:
        return RICH_STATUS_INVALID
    if output_capacity > 0 and output_address == 0:
        return RICH_STATUS_INVALID
    if requested_present == 1 and requested_model_address == 0:
        return RICH_STATUS_INVALID
    if fallback_present == 1 and fallback_model_address == 0:
        return RICH_STATUS_INVALID
    if effort_present == 1 and requested_effort_address == 0:
        return RICH_STATUS_INVALID

    var model_ids = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(model_ids_address))
    var aliases = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(aliases_address))
    var alias_models = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(alias_models_address)
    )
    if not catalog_valid_views(
        model_ids, model_count, aliases, alias_models, alias_count
    ):
        return RICH_STATUS_UTF8
    var defaults = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(defaults_address))
    for index in range(model_count):
        if not rich_view_valid(defaults[unsafe_offset=index], CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var efforts = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(efforts_address))
    var effort_models = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(effort_models_address)
    )
    for index in range(effort_count):
        if not rich_view_valid(
            efforts[unsafe_offset=index], CATALOG_MAX_QUERY_BYTES
        ) or effort_models[unsafe_offset=index] < 0 or effort_models[unsafe_offset=index] >= model_count:
            return RICH_STATUS_INVALID
    var requested_model = ProdexRichStringView(0, 0)
    if requested_present == 1:
        requested_model = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(requested_model_address))[].copy()
        if not rich_view_valid(requested_model, CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var fallback_model = ProdexRichStringView(0, 0)
    if fallback_present == 1:
        fallback_model = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(fallback_model_address))[].copy()
        if not rich_view_valid(fallback_model, CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var requested_effort = ProdexRichStringView(0, 0)
    if effort_present == 1:
        requested_effort = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(requested_effort_address))[].copy()
        if not rich_view_valid(requested_effort, CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var selected_index: Int64 = -1
    if requested_present == 1:
        selected_index = catalog_find(
            model_ids, model_count, aliases, alias_models, alias_count, requested_model
        )
    if selected_index < 0 and fallback_present == 1:
        selected_index = catalog_find(
            model_ids, model_count, aliases, alias_models, alias_count, fallback_model
        )
    result_ptr[].model_index = selected_index
    if selected_index < 0:
        return RICH_STATUS_OK

    var output_efforts = Pointer[
        mut=True, ProdexRichSlice, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_efforts_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var first_effort = ProdexRichSlice(-1, 0)
    var written: Int64 = result_ptr[].output_written
    var default_effort = defaults[unsafe_offset=selected_index].copy()
    var default_bounds = rich_trim_bounds(default_effort)
    if default_bounds[1] > default_bounds[0]:
        var copied_default = rich_copy_range(
            rich_view_ptr(default_effort),
            default_bounds[0],
            default_bounds[1],
            output,
            output_capacity,
            Pointer(to=written),
            False,
        )
        if copied_default.len < 0:
            return RICH_STATUS_CAPACITY
        result_ptr[].default_effort = copied_default.copy()
        result_ptr[].output_written = written
    for index in range(effort_count):
        if effort_models[unsafe_offset=index] != selected_index:
            continue
        var effort = efforts[unsafe_offset=index].copy()
        var bounds = rich_trim_bounds(effort)
        if bounds[1] <= bounds[0]:
            continue
        var duplicate = False
        for written_index in range(result_ptr[].efforts_written):
            if catalog_effort_output_equal(
                output,
                output_efforts[unsafe_offset=written_index],
                effort,
                bounds[0],
                bounds[1],
            ):
                duplicate = True
                break
        if duplicate:
            continue
        if result_ptr[].efforts_written >= effort_capacity:
            return RICH_STATUS_CAPACITY
        var copied = rich_copy_range(
            rich_view_ptr(effort),
            bounds[0],
            bounds[1],
            output,
            output_capacity,
            Pointer(to=written),
            False,
        )
        if copied.len < 0:
            return RICH_STATUS_CAPACITY
        result_ptr[].output_written = written
        output_efforts[unsafe_offset=result_ptr[].efforts_written] = copied.copy()
        result_ptr[].efforts_written += 1
        if first_effort.offset < 0:
            first_effort = copied.copy()
        if effort_present == 1:
            var requested_bounds = rich_trim_bounds(requested_effort)
            if catalog_effort_equal(
                requested_effort,
                requested_bounds[0],
                requested_bounds[1],
                effort,
                bounds[0],
                bounds[1],
            ):
                result_ptr[].selected_effort = copied.copy()
    if effort_present == 1 and result_ptr[].selected_effort.offset < 0:
        var requested_bounds = rich_trim_bounds(requested_effort)
        result_ptr[].issue_kind = RICH_ISSUE_EFFORT
        result_ptr[].issue_offset = requested_bounds[0]
        result_ptr[].issue_length = requested_bounds[1] - requested_bounds[0]
        return RICH_STATUS_OK
    if result_ptr[].default_effort.offset < 0:
        result_ptr[].default_effort = first_effort.copy()
    if effort_present == 0:
        result_ptr[].selected_effort = result_ptr[].default_effort.copy()
    return RICH_STATUS_OK


@export("prodex_mojo_rich_catalog_resolve_v1")
def prodex_mojo_rich_catalog_resolve_v1(
    abi_version: Int64,
    model_ids_address: UInt,
    model_count: Int64,
    aliases_address: UInt,
    alias_models_address: UInt,
    alias_count: Int64,
    query_address: UInt,
    output_index_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return RICH_STATUS_ABI
    if query_address == 0 or output_index_address == 0 or not catalog_prepare(
        model_ids_address, model_count, aliases_address, alias_models_address, alias_count
    ):
        return RICH_STATUS_INVALID
    var model_ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(model_ids_address)
    )
    var aliases = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(aliases_address)
    )
    var alias_models = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(alias_models_address)
    )
    var query = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(query_address)
    )[].copy()
    if not catalog_valid_views(model_ids, model_count, aliases, alias_models, alias_count) or not rich_view_valid(query, CATALOG_MAX_QUERY_BYTES):
        return RICH_STATUS_UTF8
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_index_address)
    )
    output[] = catalog_find(model_ids, model_count, aliases, alias_models, alias_count, query)
    return RICH_STATUS_OK


@export("prodex_mojo_rich_catalog_choices_v1")
def prodex_mojo_rich_catalog_choices_v1(
    abi_version: Int64,
    model_ids_address: UInt,
    model_count: Int64,
    aliases_address: UInt,
    alias_models_address: UInt,
    alias_count: Int64,
    configured_address: UInt,
    configured_count: Int64,
    current_address: UInt,
    current_present: Int64,
    output_kinds_address: UInt,
    output_indices_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return RICH_STATUS_ABI
    if current_present < 0 or current_present > 1 or configured_count < 0 or configured_count > CATALOG_MAX_INPUT_MODELS or output_capacity < 2 or output_count_address == 0 or output_kinds_address == 0 or output_indices_address == 0 or not catalog_prepare(
        model_ids_address, model_count, aliases_address, alias_models_address, alias_count
    ):
        return RICH_STATUS_INVALID
    if configured_count > 0 and configured_address == 0:
        return RICH_STATUS_INVALID
    if current_present == 1 and current_address == 0:
        return RICH_STATUS_INVALID
    var model_ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(model_ids_address)
    )
    var aliases = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(aliases_address)
    )
    var alias_models = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(alias_models_address)
    )
    var configured = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(configured_address)
    )
    var current = ProdexRichStringView(0, 0)
    if current_present == 1:
        current = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
            unsafe_from_address=Int(current_address)
        )[].copy()
    if not catalog_valid_views(model_ids, model_count, aliases, alias_models, alias_count) or current_present == 1 and not rich_view_valid(current, CATALOG_MAX_QUERY_BYTES):
        return RICH_STATUS_UTF8
    for configured_index in range(configured_count):
        if not rich_view_valid(configured[unsafe_offset=configured_index], CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var output_kinds = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_kinds_address)
    )
    var output_indices = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_indices_address)
    )
    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    output_count[] = 0
    if not catalog_write_choice(
        CHOICE_PROVIDER_DEFAULT, -1, output_kinds, output_indices, output_count, output_capacity
    ):
        return RICH_STATUS_CAPACITY
    var model_choice_count: Int64 = 0
    for model_index in range(model_count):
        if model_choice_count >= CATALOG_MAX_CHOICES:
            break
        if not catalog_choice_seen(
            CHOICE_CATALOG, model_index, output_kinds, output_indices, output_count[], model_ids, configured, current
        ):
            if not catalog_write_choice(
                CHOICE_CATALOG, model_index, output_kinds, output_indices, output_count, output_capacity
            ):
                return RICH_STATUS_CAPACITY
            model_choice_count += 1
    var current_kind = CHOICE_CURRENT
    var current_index: Int64 = -1
    if current_present == 1:
        var resolved = catalog_find(model_ids, model_count, aliases, alias_models, alias_count, current)
        if resolved >= 0:
            current_kind = CHOICE_CATALOG
            current_index = resolved
        if catalog_choice_seen(
            current_kind, current_index, output_kinds, output_indices, output_count[], model_ids, configured, current
        ):
            current_kind = -1

    var reserve_current: Int64 = 0
    if current_present == 1 and current_kind != -1:
        reserve_current = 1
    for configured_index in range(configured_count):
        if model_choice_count >= CATALOG_MAX_CHOICES - reserve_current:
            break
        var configured_model = configured[unsafe_offset=configured_index].copy()
        var bounds = rich_trim_bounds(configured_model)
        if bounds[1] <= bounds[0]:
            continue
        var kind = CHOICE_CONFIGURED
        var index = configured_index
        var resolved = catalog_find(model_ids, model_count, aliases, alias_models, alias_count, configured_model)
        if resolved >= 0:
            kind = CHOICE_CATALOG
            index = resolved
        if current_present == 1 and current_kind != -1 and catalog_choices_equal(
            kind,
            index,
            current_kind,
            current_index,
            model_ids,
            configured,
            current,
        ):
            continue
        if not catalog_choice_seen(
            kind, index, output_kinds, output_indices, output_count[], model_ids, configured, current
        ):
            if not catalog_write_choice(
                kind, index, output_kinds, output_indices, output_count, output_capacity
            ):
                return RICH_STATUS_CAPACITY
            model_choice_count += 1
    if current_present == 1 and current_kind != -1 and model_choice_count < CATALOG_MAX_CHOICES and not catalog_choice_seen(
        current_kind, current_index, output_kinds, output_indices, output_count[], model_ids, configured, current
    ):
        if not catalog_write_choice(
            current_kind, current_index, output_kinds, output_indices, output_count, output_capacity
        ):
            return RICH_STATUS_CAPACITY
    if not catalog_write_choice(
        CHOICE_CUSTOM, -1, output_kinds, output_indices, output_count, output_capacity
    ):
        return RICH_STATUS_CAPACITY
    return RICH_STATUS_OK


def catalog_merge_seen(
    query: ProdexRichStringView,
    resolved: Int64,
    model_ids: Pointer[mut=False, ProdexRichStringView, _],
    model_count: Int64,
    aliases: Pointer[mut=False, ProdexRichStringView, _],
    alias_models: Pointer[mut=False, Int64, _],
    alias_count: Int64,
    additional: Pointer[mut=False, ProdexRichStringView, _],
    accepted: Pointer[mut=False, Int64, _],
    accepted_count: Int64,
) -> Bool:
    var query_view = query.copy()
    var query_bounds = rich_trim_bounds(query_view)
    var query_start = query_bounds[0]
    var query_end = query_bounds[1]
    if resolved >= 0:
        query_start = 0
        query_end = Int64(model_ids[unsafe_offset=resolved].len)
        query_view = model_ids[unsafe_offset=resolved].copy()
    for model_index in range(model_count):
        if catalog_view_equal_range(
            query_view, query_start, query_end, model_ids[unsafe_offset=model_index], 0, Int64(model_ids[unsafe_offset=model_index].len)
        ):
            return True
    for position in range(accepted_count):
        var prior = additional[unsafe_offset=accepted[unsafe_offset=position]].copy()
        var prior_bounds = rich_trim_bounds(prior)
        var prior_resolved = catalog_find(
            model_ids, model_count, aliases, alias_models, alias_count, prior
        )
        if prior_resolved >= 0:
            prior = model_ids[unsafe_offset=prior_resolved].copy()
            prior_bounds[0] = 0
            prior_bounds[1] = Int64(prior.len)
        if catalog_view_equal_range(
            query_view, query_start, query_end, prior, prior_bounds[0], prior_bounds[1]
        ):
            return True
    return False


def catalog_plan_model_allowed(model: ProdexRichCatalogPlanModel) -> Bool:
    var bounds = rich_trim_bounds(model.id)
    return (
        bounds[1] > bounds[0]
        and (model.flags & CATALOG_PLAN_FLAG_SUPPORTED) != 0
        and (model.flags & CATALOG_PLAN_FLAG_HIDDEN) == 0
        and (model.flags & CATALOG_PLAN_FLAG_NOT_LISTED) == 0
    )


def catalog_plan_find(
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    model_count: Int64,
    aliases: Pointer[mut=False, ProdexRichStringView, _],
    query: ProdexRichStringView,
) -> Int64:
    var bounds = rich_trim_bounds(query)
    if bounds[1] <= bounds[0]:
        return -1
    for model_index in range(model_count):
        var model = models[unsafe_offset=model_index].copy()
        if not catalog_plan_model_allowed(model):
            continue
        var model_bounds = rich_trim_bounds(model.id)
        if catalog_view_equal_range(
            query,
            bounds[0],
            bounds[1],
            model.id,
            model_bounds[0],
            model_bounds[1],
        ):
            return model_index
        for alias_offset in range(model.alias_count):
            var alias_view = aliases[unsafe_offset=model.alias_start + alias_offset].copy()
            if catalog_view_equal_range(
                query,
                bounds[0],
                bounds[1],
                alias_view,
                0,
                Int64(alias_view.len),
            ):
                return model_index
    return -1


def catalog_plan_id_less_trimmed(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    var left_bounds = rich_trim_bounds(left)
    var right_bounds = rich_trim_bounds(right)
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    var shared = left_bounds[1] - left_bounds[0]
    var right_length = right_bounds[1] - right_bounds[0]
    if right_length < shared:
        shared = right_length
    for index in range(shared):
        var left_byte = left_ptr[unsafe_offset=left_bounds[0] + index]
        var right_byte = right_ptr[unsafe_offset=right_bounds[0] + index]
        if left_byte < right_byte:
            return True
        if left_byte > right_byte:
            return False
    return left_bounds[1] - left_bounds[0] < right_length


def catalog_plan_model_less(
    left: ProdexRichCatalogPlanModel,
    right: ProdexRichCatalogPlanModel,
) -> Bool:
    if left.priority != right.priority:
        return left.priority < right.priority
    return catalog_plan_id_less_trimmed(left.id, right.id)


def catalog_plan_model_matches(
    left: ProdexRichCatalogPlanModel,
    right: ProdexRichCatalogPlanModel,
) -> Bool:
    var left_bounds = rich_trim_bounds(left.id)
    var right_bounds = rich_trim_bounds(right.id)
    return catalog_view_equal_range(
        left.id,
        left_bounds[0],
        left_bounds[1],
        right.id,
        right_bounds[0],
        right_bounds[1],
    )


def catalog_plan_validate_models(
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    model_count: Int64,
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    effort_count: Int64,
    aliases: Pointer[mut=False, ProdexRichStringView, _],
    alias_count: Int64,
) -> Bool:
    for model_index in range(model_count):
        var model = models[unsafe_offset=model_index].copy()
        if (
            not rich_view_valid(model.id, CATALOG_MAX_IDENTIFIER_BYTES)
            or not rich_view_valid(model.label, CATALOG_MAX_QUERY_BYTES)
            or not rich_view_valid(model.default_effort, CATALOG_MAX_QUERY_BYTES)
            or model.flags < 0
            or model.flags > CATALOG_PLAN_FLAG_MASK
            or model.priority < 0
            or model.effort_start < 0
            or model.effort_count < 0
            or model.effort_start > effort_count
            or model.effort_count > effort_count - model.effort_start
            or model.alias_start < 0
            or model.alias_count < 0
            or model.alias_start > alias_count
            or model.alias_count > alias_count - model.alias_start
        ):
            return False
        for effort_offset in range(model.effort_count):
            if not rich_view_valid(
                efforts[unsafe_offset=model.effort_start + effort_offset],
                CATALOG_MAX_QUERY_BYTES,
            ):
                return False
        for alias_offset in range(model.alias_count):
            if not rich_view_valid(
                aliases[unsafe_offset=model.alias_start + alias_offset],
                CATALOG_MAX_IDENTIFIER_BYTES,
            ):
                return False
    return True


def catalog_plan_write_efforts(
    model: ProdexRichCatalogPlanModel,
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    output_efforts: Pointer[mut=True, ProdexRichSlice, _],
    effort_count: Pointer[mut=True, Int64, _],
    effort_capacity: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var start = effort_count[]
    for effort_offset in range(model.effort_count):
        var effort = efforts[
            unsafe_offset=model.effort_start + effort_offset
        ].copy()
        var bounds = rich_trim_bounds(effort)
        if bounds[1] <= bounds[0]:
            continue
        var duplicate = False
        for position in range(start, effort_count[]):
            if catalog_effort_output_equal(
                output,
                output_efforts[unsafe_offset=position],
                effort,
                bounds[0],
                bounds[1],
            ):
                duplicate = True
                break
        if duplicate:
            continue
        if effort_count[] >= effort_capacity:
            return False
        var copied = rich_copy_range(
            rich_view_ptr(effort),
            bounds[0],
            bounds[1],
            output,
            output_capacity,
            written,
            False,
        )
        if copied.len < 0:
            return False
        output_efforts[unsafe_offset=effort_count[]] = copied.copy()
        effort_count[] += 1
    return True


def catalog_plan_fill_choice(
    kind: Int64,
    index: Int64,
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    output_choices: Pointer[mut=True, ProdexRichCatalogPlanChoice, _],
    output_index: Int64,
    choice_capacity: Int64,
    output_efforts: Pointer[mut=True, ProdexRichSlice, _],
    effort_count: Pointer[mut=True, Int64, _],
    effort_capacity: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if output_index < 0 or output_index >= choice_capacity:
        return False
    var effort_start: Int64 = -1
    var written_efforts: Int64 = 0
    if kind == CHOICE_CATALOG:
        effort_start = effort_count[]
        if not catalog_plan_write_efforts(
            models[unsafe_offset=index].copy(),
            efforts,
            output_efforts,
            effort_count,
            effort_capacity,
            output,
            output_capacity,
            written,
        ):
            return False
        written_efforts = effort_count[] - effort_start
    output_choices[unsafe_offset=output_index].kind = kind
    output_choices[unsafe_offset=output_index].index = index
    output_choices[unsafe_offset=output_index].effort_start = effort_start
    output_choices[unsafe_offset=output_index].effort_count = written_efforts
    return True


def catalog_plan_write_choice(
    kind: Int64,
    index: Int64,
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    output_choices: Pointer[mut=True, ProdexRichCatalogPlanChoice, _],
    output_count: Pointer[mut=True, Int64, _],
    choice_capacity: Int64,
    output_efforts: Pointer[mut=True, ProdexRichSlice, _],
    effort_count: Pointer[mut=True, Int64, _],
    effort_capacity: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if output_count[] >= choice_capacity:
        return False
    if not catalog_plan_fill_choice(
        kind,
        index,
        models,
        efforts,
        output_choices,
        output_count[],
        choice_capacity,
        output_efforts,
        effort_count,
        effort_capacity,
        output,
        output_capacity,
        written,
    ):
        return False
    output_count[] += 1
    return True


def catalog_plan_copy_full(
    value: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    if value.len == 0:
        return ProdexRichSlice(written[], 0)
    return rich_copy_range(
        rich_view_ptr(value),
        0,
        Int64(value.len),
        output,
        output_capacity,
        written,
        False,
    )


def catalog_plan_copy_trimmed(
    value: ProdexRichStringView,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    var bounds = rich_trim_bounds(value)
    if bounds[1] <= bounds[0]:
        return ProdexRichSlice(-1, 0)
    return rich_copy_range(
        rich_view_ptr(value),
        bounds[0],
        bounds[1],
        output,
        output_capacity,
        written,
        False,
    )


def catalog_plan_copy_first_effort(
    model_index: Int64,
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    if model_index < 0:
        return ProdexRichSlice(-1, 0)
    var model = models[unsafe_offset=model_index].copy()
    for effort_offset in range(model.effort_count):
        var effort = efforts[unsafe_offset=model.effort_start + effort_offset].copy()
        var copied = catalog_plan_copy_trimmed(effort, output, output_capacity, written)
        if copied.len < 0:
            return copied.copy()
        if copied.offset >= 0:
            return copied.copy()
    return ProdexRichSlice(-1, 0)


def catalog_plan_copy_matching_effort_range(
    requested: ProdexRichStringView,
    source: Pointer[mut=False, ProdexRichStringView, _],
    source_start: Int64,
    source_count: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    var requested_bounds = rich_trim_bounds(requested)
    if requested_bounds[1] <= requested_bounds[0]:
        return ProdexRichSlice(-1, 0)
    for effort_offset in range(source_count):
        var effort = source[unsafe_offset=source_start + effort_offset].copy()
        var bounds = rich_trim_bounds(effort)
        if bounds[1] > bounds[0] and catalog_view_equal_range(
            requested,
            requested_bounds[0],
            requested_bounds[1],
            effort,
            bounds[0],
            bounds[1],
        ):
            var copied = rich_copy_range(
                rich_view_ptr(effort),
                bounds[0],
                bounds[1],
                output,
                output_capacity,
                written,
                False,
            )
            return copied.copy()
    return ProdexRichSlice(-1, 0)


def catalog_plan_copy_matching_effort(
    requested: ProdexRichStringView,
    model_index: Int64,
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    fallback_efforts: Pointer[mut=False, ProdexRichStringView, _],
    fallback_effort_count: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    if model_index >= 0:
        var model = models[unsafe_offset=model_index].copy()
        return catalog_plan_copy_matching_effort_range(
            requested,
            efforts,
            model.effort_start,
            model.effort_count,
            output,
            output_capacity,
            written,
        )
    return catalog_plan_copy_matching_effort_range(
        requested,
        fallback_efforts,
        0,
        fallback_effort_count,
        output,
        output_capacity,
        written,
    )


def catalog_plan_copy_default_effort(
    model_index: Int64,
    models: Pointer[mut=False, ProdexRichCatalogPlanModel, _],
    efforts: Pointer[mut=False, ProdexRichStringView, _],
    fallback_efforts: Pointer[mut=False, ProdexRichStringView, _],
    fallback_effort_count: Int64,
    output: Pointer[mut=True, UInt8, _],
    output_capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> ProdexRichSlice:
    if model_index >= 0:
        var model = models[unsafe_offset=model_index].copy()
        var copied = catalog_plan_copy_trimmed(
            model.default_effort, output, output_capacity, written
        )
        if copied.len < 0:
            return copied.copy()
        if copied.offset >= 0:
            return copied.copy()
        return catalog_plan_copy_first_effort(
            model_index,
            models,
            efforts,
            output,
            output_capacity,
            written,
        )
    if fallback_effort_count > 0:
        for effort_offset in range(fallback_effort_count):
            var copied = catalog_plan_copy_trimmed(
                fallback_efforts[unsafe_offset=effort_offset],
                output,
                output_capacity,
                written,
            )
            if copied.len < 0:
                return copied.copy()
            if copied.offset >= 0:
                return copied.copy()
    return ProdexRichSlice(-1, 0)


@export("prodex_mojo_rich_catalog_config_v1")
def prodex_mojo_rich_catalog_config_v1(
    abi_version: Int64,
    role: Int64,
    models_address: UInt,
    model_count: Int64,
    efforts_address: UInt,
    effort_count: Int64,
    aliases_address: UInt,
    alias_count: Int64,
    current_address: UInt,
    current_present: Int64,
    provider_default_address: UInt,
    provider_default_present: Int64,
    catalog_default_address: UInt,
    catalog_default_present: Int64,
    explicit_model_address: UInt,
    explicit_model_present: Int64,
    remembered_model_address: UInt,
    remembered_model_present: Int64,
    explicit_effort_address: UInt,
    explicit_effort_present: Int64,
    remembered_effort_address: UInt,
    remembered_effort_present: Int64,
    fallback_efforts_address: UInt,
    fallback_effort_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[
        mut=True, ProdexRichCatalogPlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].choices_written = 0
    result_ptr[].required_choices = 0
    result_ptr[].efforts_written = 0
    result_ptr[].required_efforts = 0
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].selected_model = ProdexRichSlice(-1, 0)
    result_ptr[].selected_effort = ProdexRichSlice(-1, 0)
    result_ptr[].default_effort = ProdexRichSlice(-1, 0)
    result_ptr[].issue_kind = 0
    result_ptr[].issue_index = -1
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result_ptr[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if (
        (role != CATALOG_PLAN_ROLE_MAIN and role != CATALOG_PLAN_ROLE_SUB_AGENT)
        or model_count < 0
        or model_count > CATALOG_MAX_MODELS
        or effort_count < 0
        or effort_count > CATALOG_MAX_INPUT_MODELS
        or alias_count < 0
        or alias_count > CATALOG_MAX_INPUT_MODELS
        or current_present < 0
        or current_present > 1
        or provider_default_present < 0
        or provider_default_present > 1
        or catalog_default_present < 0
        or catalog_default_present > 1
        or explicit_model_present < 0
        or explicit_model_present > 1
        or remembered_model_present < 0
        or remembered_model_present > 1
        or explicit_effort_present < 0
        or explicit_effort_present > 1
        or remembered_effort_present < 0
        or remembered_effort_present > 1
        or fallback_effort_count < 0
        or fallback_effort_count > CATALOG_MAX_INPUT_MODELS
        or output_capacity < 0
    ):
        return RICH_STATUS_INVALID
    if model_count > 0 and models_address == 0:
        return RICH_STATUS_INVALID
    if effort_count > 0 and efforts_address == 0:
        return RICH_STATUS_INVALID
    if alias_count > 0 and aliases_address == 0:
        return RICH_STATUS_INVALID
    if fallback_effort_count > 0 and fallback_efforts_address == 0:
        return RICH_STATUS_INVALID
    if output_capacity > 0 and output_address == 0:
        return RICH_STATUS_INVALID
    if current_present == 1 and current_address == 0 or provider_default_present == 1 and provider_default_address == 0 or catalog_default_present == 1 and catalog_default_address == 0 or explicit_model_present == 1 and explicit_model_address == 0 or remembered_model_present == 1 and remembered_model_address == 0 or explicit_effort_present == 1 and explicit_effort_address == 0 or remembered_effort_present == 1 and remembered_effort_address == 0:
        return RICH_STATUS_INVALID

    var models = Pointer[
        mut=False, ProdexRichCatalogPlanModel, ImmUntrackedOrigin
    ](unsafe_from_address=Int(models_address))
    var efforts = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(efforts_address))
    var aliases = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(aliases_address))
    if not catalog_plan_validate_models(
        models, model_count, efforts, effort_count, aliases, alias_count
    ):
        return RICH_STATUS_UTF8
    var current = ProdexRichStringView(0, 0)
    if current_present == 1:
        current = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(current_address))[].copy()
        if not rich_view_valid(current, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var provider_default = ProdexRichStringView(0, 0)
    if provider_default_present == 1:
        provider_default = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(provider_default_address))[].copy()
        if not rich_view_valid(provider_default, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var catalog_default = ProdexRichStringView(0, 0)
    if catalog_default_present == 1:
        catalog_default = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(catalog_default_address))[].copy()
        if not rich_view_valid(catalog_default, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var explicit_model = ProdexRichStringView(0, 0)
    if explicit_model_present == 1:
        explicit_model = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(explicit_model_address))[].copy()
        if not rich_view_valid(explicit_model, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var remembered_model = ProdexRichStringView(0, 0)
    if remembered_model_present == 1:
        remembered_model = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(remembered_model_address))[].copy()
        if not rich_view_valid(remembered_model, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var explicit_effort = ProdexRichStringView(0, 0)
    if explicit_effort_present == 1:
        explicit_effort = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(explicit_effort_address))[].copy()
        if not rich_view_valid(explicit_effort, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var remembered_effort = ProdexRichStringView(0, 0)
    if remembered_effort_present == 1:
        remembered_effort = Pointer[
            mut=False, ProdexRichStringView, ImmUntrackedOrigin
        ](unsafe_from_address=Int(remembered_effort_address))[].copy()
        if not rich_view_valid(remembered_effort, CATALOG_PLAN_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var fallback_efforts = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(fallback_efforts_address))
    for effort_offset in range(fallback_effort_count):
        if not rich_view_valid(
            fallback_efforts[unsafe_offset=effort_offset],
            CATALOG_PLAN_MAX_QUERY_BYTES,
        ):
            return RICH_STATUS_UTF8

    var selected = ProdexRichStringView(0, 0)
    var selected_present = False
    var selected_index: Int64 = -1
    if explicit_model_present == 1:
        selected = explicit_model.copy()
        selected_index = catalog_plan_find(
            models,
            model_count,
            aliases,
            selected,
        )
        selected_present = True
        if role == CATALOG_PLAN_ROLE_SUB_AGENT and selected_index >= 0:
            selected = models[unsafe_offset=selected_index].id.copy()
        else:
            selected = explicit_model.copy()
    elif role == CATALOG_PLAN_ROLE_MAIN:
        if remembered_model_present == 1:
            var remembered_index = catalog_plan_find(
                models,
                model_count,
                aliases,
                remembered_model,
            )
            if remembered_index >= 0:
                selected = remembered_model.copy()
                selected_present = True
                selected_index = remembered_index
        if not selected_present and current_present == 1:
            var current_index = catalog_plan_find(
                models,
                model_count,
                aliases,
                current,
            )
            if current_index >= 0:
                selected = current.copy()
                selected_present = True
                selected_index = current_index
        if not selected_present and catalog_default_present == 1:
            selected = catalog_default.copy()
            selected_present = True
            selected_index = catalog_plan_find(
                models,
                model_count,
                aliases,
                selected,
            )
        if not selected_present and provider_default_present == 1:
            selected = provider_default.copy()
            selected_present = True
            selected_index = catalog_plan_find(
                models,
                model_count,
                aliases,
                selected,
            )
    var written: Int64 = 0
    if selected_present:
        var copied_model = catalog_plan_copy_full(
            selected, Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)), output_capacity, Pointer(to=written)
        )
        if copied_model.len < 0:
            return RICH_STATUS_CAPACITY
        result_ptr[].selected_model = copied_model.copy()
    var effort_model_index = selected_index
    if role == CATALOG_PLAN_ROLE_SUB_AGENT and effort_model_index < 0 and provider_default_present == 1:
        effort_model_index = catalog_plan_find(
            models,
            model_count,
            aliases,
            provider_default,
        )
    var default_effort = catalog_plan_copy_default_effort(
        effort_model_index,
        models,
        efforts,
        fallback_efforts,
        fallback_effort_count,
        Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
        output_capacity,
        Pointer(to=written),
    )
    if default_effort.len < 0:
        return RICH_STATUS_CAPACITY
    result_ptr[].default_effort = default_effort.copy()
    var selected_effort = ProdexRichSlice(-1, 0)
    if explicit_effort_present == 1:
        selected_effort = catalog_plan_copy_matching_effort(
            explicit_effort,
            effort_model_index,
            models,
            efforts,
            fallback_efforts,
            fallback_effort_count,
            Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
            output_capacity,
            Pointer(to=written),
        )
        if selected_effort.offset < 0:
            if selected_effort.len < 0:
                return RICH_STATUS_CAPACITY
            var bounds = rich_trim_bounds(explicit_effort)
            result_ptr[].issue_kind = RICH_ISSUE_EFFORT
            result_ptr[].issue_index = effort_model_index
            result_ptr[].issue_offset = bounds[0]
            result_ptr[].issue_length = bounds[1] - bounds[0]
            result_ptr[].output_written = written
            result_ptr[].required_output = written
            return RICH_STATUS_OK
    elif role == CATALOG_PLAN_ROLE_MAIN and remembered_effort_present == 1 and remembered_model_present == 1 and selected_present:
        var selected_bounds = rich_trim_bounds(selected)
        var remembered_bounds = rich_trim_bounds(remembered_model)
        if catalog_view_equal_range(
            selected,
            selected_bounds[0],
            selected_bounds[1],
            remembered_model,
            remembered_bounds[0],
            remembered_bounds[1],
        ):
            selected_effort = catalog_plan_copy_matching_effort(
                remembered_effort,
                effort_model_index,
                models,
                efforts,
                fallback_efforts,
                fallback_effort_count,
                Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address)),
                output_capacity,
                Pointer(to=written),
            )
            if selected_effort.len < 0:
                return RICH_STATUS_CAPACITY
    if selected_effort.offset < 0:
        selected_effort = default_effort.copy()
    result_ptr[].selected_effort = selected_effort.copy()
    result_ptr[].output_written = written
    result_ptr[].required_output = written
    return RICH_STATUS_OK


@export("prodex_mojo_rich_catalog_choices_v2")
def prodex_mojo_rich_catalog_choices_v2(
    abi_version: Int64,
    models_address: UInt,
    model_count: Int64,
    efforts_address: UInt,
    effort_count: Int64,
    aliases_address: UInt,
    alias_count: Int64,
    output_choices_address: UInt,
    output_ids_address: UInt,
    output_labels_address: UInt,
    choice_capacity: Int64,
    output_efforts_address: UInt,
    effort_capacity: Int64,
    output_address: UInt,
    output_capacity: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if result_address == 0:
        return RICH_STATUS_INVALID
    var result_ptr = Pointer[
        mut=True, ProdexRichCatalogPlanResult, MutUntrackedOrigin
    ](unsafe_from_address=Int(result_address))
    result_ptr[].abi_version = PRODEX_RICH_ABI_VERSION
    result_ptr[].choices_written = 0
    result_ptr[].required_choices = 2
    result_ptr[].efforts_written = 0
    result_ptr[].required_efforts = 0
    result_ptr[].output_written = 0
    result_ptr[].required_output = 0
    result_ptr[].selected_model = ProdexRichSlice(-1, 0)
    result_ptr[].selected_effort = ProdexRichSlice(-1, 0)
    result_ptr[].default_effort = ProdexRichSlice(-1, 0)
    result_ptr[].issue_kind = 0
    result_ptr[].issue_index = -1
    result_ptr[].issue_offset = -1
    result_ptr[].issue_length = 0
    if abi_version != PRODEX_RICH_ABI_VERSION:
        result_ptr[].issue_kind = RICH_STATUS_ABI
        return RICH_STATUS_ABI
    if model_count < 0 or model_count > CATALOG_MAX_MODELS or effort_count < 0 or effort_count > CATALOG_MAX_INPUT_MODELS or alias_count < 0 or alias_count > CATALOG_MAX_INPUT_MODELS or choice_capacity < 2 or effort_capacity < 0 or output_capacity < 0:
        return RICH_STATUS_INVALID
    if model_count > 0 and models_address == 0:
        return RICH_STATUS_INVALID
    if effort_count > 0 and efforts_address == 0:
        return RICH_STATUS_INVALID
    if alias_count > 0 and aliases_address == 0:
        return RICH_STATUS_INVALID
    if output_choices_address == 0 or output_ids_address == 0 or output_labels_address == 0 or effort_capacity > 0 and output_efforts_address == 0 or output_capacity > 0 and output_address == 0:
        return RICH_STATUS_INVALID

    var models = Pointer[
        mut=False, ProdexRichCatalogPlanModel, ImmUntrackedOrigin
    ](unsafe_from_address=Int(models_address))
    var efforts = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(efforts_address))
    var aliases = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(aliases_address))
    if not catalog_plan_validate_models(
        models, model_count, efforts, effort_count, aliases, alias_count
    ):
        return RICH_STATUS_UTF8
    var output_choices = Pointer[
        mut=True, ProdexRichCatalogPlanChoice, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_choices_address))
    var output_ids = Pointer[
        mut=True, ProdexRichSlice, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_ids_address))
    var output_labels = Pointer[
        mut=True, ProdexRichSlice, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_labels_address))
    var output_efforts = Pointer[
        mut=True, ProdexRichSlice, MutUntrackedOrigin
    ](unsafe_from_address=Int(output_efforts_address))
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var unique_count: Int64 = 0
    for model_index in range(model_count):
        var candidate = models[unsafe_offset=model_index].copy()
        if not catalog_plan_model_allowed(candidate):
            continue
        var duplicate = False
        for prior_index in range(model_index):
            var prior = models[unsafe_offset=prior_index].copy()
            if catalog_plan_model_matches(
                candidate,
                prior,
            ):
                if catalog_plan_model_allowed(prior):
                    duplicate = True
                    break
        if duplicate:
            continue
        unique_count += 1
    result_ptr[].required_choices = unique_count + 2
    if choice_capacity < result_ptr[].required_choices:
        return RICH_STATUS_CAPACITY

    # ponytail: bounded selection scan (<=1,024 models) keeps scratch allocation-free; add an
    # index only if catalog size makes O(n^2) selection measurable.
    for position in range(unique_count):
        var best_index: Int64 = -1
        for candidate_index in range(model_count):
            var candidate = models[unsafe_offset=candidate_index].copy()
            if not catalog_plan_model_allowed(candidate):
                continue
            var already_written = False
            for prior_position in range(position):
                var prior_index = output_choices[
                    unsafe_offset=prior_position + 1
                ].index
                if catalog_plan_model_matches(
                    candidate, models[unsafe_offset=prior_index].copy()
                ):
                    already_written = True
                    break
            if already_written:
                continue
            if best_index < 0 or catalog_plan_model_less(
                candidate,
                models[unsafe_offset=best_index].copy(),
            ):
                best_index = candidate_index
        if best_index < 0:
            return RICH_STATUS_INVALID
        output_choices[unsafe_offset=position + 1].kind = CHOICE_CATALOG
        output_choices[unsafe_offset=position + 1].index = best_index
        output_choices[unsafe_offset=position + 1].effort_start = -1
        output_choices[unsafe_offset=position + 1].effort_count = 0

    var written: Int64 = 0
    var choices_written: Int64 = 0
    var efforts_written: Int64 = 0
    if not catalog_plan_write_choice(
        CHOICE_PROVIDER_DEFAULT,
        -1,
        models,
        efforts,
        output_choices,
        Pointer(to=choices_written),
        choice_capacity,
        output_efforts,
        Pointer(to=efforts_written),
        effort_capacity,
        output,
        output_capacity,
        Pointer(to=written),
    ):
        return RICH_STATUS_CAPACITY
    output_ids[unsafe_offset=0] = ProdexRichSlice(-1, 0)
    output_labels[unsafe_offset=0] = ProdexRichSlice(-1, 0)
    for position in range(unique_count):
        var model_index = output_choices[unsafe_offset=position + 1].index
        if not catalog_plan_write_choice(
            CHOICE_CATALOG,
            model_index,
            models,
            efforts,
            output_choices,
            Pointer(to=choices_written),
            choice_capacity,
            output_efforts,
            Pointer(to=efforts_written),
            effort_capacity,
            output,
            output_capacity,
            Pointer(to=written),
        ):
            return RICH_STATUS_CAPACITY
        var copied_id = catalog_plan_copy_trimmed(
            models[unsafe_offset=model_index].id,
            output,
            output_capacity,
            Pointer(to=written),
        )
        var copied_label = catalog_plan_copy_trimmed(
            models[unsafe_offset=model_index].label,
            output,
            output_capacity,
            Pointer(to=written),
        )
        if copied_id.len < 0 or copied_label.len < 0:
            return RICH_STATUS_CAPACITY
        output_ids[unsafe_offset=position + 1] = copied_id.copy()
        output_labels[unsafe_offset=position + 1] = copied_label.copy()
    output_ids[unsafe_offset=unique_count + 1] = ProdexRichSlice(-1, 0)
    output_labels[unsafe_offset=unique_count + 1] = ProdexRichSlice(-1, 0)
    if not catalog_plan_write_choice(
        CHOICE_CUSTOM,
        -1,
        models,
        efforts,
        output_choices,
        Pointer(to=choices_written),
        choice_capacity,
        output_efforts,
        Pointer(to=efforts_written),
        effort_capacity,
        output,
        output_capacity,
        Pointer(to=written),
    ):
        return RICH_STATUS_CAPACITY
    result_ptr[].choices_written = choices_written
    result_ptr[].efforts_written = efforts_written
    result_ptr[].output_written = written
    result_ptr[].required_efforts = efforts_written
    result_ptr[].required_output = written
    return RICH_STATUS_OK


@export("prodex_mojo_rich_catalog_merge_v1")
def prodex_mojo_rich_catalog_merge_v1(
    abi_version: Int64,
    model_ids_address: UInt,
    model_count: Int64,
    aliases_address: UInt,
    alias_models_address: UInt,
    alias_count: Int64,
    additional_address: UInt,
    additional_count: Int64,
    accepted_indices_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != PRODEX_RICH_ABI_VERSION:
        return RICH_STATUS_ABI
    if additional_count < 0 or additional_count > CATALOG_MAX_INPUT_MODELS or output_capacity < 0 or output_count_address == 0 or not catalog_prepare(
        model_ids_address, model_count, aliases_address, alias_models_address, alias_count
    ):
        return RICH_STATUS_INVALID
    if additional_count > 0 and (additional_address == 0 or accepted_indices_address == 0):
        return RICH_STATUS_INVALID
    var model_ids = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(model_ids_address)
    )
    var aliases = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(aliases_address)
    )
    var alias_models = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(alias_models_address)
    )
    var additional = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(additional_address)
    )
    if not catalog_valid_views(model_ids, model_count, aliases, alias_models, alias_count):
        return RICH_STATUS_UTF8
    for additional_index in range(additional_count):
        if not rich_view_valid(additional[unsafe_offset=additional_index], CATALOG_MAX_QUERY_BYTES):
            return RICH_STATUS_UTF8
    var accepted = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(accepted_indices_address)
    )
    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    output_count[] = 0
    for additional_index in range(additional_count):
        var query = additional[unsafe_offset=additional_index].copy()
        var bounds = rich_trim_bounds(query)
        if bounds[1] <= bounds[0]:
            continue
        var resolved = catalog_find(model_ids, model_count, aliases, alias_models, alias_count, query)
        if not catalog_merge_seen(
            query,
            resolved,
            model_ids,
            model_count,
            aliases,
            alias_models,
            alias_count,
            additional,
            accepted,
            output_count[],
        ):
            if output_count[] >= output_capacity:
                return RICH_STATUS_CAPACITY
            accepted[unsafe_offset=output_count[]] = additional_index
            output_count[] += 1
    return RICH_STATUS_OK
