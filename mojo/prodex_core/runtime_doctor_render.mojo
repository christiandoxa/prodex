from std.memory import Pointer

from rich_text import rich_view_ptr, rich_view_valid
from rich_types import ProdexRichStringView

comptime RUNTIME_DOCTOR_RENDER_ABI_VERSION: Int64 = 1
comptime RUNTIME_DOCTOR_RENDER_MAX_VALUE_BYTES: Int64 = 9_223_372_036_854_775_807

comptime RENDER_PREVIOUS_RESPONSE: Int64 = 0
comptime RENDER_COMPACT_FINAL_FAILURE: Int64 = 1
comptime RENDER_LANE_PRESSURE: Int64 = 2
comptime RENDER_ACTIVE_PRESSURE: Int64 = 3
comptime RENDER_PROFILE_INFLIGHT: Int64 = 4
comptime RENDER_ROUTE_HEALTH: Int64 = 5
comptime RENDER_WEBSOCKET_CONNECT: Int64 = 6
comptime RENDER_PROFILE_AUTH: Int64 = 7
comptime RENDER_PERSISTENCE: Int64 = 8
comptime RENDER_SYNC_PROBE: Int64 = 9
comptime RENDER_PROBE_REFRESH: Int64 = 10
comptime RENDER_TRANSPORT: Int64 = 11
comptime RENDER_QUOTA: Int64 = 12
comptime RENDER_PRECOMMIT: Int64 = 13

comptime DETAIL_CONTEXT_DEPENDENT: Int64 = 1
comptime DETAIL_COMPACT_PRESSURE: Int64 = 2
comptime DETAIL_COMPACT_QUOTA: Int64 = 3
comptime DETAIL_COMPACT_OVERLOAD: Int64 = 4
comptime DETAIL_COMPACT_TRANSPORT: Int64 = 5
comptime DETAIL_COMPACT_INFLIGHT: Int64 = 6
comptime DETAIL_LANE_RESPONSES: Int64 = 7
comptime DETAIL_PROFILE_HARD_LIMIT: Int64 = 8
comptime DETAIL_WEBSOCKET_REJECTED: Int64 = 9
comptime DETAIL_WEBSOCKET_DISPATCH: Int64 = 10
comptime DETAIL_AUTH_FAILED: Int64 = 12
comptime DETAIL_QUOTA_SYNC: Int64 = 16
comptime DETAIL_QUOTA_PROBE: Int64 = 17
comptime DETAIL_QUOTA_STALE: Int64 = 18
comptime DETAIL_PRECOMMIT_COMPACT: Int64 = 19


@fieldwise_init
struct ProdexRuntimeDoctorRenderInput(Copyable):
    var operation: Int64
    var detail: Int64
    var presence: UInt64
    var values_address: UInt


@fieldwise_init
struct RuntimeDoctorRenderWriter(Copyable):
    var output: Pointer[mut=True, UInt8, MutUntrackedOrigin]
    var capacity: Int64
    var written: Int64


def runtime_doctor_render_input_value(
    input: ProdexRuntimeDoctorRenderInput, index: Int
) -> ProdexRichStringView:
    var values = Pointer[
        mut=False, ProdexRichStringView, ImmUntrackedOrigin
    ](unsafe_from_address=Int(input.values_address))
    return values[unsafe_offset=index].copy()


def runtime_doctor_render_input_present(
    input: ProdexRuntimeDoctorRenderInput, index: Int
) -> Bool:
    return input.presence & (UInt64(1) << UInt64(index)) != 0


def runtime_doctor_render_put_value_or_literal(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
    index: Int,
    fallback: StringSlice,
) -> Bool:
    if runtime_doctor_render_input_present(input, index):
        return runtime_doctor_render_put_view(
            writer, runtime_doctor_render_input_value(input, index)
        )
    return runtime_doctor_render_put_literal(writer, fallback)


def runtime_doctor_render_put_scope(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
    profile_index: Int,
    route_index: Int,
    fallback: StringSlice,
) -> Bool:
    var profile = runtime_doctor_render_input_present(input, profile_index)
    var route = runtime_doctor_render_input_present(input, route_index)
    if not profile and not route:
        return runtime_doctor_render_put_literal(writer, fallback)
    if profile and not runtime_doctor_render_put_view(
        writer, runtime_doctor_render_input_value(input, profile_index)
    ):
        return False
    if profile and route and not runtime_doctor_render_put_byte(writer, 47):
        return False
    return not route or runtime_doctor_render_put_view(
        writer, runtime_doctor_render_input_value(input, route_index)
    )


def runtime_doctor_render_put_prefixed_if_present(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
    index: Int,
    prefix: StringSlice,
) -> Bool:
    if not runtime_doctor_render_input_present(input, index):
        return True
    return runtime_doctor_render_put_literal(
        writer, prefix
    ) and runtime_doctor_render_put_view(
        writer, runtime_doctor_render_input_value(input, index)
    )


def runtime_doctor_render_put_byte(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], value: UInt8
) -> Bool:
    if writer[].written < 0 or writer[].written >= writer[].capacity:
        return False
    writer[].output[unsafe_offset=writer[].written] = value
    writer[].written += 1
    return True


def runtime_doctor_render_put_literal(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], value: StringSlice
) -> Bool:
    var ptr = value.unsafe_ptr()
    for index in range(Int64(value.byte_length())):
        if not runtime_doctor_render_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def runtime_doctor_render_put_view(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    value: ProdexRichStringView,
) -> Bool:
    var ptr = rich_view_ptr(value)
    for index in range(Int64(value.len)):
        if not runtime_doctor_render_put_byte(writer, ptr[unsafe_offset=index]):
            return False
    return True


def runtime_doctor_render_previous(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    if input.detail == DETAIL_CONTEXT_DEPENDENT:
        return (
            runtime_doctor_render_put_literal(writer, StringSlice("Inspect `previous_response_not_found`, affinity bindings, and owning-profile chain markers before retrying; Prodex failed closed because this follow-up is context-dependent and cannot be replayed safely. Start a fresh turn only if context continuity can be abandoned. Latest guard: "))
            and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown_reason"))
            and runtime_doctor_render_put_literal(writer, StringSlice("."))
        )
    return (
        runtime_doctor_render_put_literal(writer, StringSlice("Inspect `previous_response_not_found` and `chain_dead_upstream_confirmed` for the owning context before retrying; fail-closed stale continuation handling blocks fresh replay when continuity is unverified. Start a fresh turn instead of forcing rotation if the owner cannot be recovered. Latest guard: "))
        and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown_reason"))
        and runtime_doctor_render_put_literal(writer, StringSlice("."))
    )


def runtime_doctor_render_compact(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    if input.detail == DETAIL_COMPACT_PRESSURE:
        return runtime_doctor_render_put_literal(writer, StringSlice("Reduce fresh compact volume or wait for continuation-heavy traffic to drain before retrying compact."))
    if input.detail == DETAIL_COMPACT_QUOTA:
        return runtime_doctor_render_put_literal(writer, StringSlice("Inspect compact budget and candidate-exhausted markers")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" on profile ")) and runtime_doctor_render_put_literal(writer, StringSlice(", then retry after compact quota refreshes or another profile becomes eligible."))
    if input.detail == DETAIL_COMPACT_OVERLOAD:
        return runtime_doctor_render_put_literal(writer, StringSlice("Inspect compact overload and backoff markers")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" on profile ")) and runtime_doctor_render_put_literal(writer, StringSlice(", then retry after the local pressure clears."))
    if input.detail == DETAIL_COMPACT_TRANSPORT:
        return runtime_doctor_render_put_literal(writer, StringSlice("Inspect compact transport markers")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" on profile ")) and runtime_doctor_render_put_literal(writer, StringSlice("; Prodex backed off the failing route, so retry after short transport backoff or let a fresh compact select another eligible profile."))
    if input.detail == DETAIL_COMPACT_INFLIGHT:
        return runtime_doctor_render_put_literal(writer, StringSlice("Wait for in-flight compact work to drain")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" on profile ")) and runtime_doctor_render_put_literal(writer, StringSlice(" before retrying."))
    return runtime_doctor_render_put_literal(writer, StringSlice("Inspect compact exit markers around `")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("`")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" on profile ")) and runtime_doctor_render_put_literal(writer, StringSlice(" and retry after the blocking condition clears."))


def runtime_doctor_render_sync_probe(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    return (
        runtime_doctor_render_put_literal(writer, StringSlice("Inspect `selection_skip_sync_probe`, `profile_probe_refresh_backpressure`, and `profile_probe_refresh_queued` markers for route "))
        and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown"))
        and runtime_doctor_render_put_literal(writer, StringSlice("; pressure mode ("))
        and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("unknown_reason"))
        and runtime_doctor_render_put_literal(writer, StringSlice(") deferred "))
        and (
            runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 2))
            and runtime_doctor_render_put_literal(writer, StringSlice(" cold-start job(s)"))
            if runtime_doctor_render_input_present(input, 2)
            else runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 3))
            and runtime_doctor_render_put_literal(writer, StringSlice(" cold-start profile(s)"))
            if runtime_doctor_render_input_present(input, 3)
            else runtime_doctor_render_put_literal(writer, StringSlice("cold-start work"))
        ) and runtime_doctor_render_put_literal(writer, StringSlice(", so cold-start profiles may stay on stale quota data until background probes finish."))
    )


def runtime_doctor_render_probe_refresh(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    return runtime_doctor_render_put_literal(writer, StringSlice("Let the background quota-refresh queue drain")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 0, StringSlice(" for profile ")) and runtime_doctor_render_put_literal(writer, StringSlice(" before expecting cold-start profiles to become selectable again.")) and runtime_doctor_render_put_prefixed_if_present(writer, input, 1, StringSlice(" Latest probe backlog: ")) and (not runtime_doctor_render_input_present(input, 1) or runtime_doctor_render_put_literal(writer, StringSlice(".")))


def runtime_doctor_render_value(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    if input.operation == RENDER_PREVIOUS_RESPONSE:
        return runtime_doctor_render_previous(writer, input)
    if input.operation == RENDER_COMPACT_FINAL_FAILURE:
        return runtime_doctor_render_compact(writer, input)
    if input.operation == RENDER_LANE_PRESSURE:
        if input.detail == DETAIL_LANE_RESPONSES:
            if not runtime_doctor_render_put_literal(writer, StringSlice("Reduce concurrent terminals or bursty side-lane work until the responses lane drains.")):
                return False
        elif not (runtime_doctor_render_put_literal(writer, StringSlice("Inspect repeated lane=")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(" markers and trim bursty ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(" traffic if it is starving responses."))):
            return False
        if runtime_doctor_render_input_present(input, 1) and runtime_doctor_render_input_present(input, 2):
            return runtime_doctor_render_put_literal(writer, StringSlice(" Latest load: ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1)) and runtime_doctor_render_put_byte(writer, 47) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 2)) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        return True
    if input.operation == RENDER_ACTIVE_PRESSURE:
        if not runtime_doctor_render_put_literal(writer, StringSlice("Reduce concurrent fresh work or wait for in-flight requests to drain before retrying.")):
            return False
        if runtime_doctor_render_input_present(input, 0) and runtime_doctor_render_input_present(input, 1):
            return runtime_doctor_render_put_literal(writer, StringSlice(" Latest load: ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 0)) and runtime_doctor_render_put_byte(writer, 47) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1)) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        return True
    if input.operation == RENDER_PROFILE_INFLIGHT:
        if not runtime_doctor_render_put_literal(writer, StringSlice("Wait for in-flight work")) or not runtime_doctor_render_put_prefixed_if_present(writer, input, 0, StringSlice(" on profile ")):
            return False
        if runtime_doctor_render_input_present(input, 1):
            return runtime_doctor_render_put_literal(writer, StringSlice(" to drop below hard limit ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1)) and runtime_doctor_render_put_literal(writer, StringSlice(" before retrying, or let fresh selection land on another eligible profile."))
        return runtime_doctor_render_put_literal(writer, StringSlice(" to drain before retrying, or let fresh selection land on another eligible profile."))
    if input.operation == RENDER_ROUTE_HEALTH:
        return runtime_doctor_render_put_literal(writer, StringSlice("Inspect recent transport or overload markers for ")) and runtime_doctor_render_put_scope(writer, input, 0, 1, StringSlice("that route")) and runtime_doctor_render_put_literal(writer, StringSlice(", especially `")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("unknown_reason")) and runtime_doctor_render_put_literal(writer, StringSlice("`, and wait for that route score to decay before expecting fresh selection to reuse it."))
    if input.operation == RENDER_WEBSOCKET_CONNECT:
        if input.detail == DETAIL_WEBSOCKET_REJECTED:
            if not runtime_doctor_render_put_literal(writer, StringSlice("Reduce concurrent websocket session starts or wait for websocket connect workers to drain before retrying. Latest reason: ")):
                return False
        elif input.detail == DETAIL_WEBSOCKET_DISPATCH:
            if not runtime_doctor_render_put_literal(writer, StringSlice("Overflow queued websocket connect work drained back into the bounded workers; inspect earlier enqueue/reject markers if dispatch repeats. Latest reason: ")):
                return False
        elif not runtime_doctor_render_put_literal(writer, StringSlice("Watch for matching dispatch or rejected markers; repeated enqueue means websocket connect workers are saturated. Latest reason: ")):
            return False
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown_reason")) and runtime_doctor_render_put_literal(writer, StringSlice("; pending=")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("/")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(", workers=")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(", queue_capacity=")) and runtime_doctor_render_put_value_or_literal(writer, input, 4, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if input.operation == RENDER_PROFILE_AUTH:
        if input.detail == DETAIL_AUTH_FAILED:
            return runtime_doctor_render_put_literal(writer, StringSlice("Refresh credentials for profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" with `prodex login --profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("` and retry route ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("; latest recovery error: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("unknown_error")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        return runtime_doctor_render_put_literal(writer, StringSlice("Auth recovered for profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" on route ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" via ")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" (changed=")) and runtime_doctor_render_put_value_or_literal(writer, input, 4, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("); if this repeats, restart active sessions after login refresh."))
    if input.operation == RENDER_PERSISTENCE:
        if not runtime_doctor_render_put_literal(writer, StringSlice("Reduce rapid rotation or continuation churn and wait for background persistence queues to drain.")):
            return False
        if runtime_doctor_render_input_present(input, 0) or runtime_doctor_render_input_present(input, 1):
            if not runtime_doctor_render_put_literal(writer, StringSlice(" Latest backlog:")):
                return False
            if runtime_doctor_render_input_present(input, 0) and not (runtime_doctor_render_put_literal(writer, StringSlice(" state=")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 0))):
                return False
            if runtime_doctor_render_input_present(input, 1) and not (runtime_doctor_render_put_literal(writer, StringSlice(" journal=")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1))):
                return False
            if not runtime_doctor_render_put_literal(writer, StringSlice(".")):
                return False
        if runtime_doctor_render_input_present(input, 2):
            return runtime_doctor_render_put_literal(writer, StringSlice(" Latest reason: ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 2)) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        if runtime_doctor_render_input_present(input, 3):
            return runtime_doctor_render_put_literal(writer, StringSlice(" Latest reason: ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 3)) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        return True
    if input.operation == RENDER_SYNC_PROBE:
        return runtime_doctor_render_sync_probe(writer, input)
    if input.operation == RENDER_PROBE_REFRESH:
        return runtime_doctor_render_probe_refresh(writer, input)
    if input.operation == RENDER_TRANSPORT:
        if not runtime_doctor_render_put_literal(writer, StringSlice("Inspect network/proxy and upstream transport markers for ")):
            return False
        var selected: Int = -1
        for pair in range(7):
            if selected < 0 and (runtime_doctor_render_input_present(input, pair * 2) or runtime_doctor_render_input_present(input, pair * 2 + 1)):
                selected = pair
        if selected < 0:
            if not runtime_doctor_render_put_literal(writer, StringSlice("affected route")):
                return False
        elif not runtime_doctor_render_put_scope(writer, input, selected * 2, selected * 2 + 1, StringSlice("affected route")):
            return False
        return runtime_doctor_render_put_literal(writer, StringSlice("; wait for short transport backoff to expire before retrying fresh work. Top reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 14, StringSlice("inspect latest transport marker")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if input.operation == RENDER_QUOTA:
        if input.detail == DETAIL_QUOTA_SYNC:
            return runtime_doctor_render_sync_probe(writer, input)
        if input.detail == DETAIL_QUOTA_PROBE:
            return runtime_doctor_render_probe_refresh(writer, input)
        if input.detail == DETAIL_QUOTA_STALE:
            if not runtime_doctor_render_put_literal(writer, StringSlice("Refresh quota visibility with `prodex quota --all --once` and let background probes drain")):
                return False
        elif not runtime_doctor_render_put_literal(writer, StringSlice("Wait for quota reset or use another eligible profile")):
            return False
        for index in range(3):
            if runtime_doctor_render_input_present(input, index):
                if not runtime_doctor_render_put_literal(writer, StringSlice(" for profile ")) or not runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, index)):
                    return False
                break
        if input.detail == DETAIL_QUOTA_STALE:
            return runtime_doctor_render_put_literal(writer, StringSlice(" before retrying selection-heavy work."))
        return runtime_doctor_render_put_literal(writer, StringSlice("; verify current limits with `prodex quota --all --once`."))
    if input.operation == RENDER_PRECOMMIT:
        if input.detail == DETAIL_PRECOMMIT_COMPACT:
            if not runtime_doctor_render_put_literal(writer, StringSlice("Reduce fresh compact volume on ")):
                return False
        elif not runtime_doctor_render_put_literal(writer, StringSlice("Inspect selection skip, quota, and transport backoff markers for ")):
            return False
        var route_written = False
        for index in range(3):
            if not route_written and runtime_doctor_render_input_present(input, index):
                route_written = runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, index))
        if not route_written and not runtime_doctor_render_put_literal(writer, StringSlice("affected route")):
            return False
        if input.detail == DETAIL_PRECOMMIT_COMPACT:
            return runtime_doctor_render_put_literal(writer, StringSlice(" or wait for quota/backoff pressure to clear before retrying compact."))
        return runtime_doctor_render_put_literal(writer, StringSlice("; retry after an eligible profile becomes available."))
    return False


@export("prodex_mojo_runtime_doctor_render_v1")
def prodex_mojo_runtime_doctor_render_v1(
    abi_version: Int64,
    input_address: UInt,
    output_address: UInt,
    output_capacity: Int64,
    written_address: UInt,
) abi("C") -> Int64:
    if abi_version != RUNTIME_DOCTOR_RENDER_ABI_VERSION:
        return 4
    if input_address == 0 or output_address == 0 or written_address == 0 or output_capacity <= 0:
        return 1
    var input_pointer = Pointer[mut=False, ProdexRuntimeDoctorRenderInput, ImmUntrackedOrigin](unsafe_from_address=Int(input_address))
    var input = input_pointer[].copy()
    if input.operation < RENDER_PREVIOUS_RESPONSE or input.operation > RENDER_PRECOMMIT or input.detail < 0 or input.detail > 23 or input.values_address == 0:
        return 1
    for index in range(8):
        if not rich_view_valid(runtime_doctor_render_input_value(input, index), RUNTIME_DOCTOR_RENDER_MAX_VALUE_BYTES):
            return 2
    var output = Pointer[mut=True, UInt8, MutUntrackedOrigin](unsafe_from_address=Int(output_address))
    var written = Pointer[mut=True, Int64, MutUntrackedOrigin](unsafe_from_address=Int(written_address))
    written[] = 0
    var writer = RuntimeDoctorRenderWriter(output, output_capacity, 0)
    if not runtime_doctor_render_value(Pointer(to=writer), input):
        return 3
    written[] = writer.written
    return 0
