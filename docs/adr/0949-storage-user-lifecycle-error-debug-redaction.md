# ADR 0949: Storage User Lifecycle Error Debug Redaction

## Status

Accepted

## Context

User lifecycle planner errors can carry tenant identifiers when storage keys do
not match lifecycle request tenants. Display output is generic, but derived
debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `UserLifecyclePlanError`. Redact tenant
identifiers while preserving error variant names.

## Consequences

Storage diagnostics can distinguish user lifecycle planner failures without
leaking tenant identifiers from mismatch errors.
