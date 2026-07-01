# ADR 0830: Redact domain secret request and rotation debug output

Status: Accepted

## Context

Domain secret resolution requests and rotation status values carry secret
references, versions, and refresh timestamps. Derived `Debug` output exposed
those details through diagnostics.

## Decision

Use custom `Debug` implementations for `SecretResolutionRequest` and
`SecretRotationStatus` that preserve purpose and rotation shape while redacting
secret references and refresh timestamps.

## Consequences

Diagnostics can still distinguish secret purpose, previous-version presence,
and scheduled refresh presence, but raw secret references, versions, and timing
details no longer appear through these debug formatters.
