# ADR 0045: Runtime proxy Presidio failures use generic responses and redacted diagnostics

## Status

Accepted

## Context

The standard runtime rotation proxy can apply Presidio redaction before forwarding HTTP requests or WebSocket messages upstream. When Presidio failed in fail-closed mode, the helper logged raw error details and the HTTP dispatch path returned the raw error string to the client.

Presidio errors may include endpoint URLs, environment-specific details, or other internals. Request payloads can contain PII. In regulated deployments, fail-closed redaction failures must remain observable without exposing endpoint secrets, credentials, or payload content.

## Decision

The shared Presidio redaction helper now logs stable metadata only:

- request id;
- transport;
- fail mode;
- `reason=presidio_redaction_failed`.

It returns a stable internal error marker in fail-closed mode. The standard HTTP dispatch path now returns the generic `502` response body:

```text
gateway PII redaction failed
```

The local rewrite gateway already audits this condition separately. This ADR covers the standard runtime proxy path, where the immediate hardening target is non-leaking runtime diagnostics and client response compatibility for a proxy-local failure.

## Consequences

Operators still see that Presidio fail-closed enforcement blocked a request, while aggregated runtime logs and client responses no longer expose raw endpoint URLs or error chains. WebSocket Presidio failures use the same stable error marker before propagating the transport failure.

## Validation

A regression test starts the standard runtime proxy, registers a fail-closed Presidio config with a secret-bearing unavailable endpoint, and sends a sensitive request. It verifies that:

- the client receives the generic `502` response;
- runtime logs include the stable Presidio failure events and reason;
- runtime logs omit the gateway token, sensitive input, endpoint secret, and raw endpoint.
