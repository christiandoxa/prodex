# ADR 0562: Redact Gemini Code Assist error bodies

## Status

Accepted

## Context

Gemini Code Assist setup and onboarding helpers include non-success response
bodies in local errors. Validation-required responses intentionally expose
account-verification guidance, but generic error bodies can include
authorization text, key-bearing URLs, or provider diagnostics.

## Decision

Non-validation Gemini Code Assist error bodies pass through the shared
secret-like text redactor before being included in local errors. Validation
handling and successful setup behavior are unchanged.

## Consequences

- Account validation guidance remains visible.
- Bearer tokens and key-bearing URL query values are removed from generic Code
  Assist setup errors.
- Regression coverage pins the non-validation error body boundary.
