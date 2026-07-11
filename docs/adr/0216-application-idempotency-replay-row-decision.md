# 0216: Application Idempotency Replay Row Decision

## Status

Accepted

## Context

The application boundary can plan durable idempotency lookup, pending insert,
and completion update. Storage can also materialize lookup rows into domain
`IdempotencyEntry<Vec<u8>>` values. Composition roots still need one safe
application helper that turns an optional durable lookup row into the replay
decision before any mutating admin storage execution.

If every composition root wires row materialization and domain replay decision
manually, one path can accidentally treat malformed rows as misses or drop the
stored request fingerprint before the domain conflict check.

## Decision

Add `plan_application_control_plane_idempotency_replay_from_lookup_row`.

The function accepts the current `IdempotentOperation` plus an optional
`IdempotencyRecordLookupRow`. Missing rows produce
`ExecuteAndRecordPending`. Present rows are materialized through
`materialize_idempotency_record_lookup_row` and then passed to
`decide_idempotency_replay`. Materialization failures map to a stable redacted
application error; domain fingerprint/key/tenant conflicts retain the existing
idempotency conflict envelope.

## Consequences

- Composition roots get a single application-level replay decision helper for
  durable SQL lookup rows.
- Malformed lookup rows fail closed instead of silently executing a mutation.
- Reused idempotency keys still surface as domain replay conflicts.
- Replay responses remain opaque bytes until adapted by the caller.
