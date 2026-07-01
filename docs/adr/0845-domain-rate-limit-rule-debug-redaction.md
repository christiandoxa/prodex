# ADR 0845: Redact domain rate-limit rule debug output

Status: Accepted

## Context

`RateLimitRule` carries configured request ceilings and window duration. Derived
`Debug` output exposed those policy values through diagnostics.

## Decision

Use a custom `Debug` implementation for `RateLimitRule` that preserves field
shape while redacting the configured ceiling and window.

## Consequences

Rate-limit evaluation and serialization remain unchanged, but generic debug
diagnostics no longer expose rule configuration values.
