# ADR 0944: Storage Append-Only Audit Error Debug Redaction

## Status

Accepted

## Context

Append-only audit planner errors can carry tenant identifiers when storage keys
do not match audit event tenants. Display output is generic, but derived debug
output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `AppendOnlyAuditPlanError`. Redact
tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish append-only audit planner failures without
leaking tenant identifiers from mismatch errors.
