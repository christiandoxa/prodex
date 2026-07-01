# ADR 0947: Storage Role-Binding Error Debug Redaction

## Status

Accepted

## Context

Role-binding mutation planner errors can carry tenant identifiers when storage
keys do not match mutation request tenants. Display output is generic, but
derived debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `RoleBindingMutationPlanError`. Redact
tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish role-binding mutation planner failures
without leaking tenant identifiers from mismatch errors.
