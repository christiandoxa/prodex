# ADR 0945: Storage Audit Retention Purge Error Debug Redaction

## Status

Accepted

## Context

Audit retention purge planner errors can carry tenant identifiers when storage
keys do not match purge batch tenants. Display output is generic, but derived
debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `AuditRetentionPurgePlanError`. Redact
tenant identifiers while preserving the error variant name.

## Consequences

Storage diagnostics can distinguish audit retention purge planner failures
without leaking tenant identifiers from mismatch errors.
