# ADR 0760: Redact domain audit time-range error debug output

## Status

Accepted

## Context

`AuditTimeRange` rejects invalid query and export time windows before audit
storage or serialization paths run. Range boundaries can reveal operational
timing and can be adjacent to tenant, resource, or credential-bearing request
data in diagnostics.

## Decision

Implement custom `Debug` for `AuditTimeRangeError`. Keep only the
`StartAfterEnd` variant name in formatter output.

## Consequences

Diagnostics still show the range ordering failure. Future time-range error
variants must make redaction decisions explicitly instead of inheriting derived
`Debug`.
