# 0634: Policy Issued-At Zero Guard

## Status

Accepted

## Context

Policy snapshots carry an `issued_at_unix_ms` value used by activation,
readiness, age telemetry, and last-known-good diagnostics. A zero timestamp is
not a valid publication time and can make stale-policy age calculations
ambiguous.

## Decision

`validate_policy_snapshot` now rejects snapshots whose `issued_at_unix_ms` is
zero. The activation error planner maps this to `policy_issued_at_invalid`.

## Consequences

- Policy activation requires a concrete publication timestamp.
- Readiness and policy-age telemetry do not need to interpret zero as a special
  valid snapshot time.
- Client-visible errors stay stable and redacted.
