# ADR 0934: Storage tenant lifecycle debug redaction

## Status

Accepted.

## Context

Tenant lifecycle commands and plans carry tenant IDs, display names, lifecycle
kinds, storage keys, and timestamps. These fields are needed by storage adapters
but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `TenantLifecycleCommand` and
`TenantLifecyclePlan`. Redact storage keys, tenant IDs, display names, and
timestamps while preserving lifecycle kind.

Regression coverage rejects tenant ID, display-name, and timestamp values in
rendered tenant lifecycle debug output.

## Consequences

Storage diagnostics can identify tenant lifecycle command/plan shape without
exposing tenant identifiers or names. Planning behavior is unchanged.
