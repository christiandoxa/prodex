# ADR 0020: Add No-Store and No-Sniff to Local JSON Errors

## Status

Accepted

## Context

Prodex-generated local JSON errors can contain control-plane status, local admission decisions, or operator-actionable diagnostics. They are not upstream pass-through payloads. Enterprise deployments may place the gateway behind proxies or browsers, so local error responses should avoid cache persistence and MIME confusion by default.

## Decision

The shared Prodex local JSON error response builder now appends:

```text
Cache-Control: no-store
X-Content-Type-Options: nosniff
```

This applies only to locally generated JSON errors built through `build_runtime_proxy_json_error_response` / `build_runtime_proxy_json_error_parts`. Upstream HTTP status, body, and streaming payload pass-through behavior is unchanged.

## Consequences

Local control-plane and data-plane guardrail errors are safer under shared proxy/browser exposure. API bodies and status codes remain unchanged.
