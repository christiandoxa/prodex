from std.memory import Pointer

comptime RUNTIME_STATE_BACKGROUND_ABI_VERSION: Int64 = 1
comptime RUNTIME_STATE_BACKGROUND_OK: Int64 = 0
comptime RUNTIME_STATE_BACKGROUND_INVALID: Int64 = 1
comptime RUNTIME_STATE_BACKGROUND_ABI: Int64 = 4

comptime MODE_MUTATION_PLAN: Int64 = 0
comptime MODE_QUEUE_PRESSURE: Int64 = 1
comptime MODE_QUEUE_ENQUEUE: Int64 = 2
comptime MODE_ENQUEUE_BACKLOG: Int64 = 3
comptime MODE_QUEUE_THRESHOLD: Int64 = 4

comptime SECTION_NONE: UInt64 = 0
comptime SECTION_CORE: UInt64 = 1
comptime SECTION_FULL: UInt64 = 2


def runtime_state_flag(value: Bool) -> UInt64:
    return UInt64(1) if value else UInt64(0)


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
        or mode > MODE_QUEUE_THRESHOLD
        or output_address == 0
    ):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if mode == MODE_MUTATION_PLAN and (mutation_kind < 0 or mutation_kind > 31):
        return RUNTIME_STATE_BACKGROUND_INVALID
    if (
        (mode == MODE_QUEUE_ENQUEUE or mode == MODE_QUEUE_THRESHOLD)
        and (queue_kind < 0 or queue_kind > 2)
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

    if mode == MODE_QUEUE_PRESSURE:
        output[0] = runtime_state_flag(
            state_save_backlog >= state_save_threshold
            or continuation_journal_backlog >= continuation_journal_threshold
            or probe_refresh_backlog >= probe_refresh_threshold
        )
        return RUNTIME_STATE_BACKGROUND_OK

    if mode == MODE_ENQUEUE_BACKLOG:
        output[0] = (
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
        output[0] = threshold
        return RUNTIME_STATE_BACKGROUND_OK

    var backlog = (
        UInt64(0)
        if pending_len_after_enqueue <= UInt64(1)
        else pending_len_after_enqueue - UInt64(1)
    )
    output[0] = backlog
    output[1] = runtime_state_flag(backlog >= threshold)
    return RUNTIME_STATE_BACKGROUND_OK
