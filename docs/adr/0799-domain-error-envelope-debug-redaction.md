# ADR 0799: Redact error envelope debug output

Status: Accepted

## Context

`ErrorEnvelope` carries machine-readable codes, public messages, and correlation
metadata. Its derived `Debug` formatter exposed those fields directly whenever
an envelope or containing value was logged for diagnostics.

## Decision

Use a custom `Debug` implementation for `ErrorEnvelope` that preserves envelope
version and category while redacting message content and relying on the redacted
debug implementations for code and metadata.

## Consequences

Serialization remains unchanged for API contracts, but diagnostic debug output
no longer exposes raw error codes, messages, or correlation metadata through
`ErrorEnvelope`.
