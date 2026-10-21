from std.memory import Pointer

comptime RUNTIME_STATE_QUOTA_ABI_VERSION: Int64 = 1
comptime RUNTIME_STATE_QUOTA_OK: Int64 = 0
comptime RUNTIME_STATE_QUOTA_INVALID: Int64 = 1
comptime RUNTIME_STATE_QUOTA_ABI: Int64 = 4

comptime MODE_TIMESTAMP_PERSIST: Int64 = 0
comptime MODE_FRESHNESS: Int64 = 1
comptime MODE_SNAPSHOT_USABLE: Int64 = 2
comptime MODE_PROBE_APPLY: Int64 = 3
comptime MODE_CACHED_SOURCE: Int64 = 4
comptime MODE_MODEL_CACHED_SOURCE: Int64 = 5
comptime MODE_MODEL_FINALIZE: Int64 = 6
comptime MODE_UNKNOWN_WINDOW: Int64 = 7
comptime MODE_USAGE_SNAPSHOT_RETAIN: Int64 = 8
comptime MODE_STARTUP_PROBE_REFRESH: Int64 = 9

comptime CACHED_SUMMARY_UNKNOWN: Int64 = 0
comptime CACHED_SUMMARY_LIVE: Int64 = 1
comptime CACHED_SUMMARY_SNAPSHOT: Int64 = 2
comptime CACHED_SUMMARY_RETIRED: Int64 = 3

comptime CACHED_SOURCE_NONE: Int64 = 0
comptime CACHED_SOURCE_LIVE: Int64 = 1
comptime CACHED_SOURCE_SNAPSHOT: Int64 = 2

comptime CACHED_MODEL_STANDARD: Int64 = 0
comptime CACHED_MODEL_LUNA: Int64 = 1
comptime CACHED_MODEL_RETIRED: Int64 = 2

comptime INT64_MAX: Int64 = 9223372036854775807
comptime INT64_MIN: Int64 = -9223372036854775808
comptime LOG_FIELDS_MAX_TEXT_BYTES: Int64 = 64
comptime LOG_FIELDS_OK: Int64 = 0
comptime LOG_FIELDS_INVALID: Int64 = 1
comptime LOG_FIELDS_CAPACITY: Int64 = 2

@fieldwise_init
struct RuntimeStateQuotaLogWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def state_quota_log_put_byte(
    writer: Pointer[mut=True, RuntimeStateQuotaLogWriter, _], byte: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = byte
    writer[].written += 1
    return True


def state_quota_log_put_literal(
    writer: Pointer[mut=True, RuntimeStateQuotaLogWriter, _], value: StringSlice
) -> Bool:
    var source = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not state_quota_log_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def state_quota_log_put_text(
    writer: Pointer[mut=True, RuntimeStateQuotaLogWriter, _],
    address: UInt,
    length: Int64,
) -> Bool:
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    for index in range(length):
        if not state_quota_log_put_byte(writer, source[unsafe_offset=index]):
            return False
    return True


def state_quota_log_put_u64(
    writer: Pointer[mut=True, RuntimeStateQuotaLogWriter, _], value: UInt64
) -> Bool:
    if value == 0:
        return state_quota_log_put_byte(writer, UInt8(48))
    var divisor: UInt64 = 1
    while value / divisor >= UInt64(10):
        divisor *= UInt64(10)
    var remaining = value
    while divisor > 0:
        if not state_quota_log_put_byte(
            writer, UInt8(remaining / divisor) + UInt8(48)
        ):
            return False
        remaining %= divisor
        divisor //= UInt64(10)
    return True


def state_quota_log_put_i64(
    writer: Pointer[mut=True, RuntimeStateQuotaLogWriter, _], value: Int64
) -> Bool:
    if value >= 0:
        return state_quota_log_put_u64(writer, UInt64(value))
    if not state_quota_log_put_byte(writer, UInt8(45)):
        return False
    var magnitude = (
        UInt64(9_223_372_036_854_775_808)
        if value == INT64_MIN
        else UInt64(-value)
    )
    return state_quota_log_put_u64(writer, magnitude)


@export("prodex_runtime_state_quota_summary_log_fields_v1")
def prodex_runtime_state_quota_summary_log_fields_v1(
    abi_version: Int64,
    band_address: UInt,
    band_length: Int64,
    five_hour_status_address: UInt,
    five_hour_status_length: Int64,
    five_hour_remaining: Int64,
    five_hour_reset_at: Int64,
    weekly_status_address: UInt,
    weekly_status_length: Int64,
    weekly_remaining: Int64,
    weekly_reset_at: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_QUOTA_ABI_VERSION:
        return RUNTIME_STATE_QUOTA_ABI
    if (
        band_address == 0
        or band_length <= 0
        or band_length > LOG_FIELDS_MAX_TEXT_BYTES
        or five_hour_status_address == 0
        or five_hour_status_length <= 0
        or five_hour_status_length > LOG_FIELDS_MAX_TEXT_BYTES
        or weekly_status_address == 0
        or weekly_status_length <= 0
        or weekly_status_length > LOG_FIELDS_MAX_TEXT_BYTES
        or output_address == 0
        or output_capacity < 0
        or written_address == 0
    ):
        return LOG_FIELDS_INVALID

    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    var writer = RuntimeStateQuotaLogWriter(output, output_capacity, 0)
    var writer_ptr = Pointer(to=writer)
    if (
        not state_quota_log_put_literal(
            writer_ptr,
            StringSlice("quota_band="),
        )
    ):
        return LOG_FIELDS_CAPACITY
    if (
        not state_quota_log_put_text(writer_ptr, band_address, band_length)
        or not state_quota_log_put_literal(writer_ptr, StringSlice(" five_hour_status="))
        or not state_quota_log_put_text(
            writer_ptr, five_hour_status_address, five_hour_status_length
        )
        or not state_quota_log_put_literal(
            writer_ptr, StringSlice(" five_hour_remaining=")
        )
        or not state_quota_log_put_i64(writer_ptr, five_hour_remaining)
        or not state_quota_log_put_literal(
            writer_ptr, StringSlice(" five_hour_reset_at=")
        )
        or not state_quota_log_put_i64(writer_ptr, five_hour_reset_at)
        or not state_quota_log_put_literal(writer_ptr, StringSlice(" weekly_status="))
        or not state_quota_log_put_text(
            writer_ptr, weekly_status_address, weekly_status_length
        )
        or not state_quota_log_put_literal(
            writer_ptr, StringSlice(" weekly_remaining=")
        )
        or not state_quota_log_put_i64(writer_ptr, weekly_remaining)
        or not state_quota_log_put_literal(
            writer_ptr, StringSlice(" weekly_reset_at=")
        )
        or not state_quota_log_put_i64(writer_ptr, weekly_reset_at)
    ):
        return LOG_FIELDS_CAPACITY
    written[] = writer.written
    return LOG_FIELDS_OK


def state_quota_valid_bool(value: Int64) -> Bool:
    return value == 0 or value == 1


def state_quota_saturating_sub(left: Int64, right: Int64) -> Int64:
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    return left - right


def state_quota_saturating_add(left: Int64, right: Int64) -> Int64:
    if right > 0 and left > INT64_MAX - right:
        return INT64_MAX
    if right < 0 and left < INT64_MIN - right:
        return INT64_MIN
    return left + right


@export("prodex_runtime_state_quota_policy_v1")
def prodex_runtime_state_quota_policy_v1(
    abi_version: Int64,
    mode: Int64,
    input0: Int64,
    input1: Int64,
    input2: Int64,
    input3: Int64,
    input4: Int64,
    input5: Int64,
    input6: Int64,
    input7: Int64,
    input8: Int64,
    input9: Int64,
    input10: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_QUOTA_ABI_VERSION:
        return RUNTIME_STATE_QUOTA_ABI
    if mode < MODE_TIMESTAMP_PERSIST or mode > MODE_STARTUP_PROBE_REFRESH or output_address == 0:
        return RUNTIME_STATE_QUOTA_INVALID

    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(8):
        output[unsafe_offset=index] = 0

    if mode == MODE_TIMESTAMP_PERSIST:
        output[unsafe_offset=0] = Int64(
            state_quota_saturating_sub(input1, input0) > input2
        )
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_FRESHNESS:
        var age = state_quota_saturating_sub(input1, input0)
        if age <= input2:
            output[unsafe_offset=0] = 0
        elif age <= input3:
            output[unsafe_offset=0] = 1
        else:
            output[unsafe_offset=0] = 2
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_SNAPSHOT_USABLE:
        if not state_quota_valid_bool(input0) or not state_quota_valid_bool(input2):
            return RUNTIME_STATE_QUOTA_INVALID
        var five_active = (
            input0 == 1 and input1 != INT64_MAX and input1 > input5
        )
        var weekly_active = (
            input2 == 1 and input3 != INT64_MAX and input3 > input5
        )
        var five_expired = (
            input0 == 1 and input1 != INT64_MAX and input1 <= input5
        )
        var weekly_expired = (
            input2 == 1 and input3 != INT64_MAX and input3 <= input5
        )
        var hold_active = five_active or weekly_active
        var hold_expired = five_expired or weekly_expired
        output[unsafe_offset=1] = Int64(hold_active)
        output[unsafe_offset=2] = Int64(hold_expired)

        if hold_active:
            output[unsafe_offset=0] = 1
        elif hold_expired:
            output[unsafe_offset=0] = 0
        else:
            output[unsafe_offset=0] = Int64(
                state_quota_saturating_sub(input5, input4) <= input6
            )
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_CACHED_SOURCE:
        if (
            not state_quota_valid_bool(input0)
            or not state_quota_valid_bool(input1)
            or not state_quota_valid_bool(input2)
        ):
            return RUNTIME_STATE_QUOTA_INVALID
        if input0 == 1:
            output[unsafe_offset=0] = CACHED_SUMMARY_LIVE
            output[unsafe_offset=1] = CACHED_SOURCE_LIVE
        elif input1 == 1 and input2 == 1:
            output[unsafe_offset=0] = CACHED_SUMMARY_SNAPSHOT
            output[unsafe_offset=1] = CACHED_SOURCE_SNAPSHOT
        else:
            output[unsafe_offset=0] = CACHED_SUMMARY_UNKNOWN
            output[unsafe_offset=1] = CACHED_SOURCE_NONE
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_MODEL_CACHED_SOURCE:
        if (
            input0 < CACHED_MODEL_STANDARD
            or input0 > CACHED_MODEL_RETIRED
            or not state_quota_valid_bool(input1)
            or not state_quota_valid_bool(input2)
            or not state_quota_valid_bool(input3)
            or not state_quota_valid_bool(input4)
            or not state_quota_valid_bool(input5)
        ):
            return RUNTIME_STATE_QUOTA_INVALID

        if input0 == CACHED_MODEL_RETIRED:
            output[unsafe_offset=0] = CACHED_SUMMARY_RETIRED
            if input1 == 1:
                output[unsafe_offset=1] = CACHED_SOURCE_LIVE
            elif input2 == 1 and input3 == 1:
                output[unsafe_offset=1] = CACHED_SOURCE_SNAPSHOT
            else:
                output[unsafe_offset=1] = CACHED_SOURCE_NONE
            return RUNTIME_STATE_QUOTA_OK

        if input1 == 1:
            output[unsafe_offset=0] = CACHED_SUMMARY_LIVE
            output[unsafe_offset=1] = CACHED_SOURCE_LIVE
            return RUNTIME_STATE_QUOTA_OK

        if input4 == 1 and input2 == 1 and input5 == 0:
            output[unsafe_offset=0] = CACHED_SUMMARY_UNKNOWN
            output[unsafe_offset=1] = CACHED_SOURCE_NONE
            return RUNTIME_STATE_QUOTA_OK

        if input2 == 1 and input3 == 1:
            output[unsafe_offset=0] = CACHED_SUMMARY_SNAPSHOT
            output[unsafe_offset=1] = CACHED_SOURCE_SNAPSHOT
        else:
            output[unsafe_offset=0] = CACHED_SUMMARY_UNKNOWN
            output[unsafe_offset=1] = CACHED_SOURCE_NONE
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_MODEL_FINALIZE:
        if (
            input0 < CACHED_MODEL_STANDARD
            or input0 > CACHED_MODEL_RETIRED
            or not state_quota_valid_bool(input1)
        ):
            return RUNTIME_STATE_QUOTA_INVALID
        output[unsafe_offset=0] = Int64(
            input0 == CACHED_MODEL_LUNA and input1 == 1
        )
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_UNKNOWN_WINDOW:
        if (
            not state_quota_valid_bool(input0)
            or not state_quota_valid_bool(input1)
        ):
            return RUNTIME_STATE_QUOTA_INVALID
        output[unsafe_offset=0] = Int64(input0 == 0)
        if input0 == 0:
            output[unsafe_offset=1] = input2 if input1 == 1 else INT64_MAX
        return RUNTIME_STATE_QUOTA_OK

    if mode == MODE_USAGE_SNAPSHOT_RETAIN:
        if not state_quota_valid_bool(input0):
            return RUNTIME_STATE_QUOTA_INVALID
        output[unsafe_offset=0] = Int64(
            input0 == 1
            and input1 >= state_quota_saturating_sub(input2, input3)
        )
        return RUNTIME_STATE_QUOTA_OK

    # startup probe refresh:
    # 0 probe is fresh, 1 usage snapshot is usable, 2 remaining warm slots
    if mode == MODE_STARTUP_PROBE_REFRESH:
        if (
            not state_quota_valid_bool(input0)
            or not state_quota_valid_bool(input1)
            or input2 < 0
        ):
            return RUNTIME_STATE_QUOTA_INVALID
        output[unsafe_offset=0] = Int64(
            input2 > 0 and input0 == 0 and input1 == 0
        )
        output[unsafe_offset=1] = input2 - output[unsafe_offset=0]
        return RUNTIME_STATE_QUOTA_OK

    # probe apply:
    # 0 previous snapshot present
    # 1 snapshots materially match
    # 2 previous checked_at
    # 3 previous retry present
    # 4 previous retry until
    # 5 blocking reset present
    # 6 blocking reset
    # 7 quota blocked
    # 8 now
    # 9 quota quarantine fallback seconds
    # 10 touch persist interval seconds
    if (
        not state_quota_valid_bool(input0)
        or not state_quota_valid_bool(input1)
        or not state_quota_valid_bool(input3)
        or not state_quota_valid_bool(input5)
        or not state_quota_valid_bool(input7)
    ):
        return RUNTIME_STATE_QUOTA_INVALID


    var snapshot_should_persist = (
        input0 == 0
        or input1 == 0
        or state_quota_saturating_sub(input8, input2) > input10
    )
    output[unsafe_offset=0] = Int64(snapshot_should_persist)

    var blocking_present = input5 == 1 and input6 > input8
    output[unsafe_offset=1] = Int64(blocking_present)
    if blocking_present:
        output[unsafe_offset=2] = input6

    if input7 == 0:
        return RUNTIME_STATE_QUOTA_OK

    var quarantine_until = (
        input6 if blocking_present
        else state_quota_saturating_add(input8, input9)
    )
    var retry_until = quarantine_until
    if input3 == 1:
        retry_until = max(input4, quarantine_until)
    output[unsafe_offset=3] = 1
    output[unsafe_offset=4] = retry_until
    output[unsafe_offset=5] = Int64(input3 == 0 or retry_until != input4)
    return RUNTIME_STATE_QUOTA_OK


# Entire ordered startup-probe admission budget is compiled policy.
# Rust supplies preclassified observations and applies the selected flags;
# no per-row Rust remaining-budget decision is retained.
comptime STARTUP_PROBE_BATCH_ABI: Int64 = 1
comptime STARTUP_PROBE_BATCH_MAX: Int64 = 65536


@export("prodex_runtime_state_startup_probe_batch_v1")
def prodex_runtime_state_startup_probe_batch_v1(
    abi_version: Int64,
    rows_address: UInt, candidate_count: Int64,
    warm_remaining: Int64,
    output_address: UInt, output_capacity: Int64,
    remaining_address: UInt,
) abi("C") -> Int64:
    if abi_version != STARTUP_PROBE_BATCH_ABI:
        return RUNTIME_STATE_QUOTA_ABI
    if (
        candidate_count < 0 or candidate_count > STARTUP_PROBE_BATCH_MAX
        or warm_remaining < 0
        or (candidate_count > 0 and (rows_address == 0 or rows_address % 8 != 0))
        or output_address == 0 or output_address % 8 != 0
        or output_capacity < candidate_count
        or remaining_address == 0 or remaining_address % 8 != 0
    ):
        return RUNTIME_STATE_QUOTA_INVALID
    var rows = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(rows_address)
    )
    for index in range(candidate_count):
        if (
            not state_quota_valid_bool(rows[unsafe_offset=index * 2])
            or not state_quota_valid_bool(rows[unsafe_offset=index * 2 + 1])
        ):
            return RUNTIME_STATE_QUOTA_INVALID
    var selected = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var remaining = warm_remaining
    for index in range(candidate_count):
        var probe_fresh = rows[unsafe_offset=index * 2]
        var snapshot_usable = rows[unsafe_offset=index * 2 + 1]
        var take = remaining > 0 and probe_fresh == 0 and snapshot_usable == 0
        selected[unsafe_offset=index] = Int64(take)
        if take:
            remaining -= 1
    Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(remaining_address)
    )[] = remaining
    return RUNTIME_STATE_QUOTA_OK


# Compare provider-normalized quota snapshot facts and decide persistence.
# The host supplies only equality facts for generic status types and plan
# labels; numeric window comparisons and touch-time policy are compiled.
comptime QUOTA_SNAPSHOT_COMPARE_ABI: Int64 = 1
comptime QUOTA_SNAPSHOT_COMPARE_FIELD_COUNT: Int64 = 15


@export("prodex_runtime_state_quota_snapshot_compare_v1")
def prodex_runtime_state_quota_snapshot_compare_v1(
    abi_version: Int64, operation: Int64,
    facts_address: UInt, fact_count: Int64,
    result_address: UInt,
) abi("C") -> Int64:
    if abi_version != QUOTA_SNAPSHOT_COMPARE_ABI:
        return RUNTIME_STATE_QUOTA_ABI
    if (
        operation < 0 or operation > 1
        or facts_address == 0 or facts_address % 8 != 0
        or fact_count != QUOTA_SNAPSHOT_COMPARE_FIELD_COUNT
        or result_address == 0 or result_address % 8 != 0
    ):
        return RUNTIME_STATE_QUOTA_INVALID
    var facts = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    for index in range(8, 12):
        if not state_quota_valid_bool(facts[unsafe_offset=index]):
            return RUNTIME_STATE_QUOTA_INVALID
    var present = facts[unsafe_offset=8] == 1
    var same = present
    if present:
        for index in range(4):
            if facts[unsafe_offset=index] != facts[unsafe_offset=index + 4]:
                same = False
        if facts[unsafe_offset=9] != 1 or facts[unsafe_offset=10] != 1 or facts[unsafe_offset=11] != 1:
            same = False
    var persist = not same
    if operation == 1 and not persist:
        persist = state_quota_saturating_sub(
            facts[unsafe_offset=13], facts[unsafe_offset=12],
        ) > facts[unsafe_offset=14]
    var result = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(result_address)
    )
    result[unsafe_offset=0] = Int64(same)
    result[unsafe_offset=1] = Int64(persist)
    return RUNTIME_STATE_QUOTA_OK

# Reconstruct usage windows from persisted quota snapshot metadata.
# Status tags: 0 ready, 1 thin, 2 critical, 3 exhausted, 4 unknown.
# Rust provides enum tags and retains ownership of serde usage models.
comptime QUOTA_SNAPSHOT_REBUILD_ABI: Int64 = 1
comptime QUOTA_SNAPSHOT_UNKNOWN: Int64 = 4


@export("prodex_runtime_quota_snapshot_rebuild_v1")
def prodex_runtime_quota_snapshot_rebuild_v1(
    abi_version: Int64,
    facts_address: UInt,
    fact_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
) abi("C") -> Int64:
    if abi_version != QUOTA_SNAPSHOT_REBUILD_ABI:
        return 4
    if (
        fact_count != 6 or output_capacity != 6
        or facts_address == 0 or facts_address % 8 != 0
        or output_address == 0 or output_address % 8 != 0
    ):
        return 1
    var facts = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(facts_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(2):
        var status = facts[unsafe_offset=index * 3]
        if status < 0 or status > QUOTA_SNAPSHOT_UNKNOWN:
            return 1
    for index in range(2):
        var offset = index * 3
        var status = facts[unsafe_offset=offset]
        var remaining = facts[unsafe_offset=offset + 1]
        var reset_at = facts[unsafe_offset=offset + 2]
        if status == QUOTA_SNAPSHOT_UNKNOWN:
            output[unsafe_offset=offset] = 0
            output[unsafe_offset=offset + 1] = 0
            output[unsafe_offset=offset + 2] = 0
            continue
        var used: Int64 = 0
        if remaining <= 0:
            used = 100
        elif remaining < 100:
            used = 100 - remaining
        output[unsafe_offset=offset] = 1
        output[unsafe_offset=offset + 1] = used
        output[unsafe_offset=offset + 2] = Int64(reset_at != INT64_MAX)
    return 0
