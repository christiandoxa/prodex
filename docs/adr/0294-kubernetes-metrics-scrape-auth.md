# ADR 0294: Kubernetes Metrics Scrape Auth

## Status

Accepted.

## Context

Gateway Prometheus metrics are exposed through the admin API and filtered by the
same admin authorization model as key, usage, ledger, and OpenAPI reads. The
Kubernetes baseline already defines a `ServiceMonitor` and a monitoring
NetworkPolicy path, but an unauthenticated scrape would fail and using the
gateway root token would give Prometheus mutation-capable admin privileges.

## Decision

Add a dedicated `PRODEX_GATEWAY_METRICS_TOKEN` secret reference and configure
the `ServiceMonitor` to use it as a bearer token.

Operators must bind that token in `policy.toml` as a `viewer`
`[[gateway.admin_tokens]]` entry, optionally with tenant or key-prefix scope.
The deployment security guard rejects unauthenticated ServiceMonitors and
ServiceMonitors that use `PRODEX_GATEWAY_TOKEN`.

## Consequences

- Metrics scraping can authenticate without reusing the gateway/root token.
- Prometheus gets read-only metrics visibility instead of gateway admin
  mutation rights.
- Tenant or prefix scoping can limit scraped series for regulated deployments.
