from std.memory import Pointer

comptime RUNTIME_STATE_BACKGROUND_ABI_VERSION: Int64 = 1
comptime RUNTIME_STATE_BACKGROUND_OK: Int64 = 0
comptime RUNTIME_STATE_BACKGROUND_INVALID: Int64 = 1
comptime RUNTIME_STATE_BACKGROUND_ABI: Int64 = 4

comptime RUNTIME_PROXY_ADMISSION_ABI_VERSION: Int64 = 1
comptime RUNTIME_PROXY_ADMISSION_OK: Int64 = 0
comptime RUNTIME_PROXY_ADMISSION_INVALID: Int64 = 1
comptime RUNTIME_PROXY_ADMISSION_ABI: Int64 = 4

comptime MODE_MUTATION_PLAN: Int64 = 0
comptime MODE_QUEUE_PRESSURE: Int64 = 1
comptime MODE_QUEUE_ENQUEUE: Int64 = 2
comptime MODE_ENQUEUE_BACKLOG: Int64 = 3
comptime MODE_QUEUE_THRESHOLD: Int64 = 4
comptime MODE_ADMISSION_PLAN: Int64 = 5
comptime MODE_PROFILE_INFLIGHT_ACQUIRE: Int64 = 6
comptime MODE_PROFILE_INFLIGHT_RELEASE: Int64 = 7
comptime MODE_LANE_LIMIT: Int64 = 8
comptime MODE_SECTIONS_UNION: Int64 = 9
comptime MODE_ADMISSION_RELEASE: Int64 = 10
comptime MODE_ROUTE_LABEL: Int64 = 11
comptime MODE_ROUTE_FROM_LABEL: Int64 = 12
comptime MODE_ROUTE_COUPLED: Int64 = 13
comptime MODE_ROUTE_BUILD_KEY: Int64 = 14
comptime MODE_ROUTE_KEY_PARTS: Int64 = 15
comptime MODE_ROUTE_PROFILE_SUFFIX: Int64 = 16
comptime MODE_ROUTE_CIRCUIT_HEALTH_KEY: Int64 = 17

comptime ADMISSION_ALLOW: UInt64 = 0
comptime ADMISSION_GLOBAL_LIMIT: UInt64 = 1
comptime ADMISSION_LANE_LIMIT: UInt64 = 2
comptime UINT64_MAX: UInt64 = 18_446_744_073_709_551_615

comptime SECTION_NONE: UInt64 = 0
comptime SECTION_CORE: UInt64 = 1
comptime SECTION_FULL: UInt64 = 2

comptime RUNTIME_PROXY_ROUTE_RESPONSES: Int64 = 0
comptime RUNTIME_PROXY_ROUTE_COMPACT: Int64 = 1
comptime RUNTIME_PROXY_ROUTE_WEBSOCKET: Int64 = 2
comptime RUNTIME_PROXY_ROUTE_STANDARD: Int64 = 3

comptime ROUTE_KEY_HEALTH: Int64 = 0
comptime ROUTE_KEY_BAD_PAIRING: Int64 = 1
comptime ROUTE_KEY_SUCCESS_STREAK: Int64 = 2
comptime ROUTE_KEY_PERFORMANCE: Int64 = 3
comptime ROUTE_KEY_CIRCUIT: Int64 = 4
comptime ROUTE_KEY_CIRCUIT_REOPEN: Int64 = 5
comptime ROUTE_KEY_TRANSPORT_BACKOFF: Int64 = 6


def runtime_state_flag(value: Bool) -> UInt64:
    return UInt64(1) if value else UInt64(0)


def runtime_state_route_input_valid(address: UInt, length: Int64) -> Bool:
    return length >= 0 and (length == 0 or address != 0)


def runtime_state_route_matches_literal(
    address: UInt, length: Int64, literal: StringSlice
) -> Bool:
    var literal_length = Int64(literal.byte_length())
    if length != literal_length:
        return False
    if literal_length == 0:
        return True
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    var expected = literal.unsafe_ptr()
    for index in range(literal_length):
        if source[unsafe_offset=index] != expected[unsafe_offset=index]:
            return False
    return True


def runtime_state_route_put_input(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    address: UInt,
    length: Int64,
) -> Bool:
    if length == 0:
        return True
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    return runtime_state_reason_put(output, capacity, written, source, length)


def runtime_state_route_put_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    return runtime_state_reason_put_literal(output, capacity, written, literal)


def runtime_state_route_label(
    route_kind: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if route_kind == RUNTIME_PROXY_ROUTE_RESPONSES:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("responses")
        )
    if route_kind == RUNTIME_PROXY_ROUTE_COMPACT:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("compact")
        )
    if route_kind == RUNTIME_PROXY_ROUTE_WEBSOCKET:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("websocket")
        )
    if route_kind == RUNTIME_PROXY_ROUTE_STANDARD:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("standard")
        )
    return False


def runtime_state_route_key_prefix(
    key_kind: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if key_kind == ROUTE_KEY_HEALTH:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_health__:")
        )
    if key_kind == ROUTE_KEY_BAD_PAIRING:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_bad_pairing__:")
        )
    if key_kind == ROUTE_KEY_SUCCESS_STREAK:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_success__:")
        )
    if key_kind == ROUTE_KEY_PERFORMANCE:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_performance__:")
        )
    if key_kind == ROUTE_KEY_CIRCUIT:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_circuit__:")
        )
    if key_kind == ROUTE_KEY_CIRCUIT_REOPEN:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_circuit_reopen__:")
        )
    if key_kind == ROUTE_KEY_TRANSPORT_BACKOFF:
        return runtime_state_route_put_literal(
            output, capacity, written, StringSlice("__route_transport_backoff__:")
        )
    return False


def runtime_state_route_key(
    key_kind: Int64,
    route_kind: Int64,
    profile_address: UInt,
    profile_length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    if not runtime_state_route_key_prefix(key_kind, output, capacity, written):
        return False
    if not runtime_state_route_label(route_kind, output, capacity, written):
        return False
    if not runtime_state_route_put_literal(
        output, capacity, written, StringSlice(":")
    ):
        return False
    return runtime_state_route_put_input(
        output, capacity, written, profile_address, profile_length
    )


def runtime_state_route_find_first_colon(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, Int64, _],
) -> Bool:
    if length == 0:
        return False
    var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
        unsafe_from_address=Int(address)
    )
    for index in range(length):
        if source[unsafe_offset=index] == UInt8(58):
            output[unsafe_offset=0] = 0
            output[unsafe_offset=1] = index
            output[unsafe_offset=2] = index + 1
            output[unsafe_offset=3] = length
            return True
    return False


def runtime_state_route_find_last_colon(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, Int64, _],
):
    var start: Int64 = 0
    if length > 0:
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(address)
        )
        for index in range(length):
            if source[unsafe_offset=index] == UInt8(58):
                start = Int64(index) + 1
    output[unsafe_offset=0] = start
    output[unsafe_offset=1] = length


def runtime_state_route_replace_circuit_health(
    address: UInt,
    length: Int64,
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
) -> Bool:
    var circuit = StringSlice("__route_circuit__")
    if runtime_state_route_input_valid(address, length) and length >= Int64(circuit.byte_length()):
        var source = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(address)
        )
        var expected = circuit.unsafe_ptr()
        var found: Int64 = -1
        for start in range(length - Int64(circuit.byte_length()) + 1):
            var matches = True
            for index in range(Int64(circuit.byte_length())):
                if source[unsafe_offset=start + index] != expected[unsafe_offset=index]:
                    matches = False
                    break
            if matches:
                found = start
                break
        if found >= 0:
            if not runtime_state_route_put_input(
                output, capacity, written, address, found
            ):
                return False
            if not runtime_state_route_put_literal(
                output, capacity, written, StringSlice("__route_health__")
            ):
                return False
            return runtime_state_route_put_input(
                output,
                capacity,
                written,
                address + UInt(found + Int64(circuit.byte_length())),
                length - found - Int64(circuit.byte_length()),
            )
    return runtime_state_route_put_input(output, capacity, written, address, length)


@export("prodex_runtime_state_route_policy_v1")
def prodex_runtime_state_route_policy_v1(
    abi_version: Int64,
    mode: Int64,
    route_kind: Int64,
    key_kind: Int64,
    key_address: UInt,
    key_length: Int64,
    prefix_address: UInt,
    prefix_length: Int64,
    profile_address: UInt,
    profile_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
    spans_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_BACKGROUND_ABI_VERSION:
        return RUNTIME_STATE_BACKGROUND_ABI
    if mode < MODE_ROUTE_LABEL or mode > MODE_ROUTE_CIRCUIT_HEALTH_KEY:
        return RUNTIME_STATE_BACKGROUND_INVALID

    if mode == MODE_ROUTE_LABEL:
        if (
            route_kind < RUNTIME_PROXY_ROUTE_RESPONSES
            or route_kind > RUNTIME_PROXY_ROUTE_STANDARD
            or output_address == 0
            or output_capacity <= 0
            or written_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = 0
        if not runtime_state_route_label(route_kind, output, output_capacity, written):
            return RUNTIME_STATE_BACKGROUND_INVALID
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ROUTE_FROM_LABEL:
        if (
            not runtime_state_route_input_valid(key_address, key_length)
            or spans_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(spans_address)
        )
        spans[unsafe_offset=0] = -1
        if runtime_state_route_matches_literal(
            key_address, key_length, StringSlice("responses")
        ):
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_RESPONSES
        elif runtime_state_route_matches_literal(
            key_address, key_length, StringSlice("compact")
        ):
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_COMPACT
        elif runtime_state_route_matches_literal(
            key_address, key_length, StringSlice("websocket")
        ):
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_WEBSOCKET
        elif runtime_state_route_matches_literal(
            key_address, key_length, StringSlice("standard")
        ):
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_STANDARD
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ROUTE_COUPLED:
        if (
            route_kind < RUNTIME_PROXY_ROUTE_RESPONSES
            or route_kind > RUNTIME_PROXY_ROUTE_STANDARD
            or spans_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(spans_address)
        )
        if route_kind == RUNTIME_PROXY_ROUTE_RESPONSES:
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_WEBSOCKET
        elif route_kind == RUNTIME_PROXY_ROUTE_COMPACT:
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_STANDARD
        elif route_kind == RUNTIME_PROXY_ROUTE_WEBSOCKET:
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_RESPONSES
        else:
            spans[unsafe_offset=0] = RUNTIME_PROXY_ROUTE_COMPACT
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ROUTE_BUILD_KEY:
        if (
            route_kind < RUNTIME_PROXY_ROUTE_RESPONSES
            or route_kind > RUNTIME_PROXY_ROUTE_STANDARD
            or key_kind < ROUTE_KEY_HEALTH
            or key_kind > ROUTE_KEY_TRANSPORT_BACKOFF
            or not runtime_state_route_input_valid(profile_address, profile_length)
            or output_address == 0
            or output_capacity <= 0
            or written_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
            unsafe_from_address=Int(output_address)
        )
        var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(written_address)
        )
        written[] = 0
        if not runtime_state_route_key(
            key_kind,
            route_kind,
            profile_address,
            profile_length,
            output,
            output_capacity,
            written,
        ):
            return 3
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ROUTE_KEY_PARTS:
        if (
            not runtime_state_route_input_valid(key_address, key_length)
            or not runtime_state_route_input_valid(prefix_address, prefix_length)
            or spans_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(spans_address)
        )
        for index in range(4):
            spans[unsafe_offset=index] = -1
        var key = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(key_address)
        )
        var prefix = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
            unsafe_from_address=Int(prefix_address)
        )
        if key_length < prefix_length:
            return RUNTIME_STATE_BACKGROUND_OK
        for index in range(prefix_length):
            if key[unsafe_offset=index] != prefix[unsafe_offset=index]:
                return RUNTIME_STATE_BACKGROUND_OK
        var rest_address = key_address + UInt(prefix_length)
        var rest_length = key_length - prefix_length
        var parts = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(spans_address)
        )
        if runtime_state_route_find_first_colon(rest_address, rest_length, parts):
            parts[unsafe_offset=0] += prefix_length
            parts[unsafe_offset=1] += prefix_length
            parts[unsafe_offset=2] += prefix_length
            parts[unsafe_offset=3] += prefix_length
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ROUTE_PROFILE_SUFFIX:
        if (
            not runtime_state_route_input_valid(key_address, key_length)
            or spans_address == 0
        ):
            return RUNTIME_STATE_BACKGROUND_INVALID
        var spans = Pointer[mut=True, Int64, MutUntrackedOrigin](
            unsafe_from_address=Int(spans_address)
        )
        runtime_state_route_find_last_colon(key_address, key_length, spans)
        return RUNTIME_STATE_BACKGROUND_OK

    if (
        not runtime_state_route_input_valid(key_address, key_length)
        or output_address == 0
        or output_capacity <= 0
        or written_address == 0
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not runtime_state_route_replace_circuit_health(
        key_address, key_length, output, output_capacity, written
    ):
        return 3
    return RUNTIME_STATE_BACKGROUND_OK


@export("prodex_runtime_proxy_admission_policy_v1")
def prodex_runtime_proxy_admission_policy_v1(
    abi_version: Int64,
    route_kind: Int64,
    local_overload_pressure: Int64,
    background_queue_pressure: Int64,
    session_profile_present: Int64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_PROXY_ADMISSION_ABI_VERSION:
        return RUNTIME_PROXY_ADMISSION_ABI
    if (
        route_kind < RUNTIME_PROXY_ROUTE_RESPONSES
        or route_kind > RUNTIME_PROXY_ROUTE_STANDARD
        or local_overload_pressure < 0
        or local_overload_pressure > 1
        or background_queue_pressure < 0
        or background_queue_pressure > 1
        or session_profile_present < 0
        or session_profile_present > 1
        or output_address == 0
    ):
        return RUNTIME_PROXY_ADMISSION_INVALID

    var background_affects_route = (
        route_kind == RUNTIME_PROXY_ROUTE_COMPACT
        or route_kind == RUNTIME_PROXY_ROUTE_STANDARD
    )
    var pressure_mode = (
        local_overload_pressure == 1
        or (
            background_queue_pressure == 1 and background_affects_route
        )
    )
    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    output[unsafe_offset=0] = runtime_state_flag(background_affects_route)
    output[unsafe_offset=1] = runtime_state_flag(pressure_mode)
    output[unsafe_offset=2] = runtime_state_flag(
        route_kind == RUNTIME_PROXY_ROUTE_RESPONSES
    )
    output[unsafe_offset=3] = runtime_state_flag(
        pressure_mode and session_profile_present == 0
    )
    return RUNTIME_PROXY_ADMISSION_OK


def runtime_state_reason_put(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    source: Pointer[mut=False, UInt8, _],
    length: Int64,
) -> Bool:
    if length < 0 or written[] > capacity - length:
        return False
    for index in range(length):
        output[unsafe_offset=written[] + index] = source[unsafe_offset=index]
    written[] += length
    return True


def runtime_state_reason_put_literal(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    literal: StringSlice,
) -> Bool:
    return runtime_state_reason_put(
        output,
        capacity,
        written,
        literal.unsafe_ptr(),
        Int64(literal.byte_length()),
    )


def runtime_state_put_mutation_label(
    output: Pointer[mut=True, UInt8, _],
    capacity: Int64,
    written: Pointer[mut=True, Int64, _],
    kind: Int64,
) -> Bool:
    if kind == 0: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("full_state"))
    if kind == 1: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("startup_audit"))
    if kind == 2: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("startup_continuation_migration"))
    if kind == 3: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("startup_backoff_soften"))
    if kind == 4: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("response_ids"))
    if kind == 5: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("previous_response_owner"))
    if kind == 6: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("previous_response_negative_cache"))
    if kind == 7: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("previous_response_release"))
    if kind == 8: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("response_touch"))
    if kind == 9: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("turn_state"))
    if kind == 10: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("turn_state_touch"))
    if kind == 11: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("session_id"))
    if kind == 12: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("session_touch"))
    if kind == 13: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("session_affinity_release"))
    if kind == 14: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("compact_lineage"))
    if kind == 15: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("compact_lineage_release"))
    if kind == 16: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("compact_session_touch"))
    if kind == 17: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("compact_turn_state_touch"))
    if kind == 18: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("dead_response_binding_clear"))
    if kind == 19: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("quota_release"))
    if kind == 20: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("auth_failed_release"))
    if kind == 21: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("continuation_stale"))
    if kind == 22: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_commit"))
    if kind == 23: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("usage_snapshot"))
    if kind == 24: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_retry_backoff"))
    if kind == 25: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_transport_backoff"))
    if kind == 26: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_circuit_half_open_probe"))
    if kind == 27: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_health"))
    if kind == 28: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_circuit_clear"))
    if kind == 29: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_bad_pairing"))
    if kind == 30: return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_auth_backoff"))
    return runtime_state_reason_put_literal(output, capacity, written, StringSlice("profile_auth_backoff_cleared"))


@export("prodex_runtime_state_mutation_reason_v1")
def prodex_runtime_state_mutation_reason_v1(
    abi_version: Int64,
    mutation_kind: Int64,
    value_address: UInt,
    value_length: Int64,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_BACKGROUND_ABI_VERSION:
        return RUNTIME_STATE_BACKGROUND_ABI
    if (
        mutation_kind < 0
        or mutation_kind > 31
        or value_length < 0
        or (value_length > 0 and value_address == 0)
        or output_address == 0
        or output_capacity <= 0
        or written_address == 0
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(written_address)
    )
    written[] = 0
    if not runtime_state_put_mutation_label(
        output, output_capacity, written, mutation_kind
    ):
        return 3
    if mutation_kind >= 4:
        var colon = StringSlice(":")
        if not runtime_state_reason_put(
            output,
            output_capacity,
            written,
            colon.unsafe_ptr(),
            1,
        ):
            return 3
        if value_length > 0:
            var value = Pointer[mut=False, UInt8, ImmUntrackedOrigin](
                unsafe_from_address=Int(value_address)
            )
            if not runtime_state_reason_put(
                output, output_capacity, written, value, value_length
            ):
                return 3
    return RUNTIME_STATE_BACKGROUND_OK



def runtime_state_mutation_plan(
    mutation_kind: Int64,
    output: Pointer[mut=True, UInt64, _],
):
    var state = SECTION_FULL
    var continuations = True
    var profile_scores = True
    var usage_snapshots = True
    var backoffs = True

    if (
        mutation_kind == 0
        or mutation_kind == 1
        or mutation_kind == 2
        or mutation_kind == 20
    ):
        pass
    elif (
        mutation_kind >= 4 and mutation_kind <= 19
        or mutation_kind == 21
    ):
        state = SECTION_CORE
        continuations = True
        profile_scores = mutation_kind >= 4 and mutation_kind <= 7
        usage_snapshots = False
        backoffs = False
    elif mutation_kind == 22:
        state = SECTION_CORE
        continuations = False
        profile_scores = True
        usage_snapshots = False
        backoffs = True
    elif mutation_kind == 23 or mutation_kind == 24:
        state = SECTION_NONE
        continuations = False
        profile_scores = False
        usage_snapshots = True
        backoffs = True
    elif (
        mutation_kind == 3
        or mutation_kind == 25
        or mutation_kind == 26
    ):
        state = SECTION_NONE
        continuations = False
        profile_scores = False
        usage_snapshots = False
        backoffs = True
    elif mutation_kind == 27 or mutation_kind == 28:
        state = SECTION_NONE
        continuations = False
        profile_scores = True
        usage_snapshots = False
        backoffs = True
    elif mutation_kind >= 29 and mutation_kind <= 31:
        state = SECTION_NONE
        continuations = False
        profile_scores = True
        usage_snapshots = False
        backoffs = False

    var requires_journal = (
        mutation_kind == 4
        or mutation_kind == 5
        or mutation_kind == 7
        or mutation_kind == 9
        or mutation_kind == 11
        or mutation_kind == 13
        or mutation_kind == 14
        or mutation_kind == 15
        or mutation_kind == 18
        or mutation_kind == 19
        or mutation_kind == 20
    )
    var hot = (
        mutation_kind == 4
        or mutation_kind == 5
        or mutation_kind == 8
        or mutation_kind == 9
        or mutation_kind == 10
        or mutation_kind == 11
        or mutation_kind == 12
        or mutation_kind == 14
        or mutation_kind == 15
        or mutation_kind == 16
        or mutation_kind == 17
    )

    output[unsafe_offset=0] = state
    output[unsafe_offset=1] = runtime_state_flag(continuations)
    output[unsafe_offset=2] = runtime_state_flag(profile_scores)
    output[unsafe_offset=3] = runtime_state_flag(usage_snapshots)
    output[unsafe_offset=4] = runtime_state_flag(backoffs)
    output[unsafe_offset=5] = runtime_state_flag(requires_journal)
    output[unsafe_offset=6] = runtime_state_flag(hot)


def runtime_state_sections_union(
    left_state: Int64,
    left_flags: UInt64,
    right_state: Int64,
    right_flags: UInt64,
    output: Pointer[mut=True, UInt64, _],
):
    var state = SECTION_NONE
    if left_state == Int64(SECTION_FULL) or right_state == Int64(SECTION_FULL):
        state = SECTION_FULL
    elif left_state == Int64(SECTION_CORE) or right_state == Int64(SECTION_CORE):
        state = SECTION_CORE
    output[unsafe_offset=0] = state
    output[unsafe_offset=1] = left_flags | right_flags


def runtime_state_u64_saturating_add(left: UInt64, right: UInt64) -> UInt64:
    if left > UINT64_MAX - right:
        return UINT64_MAX
    return left + right


def runtime_state_threshold(
    queue_kind: Int64,
    state_save: UInt64,
    continuation_journal: UInt64,
    probe_refresh: UInt64,
) -> UInt64:
    if queue_kind == 0:
        return state_save
    if queue_kind == 1:
        return continuation_journal
    return probe_refresh


@export("prodex_runtime_state_background_policy_v1")
def prodex_runtime_state_background_policy_v1(
    abi_version: Int64,
    mode: Int64,
    mutation_kind: Int64,
    queue_kind: Int64,
    state_save_backlog: UInt64,
    continuation_journal_backlog: UInt64,
    probe_refresh_backlog: UInt64,
    state_save_threshold: UInt64,
    continuation_journal_threshold: UInt64,
    probe_refresh_threshold: UInt64,
    pending_len_after_enqueue: UInt64,
    output_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_STATE_BACKGROUND_ABI_VERSION:
        return RUNTIME_STATE_BACKGROUND_ABI
    if (
        mode < MODE_MUTATION_PLAN
        or mode > MODE_ADMISSION_RELEASE
        or output_address == 0
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if mode == MODE_SECTIONS_UNION and (
        mutation_kind < 0
        or mutation_kind > 2
        or queue_kind < 0
        or queue_kind > 2
        or state_save_backlog > 15
        or continuation_journal_backlog > 15
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if mode == MODE_MUTATION_PLAN and (mutation_kind < 0 or mutation_kind > 31):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if (
        (mode == MODE_QUEUE_ENQUEUE or mode == MODE_QUEUE_THRESHOLD)
        and (queue_kind < 0 or queue_kind > 2)
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if mode == MODE_LANE_LIMIT and (queue_kind < 0 or queue_kind > 3):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if (
        (mode == MODE_ADMISSION_PLAN
         or mode == MODE_PROFILE_INFLIGHT_ACQUIRE
         or mode == MODE_PROFILE_INFLIGHT_RELEASE)
        and (mutation_kind < 0 or mutation_kind > 1)
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID

    var output = Pointer[mut=True, UInt64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(8):
        output[unsafe_offset=index] = 0

    if mode == MODE_MUTATION_PLAN:
        runtime_state_mutation_plan(mutation_kind, output)
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ADMISSION_PLAN:
        var active = state_save_backlog
        var lane_active = continuation_journal_backlog
        var active_limit = state_save_threshold
        var lane_limit = continuation_journal_threshold
        var bypass_lane_limit = mutation_kind == 1
        if active >= active_limit:
            output[unsafe_offset=0] = ADMISSION_GLOBAL_LIMIT
            output[unsafe_offset=1] = active
            output[unsafe_offset=2] = active_limit
            return RUNTIME_STATE_BACKGROUND_OK
        if lane_active >= lane_limit and not bypass_lane_limit:
            output[unsafe_offset=0] = ADMISSION_LANE_LIMIT
            output[unsafe_offset=1] = lane_active
            output[unsafe_offset=2] = lane_limit
            return RUNTIME_STATE_BACKGROUND_OK
        output[unsafe_offset=0] = ADMISSION_ALLOW
        output[unsafe_offset=1] = runtime_state_u64_saturating_add(active, 1)
        output[unsafe_offset=2] = runtime_state_u64_saturating_add(lane_active, 1)
        output[unsafe_offset=3] = runtime_state_flag(
            lane_active >= lane_limit and bypass_lane_limit
        )
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ADMISSION_RELEASE:
        var current = state_save_backlog
        output[unsafe_offset=0] = current - 1 if current > 0 else UInt64(0)
        output[unsafe_offset=1] = UInt64(1) if current == 0 else UInt64(0)
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_PROFILE_INFLIGHT_ACQUIRE:
        var weight = continuation_journal_backlog
        if weight < 1:
            weight = 1
        var next = runtime_state_u64_saturating_add(state_save_backlog, weight)
        var hard_limit_present = mutation_kind == 1
        if hard_limit_present and next > state_save_threshold:
            output[unsafe_offset=0] = 0
            output[unsafe_offset=1] = state_save_backlog
            output[unsafe_offset=2] = weight
            return RUNTIME_STATE_BACKGROUND_OK
        output[unsafe_offset=0] = 1
        output[unsafe_offset=1] = next
        output[unsafe_offset=2] = weight
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_PROFILE_INFLIGHT_RELEASE:
        var weight = continuation_journal_backlog
        if weight < 1:
            weight = 1
        var present = mutation_kind == 1
        var current = state_save_backlog
        output[unsafe_offset=1] = current if present else 0
        output[unsafe_offset=3] = weight
        if not present:
            output[unsafe_offset=0] = 0
            output[unsafe_offset=2] = 1
            return RUNTIME_STATE_BACKGROUND_OK
        if current >= weight:
            output[unsafe_offset=0] = current - weight
            output[unsafe_offset=2] = 0
        else:
            output[unsafe_offset=0] = 0
            output[unsafe_offset=2] = 1
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_SECTIONS_UNION:
        runtime_state_sections_union(
            mutation_kind,
            state_save_backlog,
            queue_kind,
            continuation_journal_backlog,
            output,
        )
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_LANE_LIMIT:
        if queue_kind == 0:
            output[unsafe_offset=0] = state_save_backlog
        elif queue_kind == 1:
            output[unsafe_offset=0] = continuation_journal_backlog
        elif queue_kind == 2:
            output[unsafe_offset=0] = probe_refresh_backlog
        else:
            output[unsafe_offset=0] = pending_len_after_enqueue
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_QUEUE_PRESSURE:
        output[unsafe_offset=0] = runtime_state_flag(
            state_save_backlog >= state_save_threshold
            or continuation_journal_backlog >= continuation_journal_threshold
            or probe_refresh_backlog >= probe_refresh_threshold
        )
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ENQUEUE_BACKLOG:
        output[unsafe_offset=0] = (
            UInt64(0)
            if pending_len_after_enqueue <= UInt64(1)
            else pending_len_after_enqueue - UInt64(1)
        )
        return RUNTIME_STATE_BACKGROUND_OK

    var threshold = runtime_state_threshold(
        queue_kind,
        state_save_threshold,
        continuation_journal_threshold,
        probe_refresh_threshold,
    )

    if mode == MODE_QUEUE_THRESHOLD:
        output[unsafe_offset=0] = threshold
        return RUNTIME_STATE_BACKGROUND_OK

    var backlog = (
        UInt64(0)
        if pending_len_after_enqueue <= UInt64(1)
        else pending_len_after_enqueue - UInt64(1)
    )
    output[unsafe_offset=0] = backlog
    output[unsafe_offset=1] = runtime_state_flag(backlog >= threshold)
    return RUNTIME_STATE_BACKGROUND_OK
