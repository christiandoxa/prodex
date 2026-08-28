from std.memory import Pointer
from rich_text import (
    rich_trim_bounds,
    rich_view_ptr,
    rich_view_valid,
)
from rich_types import ProdexRichStringView
comptime PRODEX_RICH_ABI_VERSION: Int64 = 5
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

comptime CHOICE_PROVIDER_DEFAULT: Int64 = 0
comptime CHOICE_CATALOG: Int64 = 1
comptime CHOICE_CONFIGURED: Int64 = 2
comptime CHOICE_CURRENT: Int64 = 3
comptime CHOICE_CUSTOM: Int64 = 4


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
