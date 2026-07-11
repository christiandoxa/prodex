# ADR 0296: Kubernetes Postgres State Policy

## Status

Accepted.

## Context

The Kubernetes baseline runs multiple gateway replicas and provides
`PRODEX_GATEWAY_POSTGRES_URL`, but the mounted `policy.toml` did not select a
shared gateway state backend. Without an explicit `[gateway.state]` section,
gateway admin keys, usage counters, and billing ledger records would fall back
to per-pod file state under `PRODEX_HOME`, contradicting the multi-replica
accounting baseline.

## Decision

Configure the Kubernetes `prodex-gateway-policy` ConfigMap with:

```toml
[gateway.state]
backend = "postgres"
postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"
```

The deployment security guard now rejects missing shared state policy,
non-PostgreSQL gateway state backends, or PostgreSQL URL environment names other
than `PRODEX_GATEWAY_POSTGRES_URL`.

## Consequences

- Kubernetes gateway replicas share admin-managed keys, SCIM users, usage
  counters, and billing ledger records through PostgreSQL.
- File-backed state remains available for local compose and single-node
  deployments, but not for the production-shaped Kubernetes baseline.
- The existing external migration placeholder remains the schema-change path;
  request-serving pods still must not act as schema migrators.
