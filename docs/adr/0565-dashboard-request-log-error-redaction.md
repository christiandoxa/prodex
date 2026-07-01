# ADR 0565: Redact dashboard request-loop errors

## Status

Accepted

## Context

The local dashboard API already redacts JSON error responses, but the dashboard
server request loop still printed detailed `{err:#}` chains to stderr when
request handling failed. Those chains can include authorization headers,
key-bearing URLs, request values, or local backend diagnostics.

## Decision

Keep the existing stderr notice, but pass the detailed dashboard request error
chain through the shared secret-like text redactor before printing it.

## Consequences

- Operators still see that a dashboard request failed and retain non-sensitive
  context from the error chain.
- Secret-like bearer tokens and key-bearing URLs are removed from the local
  dashboard request-loop diagnostic.
- Dashboard response behavior is unchanged.
