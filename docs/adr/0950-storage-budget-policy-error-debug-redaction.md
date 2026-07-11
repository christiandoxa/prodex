# ADR 0950: Storage Budget Policy Error Debug Redaction

## Status

Accepted

## Context

Budget policy update planner errors can carry tenant identifiers when storage
keys do not match update request tenants. Display output is generic, but
derived debug output would expose the mismatched tenant values.

## Decision

Use a custom `Debug` implementation for `BudgetPolicyUpdatePlanError`. Redact
tenant identifiers while preserving error variant names.

## Consequences

Storage diagnostics can distinguish budget policy update planner failures
without leaking tenant identifiers from mismatch errors.
