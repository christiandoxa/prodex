# ADR 0948: Storage Service Identity Error Debug Redaction

## Status

Accepted

## Context

Service identity creation planner errors can carry tenant identifiers when
storage keys do not match creation request tenants. Display output is generic,
but derived debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `ServiceIdentityCreatePlanError`.
Redact tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish service identity creation planner failures
without leaking tenant identifiers from mismatch errors.
