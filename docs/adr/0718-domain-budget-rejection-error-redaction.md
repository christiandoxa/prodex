# ADR 0718: Redact Budget Rejection Error Details

## Status

Accepted

## Context

Budget reservation happens before upstream provider calls. The domain
`BudgetRejection` keeps structured requested and available usage values so
trusted callers can classify limit failures. Its `Display` string still echoed
those amounts.

## Decision

Keep the structured `BudgetRejection` fields and stable response planner, but
make the `Display` string generic.

## Consequences

- Requested and available usage amounts are not echoed through budget rejection
  error strings.
- Internal code can still inspect structured fields where trusted diagnostics
  need them.
- Pre-upstream accounting failures stay aligned with regulated error-redaction
  requirements.
