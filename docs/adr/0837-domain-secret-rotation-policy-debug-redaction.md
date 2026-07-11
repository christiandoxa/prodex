# ADR 0837: Redact domain secret rotation policy debug timings

Status: Accepted

## Context

`SecretRotationPolicy` carries max-age and overlap timings for secret rotation.
Derived `Debug` output exposed those rotation timing values through diagnostics.

## Decision

Use a custom `Debug` implementation for `SecretRotationPolicy` that preserves
audit-event requirement shape while redacting timing values.

## Consequences

Diagnostics can still show whether rotation requires audit events, but exact
rotation timing values no longer appear through secret rotation policy debug
output.
