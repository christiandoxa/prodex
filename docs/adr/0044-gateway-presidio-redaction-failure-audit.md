# ADR 0044: Gateway Presidio redaction failures are audited without leaking payloads

## Status

Accepted

## Context

When runtime Presidio redaction is enabled, the local rewrite gateway redacts captured HTTP requests before admin routing, local guardrails, virtual-key admission, webhook checks, and upstream provider calls. A redaction failure is security-sensitive because the gateway must fail closed instead of forwarding unredacted data.

The failure path previously logged the full error chain and returned the raw error text to the client. Presidio endpoint URLs can contain environment-specific routing details, and error chains may include endpoints. Request bodies may contain sensitive payloads that should not appear in audit or runtime logs.

## Decision

Presidio redaction failures in the local rewrite gateway now fail closed with a stable generic `502` response body:

```text
gateway PII redaction failed
```

The gateway also appends a metadata-only audit event:

- `component`: `gateway_data_plane`
- `action`: `presidio_redaction_failed`
- `outcome`: `failure`
- `details.reason`: `presidio_redaction_failed`
- `details.path`: request path
- `state_backend`: active gateway state backend label

Runtime diagnostics retain the event name, request id, transport, stable reason, and path, but omit the raw error chain.

## Consequences

Redaction failures remain visible for operations and audit while avoiding leakage of request payloads, gateway tokens, Presidio endpoint secrets, or internal error chains. Compatibility impact is limited to replacing an internal error string response with a stable machine-readable operational message for a local gateway failure.

## Validation

A regression test configures Presidio endpoints with a secret-bearing unavailable URL, sends a sensitive request, and verifies that:

- the gateway returns the generic `502` message;
- the request is not forwarded upstream;
- an audit event with `presidio_redaction_failed` metadata is written;
- audit and runtime logs omit the gateway token, sensitive input, endpoint secret, and raw endpoint.
