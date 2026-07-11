# 0642: Error Envelope Invalid Code Fail Closed

## Status

Accepted

## Context

`ErrorCode::try_new` validates stable machine-readable error codes, but
`ErrorEnvelope::new` accepted any `ErrorCode`, including values created through
the unchecked constructor. Invalid codes could leak path-like or unstable
request-controlled strings into public API responses.

## Decision

`ErrorEnvelope::new` now validates its code argument and replaces invalid codes
with `internal.error`.

## Consequences

- Public error envelopes keep a valid machine-readable code even when an adapter
  passes malformed input.
- Invalid raw code strings are not serialized into API responses.
- Existing callers keep the same constructor signature.
