# 0209: Application Control Plane Idempotency Replay

## Status

Accepted

## Context

Application preflight now requires idempotency keys for mutating control-plane
operations. It also needs a side-effect-free replay decision boundary so
composition roots handle first execution, concurrent duplicate requests,
completed replay, and fingerprint conflicts consistently before adapting audit
or mutation storage.

Without this boundary, destructive operations such as audit retention purge
could retry while another attempt is pending or reuse a key with a different
request fingerprint.

## Decision

Add `plan_application_control_plane_idempotency_replay`.

The planner delegates to the domain replay guard and maps conflicts through the
stable idempotency conflict planner. It returns execute-and-record-pending,
already-in-progress, or replay decisions without performing storage I/O.

## Consequences

- Composition roots share one replay decision for mutating admin operations.
- Concurrent duplicate control-plane mutations can be blocked before execution.
- Completed mutation results can be replayed without re-running storage
  mutations.
- Tenant IDs, fingerprints, and idempotency keys stay out of client-visible
  conflict responses.
