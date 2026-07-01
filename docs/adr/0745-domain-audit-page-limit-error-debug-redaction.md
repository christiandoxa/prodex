# ADR 0745: Redact domain audit page-limit error debug output

## Status

Accepted

## Context

`AuditPageLimit` already redacts exact pagination limits in `Debug` output.
Its validation error kept the rejected oversized limit in derived `Debug`
output. Audit-facing diagnostics should keep failure shape without echoing
tenant-selected query/export limits.

## Decision

Implement custom `Debug` for `AuditPageLimitError`. Keep `Zero` as a
low-cardinality variant name and render `TooLarge.value` as `"<redacted>"`.

## Consequences

Logs and test failures still identify the validation branch. Exact rejected
limit values remain available through structured control flow, not formatter
output.
