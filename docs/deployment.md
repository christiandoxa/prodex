# Prodex Gateway Deployment

This page describes the repository-provided container scaffold for running `prodex gateway` as a standalone OpenAI-compatible service.

## Docker Compose

Copy the example environment file, fill in real secrets, then start the gateway:

```bash
cp deploy/gateway.env.example deploy/gateway.env
$EDITOR deploy/gateway.env
docker compose --env-file deploy/gateway.env up -d --build prodex-gateway
```

The default compose command runs:

```bash
prodex gateway --listen 0.0.0.0:4000
```

It expects `PRODEX_GATEWAY_TOKEN` and `OPENAI_API_KEY` in `deploy/gateway.env`. To run a provider bridge, edit the service command, for example:

```yaml
command: ["gateway", "--listen", "0.0.0.0:4000", "--provider", "gemini"]
```

Then provide the matching provider credential such as `GEMINI_API_KEY`.

## Persistent Paths

The compose stack mounts two named volumes:

- `/var/lib/prodex`: `PRODEX_HOME`, including `policy.toml`, file-backed `gateway-virtual-keys.json`, `gateway-virtual-key-usage.json`, and `gateway-billing-ledger.jsonl`, or SQLite-backed `gateway-state.sqlite`. Postgres-backed deployments keep admin keys, usage counters, and billing ledger rows in the configured database instead.
- `/var/log/prodex`: runtime logs, including the latest-runtime pointer and per-run proxy logs.

The gateway exposes the admin OpenAPI document at:

```bash
curl http://127.0.0.1:4000/v1/prodex/gateway/openapi.json \
  -H "Authorization: Bearer $PRODEX_GATEWAY_TOKEN"
```

Recent billing ledger records are available at `/v1/prodex/gateway/ledger`, aggregated billing totals are available at `/v1/prodex/gateway/ledger/summary`, billing CSV exports are available at `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, Prometheus-compatible virtual-key usage metrics are available to the same admin token at `/v1/prodex/gateway/metrics`, SCIM-compatible SSO user provisioning is available at `/v1/prodex/gateway/scim/v2/Users`, and the built-in single-node gateway admin dashboard is available at `/v1/prodex/gateway/admin`.

## Operational Limits

The gateway supports file-backed state by default, SQLite-backed state with automatic schema bootstrap, and Postgres-backed shared gateway state:

```toml
[gateway.state]
backend = "sqlite"
sqlite_path = "gateway-state.sqlite"

[[gateway.admin_tokens]]
name = "auditor"
token_env = "PRODEX_GATEWAY_AUDITOR_TOKEN"
role = "viewer"
allowed_key_prefixes = ["team-a-"]
tenant_id = "tenant-a"
```

`role = "viewer"` can read admin keys, usage, metrics, and OpenAPI only. `role = "admin"` can also create, rotate, disable, and delete admin-managed virtual keys. Set `allowed_key_prefixes` to restrict a token's key, usage, ledger, summary, CSV, and metrics visibility to matching virtual-key names. Set `tenant_id` to isolate the admin token to one tenant's keys, SCIM users, usage, ledger rows, summaries, CSV exports, and metrics.

If an upstream proxy already terminates OIDC/SAML, configure trusted-proxy SSO for admin endpoints:

```toml
[gateway.sso]
proxy_token_env = "PRODEX_GATEWAY_SSO_PROXY_TOKEN"
user_header = "x-auth-request-email"
role_header = "x-prodex-role"
key_prefixes_header = "x-prodex-key-prefixes"
tenant_header = "x-prodex-tenant"
default_role = "viewer"
```

The proxy must strip these headers from client input, inject them after authentication, and send `x-prodex-sso-token` with the shared token value. Prodex only trusts SSO headers when that proxy token matches. Admins can also provision SCIM users through `/v1/prodex/gateway/scim/v2/Users`; when role, tenant, or key-prefix SSO headers are absent, an active SCIM user matching `userName` can supply those values, and an inactive match is rejected.

Prodex can also verify admin OIDC/JWT bearer tokens directly when `[gateway.sso]` sets `oidc_issuer` and `oidc_audience`; `oidc_jwks_url` can be configured explicitly or discovered from the issuer's OpenID configuration. Role, tenant, and key-prefix claims can come from the JWT or from SCIM users when those claims are absent.

File and SQLite state are single-node deployment models. File locks make local JSON updates merge-safe; SQLite uses transactional updates for admin keys and usage counters. Do not scale multiple containers against the same `PRODEX_HOME` across hosts or network filesystems unless you have verified locking behavior and are prepared for single-writer operation.

Use `gateway.state.backend = "postgres"` with `postgres_url_env` for shared database-backed admin keys, usage counters, and billing ledger rows:

```toml
[gateway.state]
backend = "postgres"
postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"
```

The Compose file includes an optional Postgres service under the `postgres` profile:

```bash
docker compose --profile postgres --env-file deploy/gateway.env up -d --build
```

Use `gateway.state.backend = "redis"` with `redis_url_env` for Redis-backed gateway state:

```toml
[gateway.state]
backend = "redis"
redis_url_env = "PRODEX_GATEWAY_REDIS_URL"
```

The Compose file includes an optional Redis service under the `redis` profile:

```bash
docker compose --profile redis --env-file deploy/gateway.env up -d --build
```

Tenant-scoped admin tokens, trusted-proxy SSO, OIDC/JWT claims, SCIM users, and shared Postgres or Redis state provide a single-node or self-hosted gateway admin boundary. Treat this compose stack as a production-shaped gateway baseline, not a hosted central SaaS control plane.
