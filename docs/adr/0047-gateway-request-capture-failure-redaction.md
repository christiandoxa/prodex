# ADR 0047: Gateway request capture failures use generic responses and redacted diagnostics

## Status

Accepted

## Context

The local rewrite gateway captures inbound HTTP request metadata and body before authorization, guardrails, virtual-key admission, webhook calls, and provider routing. Capture can fail before a request body is safely available, for example due to transport/body read errors.

The generic capture failure path previously logged the raw error chain and returned the raw error string to the client. The body-limit branch already had metadata-only audit and logs, but still returned the detailed byte-count error string.

Enterprise and regulated deployments require proxy-local failures to avoid exposing request payload fragments, credentials, byte-count internals, or transport implementation details.

## Decision

Local rewrite gateway request capture failures now:

- append a metadata-only `gateway_data_plane` audit event with `action=request_capture_failed`;
- log stable metadata `reason=request_capture_failed` and request path;
- return generic `502` body `gateway request capture failed`.

Local rewrite gateway request body limit failures now return the stable `413` body:

```text
proxied request body is too large
```

The existing body-limit audit/log metadata remains unchanged.

## Consequences

Gateway-local request capture failures remain auditable and diagnosable without leaking raw error chains. Body-limit behavior remains a pre-upstream rejection, but the response no longer exposes exact byte-count internals.

## Validation

The body-limit regression continues to verify pre-upstream rejection, audit metadata, and absence of token/body leakage. The capture-error branch is hardened in code with the same metadata-only pattern; end-to-end truncated-body attempts can be rejected by the HTTP parser before reaching the handler, so branch coverage is best added when a lower-level request harness is available.
