# ADR 0727: Redact domain audit retention-hold debug output

## Status

Accepted

## Context

Audit retention holds are legal-hold metadata. They must serialize the tenant,
audit event, reason code, and optional expiry timestamp so retention cleanup can
protect the right events. Generic `Debug` output is a separate diagnostics
surface and should not expose those identifiers or timing details.

## Decision

Implement custom `Debug` for `AuditRetentionHold`. Debug output preserves the
presence of optional expiry metadata while redacting tenant ID, audit event ID,
reason code, and expiry timestamp. Serialization, equality, and protection logic
are unchanged.

## Consequences

- Legal-hold retention behavior remains unchanged.
- Generic diagnostics no longer expose legal-hold tenant, event, reason, or
  expiry details.
- Exact hold metadata remains available through authorized audit/control-plane
  tooling.
