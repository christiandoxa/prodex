# ADR 0554: Redact capability status diagnostics

## Status

Accepted

## Context

`prodex capability` and Super setup/status reports include JSON and terminal
diagnostics for embedded assets, local memory store readiness, and Presidio
health. Those diagnostics previously copied raw error chains or HTTP client
errors into status details. In regulated deployments those strings can contain
bearer tokens, key-bearing URLs, local paths, or backend routing details.

## Decision

Capability status error/detail strings pass through the shared secret-like text
redactor before being rendered or serialized. Status names, readiness booleans,
checks, and non-error explanatory text are unchanged.

## Consequences

- Operators still see which capability check failed.
- Secret-like bearer token and key-bearing URL material is removed from
  capability diagnostics.
- Regression coverage pins the redaction helper used by setup, memory, and
  Presidio status paths.
