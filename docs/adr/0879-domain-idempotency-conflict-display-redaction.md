# ADR 0879: Domain idempotency conflict display redaction

## Status

Accepted.

## Context

`IdempotencyConflict::TenantMismatch` and `KeyMismatch` already share the same
stable response plan, but their raw `Display` text still exposed whether the
conflict was cross-tenant or cross-key. That classification belongs in typed
variants and trusted diagnostics, not accidental client-visible strings.

## Decision

Render tenant and key mismatch conflicts as `idempotency replay conflict`, the
same message used by the response planner. Leave request-fingerprint reuse on
its existing stable message because it maps to a distinct client-facing conflict
code.

Regression coverage pins the exact display messages and rejects tenant/key
wording from the generic conflict variants.

## Consequences

Callers can still match `IdempotencyConflict` variants for metrics and audit
classification. Raw display output no longer reveals tenant or storage-key
relationship details.
