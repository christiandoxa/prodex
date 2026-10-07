from std.memory import Pointer

from json_view import (
    deepseek_json_byte,
    deepseek_json_fragment_valid,
    deepseek_json_object_member,
    deepseek_json_skip_ws,
    deepseek_json_string_end,
    deepseek_json_value_end,
)

from rich_text import rich_codepoint_width, rich_trim_bounds, rich_view_prefix, rich_view_valid
from rich_types import ProdexRichStringView, rich_view_ptr

comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808


@export("prodex_quota_round_f64")
def prodex_quota_round_f64(value: Float64) abi("C") -> Int64:
    if value != value:
        return 0
    if value >= 9223372036854775808.0:
        return INT64_MAX
    if value <= -9223372036854775808.0:
        return INT64_MIN
    if value >= 0.0:
        return Int64(value + 0.5)
    return Int64(value - 0.5)


@export("prodex_quota_remaining_percent")
def prodex_quota_remaining_percent(
    used_percent: Int64,
    has_value: Int64,
) abi("C") -> Int64:
    if has_value == 0:
        return 0
    if used_percent < 0:
        return 100
    if used_percent > 100:
        return 0
    return 100 - used_percent


@export("prodex_quota_window_status")
def prodex_quota_window_status(
    remaining_percent: Int64,
    has_window: Int64,
) abi("C") -> Int64:
    if has_window == 0:
        return 4
    if remaining_percent == 0:
        return 3
    if remaining_percent <= 5:
        return 2
    if remaining_percent <= 15:
        return 1
    return 0


def prodex_quota_pressure_band_for_status(status: Int64) -> Int64:
    if status == 0:
        return 0
    if status == 1:
        return 1
    if status == 2:
        return 2
    if status == 3:
        return 3
    return 4


@export("prodex_quota_pressure_band")
def prodex_quota_pressure_band(
    five_hour_status: Int64,
    weekly_status: Int64,
) abi("C") -> Int64:
    var five_hour_band = prodex_quota_pressure_band_for_status(five_hour_status)
    var weekly_band = prodex_quota_pressure_band_for_status(weekly_status)
    if five_hour_band > weekly_band:
        return five_hour_band
    return weekly_band


comptime QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT: Int64 = 1_024


@export("prodex_quota_gemini_bucket_batch")
def prodex_quota_gemini_bucket_batch(
    remaining_amount: Pointer[mut=False, Int64, _],
    remaining_amount_state: Pointer[mut=False, Int64, _],
    remaining_fraction: Pointer[mut=False, Float64, _],
    remaining_fraction_present: Pointer[mut=False, Int64, _],
    remaining: Pointer[mut=True, Int64, _],
    remaining_present: Pointer[mut=True, Int64, _],
    total: Pointer[mut=True, Int64, _],
    total_present: Pointer[mut=True, Int64, _],
    remaining_percent: Pointer[mut=True, Int64, _],
    remaining_percent_present: Pointer[mut=True, Int64, _],
    exhausted: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT:
        return 1

    for index in range(count):
        var amount_state = remaining_amount_state[unsafe_offset=index]
        var has_fraction = remaining_fraction_present[unsafe_offset=index]
        if amount_state < 0 or amount_state > 2:
            return 2
        if has_fraction != 0 and has_fraction != 1:
            return 2

        var has_remaining: Int64 = 0
        var remaining_value: Int64 = 0
        var has_total: Int64 = 0
        var total_value: Int64 = 0
        var has_percent: Int64 = 0
        var percent_value: Int64 = 0
        var exhausted_value: Int64 = 0
        var fraction = remaining_fraction[unsafe_offset=index]

        if amount_state == 1:
            remaining_value = remaining_amount[unsafe_offset=index]
            has_remaining = 1
            if has_fraction == 1 and fraction > 0.0:
                total_value = prodex_quota_round_f64(
                    Float64(remaining_value) / fraction
                )
                if total_value >= remaining_value:
                    has_total = 1
        elif amount_state == 0 and has_fraction == 1:
            remaining_value = prodex_quota_round_f64(fraction * 100.0)
            has_remaining = 1
            total_value = 100
            has_total = 1
        if has_fraction == 1:
            percent_value = prodex_quota_round_f64(fraction * 100.0)
            has_percent = 1
        elif has_remaining == 1 and has_total == 1 and total_value > 0:
            percent_value = prodex_quota_round_f64(
                Float64(remaining_value) / Float64(total_value) * 100.0
            )
            has_percent = 1

        if has_fraction == 1 and fraction <= 0.0:
            exhausted_value = 1
        elif has_remaining == 1 and remaining_value <= 0:
            exhausted_value = 1

        remaining[unsafe_offset=index] = remaining_value
        remaining_present[unsafe_offset=index] = has_remaining
        total[unsafe_offset=index] = total_value
        total_present[unsafe_offset=index] = has_total
        remaining_percent[unsafe_offset=index] = percent_value
        remaining_percent_present[unsafe_offset=index] = has_percent
        exhausted[unsafe_offset=index] = exhausted_value
    return 0


comptime QUOTA_MAIN_AGGREGATION_MAX_COUNT: Int64 = 1_024


def quota_reset_epoch_plan(
    fields: Array[Int64, 16],
) -> Tuple[Int64, Int64, Int64]:
    var index: Int64 = 0
    while index < 8:
        var present = fields[Int(index * 2 + 1)]
        if present != 0 and present != 1:
            return 2, 0, 0
        index += 1

    var reset_at: Int64 = 0
    var reset_present: Int64 = 0
    index = 0
    while index < 4:
        if fields[Int(index * 2 + 1)] == 1:
            reset_at = fields[Int(index * 2)]
            reset_present = 1
            break
        index += 1

    if reset_present == 0:
        var primary_used = fields[12]
        var primary_used_present = fields[13]
        var secondary_used = fields[14]
        var secondary_used_present = fields[15]
        if primary_used_present == 1 and primary_used >= 100:
            if fields[9] == 1:
                reset_at = fields[8]
                reset_present = 1
        elif secondary_used_present == 1 and secondary_used >= 100:
            if fields[11] == 1:
                reset_at = fields[10]
                reset_present = 1
        elif fields[9] == 1:
            reset_at = fields[8]
            reset_present = 1
        elif fields[11] == 1:
            reset_at = fields[10]
            reset_present = 1
    return 0, reset_at, reset_present


@export("prodex_quota_reset_epoch_v1")
def prodex_quota_reset_epoch_v1(
    fields_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if fields_address == 0 or output_address == 0:
        return 1
    var source = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var fields = Array[Int64, 16](fill=0)
    for index in range(16):
        fields[Int(index)] = source[unsafe_offset=index]
    var plan = quota_reset_epoch_plan(fields)
    if plan[0] != 0:
        return plan[0]
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = plan[1]
    output[unsafe_offset=1] = plan[2]
    return 0


def quota_json_ascii_space(value: UInt8) -> Bool:
    return value == 9 or value == 10 or value == 13 or value == 32


def quota_json_i64(
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
) -> Tuple[Bool, Int64]:
    if bounds[0] < 0 or bounds[1] <= bounds[0]:
        return False, 0
    var start = bounds[0]
    var end = bounds[1]
    if deepseek_json_byte(view, start) == 34:
        if end - start < 2 or deepseek_json_byte(view, end - 1) != 34:
            return False, 0
        start += 1
        end -= 1
        while start < end and quota_json_ascii_space(deepseek_json_byte(view, start)):
            start += 1
        while end > start and quota_json_ascii_space(deepseek_json_byte(view, end - 1)):
            end -= 1
    if start >= end:
        return False, 0

    var negative = False
    var first = deepseek_json_byte(view, start)
    if first == 45 or first == 43:
        negative = first == 45
        start += 1
    if start >= end:
        return False, 0

    var limit: UInt64 = 9223372036854775807
    if negative:
        limit = 9223372036854775808
    var magnitude: UInt64 = 0
    var index = start
    while index < end:
        var byte = deepseek_json_byte(view, index)
        if byte < 48 or byte > 57:
            return False, 0
        var digit = UInt64(byte - 48)
        if magnitude > (limit - digit) // 10:
            return False, 0
        magnitude = magnitude * 10 + digit
        index += 1
    if negative:
        if magnitude == 9223372036854775808:
            return True, INT64_MIN
        return True, -Int64(magnitude)
    return True, Int64(magnitude)


def quota_json_key_casefold_equal(
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
    expected: StringSlice,
) -> Bool:
    if start < 0 or end - start != Int64(expected.byte_length()) + 2:
        return False
    if deepseek_json_byte(view, start) != 34 or deepseek_json_byte(view, end - 1) != 34:
        return False
    var wanted = expected.unsafe_ptr()
    for offset in range(Int64(expected.byte_length())):
        if quota_ascii_lower(deepseek_json_byte(view, start + 1 + offset)) != quota_ascii_lower(
            wanted[unsafe_offset=offset]
        ):
            return False
    return True


def quota_json_object_member_casefold_first(
    view: ProdexRichStringView,
    object_bounds: Array[Int64, 2],
    expected: StringSlice,
) -> Array[Int64, 2]:
    var result = Array[Int64, 2](fill=-1)
    if (
        object_bounds[0] < 0
        or object_bounds[1] <= object_bounds[0] + 1
        or deepseek_json_byte(view, object_bounds[0]) != 123
        or deepseek_json_byte(view, object_bounds[1] - 1) != 125
    ):
        return result^
    var index = deepseek_json_skip_ws(view, object_bounds[0] + 1, object_bounds[1] - 1)
    while index < object_bounds[1] - 1:
        var key_start = index
        var key_end = deepseek_json_string_end(view, key_start, object_bounds[1] - 1)
        if key_end < 0:
            return result^
        index = deepseek_json_skip_ws(view, key_end, object_bounds[1] - 1)
        if index >= object_bounds[1] - 1 or deepseek_json_byte(view, index) != 58:
            return result^
        var value_start = deepseek_json_skip_ws(view, index + 1, object_bounds[1] - 1)
        var value_end = deepseek_json_value_end(view, value_start, object_bounds[1] - 1, 0)
        if value_end < 0:
            return result^
        if quota_json_key_casefold_equal(view, key_start, key_end, expected):
            result[0] = value_start
            result[1] = value_end
            return result^
        index = deepseek_json_skip_ws(view, value_end, object_bounds[1] - 1)
        if index < object_bounds[1] - 1 and deepseek_json_byte(view, index) == 44:
            index = deepseek_json_skip_ws(view, index + 1, object_bounds[1] - 1)
            continue
        break
    return result^


def quota_reset_field(
    fields: Pointer[mut=True, Int64, _],
    field: Int64,
    view: ProdexRichStringView,
    bounds: Array[Int64, 2],
):
    var parsed = quota_json_i64(view, bounds)
    if parsed[0]:
        fields[unsafe_offset=field * 2] = parsed[1]
        fields[unsafe_offset=field * 2 + 1] = 1


@export("prodex_quota_reset_json_epoch_v1")
def prodex_quota_reset_json_epoch_v1(
    address: UInt,
    length: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if length < 0 or (length > 0 and address == 0) or output_address == 0:
        return 1
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = 0
    output[unsafe_offset=1] = 0
    if length == 0:
        return 0

    var view = ProdexRichStringView(address, UInt(length))
    if not rich_view_valid(view, 16 * 1024 * 1024) or not deepseek_json_fragment_valid(view):
        return 0
    var root_start = deepseek_json_skip_ws(view, 0, length)
    var root_end = deepseek_json_value_end(view, root_start, length, 0)
    if (
        root_end < 0
        or deepseek_json_byte(view, root_start) != 123
        or deepseek_json_byte(view, root_end - 1) != 125
    ):
        return 0

    var fields = Array[Int64, 16](fill=0)
    var field_ptr = fields.unsafe_ptr()
    quota_reset_field(
        field_ptr, 0, view,
        deepseek_json_object_member(view, root_start, root_end, StringSlice("resets_at")),
    )
    quota_reset_field(
        field_ptr, 1, view,
        deepseek_json_object_member(view, root_start, root_end, StringSlice("reset_at")),
    )
    var error_bounds = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("error")
    )
    if (
        error_bounds[0] >= 0
        and deepseek_json_byte(view, error_bounds[0]) == 123
    ):
        quota_reset_field(
            field_ptr, 2, view,
            deepseek_json_object_member(
                view, error_bounds[0], error_bounds[1], StringSlice("resets_at")
            ),
        )
        quota_reset_field(
            field_ptr, 3, view,
            deepseek_json_object_member(
                view, error_bounds[0], error_bounds[1], StringSlice("reset_at")
            ),
        )

    var headers = deepseek_json_object_member(
        view, root_start, root_end, StringSlice("headers")
    )
    if headers[0] >= 0 and deepseek_json_byte(view, headers[0]) == 123:
        quota_reset_field(
            field_ptr, 4, view,
            quota_json_object_member_casefold_first(
                view, headers, StringSlice("X-Codex-Primary-Reset-At")
            ),
        )
        quota_reset_field(
            field_ptr, 5, view,
            quota_json_object_member_casefold_first(
                view, headers, StringSlice("X-Codex-Secondary-Reset-At")
            ),
        )
        quota_reset_field(
            field_ptr, 6, view,
            quota_json_object_member_casefold_first(
                view, headers, StringSlice("X-Codex-Primary-Used-Percent")
            ),
        )
        quota_reset_field(
            field_ptr, 7, view,
            quota_json_object_member_casefold_first(
                view, headers, StringSlice("X-Codex-Secondary-Used-Percent")
            ),
        )

    var plan = quota_reset_epoch_plan(fields)
    if plan[0] != 0:
        return plan[0]
    output[unsafe_offset=0] = plan[1]
    output[unsafe_offset=1] = plan[2]
    return 0


def quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    if right < 0 and left < INT64_MIN - right:
        return INT64_MIN
    return left + right


@export("prodex_quota_main_aggregate_batch")
def prodex_quota_main_aggregate_batch(
    remaining_percent: Pointer[mut=False, Int64, _],
    remaining_present: Pointer[mut=False, Int64, _],
    reset_at: Pointer[mut=False, Int64, _],
    reset_present: Pointer[mut=False, Int64, _],
    profiles_with_data: Pointer[mut=True, Int64, _],
    pool_remaining: Pointer[mut=True, Int64, _],
    earliest_reset_at: Pointer[mut=True, Int64, _],
    earliest_present: Pointer[mut=True, Int64, _],
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > QUOTA_MAIN_AGGREGATION_MAX_COUNT:
        return 1

    var profile_count: Int64 = 0
    var remaining_total: Int64 = 0
    var has_earliest: Int64 = 0
    var earliest: Int64 = 0
    for index in range(count):
        var has_remaining = remaining_present[unsafe_offset=index]
        var has_reset = reset_present[unsafe_offset=index]
        if (has_remaining != 0 and has_remaining != 1) or (
            has_reset != 0 and has_reset != 1
        ):
            return 2
        if has_remaining == 1:
            profile_count += 1
            remaining_total = quota_saturating_add(
                remaining_total,
                remaining_percent[unsafe_offset=index],
            )
            if has_reset == 1:
                var reset = reset_at[unsafe_offset=index]
                if has_earliest == 0 or reset < earliest:
                    earliest = reset
                    has_earliest = 1
    profiles_with_data[unsafe_offset=0] = profile_count
    pool_remaining[unsafe_offset=0] = remaining_total
    earliest_reset_at[unsafe_offset=0] = earliest
    earliest_present[unsafe_offset=0] = has_earliest
    return 0


comptime QUOTA_CAPACITY_FIELD_COUNT: Int64 = 22
comptime QUOTA_CAPACITY_BATCH_MAX_COUNT: Int64 = 256


def quota_capacity_field(
    fields: Pointer[mut=False, Int64, _], index: Int64, field: Int64
) -> Int64:
    return fields[unsafe_offset=(index * QUOTA_CAPACITY_FIELD_COUNT) + field]


def quota_capacity_saturating_mul(left: Int64, right: Int64) -> Int64:
    if left <= 0 or right <= 0:
        return 0
    if left > INT64_MAX / right:
        return INT64_MAX
    return left * right


def quota_capacity_scale_pressure(pressure: Int64, scale_bps: Int64) -> Int64:
    if pressure == INT64_MAX:
        return INT64_MAX
    if pressure == 0 or scale_bps == 0:
        return 0
    return quota_capacity_saturating_mul(pressure, scale_bps) / 10_000


def quota_capacity_pressure(
    seconds_until_reset: Int64,
    remaining_percent: Int64,
    has_value: Int64,
) -> Int64:
    if has_value == 0:
        return INT64_MAX
    var denominator = remaining_percent
    if denominator < 1:
        denominator = 1
    return quota_capacity_saturating_mul(seconds_until_reset, 1_000) / denominator


def quota_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def quota_capacity_reset_seconds(reset_at: Int64, now: Int64) -> Int64:
    if reset_at == INT64_MAX:
        return INT64_MAX
    if reset_at > now:
        return quota_saturating_sub(reset_at, now)
    return 0


def quota_capacity_weekly_weight(route_kind: Int64) -> Int64:
    if route_kind == 0 or route_kind == 2:
        return 10
    return 8


def quota_capacity_admission_allowed(
    pair_allowed: Int64,
    outer_allowed: Int64,
    pair_limit_reached: Int64,
    outer_limit_reached: Int64,
    rate_limit_reached_type: Int64,
    camel_rate_limit_reached_type: Int64,
    spend_control_reached: Int64,
    camel_spend_control_reached: Int64,
    ordinary_usage_allowed: Int64,
) -> Int64:
    if pair_allowed == 2 or outer_allowed == 2 or pair_limit_reached == 2 or outer_limit_reached == 2:
        return 0
    if (rate_limit_reached_type != 0 and rate_limit_reached_type != 1) or (
        camel_rate_limit_reached_type != 0 and camel_rate_limit_reached_type != 1
    ):
        return 0
    if spend_control_reached == 2 or camel_spend_control_reached == 2:
        return 0
    if ordinary_usage_allowed != 0 and ordinary_usage_allowed != 2:
        return 0
    return 1


@export("prodex_quota_window_pressure")
def prodex_quota_window_pressure(
    remaining_percent: Int64,
    reset_at: Int64,
    now: Int64,
) abi("C") -> Int64:
    if remaining_percent < 0 or remaining_percent > 100:
        return 1

    return quota_capacity_pressure(
        quota_capacity_reset_seconds(reset_at, now),
        remaining_percent,
        1,
    )


def quota_capacity_pressure_band_for_route(
    five_hour_remaining: Int64,
    five_hour_has_value: Int64,
    weekly_remaining: Int64,
    weekly_has_value: Int64,
    route_kind: Int64,
) -> Int64:
    if five_hour_has_value == 0 and weekly_has_value == 0:
        return 4
    if (five_hour_has_value == 1 and five_hour_remaining == 0) or (
        weekly_has_value == 1 and weekly_remaining == 0
    ):
        return 3

    var thin_weekly: Int64 = 10
    var thin_five_hour: Int64 = 5
    var critical_weekly: Int64 = 5
    var critical_five_hour: Int64 = 3
    if route_kind == 0 or route_kind == 2:
        thin_weekly = 20
        thin_five_hour = 10
        critical_weekly = 10
        critical_five_hour = 5

    var weekly_band: Int64 = 0
    if weekly_has_value == 1:
        if weekly_remaining <= critical_weekly:
            weekly_band = 2
        elif weekly_remaining <= thin_weekly:
            weekly_band = 1
    var five_hour_band: Int64 = 0
    if five_hour_has_value == 1:
        if five_hour_remaining <= critical_five_hour:
            five_hour_band = 2
        elif five_hour_remaining <= thin_five_hour:
            five_hour_band = 1
    if weekly_band > five_hour_band:
        return weekly_band
    return five_hour_band


@export("prodex_quota_capacity_batch_v2")
def prodex_quota_capacity_batch_v2(
    fields_address: UInt,
    lane_address: UInt,
    five_hour_remaining_address: UInt,
    weekly_remaining_address: UInt,
    five_hour_status_address: UInt,
    weekly_status_address: UInt,
    pressure_band_address: UInt,
    admission_allowed_address: UInt,
    pair_ready_address: UInt,
    any_window_exhausted_address: UInt,
    usable_address: UInt,
    routing_eligible_address: UInt,
    reserve_floor_address: UInt,
    five_hour_pressure_address: UInt,
    weekly_pressure_address: UInt,
    total_pressure_address: UInt,
    route_kind: Int64,
    count: Int64,
) abi("C") -> Int64:
    if count < 0 or count > QUOTA_CAPACITY_BATCH_MAX_COUNT:
        return 1
    if route_kind < 0 or route_kind > 3:
        return 1
    if count == 0:
        return 0
    if fields_address == 0 or lane_address == 0 or five_hour_remaining_address == 0 or weekly_remaining_address == 0 or five_hour_status_address == 0 or weekly_status_address == 0 or pressure_band_address == 0 or admission_allowed_address == 0 or pair_ready_address == 0 or any_window_exhausted_address == 0 or usable_address == 0 or routing_eligible_address == 0 or reserve_floor_address == 0 or five_hour_pressure_address == 0 or weekly_pressure_address == 0 or total_pressure_address == 0:
        return 1

    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](unsafe_from_address=Int(fields_address))
    var lane = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(lane_address))
    var five_hour_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_remaining_address))
    var weekly_remaining = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_remaining_address))
    var five_hour_status = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_status_address))
    var weekly_status = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_status_address))
    var pressure_band = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(pressure_band_address))
    var admission_allowed = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(admission_allowed_address))
    var pair_ready = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(pair_ready_address))
    var any_window_exhausted = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(any_window_exhausted_address))
    var usable = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(usable_address))
    var routing_eligible = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(routing_eligible_address))
    var reserve_floor = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(reserve_floor_address))
    var five_hour_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_pressure_address))
    var weekly_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_pressure_address))
    var total_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(total_pressure_address))

    for index in range(count):
        var row_lane = quota_capacity_field(fields, index, 0)
        var pair_allowed = quota_capacity_field(fields, index, 1)
        var outer_allowed = quota_capacity_field(fields, index, 2)
        var pair_limit_reached = quota_capacity_field(fields, index, 3)
        var outer_limit_reached = quota_capacity_field(fields, index, 4)
        var rate_limit_reached_type = quota_capacity_field(fields, index, 5)
        var camel_rate_limit_reached_type = quota_capacity_field(fields, index, 6)
        var spend_control_reached = quota_capacity_field(fields, index, 7)
        var camel_spend_control_reached = quota_capacity_field(fields, index, 8)
        var ordinary_usage_allowed = quota_capacity_field(fields, index, 9)
        var five_hour_used = quota_capacity_field(fields, index, 10)
        var five_hour_has_value = quota_capacity_field(fields, index, 11)
        var five_hour_reset_at = quota_capacity_field(fields, index, 12)
        var weekly_used = quota_capacity_field(fields, index, 13)
        var weekly_has_value = quota_capacity_field(fields, index, 14)
        var weekly_reset_at = quota_capacity_field(fields, index, 15)
        var primary_used = quota_capacity_field(fields, index, 16)
        var primary_has_value = quota_capacity_field(fields, index, 17)
        var secondary_used = quota_capacity_field(fields, index, 18)
        var secondary_has_value = quota_capacity_field(fields, index, 19)
        var scale_bps = quota_capacity_field(fields, index, 20)
        var now = quota_capacity_field(fields, index, 21)
        if row_lane < 0 or row_lane > 2 or pair_allowed < 0 or pair_allowed > 2 or outer_allowed < 0 or outer_allowed > 2:
            return 2
        if pair_limit_reached < 0 or pair_limit_reached > 2 or outer_limit_reached < 0 or outer_limit_reached > 2:
            return 2
        if rate_limit_reached_type < 0 or rate_limit_reached_type > 4 or camel_rate_limit_reached_type < 0 or camel_rate_limit_reached_type > 4:
            return 2
        if spend_control_reached < 0 or spend_control_reached > 4 or camel_spend_control_reached < 0 or camel_spend_control_reached > 4 or ordinary_usage_allowed < 0 or ordinary_usage_allowed > 4:
            return 2
        if five_hour_has_value < 0 or five_hour_has_value > 1 or weekly_has_value < 0 or weekly_has_value > 1 or primary_has_value < 0 or primary_has_value > 1 or secondary_has_value < 0 or secondary_has_value > 1:
            return 2
        if scale_bps < 0:
            return 2

        var five_hour_remaining_value = prodex_quota_remaining_percent(
            five_hour_used, five_hour_has_value
        )
        var weekly_remaining_value = prodex_quota_remaining_percent(
            weekly_used, weekly_has_value
        )
        var five_hour_status_value = prodex_quota_window_status(
            five_hour_remaining_value, five_hour_has_value
        )
        var weekly_status_value = prodex_quota_window_status(
            weekly_remaining_value, weekly_has_value
        )
        var five_hour_pressure_value = quota_capacity_pressure(
            quota_capacity_reset_seconds(five_hour_reset_at, now),
            five_hour_remaining_value,
            five_hour_has_value,
        )
        var weekly_pressure_value = quota_capacity_pressure(
            quota_capacity_reset_seconds(weekly_reset_at, now),
            weekly_remaining_value,
            weekly_has_value,
        )
        var band = quota_capacity_pressure_band_for_route(
            five_hour_remaining_value,
            five_hour_has_value,
            weekly_remaining_value,
            weekly_has_value,
            route_kind,
        )
        var admission_value = quota_capacity_admission_allowed(
            pair_allowed,
            outer_allowed,
            pair_limit_reached,
            outer_limit_reached,
            rate_limit_reached_type,
            camel_rate_limit_reached_type,
            spend_control_reached,
            camel_spend_control_reached,
            ordinary_usage_allowed,
        )
        var pair_ready_value: Int64 = 0
        if five_hour_has_value == 1 or weekly_has_value == 1:
            pair_ready_value = 1
            if (five_hour_has_value == 1 and five_hour_remaining_value == 0) or (
                weekly_has_value == 1 and weekly_remaining_value == 0
            ):
                pair_ready_value = 0
        var any_window_exhausted_value: Int64 = 0
        if (primary_has_value == 1 and primary_used >= 100) or (
            secondary_has_value == 1 and secondary_used >= 100
        ):
            any_window_exhausted_value = 1
        var usable_value = admission_value * pair_ready_value
        var routing_value: Int64 = 0
        if usable_value == 1 and (row_lane == 0 or row_lane == 1):
            routing_value = 1

        var reserve_floor_value = five_hour_remaining_value
        if weekly_remaining_value < reserve_floor_value:
            reserve_floor_value = weekly_remaining_value
        var reserve_bias: Int64 = 0
        if band == 1:
            reserve_bias = 250_000
        elif band == 2:
            reserve_bias = 1_000_000
        elif band == 3 or band == 4:
            reserve_bias = INT64_MAX / 4
        var raw_total = quota_saturating_add(
            reserve_bias,
            quota_saturating_add(
                quota_capacity_saturating_mul(
                    weekly_pressure_value,
                    quota_capacity_weekly_weight(route_kind),
                ),
                five_hour_pressure_value,
            ),
        )

        lane[unsafe_offset=index] = row_lane
        five_hour_remaining[unsafe_offset=index] = five_hour_remaining_value
        weekly_remaining[unsafe_offset=index] = weekly_remaining_value
        five_hour_status[unsafe_offset=index] = five_hour_status_value
        weekly_status[unsafe_offset=index] = weekly_status_value
        pressure_band[unsafe_offset=index] = band
        admission_allowed[unsafe_offset=index] = admission_value
        pair_ready[unsafe_offset=index] = pair_ready_value
        any_window_exhausted[unsafe_offset=index] = any_window_exhausted_value
        usable[unsafe_offset=index] = usable_value
        routing_eligible[unsafe_offset=index] = routing_value
        reserve_floor[unsafe_offset=index] = reserve_floor_value
        five_hour_pressure[unsafe_offset=index] = quota_capacity_scale_pressure(
            five_hour_pressure_value, scale_bps
        )
        weekly_pressure[unsafe_offset=index] = quota_capacity_scale_pressure(
            weekly_pressure_value, scale_bps
        )
        total_pressure[unsafe_offset=index] = quota_capacity_scale_pressure(
            raw_total, scale_bps
        )
    return 0
# OpenAI model-capacity policy. Rust owns JSON acquisition and pointer reconstruction;
# Mojo owns normalized model identity and the deterministic capacity decision.

comptime QUOTA_MODEL_KIND_NONE: Int64 = 0
comptime QUOTA_MODEL_KIND_LUNA: Int64 = 1
comptime QUOTA_MODEL_KIND_RETIRED_SPARK: Int64 = 2
comptime QUOTA_MODEL_KIND_OTHER: Int64 = 3

comptime QUOTA_MODEL_PAIR_NONE: Int64 = 0
comptime QUOTA_MODEL_PAIR_REGULAR: Int64 = 1
comptime QUOTA_MODEL_PAIR_RESERVE: Int64 = 2
comptime QUOTA_MODEL_PAIR_DEFAULT: Int64 = 3

comptime QUOTA_MODEL_CAPACITY_FIELD_COUNT: Int64 = 10


def quota_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def quota_ascii_alphanumeric(value: UInt8) -> Bool:
    var lowered = quota_ascii_lower(value)
    return (lowered >= 97 and lowered <= 122) or (lowered >= 48 and lowered <= 57)


def quota_text_ptr(address: UInt) -> Pointer[mut=False, UInt8, ImmUntrackedOrigin]:
    return Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )


def quota_normalized_equals(
    address: UInt,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    if length <= 0 or address == 0:
        return False
    var source = quota_text_ptr(address)
    var target = expected.unsafe_ptr()
    var target_length = Int64(expected.byte_length())
    var target_index: Int64 = 0
    for index in range(length):
        var value = source[unsafe_offset=index]
        if not quota_ascii_alphanumeric(value):
            continue
        if target_index >= target_length:
            return False
        if quota_ascii_lower(value) != quota_ascii_lower(target[unsafe_offset=target_index]):
            return False
        target_index += 1
    return target_index == target_length


def quota_normalized_contains(
    address: UInt,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    if length <= 0 or address == 0:
        return False
    var source = quota_text_ptr(address)
    var target = expected.unsafe_ptr()
    var target_length = Int64(expected.byte_length())
    if target_length <= 0:
        return True

    # The identifiers here are tiny. A bounded restart scan keeps the ABI allocation-free
    # while matching Rust's "normalize then contains" semantics exactly for ASCII tokens.
    for raw_start in range(length):
        var first = source[unsafe_offset=raw_start]
        if not quota_ascii_alphanumeric(first):
            continue
        var target_index: Int64 = 0
        var raw_index = raw_start
        while raw_index < length and target_index < target_length:
            var value = source[unsafe_offset=raw_index]
            raw_index += 1
            if not quota_ascii_alphanumeric(value):
                continue
            if quota_ascii_lower(value) != quota_ascii_lower(target[unsafe_offset=target_index]):
                break
            target_index += 1
        if target_index == target_length:
            return True
    return False


@export("prodex_quota_openai_model_kind")
def prodex_quota_openai_model_kind(
    address: UInt,
    length: Int64,
    present: Int64,
) abi("C") -> Int64:
    if present == 0:
        return QUOTA_MODEL_KIND_NONE
    if present != 1 or length < 0 or (length > 0 and address == 0):
        return -1
    if quota_normalized_equals(address, length, StringSlice("luna")) or quota_normalized_equals(
        address, length, StringSlice("gpt56luna")
    ):
        return QUOTA_MODEL_KIND_LUNA
    if (
        quota_normalized_equals(address, length, StringSlice("spark"))
        or quota_normalized_equals(address, length, StringSlice("gpt53codexspark"))
        or quota_normalized_equals(address, length, StringSlice("gpt53spark"))
    ):
        return QUOTA_MODEL_KIND_RETIRED_SPARK
    return QUOTA_MODEL_KIND_OTHER


def quota_luna_reserve_identifier_matches(address: UInt, length: Int64) -> Bool:
    return quota_normalized_equals(address, length, StringSlice("gptreserve")) or (
        quota_normalized_contains(address, length, StringSlice("luna"))
        and quota_normalized_contains(address, length, StringSlice("reserve"))
    )

@export("prodex_quota_luna_reserve_identifier")
def prodex_quota_luna_reserve_identifier(
    model_slug_address: UInt,
    model_slug_length: Int64,
    limit_id_address: UInt,
    limit_id_length: Int64,
    limit_name_address: UInt,
    limit_name_length: Int64,
    metered_feature_address: UInt,
    metered_feature_length: Int64,
) abi("C") -> Int64:
    if (
        model_slug_length < 0
        or limit_id_length < 0
        or limit_name_length < 0
        or metered_feature_length < 0
    ):
        return -1
    if not quota_normalized_equals(
        model_slug_address, model_slug_length, StringSlice("gpt56luna")
    ):
        return 0

    if (
        quota_luna_reserve_identifier_matches(limit_id_address, limit_id_length)
        or quota_luna_reserve_identifier_matches(limit_name_address, limit_name_length)
        or quota_luna_reserve_identifier_matches(
            metered_feature_address, metered_feature_length
        )
    ):
        return 1
    return 0


def quota_model_capacity_field(
    fields: Pointer[mut=False, Int64, _], field: Int64
) -> Int64:
    return fields[unsafe_offset=field]


@export("prodex_quota_openai_model_capacity_plan")
def prodex_quota_openai_model_capacity_plan(
    fields_address: UInt,
    output_address: UInt,
) abi("C") -> Int64:
    if fields_address == 0 or output_address == 0:
        return 1
    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )

    var model_kind = quota_model_capacity_field(fields, 0)
    if model_kind < QUOTA_MODEL_KIND_NONE or model_kind > QUOTA_MODEL_KIND_OTHER:
        return 2
    var field: Int64 = 1
    while field < QUOTA_MODEL_CAPACITY_FIELD_COUNT:
        var value = quota_model_capacity_field(fields, field)
        if value != 0 and value != 1:
            return 2
        field += 1

    var regular_present = quota_model_capacity_field(fields, 1)
    var regular_ready = quota_model_capacity_field(fields, 2)
    var generic_ready = quota_model_capacity_field(fields, 3)
    var reserve_ready = quota_model_capacity_field(fields, 4)
    var regular_blocked = quota_model_capacity_field(fields, 5)
    var any_unknown_window = quota_model_capacity_field(fields, 6)
    var any_exhausted_window = quota_model_capacity_field(fields, 7)
    var include_code_review = quota_model_capacity_field(fields, 8)
    var code_review_ready = quota_model_capacity_field(fields, 9)

    var unknown_luna: Int64 = 0
    if (
        regular_present == 1
        and regular_blocked == 0
        and regular_ready == 0
        and any_unknown_window == 1
        and any_exhausted_window == 0
    ):
        unknown_luna = 1

    var selected_pair = QUOTA_MODEL_PAIR_NONE
    var ready: Int64
    if model_kind == QUOTA_MODEL_KIND_NONE:
        selected_pair = QUOTA_MODEL_PAIR_DEFAULT
        ready = generic_ready
    elif model_kind == QUOTA_MODEL_KIND_RETIRED_SPARK:
        selected_pair = QUOTA_MODEL_PAIR_NONE
        ready = 0
    elif model_kind == QUOTA_MODEL_KIND_LUNA:
        ready = 1 if regular_ready == 1 or reserve_ready == 1 else 0
        if regular_ready == 1:
            selected_pair = QUOTA_MODEL_PAIR_REGULAR
        elif reserve_ready == 1:
            selected_pair = QUOTA_MODEL_PAIR_RESERVE
        elif regular_present == 1:
            selected_pair = QUOTA_MODEL_PAIR_REGULAR
    else:
        ready = regular_ready
        if regular_present == 1:
            selected_pair = QUOTA_MODEL_PAIR_REGULAR

    var supports = ready
    if model_kind == QUOTA_MODEL_KIND_LUNA and unknown_luna == 1:
        supports = 1
    if include_code_review == 1 and code_review_ready == 0:
        supports = 0

    output[unsafe_offset=0] = selected_pair
    output[unsafe_offset=1] = ready
    output[unsafe_offset=2] = supports
    output[unsafe_offset=3] = unknown_luna
    return 0
# Human-facing quota status classification. Rust keeps ownership of provider strings and
# final allocation; Mojo owns deterministic classification and precedence.

comptime QUOTA_ERROR_KIND_UNKNOWN: Int64 = 0
comptime QUOTA_ERROR_KIND_UNAVAILABLE: Int64 = 1
comptime QUOTA_ERROR_KIND_CONFIG: Int64 = 2
comptime QUOTA_ERROR_KIND_SERVER: Int64 = 3
comptime QUOTA_ERROR_KIND_TIMEOUT: Int64 = 4
comptime QUOTA_ERROR_KIND_NETWORK: Int64 = 5
comptime QUOTA_ERROR_KIND_PROXY: Int64 = 6
comptime QUOTA_ERROR_KIND_CONNECTION: Int64 = 7
comptime QUOTA_ERROR_KIND_INVALID_AUTH: Int64 = 8
comptime QUOTA_ERROR_KIND_RATE_LIMIT: Int64 = 9
comptime QUOTA_ERROR_KIND_PARSE: Int64 = 10
comptime QUOTA_ERROR_KIND_EMPTY: Int64 = 11
comptime QUOTA_ERROR_KIND_CANCELLED: Int64 = 12
comptime QUOTA_ERROR_KIND_FORBIDDEN: Int64 = 13
comptime QUOTA_ERROR_KIND_NOT_FOUND: Int64 = 14
comptime QUOTA_ERROR_KIND_OTHER: Int64 = 15

comptime QUOTA_BLOCKED_KIND_NONE: Int64 = 0
comptime QUOTA_BLOCKED_KIND_EXHAUSTED: Int64 = 1
comptime QUOTA_BLOCKED_KIND_WEEKLY: Int64 = 2
comptime QUOTA_BLOCKED_KIND_FIVE_HOUR: Int64 = 3


def quota_text_contains_ascii_case_insensitive(
    address: UInt,
    length: Int64,
    expected: StringSlice,
) -> Bool:
    if length <= 0 or address == 0:
        return False
    var target_length = Int64(expected.byte_length())
    if target_length <= 0:
        return True
    if target_length > length:
        return False
    var source = quota_text_ptr(address)
    var target = expected.unsafe_ptr()
    for start in range(length - target_length + 1):
        var matched = True
        for offset in range(target_length):
            if quota_ascii_lower(source[unsafe_offset=start + offset]) != quota_ascii_lower(
                target[unsafe_offset=offset]
            ):
                matched = False
                break
        if matched:
            return True
    return False


@export("prodex_quota_error_summary_kind")
def prodex_quota_error_summary_kind(
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if length < 0 or (length > 0 and address == 0):
        return -1
    if length == 0:
        return QUOTA_ERROR_KIND_UNKNOWN

    if quota_text_contains_ascii_case_insensitive(
        address, length, StringSlice("unavailable")
    ):
        return QUOTA_ERROR_KIND_UNAVAILABLE
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("missing"))
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("not configured")
        )
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("config"))
    ):
        return QUOTA_ERROR_KIND_CONFIG

    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("500"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("502"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("503"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("504"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("server"))
    ):
        return QUOTA_ERROR_KIND_SERVER
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("timeout"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("timed out"))
    ):
        return QUOTA_ERROR_KIND_TIMEOUT
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("dns"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("tls"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("certificate"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("network"))
    ):
        return QUOTA_ERROR_KIND_NETWORK
    if quota_text_contains_ascii_case_insensitive(address, length, StringSlice("proxy")):
        return QUOTA_ERROR_KIND_PROXY
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("refused"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("connect"))
    ):
        return QUOTA_ERROR_KIND_CONNECTION

    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("invalid auth"))
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("invalid token")
        )
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("bad credentials")
        )
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("credential")
        )
    ):
        return QUOTA_ERROR_KIND_INVALID_AUTH
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("429"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("rate limit"))
    ):
        return QUOTA_ERROR_KIND_RATE_LIMIT

    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("parse"))
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("deserialize")
        )
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("decode"))
        or quota_text_contains_ascii_case_insensitive(
            address, length, StringSlice("invalid json")
        )
    ):
        return QUOTA_ERROR_KIND_PARSE
    if quota_text_contains_ascii_case_insensitive(address, length, StringSlice("empty")):
        return QUOTA_ERROR_KIND_EMPTY
    if quota_text_contains_ascii_case_insensitive(address, length, StringSlice("cancel")):
        return QUOTA_ERROR_KIND_CANCELLED
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("403"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("forbidden"))
    ):
        return QUOTA_ERROR_KIND_FORBIDDEN
    if (
        quota_text_contains_ascii_case_insensitive(address, length, StringSlice("404"))
        or quota_text_contains_ascii_case_insensitive(address, length, StringSlice("not found"))
    ):
        return QUOTA_ERROR_KIND_NOT_FOUND
    return QUOTA_ERROR_KIND_OTHER


@export("prodex_quota_blocked_limit_kind")
def prodex_quota_blocked_limit_kind(
    address: UInt,
    length: Int64,
) abi("C") -> Int64:
    if length < 0 or (length > 0 and address == 0):
        return -1
    var exhausted = quota_text_contains_ascii_case_insensitive(
        address, length, StringSlice("exhausted")
    )
    if not exhausted:
        return QUOTA_BLOCKED_KIND_NONE
    if quota_text_contains_ascii_case_insensitive(address, length, StringSlice("5h")):
        return QUOTA_BLOCKED_KIND_FIVE_HOUR
    if quota_text_contains_ascii_case_insensitive(address, length, StringSlice("weekly")):
        return QUOTA_BLOCKED_KIND_WEEKLY
    return QUOTA_BLOCKED_KIND_EXHAUSTED

comptime QUOTA_MODEL_POLICY_ABI_VERSION: Int64 = 1
comptime QUOTA_MODEL_POLICY_OK: Int64 = 0
comptime QUOTA_MODEL_POLICY_INVALID: Int64 = 1
comptime QUOTA_MODEL_POLICY_CAPACITY: Int64 = 2
comptime QUOTA_MODEL_POLICY_ABI: Int64 = 4

comptime QUOTA_AUTH_FILTER_ALL: Int64 = 0
comptime QUOTA_AUTH_FILTER_LABEL: Int64 = 1
comptime QUOTA_AUTH_FILTER_COMPATIBLE: Int64 = 2
comptime QUOTA_AUTH_FILTER_INCOMPATIBLE: Int64 = 3


def quota_model_policy_view(address: UInt, length: Int64) -> ProdexRichStringView:
    return ProdexRichStringView(address, UInt(length))


def quota_model_policy_ascii_lower(value: UInt8) -> UInt8:
    if value >= 65 and value <= 90:
        return value + 32
    return value


def quota_model_policy_trimmed_matches(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var length = bounds[1] - bounds[0]
    if length != Int64(literal.byte_length()):
        return False
    var source = rich_view_ptr(view)
    var target = literal.unsafe_ptr()
    for index in range(length):
        if quota_model_policy_ascii_lower(
            source[unsafe_offset=bounds[0] + index]
        ) != target[unsafe_offset=index]:
            return False
    return True


def quota_plan_type_matches(
    view: ProdexRichStringView, literal: StringSlice
) -> Bool:
    var bounds = rich_trim_bounds(view)
    var source = rich_view_ptr(view)
    var target = literal.unsafe_ptr()
    var target_index: Int64 = 0
    for index in range(bounds[0], bounds[1]):
        var value = source[unsafe_offset=index]
        if value == 32 or value == 45 or value == 95:
            continue
        if value >= 0x80:
            return False
        if (
            target_index >= Int64(literal.byte_length())
            or quota_model_policy_ascii_lower(value)
            != target[unsafe_offset=target_index]
        ):
            return False
        target_index += 1
    return target_index == Int64(literal.byte_length())


comptime QUOTA_DISPLAY_LABEL_SORT: Int64 = 0
comptime QUOTA_DISPLAY_LABEL_BLOCKED_STATUS: Int64 = 1
comptime QUOTA_DISPLAY_LABEL_AUTH_SYNC_SOURCE: Int64 = 2
comptime QUOTA_DISPLAY_LABEL_AUTH_SUMMARY: Int64 = 3

comptime QUOTA_WINDOW_LABEL_USAGE: Int64 = 0
comptime QUOTA_WINDOW_LABEL_FIVE_HOUR: Int64 = 1
comptime QUOTA_WINDOW_LABEL_WEEKLY: Int64 = 2
comptime QUOTA_WINDOW_LABEL_MONTHLY: Int64 = 3
comptime QUOTA_WINDOW_LABEL_SECONDS: Int64 = 4


def quota_model_policy_copy_label(
    label: StringSlice,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if (
        output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID
    var length = Int64(label.byte_length())
    if length > output_capacity:
        return QUOTA_MODEL_POLICY_CAPACITY
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var source = label.unsafe_ptr()
    for index in range(length):
        output[unsafe_offset=index] = source[unsafe_offset=index]
    written[] = length
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_display_label_v1")
def prodex_quota_display_label_v1(
    abi_version: Int64,
    label_kind: Int64,
    value: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI

    var label = StringSlice("")
    if label_kind == QUOTA_DISPLAY_LABEL_SORT:
        if value == 0:
            label = StringSlice("current")
        elif value == 1:
            label = StringSlice("remaining")
        elif value == 2:
            label = StringSlice("profile")
        elif value == 3:
            label = StringSlice("auth")
        elif value == 4:
            label = StringSlice("account")
        elif value == 5:
            label = StringSlice("plan")
        else:
            return QUOTA_MODEL_POLICY_INVALID
    elif label_kind == QUOTA_DISPLAY_LABEL_BLOCKED_STATUS:
        if value == QUOTA_BLOCKED_KIND_NONE:
            label = StringSlice("Unavailable")
        elif value == QUOTA_BLOCKED_KIND_EXHAUSTED:
            label = StringSlice("Blocked")
        elif value == QUOTA_BLOCKED_KIND_WEEKLY:
            label = StringSlice("Blocked weekly")
        elif value == QUOTA_BLOCKED_KIND_FIVE_HOUR:
            label = StringSlice("Blocked 5h")
        else:
            return QUOTA_MODEL_POLICY_INVALID
    elif label_kind == QUOTA_DISPLAY_LABEL_AUTH_SYNC_SOURCE:
        if value == 0:
            label = StringSlice("reloaded")
        elif value == 1:
            label = StringSlice("refreshed")
        else:
            return QUOTA_MODEL_POLICY_INVALID
    elif label_kind == QUOTA_DISPLAY_LABEL_AUTH_SUMMARY:
        if value == 0:
            label = StringSlice("chatgpt")
        elif value == 1:
            label = StringSlice("bedrock-api-key")
        elif value == 2:
            label = StringSlice("api-key")
        elif value == 3:
            label = StringSlice("auth-present")
        elif value == 4:
            label = StringSlice("unreadable-auth")
        elif value == 5:
            label = StringSlice("no-auth")
        elif value == 6:
            label = StringSlice("invalid-auth")
        else:
            return QUOTA_MODEL_POLICY_INVALID
    else:
        return QUOTA_MODEL_POLICY_INVALID

    return quota_model_policy_copy_label(
        label, output_address, output_capacity, written_address
    )


@export("prodex_quota_window_label_plan_v1")
def prodex_quota_window_label_plan_v1(
    abi_version: Int64,
    seconds_present: Int64,
    seconds: Int64,
    kind_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        (seconds_present != 0 and seconds_present != 1)
        or kind_address == 0
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var kind = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(kind_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )

    if seconds_present == 0:
        kind[] = QUOTA_WINDOW_LABEL_USAGE
        return quota_model_policy_copy_label(
            StringSlice("usage"), output_address, output_capacity, written_address
        )
    if seconds >= 17_700 and seconds <= 18_300:
        kind[] = QUOTA_WINDOW_LABEL_FIVE_HOUR
        return quota_model_policy_copy_label(
            StringSlice("5h"), output_address, output_capacity, written_address
        )
    if seconds >= 601_200 and seconds <= 608_400:
        kind[] = QUOTA_WINDOW_LABEL_WEEKLY
        return quota_model_policy_copy_label(
            StringSlice("weekly"), output_address, output_capacity, written_address
        )
    if seconds >= 2_505_600 and seconds <= 2_678_400:
        kind[] = QUOTA_WINDOW_LABEL_MONTHLY
        return quota_model_policy_copy_label(
            StringSlice("monthly"), output_address, output_capacity, written_address
        )

    kind[] = QUOTA_WINDOW_LABEL_SECONDS
    written[] = 0
    return QUOTA_MODEL_POLICY_OK



@fieldwise_init
struct QuotaDisplayWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def quota_display_put_byte(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    byte: UInt8,
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def quota_display_put_literal(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    value: StringSlice,
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not quota_display_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def quota_display_put_view(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    value: ProdexRichStringView,
) -> Bool:
    var source = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not quota_display_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def quota_display_put_range(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    view: ProdexRichStringView,
    start: Int64,
    end: Int64,
) -> Bool:
    if start < 0 or end < start or end > Int64(view.len):
        return False
    var source = rich_view_ptr(view)
    for index in range(start, end):
        if not quota_display_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


@export("prodex_quota_workspace_label_v1")
def prodex_quota_workspace_label_v1(
    abi_version: Int64,
    name_address: UInt,
    name_length: Int64,
    name_present: Int64,
    id_address: UInt,
    id_length: Int64,
    id_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    present_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        name_length < 0
        or id_length < 0
        or (name_present != 0 and name_present != 1)
        or (id_present != 0 and id_present != 1)
        or (name_present == 1 and name_length > 0 and name_address == 0)
        or (id_present == 1 and id_length > 0 and id_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
        or present_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = QuotaDisplayWriter(output, output_capacity, 0)
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(present_address)
    )
    written[] = 0
    present[] = 0

    if name_present == 1:
        var name = quota_model_policy_view(name_address, name_length)
        if not rich_view_valid(name, name_length):
            return QUOTA_MODEL_POLICY_INVALID
        var bounds = rich_trim_bounds(name)
        if bounds[1] > bounds[0]:
            if not quota_display_put_range(
                Pointer(to=writer), name, bounds[0], bounds[1]
            ):
                return QUOTA_MODEL_POLICY_CAPACITY
            written[] = writer.written
            present[] = 1
            return QUOTA_MODEL_POLICY_OK

    if id_present == 0:
        return QUOTA_MODEL_POLICY_OK
    var identifier = quota_model_policy_view(id_address, id_length)
    if not rich_view_valid(identifier, id_length):
        return QUOTA_MODEL_POLICY_INVALID
    var bounds = rich_trim_bounds(identifier)
    if bounds[1] <= bounds[0]:
        return QUOTA_MODEL_POLICY_OK

    var source = rich_view_ptr(identifier)
    var cursor = bounds[0]
    var codepoints: Int64 = 0
    while cursor < bounds[1]:
        cursor += rich_codepoint_width(source[unsafe_offset=cursor])
        codepoints += 1

    if codepoints <= 24:
        if not quota_display_put_range(
            Pointer(to=writer), identifier, bounds[0], bounds[1]
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
    else:
        cursor = bounds[0]
        var index: Int64 = 0
        var first_end = bounds[0]
        var tail_start = bounds[1]
        while cursor < bounds[1]:
            if index == 12:
                first_end = cursor
            if index == codepoints - 6:
                tail_start = cursor
            cursor += rich_codepoint_width(source[unsafe_offset=cursor])
            index += 1
        if not quota_display_put_range(
            Pointer(to=writer), identifier, bounds[0], first_end
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_literal(Pointer(to=writer), StringSlice("...")):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_range(
            Pointer(to=writer), identifier, tail_start, bounds[1]
        ):
            return QUOTA_MODEL_POLICY_CAPACITY

    written[] = writer.written
    present[] = 1
    return QUOTA_MODEL_POLICY_OK


def quota_display_put_u64(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    value: UInt64,
) -> Bool:
    if value == 0:
        return quota_display_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not quota_display_put_byte(
            writer, UInt8(remaining / divisor) + UInt8(48)
        ):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def quota_display_put_i64(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    value: Int64,
) -> Bool:
    if value >= 0:
        return quota_display_put_u64(writer, UInt64(value))
    if not quota_display_put_byte(writer, UInt8(45)):
        return False
    var magnitude = (
        UInt64(9_223_372_036_854_775_808)
        if value == -9_223_372_036_854_775_808
        else UInt64(-value)
    )
    return quota_display_put_u64(writer, magnitude)


def quota_copilot_write_feature(
    writer: Pointer[mut=True, QuotaDisplayWriter, _],
    label: StringSlice,
    remaining_present: Int64,
    remaining: Int64,
    total_present: Int64,
    total: Int64,
) -> Bool:
    if remaining_present == 0:
        return True
    if not quota_display_put_literal(writer, label):
        return False
    if not quota_display_put_byte(writer, UInt8(32)):
        return False
    if not quota_display_put_i64(writer, remaining):
        return False
    if total_present == 1:
        if not quota_display_put_byte(writer, UInt8(47)):
            return False
        if not quota_display_put_i64(writer, total):
            return False
    return True


@export("prodex_quota_copilot_feature_key_v1")
def prodex_quota_copilot_feature_key_v1(
    abi_version: Int64,
    index: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if index == 0:
        return quota_model_policy_copy_label(
            StringSlice("chat"),
            output_address,
            output_capacity,
            written_address,
        )
    if index == 1:
        return quota_model_policy_copy_label(
            StringSlice("completions"),
            output_address,
            output_capacity,
            written_address,
        )
    return QUOTA_MODEL_POLICY_INVALID


@export("prodex_quota_copilot_display_v1")
def prodex_quota_copilot_display_v1(
    abi_version: Int64,
    chat_remaining_present: Int64,
    chat_remaining: Int64,
    chat_total_present: Int64,
    chat_total: Int64,
    completions_remaining_present: Int64,
    completions_remaining: Int64,
    completions_total_present: Int64,
    completions_total: Int64,
    status_output_address: UInt,
    status_output_capacity: Int64,
    status_written_address: UInt,
    main_output_address: UInt,
    main_output_capacity: Int64,
    main_written_address: UInt,
    ready_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        (chat_remaining_present != 0 and chat_remaining_present != 1)
        or (chat_total_present != 0 and chat_total_present != 1)
        or (
            completions_remaining_present != 0
            and completions_remaining_present != 1
        )
        or (completions_total_present != 0 and completions_total_present != 1)
        or status_output_address == 0
        or status_output_capacity < 0
        or status_written_address == 0
        or main_output_address == 0
        or main_output_capacity < 0
        or main_written_address == 0
        or ready_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var ready = not (
        (chat_remaining_present == 1 and chat_remaining <= 0)
        or (
            completions_remaining_present == 1
            and completions_remaining <= 0
        )
    )
    var ready_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ready_address)
    )
    ready_output[] = Int64(ready)

    var status = StringSlice("Ready") if ready else StringSlice("Blocked")
    var status_code = quota_model_policy_copy_label(
        status,
        status_output_address,
        status_output_capacity,
        status_written_address,
    )
    if status_code != QUOTA_MODEL_POLICY_OK:
        return status_code

    var main_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(main_output_address)
    )
    var writer = QuotaDisplayWriter(
        main_output,
        main_output_capacity,
        0,
    )
    var any = False
    if chat_remaining_present == 1:
        if not quota_copilot_write_feature(
            Pointer(to=writer),
            StringSlice("chat"),
            chat_remaining_present,
            chat_remaining,
            chat_total_present,
            chat_total,
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        any = True
    if completions_remaining_present == 1:
        if any:
            if not quota_display_put_literal(
                Pointer(to=writer), StringSlice(" | ")
            ):
                return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_copilot_write_feature(
            Pointer(to=writer),
            StringSlice("comp"),
            completions_remaining_present,
            completions_remaining,
            completions_total_present,
            completions_total,
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        any = True
    if not any:
        if not quota_display_put_literal(Pointer(to=writer), StringSlice("-")):
            return QUOTA_MODEL_POLICY_CAPACITY

    var main_written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(main_written_address)
    )
    main_written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_copilot_main_remaining_percent_v1")
def prodex_quota_copilot_main_remaining_percent_v1(
    abi_version: Int64,
    chat_remaining_present: Int64,
    chat_remaining: Int64,
    chat_total_present: Int64,
    chat_total: Int64,
    completions_remaining_present: Int64,
    completions_remaining: Int64,
    completions_total_present: Int64,
    completions_total: Int64,
    percent_address: UInt,
    percent_present_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        (chat_remaining_present != 0 and chat_remaining_present != 1)
        or (chat_total_present != 0 and chat_total_present != 1)
        or (
            completions_remaining_present != 0
            and completions_remaining_present != 1
        )
        or (completions_total_present != 0 and completions_total_present != 1)
        or percent_address == 0
        or percent_present_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var percent = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(percent_address)
    )
    var present = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(percent_present_address)
    )
    percent[] = 0
    present[] = 0

    if chat_total_present == 1 and chat_total > 0:
        var remaining = (
            chat_remaining if chat_remaining_present == 1 else chat_total
        )
        percent[] = prodex_quota_round_f64(
            Float64(remaining) / Float64(chat_total) * 100.0
        )
        present[] = 1

    if completions_total_present == 1 and completions_total > 0:
        var remaining = (
            completions_remaining
            if completions_remaining_present == 1
            else completions_total
        )
        var value = prodex_quota_round_f64(
            Float64(remaining) / Float64(completions_total) * 100.0
        )
        if present[] == 0 or value < percent[]:
            percent[] = value
            present[] = 1
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_ready_pool_remaining_v1")
def prodex_quota_ready_pool_remaining_v1(
    abi_version: Int64,
    five_hour_remaining: Int64,
    weekly_remaining: Int64,
    ready_profiles: Int64,
    five_hour_profiles: Int64,
    weekly_profiles: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        ready_profiles < 0
        or five_hour_profiles < 0
        or weekly_profiles < 0
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID
    if ready_profiles == 0:
        return quota_model_policy_copy_label(
            StringSlice("Unavailable"),
            output_address,
            output_capacity,
            written_address,
        )

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = QuotaDisplayWriter(output, output_capacity, 0)
    var any = False
    if five_hour_profiles > 0:
        if (
            not quota_display_put_literal(Pointer(to=writer), StringSlice("5h "))
            or not quota_display_put_i64(
                Pointer(to=writer), five_hour_remaining
            )
            or not quota_display_put_byte(Pointer(to=writer), UInt8(37))
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        any = True
    if weekly_profiles > 0:
        if any and not quota_display_put_literal(
            Pointer(to=writer), StringSlice(" | ")
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        if (
            not quota_display_put_literal(
                Pointer(to=writer), StringSlice("weekly ")
            )
            or not quota_display_put_i64(Pointer(to=writer), weekly_remaining)
            or not quota_display_put_byte(Pointer(to=writer), UInt8(37))
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
    if (
        not quota_display_put_literal(
            Pointer(to=writer), StringSlice(" across ")
        )
        or not quota_display_put_i64(Pointer(to=writer), ready_profiles)
        or not quota_display_put_literal(
            Pointer(to=writer), StringSlice(" ready profile(s)")
        )
    ):
        return QUOTA_MODEL_POLICY_CAPACITY
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_info_pool_remaining_v1")
def prodex_quota_info_pool_remaining_v1(
    abi_version: Int64,
    total_remaining: Int64,
    profiles_with_data: Int64,
    reset_address: UInt,
    reset_length: Int64,
    reset_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        profiles_with_data < 0
        or reset_length < 0
        or (reset_present != 0 and reset_present != 1)
        or (reset_present == 1 and reset_length > 0 and reset_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID
    if profiles_with_data == 0:
        return quota_model_policy_copy_label(
            StringSlice("Unavailable"),
            output_address,
            output_capacity,
            written_address,
        )

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = QuotaDisplayWriter(output, output_capacity, 0)
    if (
        not quota_display_put_i64(Pointer(to=writer), total_remaining)
        or not quota_display_put_literal(
            Pointer(to=writer), StringSlice("% across ")
        )
        or not quota_display_put_i64(Pointer(to=writer), profiles_with_data)
        or not quota_display_put_literal(
            Pointer(to=writer), StringSlice(" profile(s)")
        )
    ):
        return QUOTA_MODEL_POLICY_CAPACITY

    if reset_present == 1:
        var reset = quota_model_policy_view(reset_address, reset_length)
        if not rich_view_valid(reset, reset_length):
            return QUOTA_MODEL_POLICY_INVALID
        if (
            not quota_display_put_literal(
                Pointer(to=writer), StringSlice("; earliest reset ")
            )
            or not quota_display_put_view(Pointer(to=writer), reset)
        ):
            return QUOTA_MODEL_POLICY_CAPACITY

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


def quota_gemini_copy_label(
    model_address: UInt,
    model_length: Int64,
    model_present: Int64,
    token_address: UInt,
    token_length: Int64,
    token_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) -> Int64:
    if (
        model_length < 0
        or token_length < 0
        or (model_present != 0 and model_present != 1)
        or (token_present != 0 and token_present != 1)
        or (model_present == 1 and model_length > 0 and model_address == 0)
        or (token_present == 1 and token_length > 0 and token_address == 0)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    var writer = QuotaDisplayWriter(output, output_capacity, 0)

    if model_present == 1:
        var model = quota_model_policy_view(model_address, model_length)
        if not rich_view_valid(model, model_length):
            return QUOTA_MODEL_POLICY_INVALID
        var bounds = rich_trim_bounds(model)
        if bounds[1] > bounds[0]:
            var trimmed = ProdexRichStringView(
                UInt(Int(model.ptr) + Int(bounds[0])),
                UInt(bounds[1] - bounds[0]),
            )
            if rich_view_prefix["models/"](trimmed, False):
                trimmed = ProdexRichStringView(
                    trimmed.ptr + UInt(7), trimmed.len - UInt(7)
                )
            if not quota_display_put_view(Pointer(to=writer), trimmed):
                return QUOTA_MODEL_POLICY_CAPACITY
            written[] = writer.written
            return QUOTA_MODEL_POLICY_OK

    if token_present == 1:
        var token = quota_model_policy_view(token_address, token_length)
        if not rich_view_valid(token, token_length):
            return QUOTA_MODEL_POLICY_INVALID
        var bounds = rich_trim_bounds(token)
        if bounds[1] > bounds[0]:
            var source = rich_view_ptr(token)
            for index in range(bounds[0], bounds[1]):
                var byte = source[unsafe_offset=index]
                if byte >= 65 and byte <= 90:
                    byte += 32
                if not quota_display_put_byte(Pointer(to=writer), byte):
                    return QUOTA_MODEL_POLICY_CAPACITY
            written[] = writer.written
            return QUOTA_MODEL_POLICY_OK

    if not quota_display_put_literal(Pointer(to=writer), StringSlice("gemini")):
        return QUOTA_MODEL_POLICY_CAPACITY
    written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_gemini_bucket_label_v1")
def prodex_quota_gemini_bucket_label_v1(
    abi_version: Int64,
    model_address: UInt,
    model_length: Int64,
    model_present: Int64,
    token_address: UInt,
    token_length: Int64,
    token_present: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    return quota_gemini_copy_label(
        model_address,
        model_length,
        model_present,
        token_address,
        token_length,
        token_present,
        output_address,
        output_capacity,
        written_address,
    )


@export("prodex_quota_gemini_bucket_summary_v1")
def prodex_quota_gemini_bucket_summary_v1(
    abi_version: Int64,
    label_address: UInt,
    label_length: Int64,
    remaining_present: Int64,
    remaining: Int64,
    total_present: Int64,
    total: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        label_length < 0
        or (label_length > 0 and label_address == 0)
        or (remaining_present != 0 and remaining_present != 1)
        or (total_present != 0 and total_present != 1)
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID
    var label = quota_model_policy_view(label_address, label_length)
    if not rich_view_valid(label, label_length):
        return QUOTA_MODEL_POLICY_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var writer = QuotaDisplayWriter(output, output_capacity, 0)
    if not quota_display_put_view(Pointer(to=writer), label):
        return QUOTA_MODEL_POLICY_CAPACITY
    if remaining_present == 0:
        if not quota_display_put_literal(
            Pointer(to=writer), StringSlice(" quota unknown")
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
    else:
        if not quota_display_put_byte(Pointer(to=writer), UInt8(32)):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_i64(Pointer(to=writer), remaining):
            return QUOTA_MODEL_POLICY_CAPACITY
        if total_present == 1:
            if not quota_display_put_byte(Pointer(to=writer), UInt8(47)):
                return QUOTA_MODEL_POLICY_CAPACITY
            if not quota_display_put_i64(Pointer(to=writer), total):
                return QUOTA_MODEL_POLICY_CAPACITY

    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_gemini_display_v1")
def prodex_quota_gemini_display_v1(
    abi_version: Int64,
    remaining_address: UInt,
    remaining_present_address: UInt,
    percent_address: UInt,
    percent_present_address: UInt,
    exhausted_address: UInt,
    count: Int64,
    status_output_address: UInt,
    status_output_capacity: Int64,
    status_written_address: UInt,
    main_output_address: UInt,
    main_output_capacity: Int64,
    main_written_address: UInt,
    ready_address: UInt,
    min_percent_address: UInt,
    min_percent_present_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        count < 0
        or (count > 0 and remaining_address == 0)
        or (count > 0 and remaining_present_address == 0)
        or (count > 0 and percent_address == 0)
        or (count > 0 and percent_present_address == 0)
        or (count > 0 and exhausted_address == 0)
        or status_output_address == 0
        or status_output_capacity < 0
        or status_written_address == 0
        or main_output_address == 0
        or main_output_capacity < 0
        or main_written_address == 0
        or ready_address == 0
        or min_percent_address == 0
        or min_percent_present_address == 0
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var remaining_values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(remaining_address)
    )
    var remaining_present = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(remaining_present_address)
    )
    var percent_values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(percent_address)
    )
    var percent_present = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(percent_present_address)
    )
    var exhausted = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(exhausted_address)
    )

    var blocked = False
    var have_percent = False
    var min_percent: Int64 = 0
    var have_remaining = False
    var min_remaining: Int64 = 0

    for index in range(count):
        var has_remaining = remaining_present[unsafe_offset=index]
        var has_percent = percent_present[unsafe_offset=index]
        var is_exhausted = exhausted[unsafe_offset=index]
        if (
            (has_remaining != 0 and has_remaining != 1)
            or (has_percent != 0 and has_percent != 1)
            or (is_exhausted != 0 and is_exhausted != 1)
        ):
            return QUOTA_MODEL_POLICY_INVALID
        if is_exhausted == 1:
            blocked = True
        if has_percent == 1:
            var value = percent_values[unsafe_offset=index]
            if not have_percent or value < min_percent:
                min_percent = value
                have_percent = True
        if has_remaining == 1:
            var value = remaining_values[unsafe_offset=index]
            if not have_remaining or value < min_remaining:
                min_remaining = value
                have_remaining = True

    var ready = count > 0 and not blocked
    var ready_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(ready_address)
    )
    ready_output[] = Int64(ready)
    var min_percent_output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(min_percent_address)
    )
    var min_percent_present_output = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(min_percent_present_address))
    min_percent_output[] = min_percent if have_percent else 0
    min_percent_present_output[] = Int64(have_percent)

    var status = (
        StringSlice("Unknown")
        if count == 0
        else (StringSlice("Blocked") if blocked else StringSlice("Ready"))
    )
    var status_code = quota_model_policy_copy_label(
        status,
        status_output_address,
        status_output_capacity,
        status_written_address,
    )
    if status_code != QUOTA_MODEL_POLICY_OK:
        return status_code

    var main_output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(main_output_address)
    )
    var writer = QuotaDisplayWriter(main_output, main_output_capacity, 0)
    if count == 0:
        if not quota_display_put_literal(Pointer(to=writer), StringSlice("-")):
            return QUOTA_MODEL_POLICY_CAPACITY
    elif have_percent:
        if not quota_display_put_literal(
            Pointer(to=writer), StringSlice("gemini ")
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_i64(Pointer(to=writer), min_percent):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_byte(Pointer(to=writer), UInt8(37)):
            return QUOTA_MODEL_POLICY_CAPACITY
        if count != 1:
            if not quota_display_put_literal(
                Pointer(to=writer), StringSlice(" (")
            ):
                return QUOTA_MODEL_POLICY_CAPACITY
            if not quota_display_put_i64(Pointer(to=writer), count):
                return QUOTA_MODEL_POLICY_CAPACITY
            if not quota_display_put_literal(
                Pointer(to=writer), StringSlice(" buckets)")
            ):
                return QUOTA_MODEL_POLICY_CAPACITY
    elif have_remaining:
        if not quota_display_put_literal(
            Pointer(to=writer), StringSlice("gemini ")
        ):
            return QUOTA_MODEL_POLICY_CAPACITY
        if not quota_display_put_i64(Pointer(to=writer), min_remaining):
            return QUOTA_MODEL_POLICY_CAPACITY
    else:
        if not quota_display_put_literal(
            Pointer(to=writer), StringSlice("gemini quota unknown")
        ):
            return QUOTA_MODEL_POLICY_CAPACITY

    var main_written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(main_written_address)
    )
    main_written[] = writer.written
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_plan_capacity_pressure_scale_bps_v1")
def prodex_quota_plan_capacity_pressure_scale_bps_v1(
    abi_version: Int64,
    plan_address: UInt,
    plan_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or plan_length < 0
        or (plan_length > 0 and plan_address == 0)
    ):
        return -1
    var plan = quota_model_policy_view(plan_address, plan_length)
    if not rich_view_valid(plan, plan_length):
        return -1
    if (
        quota_plan_type_matches(plan, StringSlice("pro20x"))
        or quota_plan_type_matches(plan, StringSlice("pro20"))
        or quota_plan_type_matches(plan, StringSlice("20x"))
        or quota_plan_type_matches(plan, StringSlice("ultra"))
        or quota_plan_type_matches(plan, StringSlice("max"))
    ):
        return 2_000
    if (
        quota_plan_type_matches(plan, StringSlice("pro"))
        or quota_plan_type_matches(plan, StringSlice("prolite"))
        or quota_plan_type_matches(plan, StringSlice("pro5x"))
        or quota_plan_type_matches(plan, StringSlice("5x"))
    ):
        return 5_000
    if (
        quota_plan_type_matches(plan, StringSlice("free"))
        or quota_plan_type_matches(plan, StringSlice("basic"))
    ):
        return 12_000
    return 10_000


@export("prodex_quota_scale_pressure_for_plan_v1")
def prodex_quota_scale_pressure_for_plan_v1(
    abi_version: Int64,
    pressure: Int64,
    scale_bps: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if output_address == 0:
        return QUOTA_MODEL_POLICY_INVALID
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    if pressure == INT64_MAX:
        output[] = INT64_MAX
        return QUOTA_MODEL_POLICY_OK
    var scale = scale_bps
    if scale < 0:
        scale = 0
    var product = Int128(pressure) * Int128(scale)
    if product > Int128(INT64_MAX):
        product = Int128(INT64_MAX)
    elif product < Int128(INT64_MIN):
        product = Int128(INT64_MIN)
    output[] = Int64(product) / 10_000
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_auth_filter_parse_v1")
def prodex_quota_auth_filter_parse_v1(
    abi_version: Int64,
    input_address: UInt,
    input_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    kind_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION:
        return QUOTA_MODEL_POLICY_ABI
    if (
        input_length < 0
        or output_capacity < 0
        or written_address == 0
        or kind_address == 0
        or (input_length > 0 and input_address == 0)
        or (output_capacity > 0 and output_address == 0)
    ):
        return QUOTA_MODEL_POLICY_INVALID

    var view = quota_model_policy_view(input_address, input_length)
    if not rich_view_valid(view, input_length):
        return QUOTA_MODEL_POLICY_INVALID
    var bounds = rich_trim_bounds(view)
    if bounds[0] == bounds[1]:
        return QUOTA_MODEL_POLICY_INVALID

    var kind = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(kind_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    kind[] = QUOTA_AUTH_FILTER_LABEL
    written[] = 0

    if (
        quota_model_policy_trimmed_matches(view, StringSlice("all"))
        or quota_model_policy_trimmed_matches(view, StringSlice("*"))
    ):
        kind[] = QUOTA_AUTH_FILTER_ALL
        return QUOTA_MODEL_POLICY_OK
    if (
        quota_model_policy_trimmed_matches(
            view, StringSlice("quota-compatible")
        )
        or quota_model_policy_trimmed_matches(view, StringSlice("compatible"))
    ):
        kind[] = QUOTA_AUTH_FILTER_COMPATIBLE
        return QUOTA_MODEL_POLICY_OK
    if (
        quota_model_policy_trimmed_matches(
            view, StringSlice("non-quota-compatible")
        )
        or quota_model_policy_trimmed_matches(
            view, StringSlice("not-quota-compatible")
        )
        or quota_model_policy_trimmed_matches(
            view, StringSlice("quota-incompatible")
        )
        or quota_model_policy_trimmed_matches(view, StringSlice("incompatible"))
    ):
        kind[] = QUOTA_AUTH_FILTER_INCOMPATIBLE
        return QUOTA_MODEL_POLICY_OK

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var source = rich_view_ptr(view)
    var length = bounds[1] - bounds[0]
    if length > output_capacity:
        return QUOTA_MODEL_POLICY_CAPACITY
    for index in range(length):
        output[unsafe_offset=index] = quota_model_policy_ascii_lower(
            source[unsafe_offset=bounds[0] + index]
        )
    written[] = length
    return QUOTA_MODEL_POLICY_OK


@export("prodex_quota_auth_filter_matches_v1")
def prodex_quota_auth_filter_matches_v1(
    abi_version: Int64,
    kind: Int64,
    filter_address: UInt,
    filter_length: Int64,
    auth_address: UInt,
    auth_length: Int64,
    quota_compatible: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or kind < QUOTA_AUTH_FILTER_ALL
        or kind > QUOTA_AUTH_FILTER_INCOMPATIBLE
        or filter_length < 0
        or auth_length < 0
        or (filter_length > 0 and filter_address == 0)
        or (auth_length > 0 and auth_address == 0)
        or quota_compatible < 0
        or quota_compatible > 1
    ):
        return -1
    if kind == QUOTA_AUTH_FILTER_ALL:
        return 1
    if kind == QUOTA_AUTH_FILTER_COMPATIBLE:
        return quota_compatible
    if kind == QUOTA_AUTH_FILTER_INCOMPATIBLE:
        return 1 - quota_compatible

    var filter = quota_model_policy_view(filter_address, filter_length)
    var auth = quota_model_policy_view(auth_address, auth_length)
    if (
        not rich_view_valid(filter, filter_length)
        or not rich_view_valid(auth, auth_length)
        or filter.len != auth.len
    ):
        return 0
    var filter_ptr = rich_view_ptr(filter)
    var auth_ptr = rich_view_ptr(auth)
    for index in range(filter_length):
        if quota_model_policy_ascii_lower(
            filter_ptr[unsafe_offset=index]
        ) != quota_model_policy_ascii_lower(auth_ptr[unsafe_offset=index]):
            return 0
    return 1


def quota_report_compare_text(
    left: ProdexRichStringView,
    right: ProdexRichStringView,
) -> Int64:
    var left_bounds = rich_trim_bounds(left)
    var right_bounds = rich_trim_bounds(right)
    var left_length = left_bounds[1] - left_bounds[0]
    var right_length = right_bounds[1] - right_bounds[0]
    var left_ptr = rich_view_ptr(left)
    var right_ptr = rich_view_ptr(right)
    var common = min(left_length, right_length)
    for index in range(common):
        var left_byte = quota_model_policy_ascii_lower(
            left_ptr[unsafe_offset=left_bounds[0] + index]
        )
        var right_byte = quota_model_policy_ascii_lower(
            right_ptr[unsafe_offset=right_bounds[0] + index]
        )
        if left_byte < right_byte:
            return -1
        if left_byte > right_byte:
            return 1
    if left_length < right_length:
        return -1
    if left_length > right_length:
        return 1
    return 0


def quota_report_compare_i64(left: Int64, right: Int64) -> Int64:
    if left < right:
        return -1
    if left > right:
        return 1
    return 0


@export("prodex_quota_report_compare_v1")
def prodex_quota_report_compare_v1(
    abi_version: Int64,
    sort: Int64,
    left_active: Int64,
    right_active: Int64,
    left_status_rank: Int64,
    right_status_rank: Int64,
    left_reset_epoch: Int64,
    right_reset_epoch: Int64,
    left_profile_address: UInt,
    left_profile_length: Int64,
    right_profile_address: UInt,
    right_profile_length: Int64,
    left_auth_address: UInt,
    left_auth_length: Int64,
    right_auth_address: UInt,
    right_auth_length: Int64,
    left_account_address: UInt,
    left_account_length: Int64,
    right_account_address: UInt,
    right_account_length: Int64,
    left_plan_address: UInt,
    left_plan_length: Int64,
    right_plan_address: UInt,
    right_plan_length: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or sort < 0
        or sort > 5
        or (left_active != 0 and left_active != 1)
        or (right_active != 0 and right_active != 1)
        or left_status_rank < 0
        or right_status_rank < 0
    ):
        return -2

    if sort == 0:
        var active_order = quota_report_compare_i64(
            1 - left_active, 1 - right_active
        )
        if active_order != 0:
            return active_order
        return quota_report_compare_i64(left_status_rank, right_status_rank)

    if sort == 1:
        var rank_order = quota_report_compare_i64(
            left_status_rank, right_status_rank
        )
        if rank_order != 0:
            return rank_order
        return quota_report_compare_i64(left_reset_epoch, right_reset_epoch)

    var left_address = left_profile_address
    var left_length = left_profile_length
    var right_address = right_profile_address
    var right_length = right_profile_length
    if sort == 3:
        left_address = left_auth_address
        left_length = left_auth_length
        right_address = right_auth_address
        right_length = right_auth_length
    elif sort == 4:
        left_address = left_account_address
        left_length = left_account_length
        right_address = right_account_address
        right_length = right_account_length
    elif sort == 5:
        left_address = left_plan_address
        left_length = left_plan_length
        right_address = right_plan_address
        right_length = right_plan_length

    if (
        left_length < 0
        or right_length < 0
        or (left_length > 0 and left_address == 0)
        or (right_length > 0 and right_address == 0)
    ):
        return -2
    var left_text = quota_model_policy_view(left_address, left_length)
    var right_text = quota_model_policy_view(right_address, right_length)
    if (
        not rich_view_valid(left_text, left_length)
        or not rich_view_valid(right_text, right_length)
    ):
        return -2
    return quota_report_compare_text(left_text, right_text)


@export("prodex_quota_report_sort_next_v1")
def prodex_quota_report_sort_next_v1(
    abi_version: Int64, sort: Int64
) abi("C") -> Int64:
    if abi_version != QUOTA_MODEL_POLICY_ABI_VERSION or sort < 0 or sort > 5:
        return -1
    return (sort + 1) % 6


comptime QUOTA_AUTH_SUMMARY_CHATGPT: Int64 = 0
comptime QUOTA_AUTH_SUMMARY_BEDROCK_API_KEY: Int64 = 1
comptime QUOTA_AUTH_SUMMARY_API_KEY: Int64 = 2
comptime QUOTA_AUTH_SUMMARY_OTHER: Int64 = 3


@export("prodex_quota_auth_summary_kind_v1")
def prodex_quota_auth_summary_kind_v1(
    abi_version: Int64,
    auth_mode_address: UInt,
    auth_mode_length: Int64,
    auth_mode_present: Int64,
    has_chatgpt_token: Int64,
    has_api_key: Int64,
    has_bedrock_api_key: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or auth_mode_length < 0
        or (auth_mode_length > 0 and auth_mode_address == 0)
        or (auth_mode_present != 0 and auth_mode_present != 1)
        or (has_chatgpt_token != 0 and has_chatgpt_token != 1)
        or (has_api_key != 0 and has_api_key != 1)
        or (has_bedrock_api_key != 0 and has_bedrock_api_key != 1)
    ):
        return -1

    var mode = quota_model_policy_view(auth_mode_address, auth_mode_length)
    if auth_mode_present == 1 and not rich_view_valid(mode, auth_mode_length):
        return -1

    if (
        has_chatgpt_token == 1
        or (
            auth_mode_present == 1
            and quota_plan_type_matches(mode, StringSlice("chatgpt"))
        )
    ):
        return QUOTA_AUTH_SUMMARY_CHATGPT
    if (
        has_bedrock_api_key == 1
        or (
            auth_mode_present == 1
            and quota_plan_type_matches(mode, StringSlice("bedrockapikey"))
        )
    ):
        return QUOTA_AUTH_SUMMARY_BEDROCK_API_KEY
    if (
        has_api_key == 1
        or (
            auth_mode_present == 1
            and quota_plan_type_matches(mode, StringSlice("apikey"))
        )
    ):
        return QUOTA_AUTH_SUMMARY_API_KEY
    return QUOTA_AUTH_SUMMARY_OTHER


@export("prodex_quota_auth_proactive_refresh_v1")
def prodex_quota_auth_proactive_refresh_v1(
    abi_version: Int64,
    expires_at_present: Int64,
    expires_at: Int64,
    last_refresh_present: Int64,
    last_refresh: Int64,
    now: Int64,
    expiry_skew_seconds: Int64,
    refresh_interval_days: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or (expires_at_present != 0 and expires_at_present != 1)
        or (last_refresh_present != 0 and last_refresh_present != 1)
    ):
        return -1

    if expires_at_present == 1:
        var threshold = Int128(now) + Int128(expiry_skew_seconds)
        if threshold > Int128(INT64_MAX):
            threshold = Int128(INT64_MAX)
        elif threshold < Int128(INT64_MIN):
            threshold = Int128(INT64_MIN)
        return Int64(expires_at <= Int64(threshold))

    if last_refresh_present == 0:
        return 0

    var interval = Int128(refresh_interval_days) * Int128(86_400)
    var elapsed = Int128(now) - Int128(last_refresh)
    return Int64(elapsed >= interval)


comptime QUOTA_USAGE_AUTH_CHATGPT_ELIGIBLE: Int64 = 0
comptime QUOTA_USAGE_AUTH_BEDROCK_API_KEY: Int64 = 1
comptime QUOTA_USAGE_AUTH_API_KEY: Int64 = 2


@export("prodex_quota_usage_auth_kind_v1")
def prodex_quota_usage_auth_kind_v1(
    abi_version: Int64,
    auth_mode_address: UInt,
    auth_mode_length: Int64,
    auth_mode_present: Int64,
    has_api_key: Int64,
    has_bedrock_api_key: Int64,
) abi("C") -> Int64:
    if (
        abi_version != QUOTA_MODEL_POLICY_ABI_VERSION
        or auth_mode_length < 0
        or (auth_mode_length > 0 and auth_mode_address == 0)
        or (auth_mode_present != 0 and auth_mode_present != 1)
        or (has_api_key != 0 and has_api_key != 1)
        or (has_bedrock_api_key != 0 and has_bedrock_api_key != 1)
    ):
        return -1

    var mode = quota_model_policy_view(auth_mode_address, auth_mode_length)
    if auth_mode_present == 1 and not rich_view_valid(mode, auth_mode_length):
        return -1

    if (
        has_bedrock_api_key == 1
        or (
            auth_mode_present == 1
            and quota_plan_type_matches(mode, StringSlice("bedrockapikey"))
        )
    ):
        return QUOTA_USAGE_AUTH_BEDROCK_API_KEY
    if (
        has_api_key == 1
        or (
            auth_mode_present == 1
            and quota_plan_type_matches(mode, StringSlice("apikey"))
        )
    ):
        return QUOTA_USAGE_AUTH_API_KEY
    return QUOTA_USAGE_AUTH_CHATGPT_ELIGIBLE
