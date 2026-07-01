# ADR 0575: Redact capability fail statuses

## Status

Accepted

## Context

`prodex capability` already redacts detailed setup and status diagnostics, but
some install-check and Super Doctor rows still rendered `fail ({err:#})`
directly. Capability checks can include local paths, command output, bearer
tokens, or key-bearing URLs in lower-level error chains.

## Decision

Route capability `fail (...)` status strings through the same secret-like text
redactor used by capability detail fields.

## Consequences

- Capability status rows still show which check failed.
- Secret-like material is removed from `fail (...)` status text.
- Capability probing and setup behavior are unchanged.
