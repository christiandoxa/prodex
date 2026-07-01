# ADR 0561: Redact Gemini quota error bodies

## Status

Accepted

## Context

Gemini quota probing forwards non-success Code Assist quota response bodies into
local errors. Validation-required responses intentionally surface user-action
URLs, but generic upstream error bodies can contain authorization text,
key-bearing URLs, or provider diagnostics.

## Decision

Non-validation Gemini quota error bodies pass through the shared secret-like
text redactor before being included in the local error. Validation-required
handling and quota success parsing are unchanged.

## Consequences

- User-facing validation guidance remains available.
- Bearer tokens and key-bearing URL query values are removed from generic Gemini
  quota errors.
- Regression coverage pins the non-validation error body redaction boundary.
