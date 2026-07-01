# ADR 0933: Storage budget policy debug redaction

## Status

Accepted.

## Context

Budget policy update commands and plans carry tenant IDs, budget scopes, limits,
storage keys, and update timestamps. These fields are needed by storage adapters
but should not appear in generic debug output.

## Decision

Use custom `Debug` implementations for `BudgetPolicyUpdateCommand` and
`BudgetPolicyUpdatePlan`. Redact storage keys, tenant IDs, budget scopes,
limits, and update timestamps.

Regression coverage rejects tenant, scope, limit, and timestamp values in
rendered budget policy debug output.

## Consequences

Storage diagnostics can identify budget policy update command/plan shape without
exposing budget names or limits. Planning behavior is unchanged.
