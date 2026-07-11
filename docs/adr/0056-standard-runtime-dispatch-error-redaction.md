# ADR 0056: Redact standard runtime dispatch local errors

## Status

Accepted

## Context

The standard runtime proxy dispatch path performs local request capture, optional
request rewriting/redaction, Responses request preparation, and Anthropic request
translation before an upstream response may exist. Several local failure branches
returned raw error strings to clients. Those strings can include parser,
transport, local endpoint, or request-shape diagnostics that are useful to
operators but should not become the public API contract.

## Decision

Client-visible dispatch failures now use stable redacted messages for local
pre-upstream errors:

- `proxied request could not be captured`
- `proxied request could not be prepared`
- `Anthropic request could not be translated`
- `Anthropic request failed`

Detailed errors remain in runtime logs. Existing body-too-large and Presidio
fail-closed messages retain their prior stable messages. Upstream responses that
exist are not redefined by this change.

## Consequences

- Standard runtime proxy clients no longer receive raw local dispatch errors.
- Runtime logs continue to carry detailed diagnostics for operations.
- Regression coverage pins the stable messages and rejects common credential,
  endpoint, parser, and transport diagnostic substrings.
