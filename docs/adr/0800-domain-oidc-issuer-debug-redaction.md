# ADR 0800: Redact OIDC issuer debug output

Status: Accepted

## Context

`Issuer` stores the trusted OIDC issuer configuration used to derive discovery
and JWKS trust boundaries. Its derived `Debug` formatter exposed raw issuer
URLs to diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `Issuer` that preserves issuer presence
while redacting the raw URL.

## Consequences

Serialization and explicit `as_str()` access still expose issuer values where
configuration contracts require them, but raw issuer URLs no longer appear
through `Issuer` debug output.
