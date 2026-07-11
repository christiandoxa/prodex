# ADR 0931: Storage service identity debug redaction

## Status

Accepted.

## Context

Service identity create commands and plans carry tenant IDs, principal IDs,
display names, storage keys, and creation timestamps. These fields are needed by
storage adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `ServiceIdentityCreateCommand` and
`ServiceIdentityCreatePlan`. Redact storage keys, tenant IDs, principal IDs,
display names, and creation timestamps.

Regression coverage rejects tenant, principal, display-name, and timestamp
values in rendered service identity debug output.

## Consequences

Storage diagnostics can identify service identity create command/plan shape
without exposing account identifiers or display names. Planning behavior is
unchanged.
