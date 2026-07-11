# ADR 0907: Storage lifecycle field display redaction

## Status

Accepted.

## Context

Storage lifecycle planners reject empty user external IDs, user display names,
budget policy scopes, and tenant display names before adapters write durable
control-plane state. Their response planners already expose stable request
messages, but raw `Display` output still named the missing field.

## Decision

Render `UserLifecyclePlanError`, `BudgetPolicyUpdatePlanError`, and
`TenantLifecyclePlanError` with the same messages used by their response
planners. Keep typed variants unchanged for trusted classification.

Regression coverage pins exact display strings for tenant mismatch and empty
field variants.

## Consequences

Storage lifecycle composition roots can safely fall back to raw display strings
without exposing request shape. Diagnostics should continue matching typed
variants for exact reasons.
