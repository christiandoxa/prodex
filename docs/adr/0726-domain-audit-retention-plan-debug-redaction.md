# ADR 0726: Redact domain audit retention-plan debug output

## Status

Accepted

## Context

Audit retention plans combine tenant query scope, retention policy duration, and
the evaluation timestamp. These fields are required for legal-hold-aware purge
decisions, but generic `Debug` output can appear outside authorized audit
tooling and should not expose tenant IDs or retention timing internals.

## Decision

Implement custom `Debug` for `AuditRetentionPlan`. The debug output keeps the
redacted query scope shape while replacing policy and timestamp internals with
redacted placeholders. Retention cutoff calculation, purge selection,
serialization, and equality are unchanged.

## Consequences

- Retention and legal-hold behavior remains unchanged.
- Generic diagnostics no longer expose tenant IDs, retention duration, or
  evaluation timestamps from retention plans.
- Exact retention criteria remain available through authorized control-plane
  audit tooling.
