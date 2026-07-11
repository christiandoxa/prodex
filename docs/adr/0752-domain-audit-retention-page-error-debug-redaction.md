# ADR 0752: Redact domain audit retention-page error debug output

## Status

Accepted

## Context

Retention pagination combines retention-plan validation, legal-hold decisions,
cursor validation, and timestamp validation. `AuditRetentionPageError` wraps all
of these branches. Derived `Debug` would make page diagnostics depend on nested
formatter defaults.

## Decision

Implement custom `Debug` for `AuditRetentionPageError`. Preserve branch names
and delegate only to nested domain errors with existing redaction contracts.

## Consequences

Diagnostics still show whether retention page construction failed on retention,
decision, cursor, or timestamp validation. Tenant identifiers, timestamps, and
cursor positions remain out of formatter output.
