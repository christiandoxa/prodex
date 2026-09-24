from std.memory import Pointer

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


@export("prodex_quota_window_pair_has_ready_limit")
def prodex_quota_window_pair_has_ready_limit(
    first_used_percent: Int64,
    first_has_value: Int64,
    second_used_percent: Int64,
    second_has_value: Int64,
) abi("C") -> Int64:
    if first_has_value == 0 and second_has_value == 0:
        return 0
    if first_has_value != 0 and first_used_percent >= 100:
        return 0
    if second_has_value != 0 and second_used_percent >= 100:
        return 0
    return 1


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


comptime QUOTA_CAPACITY_FIELD_COUNT: Int64 = 11
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


@export("prodex_quota_window_pressure")
def prodex_quota_window_pressure(
    remaining_percent: Int64,
    reset_at: Int64,
    now: Int64,
) abi("C") -> Int64:
    if remaining_percent < 0 or remaining_percent > 100:
        return 1

    var seconds_until_reset: Int64 = 0
    if reset_at == INT64_MAX:
        seconds_until_reset = INT64_MAX
    elif reset_at > now:
        seconds_until_reset = quota_saturating_sub(reset_at, now)

    return quota_capacity_pressure(
        seconds_until_reset,
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


@export("prodex_quota_capacity_batch")
def prodex_quota_capacity_batch(
    fields_address: UInt,
    lane_address: UInt,
    five_hour_remaining_address: UInt,
    weekly_remaining_address: UInt,
    five_hour_status_address: UInt,
    weekly_status_address: UInt,
    pressure_band_address: UInt,
    admission_allowed_address: UInt,
    pair_ready_address: UInt,
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
    if fields_address == 0 or lane_address == 0 or five_hour_remaining_address == 0 or weekly_remaining_address == 0 or five_hour_status_address == 0 or weekly_status_address == 0 or pressure_band_address == 0 or admission_allowed_address == 0 or pair_ready_address == 0 or usable_address == 0 or routing_eligible_address == 0 or reserve_floor_address == 0 or five_hour_pressure_address == 0 or weekly_pressure_address == 0 or total_pressure_address == 0:
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
    var usable = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(usable_address))
    var routing_eligible = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(routing_eligible_address))
    var reserve_floor = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(reserve_floor_address))
    var five_hour_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(five_hour_pressure_address))
    var weekly_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(weekly_pressure_address))
    var total_pressure = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(total_pressure_address))

    for index in range(count):
        var row_lane = quota_capacity_field(fields, index, 0)
        var allowed = quota_capacity_field(fields, index, 1)
        var limit_reached = quota_capacity_field(fields, index, 2)
        var five_hour_used = quota_capacity_field(fields, index, 3)
        var five_hour_has_value = quota_capacity_field(fields, index, 4)
        var five_hour_seconds = quota_capacity_field(fields, index, 5)
        var weekly_used = quota_capacity_field(fields, index, 6)
        var weekly_has_value = quota_capacity_field(fields, index, 7)
        var weekly_seconds = quota_capacity_field(fields, index, 8)
        var scale_bps = quota_capacity_field(fields, index, 9)
        var weekly_weight = quota_capacity_field(fields, index, 10)
        if row_lane < 0 or row_lane > 2 or allowed < 0 or allowed > 2 or limit_reached < 0 or limit_reached > 2:
            return 2
        if five_hour_has_value < 0 or five_hour_has_value > 1 or weekly_has_value < 0 or weekly_has_value > 1:
            return 2
        if five_hour_seconds < 0 or weekly_seconds < 0 or scale_bps < 0 or weekly_weight < 0:
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
            five_hour_seconds,
            five_hour_remaining_value,
            five_hour_has_value,
        )
        var weekly_pressure_value = quota_capacity_pressure(
            weekly_seconds,
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
        var admission_value: Int64 = 1
        if allowed == 2 or limit_reached == 2:
            admission_value = 0
        var pair_ready_value: Int64 = 0
        if five_hour_has_value == 1 or weekly_has_value == 1:
            pair_ready_value = 1
            if (five_hour_has_value == 1 and five_hour_remaining_value == 0) or (
                weekly_has_value == 1 and weekly_remaining_value == 0
            ):
                pair_ready_value = 0
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
                quota_capacity_saturating_mul(weekly_pressure_value, weekly_weight),
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
    var ready: Int64 = 0
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
