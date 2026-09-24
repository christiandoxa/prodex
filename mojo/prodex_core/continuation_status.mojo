from std.memory import Pointer
from runtime_math import INT64_MAX, INT64_MIN

comptime CONTINUATION_STATUS_ABI_VERSION: Int64 = 1
comptime STATUS_FIELDS: Int64 = 12
comptime STATE_WARM: Int64 = 0
comptime STATE_VERIFIED: Int64 = 1
comptime STATE_SUSPECT: Int64 = 2
comptime STATE_DEAD: Int64 = 3
comptime UINT32_MAX: Int64 = 4_294_967_295

comptime OP_TOUCH_SHOULD_PERSIST: Int64 = 2
comptime OP_TERMINAL_STATUS: Int64 = 3
comptime OP_TOUCH_PLAN: Int64 = 5
comptime OP_SHOULD_REFRESH_VERIFIED: Int64 = 6
comptime OP_SHOULD_PERSIST_TOUCH: Int64 = 7
comptime OP_VERIFY_PLAN: Int64 = 8
comptime OP_SUSPECT_PLAN: Int64 = 9
comptime OP_DEAD_PLAN: Int64 = 10
comptime OP_RECENTLY_SUSPECT: Int64 = 11
comptime OP_STALE_VERIFIED: Int64 = 12
comptime OP_RETAIN_WITH_BINDING: Int64 = 13
comptime OP_RETAIN_WITHOUT_BINDING: Int64 = 14
comptime OP_DEAD_SHADOWED: Int64 = 16
comptime OP_EVIDENCE_KEY: Int64 = 17
comptime OP_SHOULD_REPLACE: Int64 = 18
comptime OP_RETENTION_KEY: Int64 = 21


def sat_sub_i64(left: Int64, right: Int64) -> Int64:
    if right < 0 and left > INT64_MAX + right:
        return INT64_MAX
    if right > 0 and left < INT64_MIN + right:
        return INT64_MIN
    return left - right


def sat_add_one_i64(value: Int64) -> Int64:
    if value == INT64_MAX:
        return INT64_MAX
    return value + 1


def sat_add_u32(left: Int64, right: Int64) -> Int64:
    if right <= 0:
        return left
    if left >= UINT32_MAX - right:
        return UINT32_MAX
    return left + right


def sat_sub_u32(left: Int64, right: Int64) -> Int64:
    if right <= 0:
        return left
    if left <= right:
        return 0
    return left - right


def bool_field(value: Int64) -> Bool:
    return value == 0 or value == 1


def status_valid_at(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin], base: Int64
) -> Bool:
    var state = fields[unsafe_offset=base]
    return (
        state >= STATE_WARM
        and state <= STATE_DEAD
        and fields[unsafe_offset=base + 1] >= 0
        and fields[unsafe_offset=base + 1] <= UINT32_MAX
        and bool_field(fields[unsafe_offset=base + 2])
        and bool_field(fields[unsafe_offset=base + 4])
        and bool_field(fields[unsafe_offset=base + 6])
        and bool_field(fields[unsafe_offset=base + 7])
        and fields[unsafe_offset=base + 9] >= 0
        and fields[unsafe_offset=base + 9] <= UINT32_MAX
        and fields[unsafe_offset=base + 10] >= 0
        and fields[unsafe_offset=base + 10] <= UINT32_MAX
        and fields[unsafe_offset=base + 11] >= 0
        and fields[unsafe_offset=base + 11] <= UINT32_MAX
    )


def status_valid(fields: Pointer[mut=False, Int64, ImmUntrackedOrigin]) -> Bool:
    return status_valid_at(fields, 0)


def last_event_at(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin], base: Int64
) -> Tuple[Int64, Int64]:
    var present: Int64 = 0
    var value: Int64 = INT64_MIN
    if fields[unsafe_offset=base + 7] == 1:
        present = 1
        value = fields[unsafe_offset=base + 8]
    if fields[unsafe_offset=base + 4] == 1:
        var candidate = fields[unsafe_offset=base + 5]
        if present == 0 or candidate > value:
            present = 1
            value = candidate
    if fields[unsafe_offset=base + 2] == 1:
        var candidate = fields[unsafe_offset=base + 3]
        if present == 0 or candidate > value:
            present = 1
            value = candidate
    return (present, value)


def last_event(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin]
) -> Tuple[Int64, Int64]:
    return last_event_at(fields, 0)

def next_event(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin], now: Int64
) -> Int64:
    var last = last_event(fields)
    if last[0] == 1 and last[1] >= now:
        return sat_add_one_i64(last[1])
    return now


def terminal_at(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    base: Int64,
    suspect_limit: Int64,
) -> Bool:
    return (
        fields[unsafe_offset=base] == STATE_DEAD
        or fields[unsafe_offset=base + 9] >= suspect_limit
        or (
            fields[unsafe_offset=base] == STATE_SUSPECT
            and fields[unsafe_offset=base + 1] == 0
            and fields[unsafe_offset=base + 11] > 0
        )
    )


def terminal(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin], suspect_limit: Int64
) -> Bool:
    return terminal_at(fields, 0, suspect_limit)


def lifecycle_rank(state: Int64) -> Int64:
    if state == STATE_DEAD:
        return 0
    if state == STATE_SUSPECT:
        return 1
    if state == STATE_WARM:
        return 2
    return 3


def evidence_field(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    base: Int64,
    confidence_max: Int64,
    index: Int64,
) -> Int64:
    if index == 0:
        return lifecycle_rank(fields[unsafe_offset=base])
    if index == 1:
        return min(fields[unsafe_offset=base + 1], confidence_max)
    if index == 2:
        return fields[unsafe_offset=base + 10]
    if index == 3:
        return UINT32_MAX - fields[unsafe_offset=base + 9]
    if index == 4:
        return fields[unsafe_offset=base + 6]
    if index == 5:
        return fields[unsafe_offset=base + 5] if fields[unsafe_offset=base + 4] == 1 else INT64_MIN
    if index == 6:
        return fields[unsafe_offset=base + 3] if fields[unsafe_offset=base + 2] == 1 else INT64_MIN
    return fields[unsafe_offset=base + 8] if fields[unsafe_offset=base + 7] == 1 else INT64_MIN


def evidence_greater(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    left_base: Int64,
    right_base: Int64,
    confidence_max: Int64,
) -> Bool:
    for index in range(8):
        var field_index = Int64(index)
        var left = evidence_field(fields, left_base, confidence_max, field_index)
        var right = evidence_field(fields, right_base, confidence_max, field_index)
        if left != right:
            return left > right
    return False


def retain_with_binding(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    now: Int64,
    suspect_grace: Int64,
    suspect_limit: Int64,
) -> Bool:
    var state = fields[unsafe_offset=0]
    if state == STATE_DEAD:
        return False
    if state == STATE_WARM or state == STATE_VERIFIED:
        return (
            fields[unsafe_offset=1] > 0
            or fields[unsafe_offset=10] > 0
            or fields[unsafe_offset=4] == 1
            or fields[unsafe_offset=2] == 1
        )
    return (
        fields[unsafe_offset=9] < suspect_limit
        and fields[unsafe_offset=1] > 0
        and fields[unsafe_offset=7] == 1
        and sat_sub_i64(now, fields[unsafe_offset=8]) < suspect_grace
    )

def copy_status(
    fields: Pointer[mut=False, Int64, ImmUntrackedOrigin],
    output: Pointer[mut=True, Int64, MutUntrackedOrigin],
):
    for index in range(STATUS_FIELDS):
        output[unsafe_offset=Int64(index)] = fields[unsafe_offset=Int64(index)]


@export("prodex_runtime_continuation_status_transition_v1")
def prodex_runtime_continuation_status_transition_v1(
    abi_version: Int64,
    operation: Int64,
    fields_address: UInt,
    field_count: Int64,
    output_address: UInt,
    output_count: Int64,
) abi("C") -> Int64:
    if abi_version != CONTINUATION_STATUS_ABI_VERSION:
        return 4
    if field_count < 1 or field_count > 32 or output_count < 1 or output_count > 12:
        return 1
    if fields_address == 0 or output_address == 0:
        return 1
    var fields = Pointer[mut=False, Int64, ImmUntrackedOrigin](
        unsafe_from_address=Int(fields_address)
    )
    var output = Pointer[mut=True, Int64, MutUntrackedOrigin](
        unsafe_from_address=Int(output_address)
    )
    for index in range(output_count):
        output[unsafe_offset=index] = 0

    if operation == OP_TOUCH_SHOULD_PERSIST:
        if field_count != 3 or output_count != 1:
            return 1
        output[unsafe_offset=0] = Int64(
            sat_sub_i64(fields[unsafe_offset=1], fields[unsafe_offset=0])
            > fields[unsafe_offset=2]
        )
        return 0

    if operation == OP_SHOULD_REPLACE:
        if field_count != 26 or output_count != 1:
            return 1
        if not status_valid_at(fields, 0) or not status_valid_at(fields, 12):
            return 2
        var suspect_limit = fields[unsafe_offset=24]
        var confidence_max = fields[unsafe_offset=25]
        if (
            suspect_limit < 0
            or suspect_limit > UINT32_MAX
            or confidence_max < 0
            or confidence_max > UINT32_MAX
        ):
            return 2
        var candidate_at = last_event_at(fields, 0)
        var current_at = last_event_at(fields, 12)
        if candidate_at[0] == 1 and current_at[0] == 1 and candidate_at[1] != current_at[1]:
            output[unsafe_offset=0] = Int64(candidate_at[1] > current_at[1])
            return 0
        if candidate_at[0] == 1 and current_at[0] == 0:
            output[unsafe_offset=0] = 1
            return 0
        if candidate_at[0] == 0 and current_at[0] == 1:
            return 0
        var candidate_terminal = terminal_at(fields, 0, suspect_limit)
        var current_terminal = terminal_at(fields, 12, suspect_limit)
        if candidate_terminal != current_terminal:
            output[unsafe_offset=0] = Int64(candidate_terminal)
            return 0
        output[unsafe_offset=0] = Int64(
            evidence_greater(fields, 0, 12, confidence_max)
        )
        return 0

    if field_count < STATUS_FIELDS or not status_valid(fields):
        return 2

    if operation == OP_TERMINAL_STATUS:
        if field_count != 13 or output_count != 1:
            return 1
        var limit = fields[unsafe_offset=12]
        if limit < 0 or limit > UINT32_MAX:
            return 2
        output[unsafe_offset=0] = Int64(terminal(fields, limit))
        return 0

    if operation == OP_TOUCH_PLAN:
        if field_count != 16 or output_count != 12:
            return 1
        var grace = fields[unsafe_offset=13]
        var confidence_max = fields[unsafe_offset=14]
        var bonus = fields[unsafe_offset=15]
        if confidence_max < 0 or confidence_max > UINT32_MAX or bonus < 0 or bonus > UINT32_MAX:
            return 2
        copy_status(fields, output)
        var event_at = next_event(fields, fields[unsafe_offset=12])
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = event_at
        if fields[unsafe_offset=0] == STATE_SUSPECT:
            if (
                fields[unsafe_offset=7] == 1
                and sat_sub_i64(event_at, fields[unsafe_offset=8]) >= grace
            ):
                output[unsafe_offset=0] = STATE_WARM
                output[unsafe_offset=7] = 0
                output[unsafe_offset=8] = 0
                output[unsafe_offset=9] = 0
            output[unsafe_offset=1] = min(
                sat_add_u32(fields[unsafe_offset=1], bonus), confidence_max
            )
        elif fields[unsafe_offset=0] != STATE_DEAD:
            output[unsafe_offset=1] = min(
                sat_add_u32(fields[unsafe_offset=1], bonus), confidence_max
            )
        return 0

    if operation == OP_SHOULD_REFRESH_VERIFIED:
        if field_count != 16 or output_count != 1:
            return 1
        var present = fields[unsafe_offset=12]
        var route_matches = fields[unsafe_offset=15]
        if not bool_field(present) or not bool_field(route_matches):
            return 2
        if present == 0 or fields[unsafe_offset=0] != STATE_VERIFIED or route_matches == 0:
            output[unsafe_offset=0] = 1
            return 0
        if fields[unsafe_offset=4] == 0:
            output[unsafe_offset=0] = 1
            return 0
        output[unsafe_offset=0] = Int64(
            sat_sub_i64(fields[unsafe_offset=13], fields[unsafe_offset=5])
            > fields[unsafe_offset=14]
        )
        return 0

    if operation == OP_SHOULD_PERSIST_TOUCH:
        if field_count != 16 or output_count != 1:
            return 1
        var present = fields[unsafe_offset=12]
        if not bool_field(present):
            return 2
        if present == 0:
            output[unsafe_offset=0] = 1
            return 0
        var now = fields[unsafe_offset=13]
        if (
            fields[unsafe_offset=0] == STATE_SUSPECT
            and fields[unsafe_offset=7] == 1
            and sat_sub_i64(now, fields[unsafe_offset=8]) >= fields[unsafe_offset=14]
        ):
            output[unsafe_offset=0] = 1
            return 0
        output[unsafe_offset=0] = Int64(
            fields[unsafe_offset=2] == 0
            or sat_sub_i64(now, fields[unsafe_offset=3]) > fields[unsafe_offset=15]
        )
        return 0

    if operation == OP_VERIFY_PLAN:
        if field_count != 16 or output_count != 12:
            return 1
        var confidence_max = fields[unsafe_offset=13]
        var bonus = fields[unsafe_offset=14]
        var route_present = fields[unsafe_offset=15]
        if (
            confidence_max < 0
            or confidence_max > UINT32_MAX
            or bonus < 0
            or bonus > UINT32_MAX
            or not bool_field(route_present)
        ):
            return 2
        copy_status(fields, output)
        var event_at = next_event(fields, fields[unsafe_offset=12])
        output[unsafe_offset=0] = STATE_VERIFIED
        output[unsafe_offset=1] = min(
            sat_add_u32(fields[unsafe_offset=1], bonus), confidence_max
        )
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = event_at
        output[unsafe_offset=4] = 1
        output[unsafe_offset=5] = event_at
        output[unsafe_offset=6] = route_present
        output[unsafe_offset=7] = 0
        output[unsafe_offset=8] = 0
        output[unsafe_offset=9] = 0
        output[unsafe_offset=10] = sat_add_u32(fields[unsafe_offset=10], 1)
        output[unsafe_offset=11] = 0
        return 0

    if operation == OP_SUSPECT_PLAN:
        if field_count != 15 or output_count != 12:
            return 1
        var limit = fields[unsafe_offset=13]
        var penalty = fields[unsafe_offset=14]
        if limit < 0 or limit > UINT32_MAX or penalty < 0 or penalty > UINT32_MAX:
            return 2
        copy_status(fields, output)
        var event_at = next_event(fields, fields[unsafe_offset=12])
        var streak = sat_add_u32(fields[unsafe_offset=9], 1)
        var failures = sat_add_u32(fields[unsafe_offset=11], 1)
        var previous_confidence = fields[unsafe_offset=1]
        var confidence = sat_sub_u32(previous_confidence, penalty)
        if previous_confidence == 0:
            confidence = 1
        output[unsafe_offset=0] = (
            STATE_DEAD
            if streak >= limit or (previous_confidence > 0 and confidence == 0)
            else STATE_SUSPECT
        )
        output[unsafe_offset=1] = confidence
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = event_at
        output[unsafe_offset=7] = 1
        output[unsafe_offset=8] = event_at
        output[unsafe_offset=9] = streak
        output[unsafe_offset=11] = failures
        return 0

    if operation == OP_DEAD_PLAN:
        if field_count != 14 or output_count != 12:
            return 1
        var limit = fields[unsafe_offset=13]
        if limit < 0 or limit > UINT32_MAX:
            return 2
        copy_status(fields, output)
        var event_at = next_event(fields, fields[unsafe_offset=12])
        output[unsafe_offset=0] = STATE_DEAD
        output[unsafe_offset=1] = 0
        output[unsafe_offset=2] = 1
        output[unsafe_offset=3] = event_at
        output[unsafe_offset=7] = 1
        output[unsafe_offset=8] = event_at
        output[unsafe_offset=9] = max(fields[unsafe_offset=9], limit)
        output[unsafe_offset=11] = sat_add_u32(fields[unsafe_offset=11], 1)
        return 0

    if operation == OP_RECENTLY_SUSPECT:
        if field_count != 16 or output_count != 1:
            return 1
        var present = fields[unsafe_offset=12]
        var limit = fields[unsafe_offset=15]
        if not bool_field(present) or limit < 0 or limit > UINT32_MAX:
            return 2
        output[unsafe_offset=0] = Int64(
            present == 1
            and fields[unsafe_offset=0] == STATE_SUSPECT
            and not terminal(fields, limit)
            and fields[unsafe_offset=7] == 1
            and sat_sub_i64(fields[unsafe_offset=13], fields[unsafe_offset=8])
            < fields[unsafe_offset=14]
        )
        return 0

    if operation == OP_EVIDENCE_KEY:
        if field_count != 13 or output_count != 8:
            return 1
        var confidence_max = fields[unsafe_offset=12]
        if confidence_max < 0 or confidence_max > UINT32_MAX:
            return 2
        for index in range(8):
            output[unsafe_offset=index] = evidence_field(
                fields, 0, confidence_max, Int64(index)
            )
        return 0

    if operation == OP_STALE_VERIFIED:
        if field_count != 14 or output_count != 1:
            return 1
        var last = last_event(fields)
        output[unsafe_offset=0] = Int64(
            fields[unsafe_offset=0] == STATE_VERIFIED
            and last[0] == 1
            and sat_sub_i64(fields[unsafe_offset=12], last[1]) >= fields[unsafe_offset=13]
        )
        return 0

    if operation == OP_RETAIN_WITH_BINDING:
        if field_count != 15 or output_count != 1:
            return 1
        var limit = fields[unsafe_offset=14]
        if limit < 0 or limit > UINT32_MAX:
            return 2
        output[unsafe_offset=0] = Int64(
            retain_with_binding(
                fields, fields[unsafe_offset=12], fields[unsafe_offset=13], limit
            )
        )
        return 0

    if operation == OP_RETAIN_WITHOUT_BINDING:
        if field_count != 16 or output_count != 1:
            return 1
        var limit = fields[unsafe_offset=14]
        if limit < 0 or limit > UINT32_MAX:
            return 2
        if fields[unsafe_offset=0] == STATE_DEAD:
            var present = fields[unsafe_offset=7]
            var last = fields[unsafe_offset=8]
            if present == 0:
                present = fields[unsafe_offset=2]
                last = fields[unsafe_offset=3]
            output[unsafe_offset=0] = Int64(
                present == 1
                and sat_sub_i64(fields[unsafe_offset=12], last) < fields[unsafe_offset=15]
            )
        else:
            output[unsafe_offset=0] = Int64(
                retain_with_binding(
                    fields, fields[unsafe_offset=12], fields[unsafe_offset=13], limit
                )
            )
        return 0

    if operation == OP_DEAD_SHADOWED:
        if field_count != 13 or output_count != 1:
            return 1
        if fields[unsafe_offset=0] == STATE_DEAD:
            if fields[unsafe_offset=7] == 1:
                output[unsafe_offset=0] = Int64(fields[unsafe_offset=12] > fields[unsafe_offset=8])
            elif fields[unsafe_offset=2] == 1:
                output[unsafe_offset=0] = Int64(fields[unsafe_offset=12] > fields[unsafe_offset=3])
        return 0

    if operation == OP_RETENTION_KEY:
        if field_count != 15 or output_count != 10:
            return 1
        var confidence_max = fields[unsafe_offset=12]
        var binding_present = fields[unsafe_offset=13]
        if (
            confidence_max < 0
            or confidence_max > UINT32_MAX
            or not bool_field(binding_present)
        ):
            return 2
        output[unsafe_offset=0] = binding_present
        for index in range(8):
            output[unsafe_offset=index + 1] = evidence_field(
                fields, 0, confidence_max, Int64(index)
            )
        output[unsafe_offset=9] = fields[unsafe_offset=14] if binding_present == 1 else INT64_MIN
        return 0

    return 1
