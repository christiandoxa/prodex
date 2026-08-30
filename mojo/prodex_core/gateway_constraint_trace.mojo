from std.memory import Pointer

comptime GATEWAY_CONSTRAINT_TRACE_ABI_VERSION: Int64 = 1
comptime GATEWAY_CONSTRAINT_TRACE_MAX_CANDIDATES: Int64 = 256
comptime STATUS_OK: Int64 = 0
comptime STATUS_INVALID: Int64 = 1
comptime STATUS_CAPACITY: Int64 = 3
comptime STATUS_ABI: Int64 = 4

comptime REJECTION_NONE: Int64 = 0
comptime REJECTION_ENDPOINT: Int64 = 1
comptime REJECTION_CONSTRAINTS: Int64 = 2

comptime AFFINITY_NOT_APPLICABLE: Int64 = 0
comptime AFFINITY_RETAINED: Int64 = 1
comptime AFFINITY_EXHAUSTED: Int64 = 2

comptime TERMINAL_SELECTED: Int64 = 0
comptime TERMINAL_NO_CANDIDATE: Int64 = 1
comptime TERMINAL_AFFINITY_EXHAUSTED: Int64 = 2


def prodex_mojo_gateway_constraint_trace_impl(
    abi_version: Int64,
    eligible_address: UInt,
    decisions_address: UInt,
    endpoint_unsupported_decision: Int64,
    candidate_count: Int64,
    selected_index: Int64,
    hard_affinity: Int64,
    ordered_indices_address: UInt,
    ordered_capacity: Int64,
    rejection_stages_address: UInt,
    rejection_capacity: Int64,
    endpoint_supported_address: UInt,
    request_constraints_outcome_address: UInt,
    affinity_outcome_address: UInt,
    terminal_outcome_address: UInt,
) abi("C") -> Int64:
    if abi_version != GATEWAY_CONSTRAINT_TRACE_ABI_VERSION:
        return STATUS_ABI
    if (
        candidate_count < 0
        or candidate_count > GATEWAY_CONSTRAINT_TRACE_MAX_CANDIDATES
        or selected_index < -1
        or selected_index >= candidate_count
        or hard_affinity < 0
        or hard_affinity > 1
        or endpoint_unsupported_decision < 0
    ):
        return STATUS_INVALID
    if ordered_capacity < candidate_count or rejection_capacity < candidate_count:
        return STATUS_CAPACITY
    if (
        endpoint_supported_address == 0
        or request_constraints_outcome_address == 0
        or affinity_outcome_address == 0
        or terminal_outcome_address == 0
    ):
        return STATUS_INVALID
    if candidate_count > 0 and (
        eligible_address == 0
        or decisions_address == 0
        or ordered_indices_address == 0
        or rejection_stages_address == 0
    ):
        return STATUS_INVALID

    var endpoint_supported = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(endpoint_supported_address))
    var request_constraints_outcome = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(request_constraints_outcome_address))
    var affinity_outcome = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(affinity_outcome_address))
    var terminal_outcome = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(terminal_outcome_address))
    endpoint_supported[] = 0
    request_constraints_outcome[] = -1
    affinity_outcome[] = AFFINITY_NOT_APPLICABLE
    terminal_outcome[] = TERMINAL_NO_CANDIDATE

    if candidate_count == 0:
        if hard_affinity == 1:
            affinity_outcome[] = AFFINITY_EXHAUSTED
            terminal_outcome[] = TERMINAL_AFFINITY_EXHAUSTED
        return STATUS_OK

    var eligible = Pointer[
        mut=False, Int64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(eligible_address))
    var decisions = Pointer[
        mut=False, Int64, ImmUntrackedOrigin
    ](unsafe_from_address=Int(decisions_address))
    var ordered_indices = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(ordered_indices_address))
    var rejection_stages = Pointer[
        mut=True, Int64, MutUntrackedOrigin
    ](unsafe_from_address=Int(rejection_stages_address))

    var endpoint_ready = False
    for index in range(candidate_count):
        var eligible_value = eligible[unsafe_offset=index]
        var decision = decisions[unsafe_offset=index]
        if eligible_value < 0 or eligible_value > 1 or decision < 0:
            return STATUS_INVALID
        if decision != endpoint_unsupported_decision:
            endpoint_ready = True
        if eligible_value == 1:
            rejection_stages[unsafe_offset=index] = REJECTION_NONE
        elif decision == endpoint_unsupported_decision:
            rejection_stages[unsafe_offset=index] = REJECTION_ENDPOINT
        else:
            rejection_stages[unsafe_offset=index] = REJECTION_CONSTRAINTS

    if endpoint_ready:
        endpoint_supported[] = 1
        if selected_index >= 0:
            request_constraints_outcome[] = 1
        else:
            request_constraints_outcome[] = 0

    var ordered_count: Int64 = 0
    if selected_index >= 0:
        ordered_indices[unsafe_offset=ordered_count] = selected_index
        ordered_count += 1
    for index in range(candidate_count):
        if index != selected_index:
            ordered_indices[unsafe_offset=ordered_count] = index
            ordered_count += 1

    if hard_affinity == 1:
        if selected_index >= 0:
            affinity_outcome[] = AFFINITY_RETAINED
        else:
            affinity_outcome[] = AFFINITY_EXHAUSTED
    if selected_index >= 0:
        terminal_outcome[] = TERMINAL_SELECTED
    elif hard_affinity == 1:
        terminal_outcome[] = TERMINAL_AFFINITY_EXHAUSTED
    else:
        terminal_outcome[] = TERMINAL_NO_CANDIDATE
    return STATUS_OK
