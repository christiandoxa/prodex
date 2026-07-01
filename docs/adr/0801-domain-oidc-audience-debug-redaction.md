# ADR 0801: Redact OIDC audience debug output

Status: Accepted

## Context

`Audience` stores trusted OIDC audience selectors used during token validation.
Its derived `Debug` formatter exposed raw audience values to diagnostics and
containing debug output.

## Decision

Use a custom `Debug` implementation for `Audience` that preserves audience
presence while redacting the raw selector.

## Consequences

Serialization and policy comparisons remain unchanged, but raw audience
selectors no longer appear through `Audience` debug output.
