# ADR 0054: Redact local gateway upstream transport failures

## Status

Accepted

## Context

The local gateway forwards authorized data-plane requests to an upstream provider.
When the upstream HTTP request fails before any upstream response exists, the
previous client response included the raw transport error. Raw transport errors
can expose loopback endpoints, proxy configuration, DNS details, or other local
infrastructure diagnostics that are not part of the gateway API contract.

## Decision

Local gateway upstream transport failures now return a stable client-visible
`502` response body:

```text
upstream request failed
```

The detailed transport error remains available in runtime logs for operators.
Successful upstream responses and upstream-provided error responses continue to
pass through unchanged.

## Consequences

- Gateway clients no longer receive local endpoint or transport diagnostics when
  no upstream response exists.
- The proxy still distinguishes this local pre-response failure from upstream
  errors, preserving upstream error compatibility where an upstream response was
  actually received.
- Regression coverage verifies the response does not leak the upstream loopback
  address, port, connection-refused text, or virtual-key token.
