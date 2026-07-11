# ADR 0749: Redact domain audit retention decision-error debug output

## Status

Accepted

## Context

`AuditRetentionPlan::purge_decision` combines retention-plan validation with
legal-hold validation. Its error type wraps both branches. Derived `Debug`
would inherit whatever nested fields future variants expose, which makes the
retention deletion path harder to audit.

## Decision

Implement custom `Debug` for `AuditRetentionDecisionError`. Preserve
`Retention` and `Hold` variant shape and delegate only to nested domain errors
that already carry explicit redaction contracts.

## Consequences

Diagnostics still show whether purge-decision validation failed in retention
planning or hold validation. Tenant IDs, timestamps, and legal-hold metadata
remain out of formatter output.
