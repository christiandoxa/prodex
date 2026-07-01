# ADR 0555: Redact dashboard JSON errors

## Status

Accepted

## Context

The local dashboard API returns JSON error bodies for profile mutation,
dashboard request parsing, state loading, and quota/status operations. Those
errors previously serialized `err.to_string()` directly. Error text can include
request values, local paths, backend URLs, or secret-like authorization material.

## Decision

Dashboard JSON error bodies now pass through the shared secret-like text
redactor at the single `respond_error` boundary. Status codes and successful
dashboard responses are unchanged.

## Consequences

- Dashboard clients still receive the same error shape.
- Bearer tokens and key-bearing URL query values are removed from dashboard
  error responses.
- Regression coverage pins the redaction helper used by all dashboard error
  responses.
