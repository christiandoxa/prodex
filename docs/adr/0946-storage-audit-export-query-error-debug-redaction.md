# ADR 0946: Storage Audit Export Query Error Debug Redaction

## Status

Accepted

## Context

Audit export query planner errors can carry tenant identifiers when storage keys
do not match query tenants. Display output is generic, but derived debug output
would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `AuditExportQueryPlanError`. Redact
tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish audit export query planner failures without
leaking tenant identifiers from mismatch errors.
