# ADR 0295: Kubernetes Gateway Policy ConfigMap

## Status

Accepted.

## Context

The Kubernetes baseline provides `PRODEX_GATEWAY_METRICS_TOKEN` for the
`ServiceMonitor`, but the gateway only accepts that token when it is registered
as a `[[gateway.admin_tokens]]` entry in `policy.toml`. Without a mounted policy
file, operators could deploy a ServiceMonitor with a valid bearer secret that
still cannot authenticate, or they could fall back to the gateway root token.

## Decision

Add a `prodex-gateway-policy` ConfigMap with `policy.toml` that binds
`PRODEX_GATEWAY_METRICS_TOKEN` as a `viewer` admin token named `prometheus`.

Mount that policy file read-only at `/var/lib/prodex/policy.toml` in the gateway
deployment. The placeholder control-plane deployment mounts the same baseline
policy so future adapters start from the same read-only admin-token contract.

The deployment security guard rejects missing policy ConfigMaps, metrics tokens
bound to `PRODEX_GATEWAY_TOKEN`, non-viewer metrics roles, and gateway
deployments that do not mount `policy.toml`.

## Consequences

- Metrics scrape auth works from the checked-in Kubernetes artifact without
  reusing the gateway root token.
- The production baseline documents and enforces the intended read-only
  Prometheus identity.
- Operators can extend the ConfigMap with tenant or key-prefix scope while
  preserving the same token environment variable.
