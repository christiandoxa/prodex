# ADR 0802: Redact OIDC policy debug output

Status: Accepted

## Context

`OidcValidationPolicy` contains trusted issuer and audience configuration. Its
derived `Debug` formatter exposed raw identity-provider URLs and audience
selectors through policy diagnostics.

## Decision

Use a custom `Debug` implementation for `OidcValidationPolicy` that preserves
policy shape and allowed algorithms while relying on redacted issuer and
audience debug output.

## Consequences

OIDC policy diagnostics still show configured algorithm allowlists, but raw
issuer URLs and audience selectors no longer appear through policy debug output.
