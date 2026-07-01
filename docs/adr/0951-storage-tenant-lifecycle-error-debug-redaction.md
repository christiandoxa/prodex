# ADR 0951: Storage Tenant Lifecycle Error Debug Redaction

## Status

Accepted

## Context

Tenant lifecycle planner errors can carry tenant identifiers when storage keys
do not match lifecycle request tenants. Display output is generic, but derived
debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `TenantLifecyclePlanError`. Redact
tenant identifiers while preserving error variant names.

## Consequences

Storage diagnostics can distinguish tenant lifecycle planner failures without
leaking tenant identifiers from mismatch errors.
