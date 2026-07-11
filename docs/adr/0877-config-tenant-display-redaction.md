# ADR 0877: Config tenant display redaction

## Status

Accepted.

## Context

`ConfigPublicationError::TenantMismatch` and
`ConfigInvalidationError::TenantMismatch` already avoid rendering tenant IDs,
but their `Display` strings still exposed the internal mismatch classification.
That is useful for trusted diagnostics but should not be available as a raw
client-facing error string.

## Decision

Render both tenant mismatch variants as the stable
`configuration request is invalid` message. Keep the existing response planners
unchanged so HTTP/API status, code, and message contracts remain stable.

Regression coverage pins the exact `Display` string and rejects tenant IDs plus
the internal `tenant mismatch` wording.

## Consequences

Trusted logs and audits should use structured variants when they need the
specific mismatch reason. Raw error display remains low-cardinality and safe for
accidental exposure at config publication or invalidation boundaries.
