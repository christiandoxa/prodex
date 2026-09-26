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
comptime RENDER_DIAGNOSIS: Int64 = 14
comptime RENDER_POLICY_SUGGESTION_ID: Int64 = 15
comptime RENDER_POLICY_SUGGESTION_TITLE: Int64 = 16
comptime RENDER_POLICY_SUGGESTION_MARKERS: Int64 = 17
comptime RENDER_POLICY_SETTING_KEY: Int64 = 18
comptime RENDER_POLICY_SETTING_RATIONALE: Int64 = 19
comptime RENDER_POLICY_SUGGESTION_REASON: Int64 = 20
comptime RENDER_POLICY_MARKER_NAME: Int64 = 21

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


def runtime_doctor_render_diagnosis(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    var kind = input.detail
    if kind == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("No runtime log pointer has been created yet."))
    if kind == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("Latest runtime log path does not exist."))
    if kind == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("Latest runtime log is empty."))
    if kind == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent local proxy overload backoff was triggered."))
    if kind == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent per-lane admission limit was triggered on ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect lane pressure"))
    if kind == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent global active-request admission limit was triggered. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect active pressure"))
    if kind == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent proxy saturation detected before commit."))
    if kind == 8:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent route-level circuit breaker opened; fresh selection is temporarily steering away from a degraded profile."))
    if kind == 9:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent route-level circuit breaker entered half-open probing; fresh selection is cautiously testing a degraded profile before fully restoring it."))
    if kind == 10:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket reuse/connect path failed to produce a first upstream frame before the pre-commit deadline."))
    if kind == 11:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket pre-commit hold timed out before an upstream terminal frame arrived."))
    if kind == 12:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket DNS resolution timed out before upstream connect completed."))
    if kind == 13:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket DNS resolution work was rejected after the overflow queue saturated."))
    if kind == 14:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket DNS resolution overflow queueing was observed."))
    if kind == 15:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket connect failed due local pressure before upstream commit."))
    if kind == 16:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket connect work was rejected after the overflow queue saturated. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect websocket connect overflow"))
    if kind == 17:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket connect overflow queueing was observed. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect websocket connect overflow"))
    if kind == 18:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket connect overflow dispatch was observed. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect websocket connect overflow"))
    if kind == 19:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket upstream proxy tunnel failed before the upstream websocket handshake completed. Next step: inspect HTTPS_PROXY/NO_PROXY and the proxy's CONNECT support."))
    if kind == 20:
        if not (runtime_doctor_render_put_literal(writer, StringSlice("Recent per-profile in-flight saturation blocked ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("an eligible profile"))):
            return False
        if runtime_doctor_render_input_present(input, 1) and not (runtime_doctor_render_put_literal(writer, StringSlice(" at hard limit ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1))):
            return False
        return runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("wait for in-flight work to drain"))
    if kind == 21:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent route-specific health penalty is steering fresh selection away from ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown route")) and runtime_doctor_render_put_literal(writer, StringSlice(" (score ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(", reason ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("unknown_reason")) and runtime_doctor_render_put_literal(writer, StringSlice("). Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("inspect route health"))
    if kind == 22:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent route-specific bad pairing memory is steering fresh selection away from a flaky account."))
    if kind == 23:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent profile auth recovery failed after an upstream unauthorized response. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("refresh profile credentials"))
    if kind == 24:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("provider")) and runtime_doctor_render_put_literal(writer, StringSlice(" provider auth failure was observed for profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice("; refresh that provider login or API key before retrying."))
    if kind == 25:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent compact lineage guard failed closed so a follow-up stayed owner-first until upstream continuity was proven dead."))
    if kind == 26:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent pressure mode is shedding fresh compact requests to preserve continuation-heavy traffic."))
    if kind == 27:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent previous_response_id chain was confirmed dead upstream after owner retries. Latest chain event: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect chain_dead_upstream_confirmed markers")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 28:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent stale continuation was surfaced to Codex via fail-closed handling. Latest reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect stale_continuation markers")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 29:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent continuation chain was retried on the owning profile before commit. Latest chain event: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect chain_retried_owner markers")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 30:
        if runtime_doctor_render_input_present(input, 2):
            return runtime_doctor_render_put_literal(writer, StringSlice("Recent context-dependent previous_response_id continuation failed closed before commit. Fresh replay is disabled to preserve continuity. Latest reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect previous_response_fresh_fallback_blocked markers")) and runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("inspect continuation state"))
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("previous_response_id continuation")) and runtime_doctor_render_put_literal(writer, StringSlice(" failed closed before commit. Fresh replay is disabled for stale continuation handling. Latest reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect previous_response_fresh_fallback_blocked markers")) and runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("inspect continuation state"))
    if kind == 31:
        return runtime_doctor_render_put_literal(writer, StringSlice("Legacy previous_response recovery marker was observed for ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("previous_response_id continuation")) and runtime_doctor_render_put_literal(writer, StringSlice(", but current runtime should fail closed instead of treating this as recoverable. Latest reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect previous_response_fresh_fallback markers")) and runtime_doctor_render_put_literal(writer, StringSlice(". Restart active prodex/codex sessions if this came from a live broker."))
    if kind == 32:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent previous_response_id continuity failures were observed: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("none")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 33:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent compact final failure exited via ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" with reason ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("inspect compact failure"))
    if kind == 34:
        if runtime_doctor_render_input_present(input, 0):
            return runtime_doctor_render_put_literal(writer, StringSlice("Recent compact exit paths were logged: ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 0)) and runtime_doctor_render_put_literal(writer, StringSlice("."))
        return runtime_doctor_render_put_literal(writer, StringSlice("No recent overload or stream-failure markers were detected in the sampled runtime tail."))
    if kind == 35:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent compatibility warnings were observed for ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown client")) and runtime_doctor_render_put_literal(writer, StringSlice(": ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect compat_warning markers")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 36:
        return runtime_doctor_render_put_literal(writer, StringSlice("Some persisted continuations are currently dead and will be pruned: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 37:
        return runtime_doctor_render_put_literal(writer, StringSlice("Some persisted continuations are currently suspect: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 38:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent websocket session reuse degraded before a terminal event; fresh reuse may be steering away from that profile."))
    if kind == 39:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent profile auth recovered after an upstream unauthorized response. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect profile auth recovery"))
    if kind == 40:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent candidate selection exhausted before commit."))
    if kind == 41:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent quota hardening skipped near-exhausted sends or passed through upstream usage-limit responses."))
    if kind == 42:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent Gemini quota or rate-limit recovery was observed for profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(" before commit; OAuth profile rotation/retry kept the Codex-facing request recoverable."))
    if kind == 43:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("provider")) and runtime_doctor_render_put_literal(writer, StringSlice(" model fallback was used before commit (")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(" -> ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice(")."))
    if kind == 44:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent Gemini stream produced an invalid pre-commit prefix (")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("invalid_stream")) and runtime_doctor_render_put_literal(writer, StringSlice("); Prodex retried or fell back before exposing it to Codex."))
    if kind == 45:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent Gemini semantic compact failed before commit, so Prodex preserved continuity with the bounded local fallback. Latest reason: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice("."))
    if kind == 46:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent Gemini Live bridge errors were observed; inspect local_rewrite_gemini_live_* markers for the failing profile/request."))
    if kind == 47:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent upstream stream read failure detected after commit."))
    if kind == 48:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent local writer failure detected while forwarding an upstream stream."))
    if kind == 49:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent upstream connect failures detected."))
    if kind == 50:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent runtime state save failures detected."))
    if kind == 51:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent background persistence queue backpressure was detected. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("inspect persistence pressure"))
    if kind == 52:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent fresh selection skipped inline quota probing on route ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(" under pressure mode. Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("inspect sync probe pressure"))
    if kind == 53:
        if not (runtime_doctor_render_put_literal(writer, StringSlice("Recent background quota refresh queue backpressure was detected for profile ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("unknown"))):
            return False
        if runtime_doctor_render_input_present(input, 1) and not (runtime_doctor_render_put_literal(writer, StringSlice(" with backlog ")) and runtime_doctor_render_put_view(writer, runtime_doctor_render_input_value(input, 1))):
            return False
        return runtime_doctor_render_put_literal(writer, StringSlice(". Next step: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("let background probes drain"))
    if kind == 54:
        return runtime_doctor_render_put_literal(writer, StringSlice("Persisted degraded runtime routes are still active: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice(""))
    if kind == 55:
        return runtime_doctor_render_put_literal(writer, StringSlice("Orphan managed profile directories were detected: ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice(""))
    if kind == 56:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent background quota refresh failures detected; fresh selection may rely on stale quota snapshots."))
    if kind == 57:
        return runtime_doctor_render_put_literal(writer, StringSlice("Background quota refresh activity was detected; inspect the last marker for the most recent profile refresh."))
    if kind == 58:
        return runtime_doctor_render_put_literal(writer, StringSlice("Likely writer stall: upstream produced data but the local writer did not emit a first chunk in the sampled tail."))
    if kind == 59:
        return runtime_doctor_render_put_literal(writer, StringSlice("A running runtime broker uses a different prodex binary than this command; restart active prodex/codex sessions so the patched runtime is loaded."))
    if kind == 60:
        return runtime_doctor_render_put_literal(writer, StringSlice("Multiple prodex binaries on PATH differ by version or hash; align installs so new sessions use the patched runtime."))
    if kind == 61:
        return runtime_doctor_render_put_literal(writer, StringSlice("Recent selection decisions were logged; inspect the last marker for why a profile was picked or skipped."))
    if kind == 0 or kind == 62:
        return runtime_doctor_render_put_literal(writer, StringSlice("No recent overload or stream-failure markers were detected in the sampled runtime tail."))
    return False



def runtime_doctor_render_policy_suggestion_id(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], detail: Int64
) -> Bool:
    if detail == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("lane_pressure"))
    if detail == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("active_request_pressure"))
    if detail == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("profile_inflight_saturation"))
    if detail == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow"))
    if detail == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow"))
    if detail == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("persistence_backpressure"))
    if detail == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("route_scoped_profile_health"))
    return False


def runtime_doctor_render_policy_suggestion_title(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], detail: Int64
) -> Bool:
    if detail == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("Lane pressure"))
    if detail == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("Active request pressure"))
    if detail == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("Profile in-flight saturation"))
    if detail == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("Websocket connect overflow"))
    if detail == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("Websocket DNS overflow"))
    if detail == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("Persistence backpressure"))
    if detail == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("Route-scoped profile health"))
    return False


def runtime_doctor_render_policy_suggestion_markers(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], detail: Int64
) -> Bool:
    if detail == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("runtime_proxy_lane_limit_reached"))
    if detail == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("runtime_proxy_active_limit_reached"))
    if detail == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("profile_inflight_saturated"))
    if detail == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_rejected\nwebsocket_connect_overflow_reject\nwebsocket_connect_overflow_enqueue\nwebsocket_connect_overflow_dispatch"))
    if detail == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow_reject\nwebsocket_dns_overflow_enqueue\nwebsocket_dns_overflow_dispatch"))
    if detail == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("state_save_queue_backpressure\ncontinuation_journal_queue_backpressure"))
    if detail == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("profile_health"))
    return False


def runtime_doctor_render_policy_setting_key(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], detail: Int64
) -> Bool:
    if detail == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("responses_active_limit"))
    if detail == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("compact_active_limit"))
    if detail == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_active_limit"))
    if detail == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("standard_active_limit"))
    if detail == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("active_request_limit"))
    if detail == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("profile_inflight_soft_limit"))
    if detail == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("profile_inflight_hard_limit"))
    if detail == 8:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_worker_count"))
    if detail == 9:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_queue_capacity"))
    if detail == 10:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_capacity"))
    if detail == 11:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_worker_count"))
    if detail == 12:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_queue_capacity"))
    if detail == 13:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow_capacity"))
    if detail == 14:
        return runtime_doctor_render_put_literal(writer, StringSlice("pressure_admission_wait_budget_ms"))
    return False


def runtime_doctor_render_policy_marker_name(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _], detail: Int64
) -> Bool:
    if detail == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_rejected"))
    if detail == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_reject"))
    if detail == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_enqueue"))
    if detail == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_connect_overflow_dispatch"))
    if detail == 102:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow_reject"))
    if detail == 103:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow_enqueue"))
    if detail == 104:
        return runtime_doctor_render_put_literal(writer, StringSlice("websocket_dns_overflow_dispatch"))
    return runtime_doctor_render_put_literal(writer, StringSlice("-"))


def runtime_doctor_render_policy_setting_rationale(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    var suggestion = input.detail // 100
    var key = input.detail % 100
    if suggestion == 1 and key == 5:
        return runtime_doctor_render_put_literal(writer, StringSlice("keep the global admission cap above the suggested lane cap"))
    if suggestion == 1:
        return runtime_doctor_render_put_literal(writer, StringSlice("raise the ")) and runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("responses")) and runtime_doctor_render_put_literal(writer, StringSlice(" lane cap after repeated lane-limit markers"))
    if suggestion == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("allow more pre-commit requests through local admission"))
    if suggestion == 3 and key == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("delay soft load penalty until a profile has more concurrent work"))
    if suggestion == 3:
        return runtime_doctor_render_put_literal(writer, StringSlice("raise the fresh-selection hard cap for a busy profile"))
    if suggestion == 4 or suggestion == 5:
        if key == 8 or key == 11:
            return runtime_doctor_render_put_literal(writer, StringSlice("increase bounded executor parallelism"))
        if key == 9 or key == 12:
            return runtime_doctor_render_put_literal(writer, StringSlice("increase bounded executor queue capacity"))
        return runtime_doctor_render_put_literal(writer, StringSlice("increase burst overflow buffering after the bounded queue fills"))
    if suggestion == 6 and key == 2:
        return runtime_doctor_render_put_literal(writer, StringSlice("reduce fresh compact churn that creates continuation state writes"))
    if suggestion == 6 and key == 4:
        return runtime_doctor_render_put_literal(writer, StringSlice("reduce side-lane churn while persistence is behind"))
    if suggestion == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("let pressure-mode admission wait briefly for queues to drain"))
    if suggestion == 7 and key == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("spread fresh work away from accounts accumulating route-specific health penalties"))
    if suggestion == 7:
        return runtime_doctor_render_put_literal(writer, StringSlice("cap fresh work per profile more tightly while route health recovers"))
    return False


def runtime_doctor_render_policy_suggestion_reason(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    var detail = input.detail
    if detail == 1:
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(" lane-limit marker(s) on lane=")) and runtime_doctor_render_put_value_or_literal(writer, input, 1, StringSlice("responses")) and runtime_doctor_render_put_literal(writer, StringSlice("; apply only if host/network headroom exists"))
    if detail == 2:
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(" global active-limit marker(s); raise only if local CPU/network is not saturated"))
    if detail == 3:
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(" per-profile in-flight saturation marker(s), latest profile=")) and runtime_doctor_render_put_value_or_literal(writer, input, 2, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice("; raise only if account fan-out is intentional"))
    if detail == 4 or detail == 5:
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(" websocket executor overflow marker(s), latest=")) and runtime_doctor_render_put_value_or_literal(writer, input, 3, StringSlice("-")) and runtime_doctor_render_put_literal(writer, StringSlice("; raise only for bursty session starts"))
    if detail == 6:
        return runtime_doctor_render_put_literal(writer, StringSlice("state-save backpressure=")) and runtime_doctor_render_put_value_or_literal(writer, input, 4, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(", continuation-journal backpressure=")) and runtime_doctor_render_put_value_or_literal(writer, input, 5, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice("; throttle churn while queues drain"))
    if detail == 7:
        return runtime_doctor_render_put_value_or_literal(writer, input, 0, StringSlice("0")) and runtime_doctor_render_put_literal(writer, StringSlice(" route-scoped health marker(s), latest=")) and runtime_doctor_render_put_value_or_literal(writer, input, 6, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice("/")) and runtime_doctor_render_put_value_or_literal(writer, input, 7, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice(" reason=")) and runtime_doctor_render_put_value_or_literal(writer, input, 8, StringSlice("unknown")) and runtime_doctor_render_put_literal(writer, StringSlice("; lower per-profile fresh pressure if this repeats"))
    return False

def runtime_doctor_render_value(
    writer: Pointer[mut=True, RuntimeDoctorRenderWriter, _],
    input: ProdexRuntimeDoctorRenderInput,
) -> Bool:
    if input.operation == RENDER_DIAGNOSIS:
        return runtime_doctor_render_diagnosis(writer, input)
    if input.operation == RENDER_POLICY_SUGGESTION_ID:
        return runtime_doctor_render_policy_suggestion_id(writer, input.detail)
    if input.operation == RENDER_POLICY_SUGGESTION_TITLE:
        return runtime_doctor_render_policy_suggestion_title(writer, input.detail)
    if input.operation == RENDER_POLICY_SUGGESTION_MARKERS:
        return runtime_doctor_render_policy_suggestion_markers(writer, input.detail)
    if input.operation == RENDER_POLICY_SETTING_KEY:
        return runtime_doctor_render_policy_setting_key(writer, input.detail)
    if input.operation == RENDER_POLICY_SETTING_RATIONALE:
        return runtime_doctor_render_policy_setting_rationale(writer, input)
    if input.operation == RENDER_POLICY_SUGGESTION_REASON:
        return runtime_doctor_render_policy_suggestion_reason(writer, input)
    if input.operation == RENDER_POLICY_MARKER_NAME:
        return runtime_doctor_render_policy_marker_name(writer, input.detail)
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
    if input.operation < RENDER_PREVIOUS_RESPONSE or input.operation > RENDER_POLICY_MARKER_NAME or input.detail < 0 or input.detail > 799 or input.values_address == 0:
        return 1
    for index in range(16):
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
