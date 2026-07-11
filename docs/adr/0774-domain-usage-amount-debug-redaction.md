# ADR 0774: Redact usage amount debug output

## Status

Accepted

## Context

`UsageAmount` is embedded across budget, reservation, reconciliation, and ledger
models. Derived `Debug` exposed raw token and cost values in every containing
diagnostic unless each container redacted them separately.

## Decision

Implement custom `Debug` for `UsageAmount`. Preserve the field shape while
redacting token and cost values.

## Consequences

Diagnostics still show that an amount was present. Raw token and cost values no
longer appear through `UsageAmount` debug formatting or through containers that
delegate to it.
