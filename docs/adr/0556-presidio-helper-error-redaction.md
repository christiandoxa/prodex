# ADR 0556: Redact Presidio helper diagnostics

## Status

Accepted

## Context

The reusable Presidio helper crate builds errors and health messages from
Presidio Analyzer/Anonymizer response bodies and HTTP client errors. Those
strings can include authorization headers, key-bearing URLs, deployment routes,
or request-derived diagnostic text before higher-level callers have a chance to
sanitize them.

## Decision

`prodex-presidio` now redacts secret-like text at the helper boundary for
non-success Analyzer/Anonymizer bodies and health probe messages. Endpoint
validation, status codes, and successful responses are unchanged.

## Consequences

- All callers receive safer Presidio diagnostics by default.
- Operator health/status signal is preserved while bearer tokens and key-bearing
  URL query values are removed.
- Regression coverage pins the shared helper redaction behavior.
