# 0188: Domain Audit Query Selection

## Status

Accepted

## Context

Audit query and export adapters need to apply tenant scope, time filtering,
sort order, and page limits consistently. Keeping only the raw query plan in
the domain still leaves each adapter responsible for sorting and truncating
results, which can drift across control-plane HTTP handlers, storage adapters,
and export serializers.

The domain can provide this selection behavior without depending on HTTP,
filesystem, database, or provider crates.

## Decision

`AuditQueryPlan` owns `select_events`.

The helper iterates candidate events, applies `matches_event`, fails closed for
cross-tenant events or invalid event timestamps, sorts by `occurred_at_unix_ms`
according to `AuditSortOrder`, breaks timestamp ties by `AuditEventId`, and
then truncates to `AuditPageLimit`.

`AuditExportPlan` delegates selection to its embedded `AuditQueryPlan` so
audit exports use the same bounded and tenant-scoped event selection path as
interactive audit queries.

## Consequences

- Audit query and export adapters can share one deterministic bounded selection
  contract.
- Cross-tenant events and invalid timestamps are not silently dropped; selection
  fails closed before serialization.
- Page limits and sort order are enforced in the domain boundary while storage
  adapters remain free to push the same predicates down for efficiency.
