# ADR 0932: Storage user lifecycle debug redaction

## Status

Accepted.

## Context

User lifecycle commands and plans carry tenant IDs, principal IDs, external
identity IDs, display names, lifecycle kinds, and timestamps. These fields are
needed by storage adapters but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `UserLifecycleCommand` and
`UserLifecyclePlan`. Redact storage keys, tenant IDs, principal IDs, external
IDs, display names, and timestamps while preserving lifecycle kind.

Regression coverage rejects tenant, principal, external ID, display-name, and
timestamp values in rendered user lifecycle debug output.

## Consequences

Storage diagnostics can identify user lifecycle command/plan shape without
exposing user identifiers or names. Planning behavior is unchanged.
