# Prodex 0.437.2 quota recovery and descriptor audit

## Evidence and scope

The reported Codex turn recorded an upstream usage-limit failure at
2026-10-10 14:42:10 WIB. The same turn previously retried HTTP 503 errors over
the local Prodex WebSocket endpoint. The quota display showed one account with
remaining weekly quota and two accounts whose weekly quota was exhausted.
That does not establish model-specific availability or immediate admission for
every account, and the original proxy's per-request trace was not retained.

A credential-free production WebSocket reproduction against the pre-patch
`586b4a1ca86325c36bbb8a20158687905f1866a1` baseline reproduced the terminal
`usage_limit_reached` leak when the only quota-positive alternative was occupied.
This proves a concrete Prodex recovery defect consistent with the incident,
without claiming an exact reconstruction of the original account timing.

## Quota eligibility is not readiness

The old recovery-eligibility helper applied local in-flight and transport-backoff
filters intended for immediate scheduling. It could therefore report an empty
fallback pool while a valid account was temporarily busy or cooling down.
The caller then passed the exhausted owner's quota error to Codex instead of
requesting the existing full-context replay protocol.

A dedicated recoverable-pool helper now uses the existing canonical Mojo
`RetryablePool` decision with observed auth, provider, requested-model quota, and quarantine
facts. Actual account quota exhaustion, auth failure, unsupported runtime
providers, and request-local exclusions remain disqualifying. The ready-now
helper, ordinary selection, one-shot last-chance policy, and atomic admission
limits are unchanged. No Rust semantic fallback is
introduced and no upstream quota is bypassed.

## Replay must not stay pinned to a soft session preference

After the full-history replay signal, a retained session preference can still
name the exhausted former owner. Capacity waiting previously used that soft
preference as an exclusive wait owner, preventing recovery on another account.
The WebSocket wait scope now uses the existing Mojo hard-owner selection with
strict, turn-state, and verified previous-response constraints, without treating
a soft session preference as an exclusive owner. Scheduling priority remains. The transient-recovery wait likewise distinguishes
soft session priority from hard continuation ownership, so a recoverable transport
cooldown on the alternative does not terminate the replay early.

The real-socket regression creates a successful first turn on account A, returns
an upstream usage-limit error on a bound follow-up, occupies account B, verifies
that Codex receives only the canonical full-context retry signal, then replays
complete history and releases B's admission capacity. It completes on B without
replaying visible output or moving an opaque continuation to another account.

## Descriptor headroom

Read-only inspection found Prodex parents inheriting a soft `RLIMIT_NOFILE` of
1,024 with a higher hard limit. No active descriptor leak was established. The
startup mitigation raises only the current process's soft limit to at most 8,192,
capped by its existing hard limit, before reading state or launching workers.
It does not lower a larger limit, alter the host hard limit, close other tasks'
descriptors, rewrite state, or substitute a fabricated empty state.

Three isolated child-process tests prove that exhausted descriptor capacity causes
`EMFILE` while reading a synthetic state file, that the helper restores that read
without changing its content, and that higher existing limits and the hard limit
are preserved. A nested exec regression verifies that spawned child processes inherit the raised soft limit. This is a tested headroom mitigation, not a claim that every
possible descriptor leak or host-wide resource shortage is fixed.

## Regression evidence

The two recovery-eligibility tests fail against the old gate and pass after its
replacement. The real-socket busy-alternative test fails before the gate fix by
observing the upstream quota frame. With only the gate fixed it exposes the
second, stale-session wait-scope defect. With both fixes it completes on account B.

Focused tests include:

- `quota_recovery_keeps_busy_positive_quota_account_eligible_for_full_context_retry`
- `quota_recovery_keeps_temporary_transport_backoff_distinct_from_quota_exhaustion`
- `runtime_proxy_websocket_owned_quota_waits_for_busy_fallback_without_quota_leak`
- `runtime_proxy_websocket_owned_quota_replays_on_ready_profile`
- `descriptor_headroom_recovers_state_reads_under_inherited_low_limit`
- `descriptor_headroom_never_lowers_existing_limits`
- `descriptor_headroom_is_inherited_by_exec_child`

A mutation that falsely marked every alternate as quota-blocked made the busy-account
regression fail with the expected assertion. Restoring the source byte-for-byte
returned the direct checks and real WebSocket replay regressions to green. The
wider validation ran 46 WebSocket flow tests, two account-handoff socket tests,
two descriptor tests, terminal/no-alternative and post-compaction cases, twelve
precommit cases, three HTTP/persistence quota cases, and the registered runtime
manifest without skipping the new case.

All reproduction inputs, accounts, and upstream responses are synthetic. No live
provider request or user credential is required for these tests. Existing running
binaries are not patched in memory; restarted Prodex processes use the new code.
