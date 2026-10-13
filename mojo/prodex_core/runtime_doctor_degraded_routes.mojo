from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView
from runtime_math import INT64_MAX

comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS: Int64 = 8
comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_INPUTS: Int64 = 65_536
comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_KEY_BYTES: Int64 = 4 * 1024
comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_GROUP_COUNT: Int64 = 4
comptime RUNTIME_DOCTOR_DEGRADED_ROUTES_GROUP_STARTS: Int64 = 5

comptime ROUTE_GROUP_RETRY: Int64 = 0
comptime ROUTE_GROUP_TRANSPORT: Int64 = 1
comptime ROUTE_GROUP_CIRCUIT: Int64 = 2
comptime ROUTE_GROUP_SCORE: Int64 = 3

comptime ROUTE_KIND_CIRCUIT: Int64 = 0
comptime ROUTE_KIND_TRANSPORT: Int64 = 1
comptime ROUTE_KIND_RETRY: Int64 = 2
comptime ROUTE_KIND_BAD_PAIRING: Int64 = 3
comptime ROUTE_KIND_HEALTH: Int64 = 4

comptime ROUTE_RECORD_STRIDE: Int64 = 7

@fieldwise_init
struct ProdexRuntimeDoctorDegradedRouteRecord(Copyable):
    var kind: Int64
    var source_index: Int64
    var profile_start: Int64
    var profile_len: Int64
    var route_start: Int64
    var route_len: Int64
    var value: Int64


def degraded_route_prefix_length(kind: Int64) -> Int64:
    if kind == ROUTE_KIND_CIRCUIT:
        return Int64(StringSlice("__route_circuit__:").byte_length())
    if kind == ROUTE_KIND_TRANSPORT:
        return Int64(StringSlice("__route_transport_backoff__:").byte_length())
    if kind == ROUTE_KIND_BAD_PAIRING:
        return Int64(StringSlice("__route_bad_pairing__:").byte_length())
    return Int64(StringSlice("__route_health__:").byte_length())


def degraded_route_key_parts(
    key: ProdexRichStringView,
    kind: Int64,
)-> Tuple[Bool, Int64, Int64, Int64, Int64]:
    var prefix = degraded_route_prefix_length(kind)
    if key.len < UInt(prefix) or key.ptr == 0 and key.len > 0:
        return (False, 0, 0, 0, 0)
    var ptr = rich_view_ptr(key)
    var prefix_bytes = (
        StringSlice("__route_circuit__:")
        if kind == ROUTE_KIND_CIRCUIT
        else StringSlice("__route_transport_backoff__:")
        if kind == ROUTE_KIND_TRANSPORT
        else StringSlice("__route_bad_pairing__:")
        if kind == ROUTE_KIND_BAD_PAIRING
        else StringSlice("__route_health__:")
    )
    for index in range(prefix):
        if ptr[unsafe_offset=index] != prefix_bytes.unsafe_ptr()[unsafe_offset=index]:
            return (False, 0, 0, 0, 0)
    var separator = prefix
    while separator < Int64(key.len) and ptr[unsafe_offset=separator] != 58:
        separator += 1
    if separator >= Int64(key.len):
        return (False, 0, 0, 0, 0)
    return (
        True,
        separator + 1,
        Int64(key.len) - separator - 1,
        prefix,
        separator - prefix,
    )


def degraded_route_score(
    score: Int64, updated_at: Int64, now: Int64, decay_seconds: Int64
) -> Int64:
    if now <= updated_at:
        return score
    var elapsed = Int64(0)
    if updated_at < 0 and now > INT64_MAX + updated_at:
        elapsed = INT64_MAX
    else:
        elapsed = now - updated_at
    var decay = elapsed / decay_seconds
    if decay >= score:
        return 0
    return score - decay


def degraded_route_byte_compare(
    left: Pointer[mut=False, UInt8, _],
    left_len: Int64,
    right: Pointer[mut=False, UInt8, _],
    right_len: Int64,
) -> Int64:
    var common = min(left_len, right_len)
    for index in range(common):
        var left_byte = left[unsafe_offset=index]
        var right_byte = right[unsafe_offset=index]
        if left_byte < right_byte:
            return -1
        if left_byte > right_byte:
            return 1
    if left_len < right_len:
        return -1
    if left_len > right_len:
        return 1
    return 0


def degraded_route_literal_compare(
    left: StringSlice, right: StringSlice
) -> Int64:
    return degraded_route_byte_compare(
        left.unsafe_ptr(),
        Int64(left.byte_length()),
        right.unsafe_ptr(),
        Int64(right.byte_length()),
    )


def degraded_route_magnitude(value: Int64) -> UInt64:
    if value >= 0:
        return UInt64(value)
    return UInt64(-(value + 1)) + 1


def degraded_route_decimal_digits(value: Int64) -> Int64:
    var magnitude = degraded_route_magnitude(value)
    var digits: Int64 = 1
    while magnitude >= 10:
        magnitude = magnitude // 10
        digits += 1
    return digits


def degraded_route_decimal_compare(left: Int64, right: Int64) -> Int64:
    if left < 0 and right >= 0:
        return -1
    if left >= 0 and right < 0:
        return 1
    var left_digits = degraded_route_decimal_digits(left)
    var right_digits = degraded_route_decimal_digits(right)
    if left < 0:
        if left_digits > right_digits:
            return -1
        if left_digits < right_digits:
            return 1
    else:
        if left_digits < right_digits:
            return -1
        if left_digits > right_digits:
            return 1
    var left_magnitude = degraded_route_magnitude(left)
    var right_magnitude = degraded_route_magnitude(right)
    var divisor: UInt64 = 1
    for _ in range(left_digits - 1):
        divisor *= 10
    for _ in range(left_digits):
        var left_digit = left_magnitude // divisor % 10
        var right_digit = right_magnitude // divisor % 10
        if left_digit < right_digit:
            return 1 if left < 0 else -1
        if left_digit > right_digit:
            return -1 if left < 0 else 1
        if divisor > 1:
            divisor = divisor // 10
    return 0


def degraded_route_record_view(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    record: ProdexRuntimeDoctorDegradedRouteRecord,
    route: Bool,
) -> Tuple[Pointer[mut=False, UInt8, ImmUntrackedOrigin], Int64]:
    var key = keys[unsafe_offset=record.source_index].copy()
    var start = record.route_start if route else record.profile_start
    var length = record.route_len if route else record.profile_len
    return (
        Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(key.ptr) + Int(start)
        ),
        length,
    )


def degraded_route_route_compare(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    left: ProdexRuntimeDoctorDegradedRouteRecord,
    right: ProdexRuntimeDoctorDegradedRouteRecord,
) -> Int64:
    if left.route_start < 0 or right.route_start < 0:
        var left_fixed = StringSlice("retry") if left.kind == ROUTE_KIND_RETRY else StringSlice("transport")
        var right_fixed = StringSlice("retry") if right.kind == ROUTE_KIND_RETRY else StringSlice("transport")
        if left.route_start >= 0:
            var left_view = degraded_route_record_view(keys, left, True)
            return degraded_route_byte_compare(left_view[0], left_view[1], right_fixed.unsafe_ptr(), Int64(right_fixed.byte_length()))
        if right.route_start >= 0:
            var right_view = degraded_route_record_view(keys, right, True)
            return degraded_route_byte_compare(left_fixed.unsafe_ptr(), Int64(left_fixed.byte_length()), right_view[0], right_view[1])
        return degraded_route_literal_compare(left_fixed, right_fixed)
    var left_view = degraded_route_record_view(keys, left, True)
    var right_view = degraded_route_record_view(keys, right, True)
    return degraded_route_byte_compare(left_view[0], left_view[1], right_view[0], right_view[1])


def degraded_route_kind_suffix_compare(
    left: ProdexRuntimeDoctorDegradedRouteRecord,
    right: ProdexRuntimeDoctorDegradedRouteRecord,
    now: Int64,
) -> Int64:
    var left_label = StringSlice("")
    var right_label = StringSlice("")
    if left.kind == ROUTE_KIND_CIRCUIT:
        left_label = StringSlice("circuit=")
    elif left.kind == ROUTE_KIND_TRANSPORT:
        left_label = StringSlice("transport_backoff until=")
    elif left.kind == ROUTE_KIND_RETRY:
        left_label = StringSlice("retry_backoff until=")
    elif left.kind == ROUTE_KIND_BAD_PAIRING:
        left_label = StringSlice("bad_pairing=")
    else:
        left_label = StringSlice("health=")
    if right.kind == ROUTE_KIND_CIRCUIT:
        right_label = StringSlice("circuit=")
    elif right.kind == ROUTE_KIND_TRANSPORT:
        right_label = StringSlice("transport_backoff until=")
    elif right.kind == ROUTE_KIND_RETRY:
        right_label = StringSlice("retry_backoff until=")
    elif right.kind == ROUTE_KIND_BAD_PAIRING:
        right_label = StringSlice("bad_pairing=")
    else:
        right_label = StringSlice("health=")
    var comparison = degraded_route_literal_compare(left_label, right_label)
    if comparison != 0:
        return comparison
    if left.kind == ROUTE_KIND_CIRCUIT and right.kind == ROUTE_KIND_CIRCUIT:
        var left_state = StringSlice("open") if left.value > now else StringSlice("half-open")
        var right_state = StringSlice("open") if right.value > now else StringSlice("half-open")
        comparison = degraded_route_literal_compare(left_state, right_state)
        if comparison != 0:
            return comparison
        comparison = degraded_route_literal_compare(StringSlice(" until="), StringSlice(" until="))
        if comparison != 0:
            return comparison
    return degraded_route_decimal_compare(left.value, right.value)


def degraded_route_record_compare(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    left: ProdexRuntimeDoctorDegradedRouteRecord,
    right: ProdexRuntimeDoctorDegradedRouteRecord,
    now: Int64,
) -> Int64:
    var left_profile = degraded_route_record_view(keys, left, False)
    var right_profile = degraded_route_record_view(keys, right, False)
    var comparison = degraded_route_byte_compare(
        left_profile[0], left_profile[1], right_profile[0], right_profile[1]
    )
    if comparison != 0:
        return comparison
    comparison = degraded_route_literal_compare(StringSlice("/"), StringSlice("/"))
    if comparison != 0:
        return comparison
    comparison = degraded_route_route_compare(keys, left, right)
    if comparison != 0:
        return comparison
    comparison = degraded_route_literal_compare(StringSlice(" "), StringSlice(" "))
    if comparison != 0:
        return comparison
    return degraded_route_kind_suffix_compare(left, right, now)


def degraded_route_record_equal(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    left: ProdexRuntimeDoctorDegradedRouteRecord,
    right: ProdexRuntimeDoctorDegradedRouteRecord,
    now: Int64,
) -> Bool:
    return degraded_route_record_compare(keys, left, right, now) == 0


def degraded_route_record_write(
    output: Pointer[mut=True, ProdexRuntimeDoctorDegradedRouteRecord, _],
    index: Int64,
    record: ProdexRuntimeDoctorDegradedRouteRecord,
) -> None:
    output[unsafe_offset=index] = record.copy()


def degraded_route_add(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    output: Pointer[mut=True, ProdexRuntimeDoctorDegradedRouteRecord, _],
    output_count: Pointer[mut=True, Int64, _],
    record: ProdexRuntimeDoctorDegradedRouteRecord,
    now: Int64,
) -> None:
    var count = output_count[]
    for index in range(count):
        var current = output[unsafe_offset=index].copy()
        if degraded_route_record_equal(keys, current, record, now):
            return
    var position: Int64 = 0
    while position < count and degraded_route_record_compare(
        keys, output[unsafe_offset=position].copy(), record, now
    ) <= 0:
        position += 1
    if position >= RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS and count >= RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS:
        return
    var new_count = count
    if new_count < RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS:
        new_count += 1
    var last = new_count - 1
    while last > position:
        output[unsafe_offset=last] = output[unsafe_offset=last - 1].copy()
        last -= 1
    if position < RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS:
        degraded_route_record_write(output, position, record)
    output_count[] = new_count


def degraded_route_emit(
    keys: Pointer[mut=False, ProdexRichStringView, _],
    values: Pointer[mut=False, Int64, _],
    updated: Pointer[mut=False, Int64, _],
    group_start: Int64,
    group_end: Int64,
    group: Int64,
    now: Int64,
    health_decay_seconds: Int64,
    bad_pairing_decay_seconds: Int64,
    output: Pointer[mut=True, ProdexRuntimeDoctorDegradedRouteRecord, _],
    output_count: Pointer[mut=True, Int64, _],
) -> Int64:
    for index in range(group_start, group_end):
        var key = keys[unsafe_offset=index].copy()
        var value = values[unsafe_offset=index]
        if group == ROUTE_GROUP_SCORE and value < 0:
            return 2
        var record = ProdexRuntimeDoctorDegradedRouteRecord(
            kind=ROUTE_KIND_RETRY,
            source_index=index,
            profile_start=0,
            profile_len=Int64(key.len),
            route_start=-1,
            route_len=0,
            value=value,
        )
        if group == ROUTE_GROUP_RETRY:
            degraded_route_add(keys, output, output_count, record, now)
            continue
        if group == ROUTE_GROUP_TRANSPORT:
            record.kind = ROUTE_KIND_TRANSPORT
            var profile_start: Int64 = 0
            var profile_len: Int64 = Int64(key.len)
            var route_start: Int64 = -1
            var route_len: Int64 = 0
            var parts = degraded_route_key_parts(key, ROUTE_KIND_TRANSPORT)
            if parts[0]:
                record.profile_start = parts[1]
                record.profile_len = parts[2]
                record.route_start = parts[3]
                record.route_len = parts[4]
            degraded_route_add(keys, output, output_count, record, now)
            continue
        var parsed_kind = ROUTE_KIND_CIRCUIT if group == ROUTE_GROUP_CIRCUIT else ROUTE_KIND_HEALTH
        var parts = degraded_route_key_parts(key, parsed_kind)
        if group == ROUTE_GROUP_SCORE:
            var bad_parts = degraded_route_key_parts(key, ROUTE_KIND_BAD_PAIRING)
            if bad_parts[0]:
                parts = bad_parts
                parsed_kind = ROUTE_KIND_BAD_PAIRING
        if not parts[0]:
            continue
        if group == ROUTE_GROUP_CIRCUIT:
            record.kind = ROUTE_KIND_CIRCUIT
            record.value = value
        else:
            record.kind = parsed_kind
            var decay_seconds = (
                bad_pairing_decay_seconds
                if parsed_kind == ROUTE_KIND_BAD_PAIRING
                else health_decay_seconds
            )
            record.value = degraded_route_score(value, updated[unsafe_offset=index], now, decay_seconds)
            if record.value <= 0:
                continue
        record.source_index = index
        record.profile_start = parts[1]
        record.profile_len = parts[2]
        record.route_start = parts[3]
        record.route_len = parts[4]
        degraded_route_add(keys, output, output_count, record, now)
    return 0


@export("prodex_mojo_runtime_doctor_degraded_routes_v1")
def prodex_mojo_runtime_doctor_degraded_routes_v1(
    abi_version: Int64,
    now: Int64,
    health_decay_seconds: Int64,
    bad_pairing_decay_seconds: Int64,
    keys_address: UInt,
    values_address: UInt,
    updated_address: UInt,
    group_starts_address: UInt,
    total_count: Int64,
    output_address: UInt,
    output_capacity: Int64,
    output_count_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_DEGRADED_ROUTES_ABI_VERSION:
        return 4
    if (
        total_count < 0
        or total_count > RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_INPUTS
        or health_decay_seconds <= 0
        or bad_pairing_decay_seconds <= 0
        or output_capacity < RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS
        or output_count_address == 0
        or group_starts_address == 0
        or (total_count > 0 and (keys_address == 0 or values_address == 0 or updated_address == 0))
        or (output_capacity > 0 and output_address == 0)
    ):
        return 1
    var keys = Pointer[mut=False, ProdexRichStringView, ImmUntrackedOrigin](
        unsafe_from_address=Int(keys_address)
    )
    var values = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(values_address)
    )
    var updated = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(updated_address)
    )
    var starts = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(group_starts_address)
    )
    var output_count = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_count_address)
    )
    output_count[] = 0
    if total_count == 0:
        return 0
    for index in range(total_count):
        if not rich_view_valid(keys[unsafe_offset=index].copy(), RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_KEY_BYTES):
            return 2
    if starts[unsafe_offset=0] != 0:
        return 2
    if starts[unsafe_offset=4] != total_count:
        return 2
    for index in range(RUNTIME_DOCTOR_DEGRADED_ROUTES_GROUP_STARTS - 1):
        if starts[unsafe_offset=index] < 0 or starts[unsafe_offset=index] > starts[unsafe_offset=index + 1]:
            return 2
    var output = Pointer[mut=True, ProdexRuntimeDoctorDegradedRouteRecord, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var status = degraded_route_emit(keys, values, updated, starts[unsafe_offset=0], starts[unsafe_offset=1], ROUTE_GROUP_RETRY, now, health_decay_seconds, bad_pairing_decay_seconds, output, output_count)
    if status != 0:
        return status
    status = degraded_route_emit(keys, values, updated, starts[unsafe_offset=1], starts[unsafe_offset=2], ROUTE_GROUP_TRANSPORT, now, health_decay_seconds, bad_pairing_decay_seconds, output, output_count)
    if status != 0:
        return status
    status = degraded_route_emit(keys, values, updated, starts[unsafe_offset=2], starts[unsafe_offset=3], ROUTE_GROUP_CIRCUIT, now, health_decay_seconds, bad_pairing_decay_seconds, output, output_count)
    if status != 0:
        return status
    status = degraded_route_emit(keys, values, updated, starts[unsafe_offset=3], starts[unsafe_offset=4], ROUTE_GROUP_SCORE, now, health_decay_seconds, bad_pairing_decay_seconds, output, output_count)
    if status != 0:
        return status
    return 0
