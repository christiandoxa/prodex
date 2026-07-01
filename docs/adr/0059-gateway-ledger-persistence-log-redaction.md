# ADR 0059: Redact gateway billing ledger persistence diagnostics

## Status

Accepted

## Context

Gateway billing ledger reconciliation records request accounting events after
provider responses. Reconciliation failures were logged with raw backend error
strings. Billing and quota logs are security-sensitive in regulated deployments:
raw backend diagnostics can expose filesystem layout, database details, or other
operational internals while the event itself only needs to signal that durable
ledger persistence failed.

## Decision

Gateway billing ledger reconciliation failure logs now preserve request and
backend context but replace raw backend errors with a stable field:

```text
error_kind=gateway_ledger_persistence_failed
```

This is a runtime-log redaction change only; ledger reconciliation behavior and
retry timing are unchanged.

## Consequences

- Operators can still identify ledger reconciliation failures by request and
  backend family.
- Runtime logs no longer expose raw ledger backend error strings from billing
  persistence failure events.
- Regression coverage pins the redacted structured log shape and rejects common
  filesystem/backend diagnostic substrings.
