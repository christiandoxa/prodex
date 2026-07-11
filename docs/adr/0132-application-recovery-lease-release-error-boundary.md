# ADR 0132: Application recovery lease release error boundary

## Status

Accepted

## Context

Recovery lease release is a Redis-backed coordination use case used by
multi-replica expired-reservation workers after a shard completes or aborts. Raw
release errors can include tenant identifiers, Redis implementation details,
shard names, or owner tokens. Those details are useful in trusted recovery-worker
logs but must not become generic client-visible or operator-facing response
text.

## Decision

Add `plan_application_recovery_lease_release_error_response` to
`prodex-application`. It maps `ApplicationRecoveryLeaseReleaseError` to a stable
`recovery_lease_release_unavailable` response with a service-unavailable status
and a generic message.

The raw Redis planning error remains available to trusted, redacted diagnostics
and recovery-worker logs.

## Consequences

Composition roots can report lease-release failures consistently without leaking
Redis details, tenant IDs, shard names, or recovery lease owner tokens.
