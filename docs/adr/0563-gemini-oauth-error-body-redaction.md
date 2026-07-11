# ADR 0563: Redact Gemini OAuth error bodies

## Status

Accepted

## Context

Gemini OAuth token exchange, refresh, and Google user-info helpers include
non-success response bodies in local errors. Those upstream bodies can include
authorization text, key-bearing URLs, or provider diagnostics.

## Decision

Gemini OAuth response bodies pass through the shared secret-like text redactor
before being included in local errors. Successful token parsing and user-info
parsing are unchanged.

## Consequences

- OAuth failures still include status and provider context.
- Bearer tokens and key-bearing URL query values are removed from Gemini OAuth
  error messages.
- Regression coverage pins the OAuth error body redaction helper.
