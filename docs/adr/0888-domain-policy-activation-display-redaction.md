# ADR 0888: Domain policy activation display redaction

## Status

Accepted.

## Context

Policy activation response planners already provide stable status, code, and
message fields for timestamp, digest, signature, and verification failures. Raw
`Display` output still named those validation classes directly.

## Decision

Render every `PolicyActivationError` variant as `policy snapshot is invalid`.
Keep response planners and typed variants unchanged so callers can still expose
low-cardinality codes and classify failures for audit or metrics.

Regression coverage pins the exact display string and rejects digest,
signature, and timestamp wording from raw display output.

## Consequences

Policy activation boundaries can safely fall back to raw display strings without
revealing integrity metadata classification. Trusted diagnostics should match
typed variants when they need the exact reason.
