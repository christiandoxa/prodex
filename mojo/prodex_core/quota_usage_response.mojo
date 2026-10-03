from std.memory import Pointer

from quota import INT64_MAX, quota_model_policy_ascii_lower
from rich_text import rich_trim_bounds, rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime QUOTA_USAGE_RESPONSE_ABI_VERSION: Int64 = 1
comptime QUOTA_USAGE_RESPONSE_HEADER_FIELDS: Int64 = 10
comptime QUOTA_USAGE_RESPONSE_ENTRY_FIELDS: Int64 = 10
comptime QUOTA_USAGE_ADMISSION_MISSING: Int64 = 0
comptime QUOTA_USAGE_ADMISSION_NULL: Int64 = 1
comptime QUOTA_USAGE_ADMISSION_TRUE: Int64 = 2
comptime QUOTA_USAGE_ADMISSION_FALSE: Int64 = 3
comptime QUOTA_USAGE_ADMISSION_OTHER: Int64 = 4


def quota_usage_response_alias_views_valid(
    values: Pointer[mut=False, ProdexRichStringView, _], count: Int64
) -> Bool:
    for index in range(count):
        if not rich_view_valid(values[unsafe_offset=index], INT64_MAX):
            return False
    return True


def quota_usage_response_alias_plan(
    snake_case: ProdexRichStringView,
    camel_case: ProdexRichStringView,
) -> Tuple[Int64, Int64, Int64]:
    var bounds = rich_trim_bounds(snake_case)
    if bounds[0] < bounds[1]:
        return (1, bounds[0], bounds[1])
    bounds = rich_trim_bounds(camel_case)
    if bounds[0] < bounds[1]:
        return (2, bounds[0], bounds[1])
    return (0, 0, 0)


def quota_usage_response_range_matches(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    literal: StringSlice,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    if end - start != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var target = literal.unsafe_ptr()
    for index in range(end - start):
        if quota_model_policy_ascii_lower(source[unsafe_offset=start + index]) != target[unsafe_offset=index]:
            return False
    return True


def quota_usage_response_text_equal(
    left: ProdexRichStringView, right: ProdexRichStringView
) -> Bool:
    if left.len != right.len:
        return False
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    for index in range(Int64(left.len)):
        if quota_model_policy_ascii_lower(left_ptr[unsafe_offset=index]) != quota_model_policy_ascii_lower(right_ptr[unsafe_offset=index]):
            return False
    return True


def quota_usage_response_write_text_plan(
    output: Pointer[mut=True, Int64, _],
    offset: Int64,
    source: Int64,
    start: Int64,
    end: Int64,
):
    output[unsafe_offset=offset] = source
    output[unsafe_offset=offset + 1] = start
    output[unsafe_offset=offset + 2] = end


@export("prodex_quota_usage_response_plan_v1")
def prodex_quota_usage_response_plan_v1(
    abi_version: Int64,
    indexed_count: Int64,
    indexed_keys: Pointer[mut=False, ProdexRichStringView, _],
    indexed_limit_ids: Pointer[mut=False, ProdexRichStringView, _],
    indexed_plan_types: Pointer[mut=False, ProdexRichStringView, _],
    indexed_limit_names: Pointer[mut=False, ProdexRichStringView, _],
    indexed_metered_features: Pointer[mut=False, ProdexRichStringView, _],
    existing_count: Int64,
    existing_limit_ids: Pointer[mut=False, ProdexRichStringView, _],
    existing_id_present: Pointer[mut=False, Int64, _],
    rate_limit_plan_types: Pointer[mut=False, ProdexRichStringView, _],
    rate_limits_plan_types: Pointer[mut=False, ProdexRichStringView, _],
    rate_limit_present: Int64,
    rate_limits_present: Int64,
    input_plan_type: ProdexRichStringView,
    input_plan_type_present: Int64,
    admission_kinds: Pointer[mut=False, Int64, _],
    metadata_present: Pointer[mut=False, Int64, _],
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != QUOTA_USAGE_RESPONSE_ABI_VERSION:
        return 4
    if (
        indexed_count < 0
        or existing_count < 0
        or output_capacity < 0
        or indexed_count > (INT64_MAX - QUOTA_USAGE_RESPONSE_HEADER_FIELDS) // QUOTA_USAGE_RESPONSE_ENTRY_FIELDS
        or (rate_limit_present != 0 and rate_limit_present != 1)
        or (rate_limits_present != 0 and rate_limits_present != 1)
        or (input_plan_type_present != 0 and input_plan_type_present != 1)
        or output_address == 0
    ):
        return 1

    var alias_count = indexed_count * 2
    var required_output = QUOTA_USAGE_RESPONSE_HEADER_FIELDS + indexed_count * QUOTA_USAGE_RESPONSE_ENTRY_FIELDS
    if output_capacity < required_output:
        return 2
    if (
        not quota_usage_response_alias_views_valid(indexed_keys, indexed_count)
        or not quota_usage_response_alias_views_valid(indexed_limit_ids, alias_count)
        or not quota_usage_response_alias_views_valid(indexed_plan_types, alias_count)
        or not quota_usage_response_alias_views_valid(indexed_limit_names, alias_count)
        or not quota_usage_response_alias_views_valid(indexed_metered_features, alias_count)
        or not quota_usage_response_alias_views_valid(existing_limit_ids, existing_count)
        or not quota_usage_response_alias_views_valid(rate_limit_plan_types, 2)
        or not quota_usage_response_alias_views_valid(rate_limits_plan_types, 2)
        or not rich_view_valid(input_plan_type, INT64_MAX)
    ):
        return 1
    if (
        admission_kinds[unsafe_offset=0] < QUOTA_USAGE_ADMISSION_MISSING
        or admission_kinds[unsafe_offset=0] > QUOTA_USAGE_ADMISSION_OTHER
        or admission_kinds[unsafe_offset=1] < QUOTA_USAGE_ADMISSION_MISSING
        or admission_kinds[unsafe_offset=1] > QUOTA_USAGE_ADMISSION_OTHER
        or admission_kinds[unsafe_offset=2] < QUOTA_USAGE_ADMISSION_MISSING
        or admission_kinds[unsafe_offset=2] > QUOTA_USAGE_ADMISSION_OTHER
    ):
        return 1
    for index in range(4):
        var present = metadata_present[unsafe_offset=index]
        if present != 0 and present != 1:
            return 1
    for index in range(existing_count):
        var present = existing_id_present[unsafe_offset=index]
        if present != 0 and present != 1:
            return 1

    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var main_source: Int64 = -1
    for index in range(indexed_count):
        var key = indexed_keys[unsafe_offset=index].copy()
        var id_snake = indexed_limit_ids[unsafe_offset=index * 2].copy()
        var id_camel = indexed_limit_ids[unsafe_offset=index * 2 + 1].copy()
        var id_plan = quota_usage_response_alias_plan(id_snake, id_camel)
        if quota_usage_response_range_matches(key, 0, Int64(key.len), StringSlice("codex")) or (
            id_plan[0] == 1 and quota_usage_response_range_matches(id_snake, id_plan[1], id_plan[2], StringSlice("codex"))
        ) or (
            id_plan[0] == 2 and quota_usage_response_range_matches(id_camel, id_plan[1], id_plan[2], StringSlice("codex"))
        ):
            main_source = index
            break
    if main_source < 0:
        if rate_limit_present == 1:
            main_source = indexed_count
        elif rate_limits_present == 1:
            main_source = indexed_count + 1
    result[unsafe_offset=0] = main_source

    var ordinary_kind = admission_kinds[unsafe_offset=1]
    if ordinary_kind == QUOTA_USAGE_ADMISSION_MISSING:
        ordinary_kind = admission_kinds[unsafe_offset=0]
    result[unsafe_offset=1] = Int64(
        ordinary_kind == QUOTA_USAGE_ADMISSION_FALSE
        or admission_kinds[unsafe_offset=2] != QUOTA_USAGE_ADMISSION_MISSING
            and admission_kinds[unsafe_offset=2] != QUOTA_USAGE_ADMISSION_NULL
    )
    result[unsafe_offset=2] = Int64(
        ordinary_kind == QUOTA_USAGE_ADMISSION_NULL
        or ordinary_kind == QUOTA_USAGE_ADMISSION_OTHER
    )
    var ordinary_source: Int64 = 0
    if admission_kinds[unsafe_offset=1] != QUOTA_USAGE_ADMISSION_MISSING:
        ordinary_source = 2
    elif admission_kinds[unsafe_offset=0] != QUOTA_USAGE_ADMISSION_MISSING:
        ordinary_source = 1
    result[unsafe_offset=3] = ordinary_source

    var upsell_source: Int64 = 0
    if metadata_present[unsafe_offset=1] == 1:
        upsell_source = 2
    elif metadata_present[unsafe_offset=0] == 1:
        upsell_source = 1
    result[unsafe_offset=4] = upsell_source
    var account_source: Int64 = 0
    if metadata_present[unsafe_offset=3] == 1:
        account_source = 2
    elif metadata_present[unsafe_offset=2] == 1:
        account_source = 1
    result[unsafe_offset=5] = account_source

    result[unsafe_offset=6] = input_plan_type_present
    var plan_type_source: Int64 = 0
    var plan_type_start: Int64 = 0
    var plan_type_end: Int64 = 0
    if input_plan_type_present == 0 and main_source >= 0:
        var plan: Tuple[Int64, Int64, Int64] = (0, 0, 0)
        if main_source < indexed_count:
            plan = quota_usage_response_alias_plan(
                indexed_plan_types[unsafe_offset=main_source * 2].copy(),
                indexed_plan_types[unsafe_offset=main_source * 2 + 1].copy(),
            )
        elif main_source == indexed_count:
            plan = quota_usage_response_alias_plan(
                rate_limit_plan_types[unsafe_offset=0].copy(),
                rate_limit_plan_types[unsafe_offset=1].copy(),
            )
        else:
            plan = quota_usage_response_alias_plan(
                rate_limits_plan_types[unsafe_offset=0].copy(),
                rate_limits_plan_types[unsafe_offset=1].copy(),
            )
        plan_type_source = plan[0]
        plan_type_start = plan[1]
        plan_type_end = plan[2]
    result[unsafe_offset=7] = plan_type_source
    result[unsafe_offset=8] = plan_type_start
    result[unsafe_offset=9] = plan_type_end

    for index in range(indexed_count):
        var offset = QUOTA_USAGE_RESPONSE_HEADER_FIELDS + index * QUOTA_USAGE_RESPONSE_ENTRY_FIELDS
        var key = indexed_keys[unsafe_offset=index].copy()
        var id_snake = indexed_limit_ids[unsafe_offset=index * 2].copy()
        var id_camel = indexed_limit_ids[unsafe_offset=index * 2 + 1].copy()
        var id_plan = quota_usage_response_alias_plan(id_snake, id_camel)
        var is_codex = quota_usage_response_range_matches(key, 0, Int64(key.len), StringSlice("codex")) or (
            id_plan[0] == 1 and quota_usage_response_range_matches(id_snake, id_plan[1], id_plan[2], StringSlice("codex"))
        ) or (
            id_plan[0] == 2 and quota_usage_response_range_matches(id_camel, id_plan[1], id_plan[2], StringSlice("codex"))
        )
        var duplicate = False
        for existing_index in range(existing_count):
            if existing_id_present[unsafe_offset=existing_index] == 1 and quota_usage_response_text_equal(
                key, existing_limit_ids[unsafe_offset=existing_index]
            ):
                duplicate = True
                break
        result[unsafe_offset=offset] = Int64(not is_codex and not duplicate)

        var id_source = id_plan[0] + 1
        var id_start = id_plan[1]
        var id_end = id_plan[2]
        if id_plan[0] == 0:
            var key_bounds = rich_trim_bounds(key)
            if key_bounds[0] < key_bounds[1]:
                id_source = 1
                id_start = 0
                id_end = Int64(key.len)
            else:
                id_source = 0
        quota_usage_response_write_text_plan(
            result, offset + 1, id_source, id_start, id_end
        )

        var name_plan = quota_usage_response_alias_plan(
            indexed_limit_names[unsafe_offset=index * 2].copy(),
            indexed_limit_names[unsafe_offset=index * 2 + 1].copy(),
        )
        var name_source = name_plan[0]
        if name_source > 0:
            name_source += 1
        quota_usage_response_write_text_plan(
            result, offset + 4, name_source, name_plan[1], name_plan[2]
        )
        var metered_plan = quota_usage_response_alias_plan(
            indexed_metered_features[unsafe_offset=index * 2].copy(),
            indexed_metered_features[unsafe_offset=index * 2 + 1].copy(),
        )
        var metered_source = metered_plan[0]
        if metered_source > 0:
            metered_source += 1
        quota_usage_response_write_text_plan(
            result, offset + 7, metered_source, metered_plan[1], metered_plan[2]
        )
    return 0
