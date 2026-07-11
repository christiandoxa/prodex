# ADR 0750: Redact domain audit retention purge-batch error debug output

## Status

Accepted

## Context

`AuditRetentionPurgeBatch` redacts tenant and event identifiers in `Debug`
output. Its validation error still exposed rejected batch counts and limits via
derived `Debug`. Retention delete diagnostics should keep failure shape without
echoing operator-selected purge sizes.

## Decision

Implement custom `Debug` for `AuditRetentionPurgeBatchError`. Keep
`CrossTenantKey` as a low-cardinality variant name and redact `TooManyKeys`
`count` and `limit` values.

## Consequences

Diagnostics still distinguish oversize batches from cross-tenant keys. Exact
batch counts and limits remain in typed errors for programmatic handling, not
formatter output.
