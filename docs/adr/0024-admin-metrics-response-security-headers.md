# ADR 0024: Add No-Store and No-Sniff to Gateway Admin Metrics

## Status

Accepted

## Context

The gateway Prometheus metrics endpoint is a Prodex-owned admin/control-plane response. Metrics are already redacted to avoid raw tenant, user, and key labels, but the text response may still contain operational state and should follow the same browser/proxy hardening policy as other admin responses.

## Decision

The `/v1/prodex/gateway/metrics` response now includes:

```text
Cache-Control: no-store
X-Content-Type-Options: nosniff
```

The response body, status code, content type, and Prometheus format are unchanged.

## Consequences

Browsers and compliant intermediaries are instructed not to cache or sniff admin metrics responses. This is scoped to the Prodex admin metrics endpoint and does not affect upstream data-plane pass-through behavior.
