# ADR 0571: Redact runtime broker promotion errors

## Status

Accepted

## Context

Runtime brokers can attempt to promote themselves to persistence owner after
startup. Promotion failures were written to the runtime log with detailed
`{err:#}` chains. Owner-lock and filesystem diagnostics can include local paths,
environment-specific values, bearer tokens, or key-bearing URLs.

## Decision

Keep the structured runtime log event name, but pass the detailed promotion
error chain through the shared secret-like text redactor before logging.

## Consequences

- Runtime logs still expose the promotion failure event for diagnostics.
- Secret-like material is removed from broker promotion failure details.
- Owner-lock acquisition and broker persistence behavior are unchanged.
