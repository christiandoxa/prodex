# ADR 0798: Redact error metadata debug output

Status: Accepted

## Context

`ErrorMetadata` carries request, tenant, and audit-event identifiers used in API
error envelopes. Its derived `Debug` formatter exposed those raw identifiers to
diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `ErrorMetadata` that preserves
identifier presence and retryability while redacting raw identifier values.

## Consequences

Serialization still exposes metadata fields where API contracts require them,
but raw request, tenant, and audit-event identifiers no longer appear through
`ErrorMetadata` debug output.
