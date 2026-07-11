# 0643: Error Envelope Message Sanitization Boundary

## Status

Accepted

## Context

`ErrorEnvelope::new` already redacted secret-like public messages, but still
accepted empty, control-character, non-ASCII, or very large strings. Those
messages are serialized into stable API responses, so adapters could expose
unstable formatting or oversized request/provider-controlled text.

## Decision

`ErrorEnvelope::new` now replaces empty, over-512-byte, non-printable, and
non-ASCII public messages with `request failed`.

## Consequences

- Public error envelopes keep bounded printable messages.
- Secret-like message redaction remains centralized in the domain constructor.
- Existing callers keep the same constructor signature.
