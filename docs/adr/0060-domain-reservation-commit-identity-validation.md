# ADR 0060: Validate reservation commit identity in the domain model

## Status

Accepted

## Context

The enterprise target requires reservation-based accounting, tenant isolation, and
idempotent billing semantics across replicas. A reservation commit must apply only
to the reservation chain that created it: same tenant, call, and reservation ID.
The existing domain helper could apply a commit to aggregate budget state without
checking that the commit belonged to the original reservation request.

## Decision

The domain accounting model now exposes checked commit helpers:

- `validate_reservation_commit(request, commit)`
- `commit_reservation_checked(snapshot, request, commit)`

These helpers reject mismatched tenant, call, or reservation IDs before applying
usage. `commit_reservation(snapshot, commit)` is the low-level fallible commit
helper; it rejects actual usage above the reserved amount and committed usage
overflow before updating the budget snapshot.

## Consequences

- Storage adapters and future gateway/control-plane use cases have a domain-level
  guard against cross-tenant or wrong-call accounting commits.
- Existing callers must handle deterministic commit errors instead of receiving a
  silently saturated accounting snapshot.
- Regression tests cover cross-tenant, wrong-call, wrong-reservation, matching
  commit, actual-above-reserved, and committed-overflow behavior.
