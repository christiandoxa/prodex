# Prodex Gateway Deployment

This page describes the repository-provided container scaffold for running `prodex gateway` as a standalone OpenAI-compatible service.

## Docker Compose

Copy the example environment file, fill in real secrets, then start the gateway.
The compose scaffold intentionally does not provide fallback production
credentials; `PRODEX_GATEWAY_TOKEN` must be set explicitly.

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

Container health checks use the public `/readyz` probe instead of an admin
endpoint so readiness can be evaluated without placing bearer tokens in process
arguments.

Recent billing ledger records are available at `/v1/prodex/gateway/ledger`, aggregated billing totals are available at `/v1/prodex/gateway/ledger/summary`, billing CSV exports are available at `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, Prometheus-compatible virtual-key usage metrics are available to the same admin token at `/v1/prodex/gateway/metrics`, SCIM-compatible SSO user provisioning is available at `/v1/prodex/gateway/scim/v2/Users`, and the built-in single-node gateway admin dashboard is available at `/v1/prodex/gateway/admin`.

## Operational Limits

The gateway supports file-backed state by default, SQLite-backed state after explicit external migration, and Postgres-backed shared gateway state:

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
require_tenant = true
default_role = "viewer"
```

The proxy must strip these headers from client input, inject them after authentication, and send `x-prodex-sso-token` with the shared token value. Prodex only trusts SSO headers when that proxy token matches. Admins can also provision SCIM users through `/v1/prodex/gateway/scim/v2/Users`; when role, tenant, or key-prefix SSO headers are absent, an active SCIM user matching `userName` can supply those values, and an inactive match is rejected.

Prodex can also verify admin OIDC/JWT bearer tokens directly when `[gateway.sso]` sets an HTTPS `oidc_issuer` and `oidc_audience`; `oidc_jwks_url` can be configured explicitly as an HTTPS URL or discovered from the issuer's OpenID configuration. Role, tenant, and key-prefix claims can come from the JWT or from SCIM users when those claims are absent.

File and SQLite state are single-node deployment models. File locks make local JSON updates merge-safe; SQLite uses transactional updates for admin keys and usage counters. Do not scale multiple containers against the same `PRODEX_HOME` across hosts or network filesystems unless you have verified locking behavior and are prepared for single-writer operation.

Keep `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS=false` for local compose or
single-node compatibility. Set it to `true` only for a topology that uses shared
PostgreSQL as the durable source of truth, Redis as the rebuildable coordination
backend, and at least two gateway replicas. In that mode
`PRODEX_GATEWAY_REPLICA_COUNT` must match the intended gateway replica count so
the application runtime can select the multi-replica accounting concurrency
spec before adapters start. The legacy `prodex gateway` launch path now enforces
that prelaunch gate and rejects invalid multi-replica claims before the proxy
binds a port. A valid PostgreSQL+Redis declaration with at least two replicas is
accepted: PostgreSQL atomically enforces cumulative request, token, and cost
budgets, including grouped keys, while Redis enforces distributed RPM/TPM.
PostgreSQL TLS remains a separate production requirement.

Use `gateway.state.backend = "postgres"` with `postgres_url_env` for shared database-backed admin keys, usage counters, and billing ledger rows:

```toml
[gateway.state]
backend = "postgres"
postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"
```

The Compose file includes an optional Postgres service under the `postgres` profile:

```bash
openssl rand -base64 32 # use this output as PRODEX_POSTGRES_PASSWORD
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

## Kubernetes Baseline

`deploy/kubernetes/prodex-gateway.yaml` provides a production-shaped manifest
baseline with:

- `Deployment`, `ServiceAccount`, `Service`, `ConfigMap`, `ExternalSecret`,
  `PodDisruptionBudget`, `HorizontalPodAutoscaler`, `NetworkPolicy`,
  `ServiceMonitor`, and migration `Job` objects.
- non-root pod execution, read-only root filesystems, dropped Linux
  capabilities, `RuntimeDefault` seccomp, no privilege escalation, resource
  requests/limits, and `/livez`, `/readyz`, and `/startupz` probes.
- gateway topology spread constraints across zones and nodes so multi-replica
  deployments are not silently concentrated onto one failure domain.
- a gateway termination grace period plus `preStop` delay so Kubernetes rolling
  updates and evictions give readiness removal and connection draining time to
  settle before SIGTERM.
- explicit gateway, migration, and control-plane service accounts with
  automounted service account tokens disabled, so workloads do not inherit the
  default namespace identity or share operational RBAC accidentally.
- Pod Security Admission labels on the `prodex` namespace with
  `enforce`, `audit`, and `warn` set to `restricted`, so future pods must keep
  the same restricted runtime posture as the checked-in workloads.
- a namespace-wide `prodex-default-deny` NetworkPolicy, so new pods in the
  `prodex` namespace start with no ingress or egress until a workload-specific
  allow policy is added.
- immutable image digest references that operators must replace with the
  published 64-character `sha256` image digest for the release they deploy;
  malformed and all-zero digest placeholders are rejected by the deployment
  security guard.

The gateway workload mounts `prodex-gateway-secrets`, which contains the
gateway bearer token, provider API key references, PostgreSQL, and Redis
connection references. It must not mount `prodex-control-plane-secrets`; that
secret is reserved for the placeholder control-plane workload.

The migration Job runs the dedicated external `prodex-gateway migrate` command;
gateway request-serving pods do not run schema migration during startup. The
Job is labeled separately, uses the dedicated `prodex-gateway-migration`
ServiceAccount, reads only `prodex-gateway-migration-secrets` containing the
PostgreSQL URL, and has a dedicated egress-only NetworkPolicy that permits DNS
and PostgreSQL but not Redis, provider HTTPS, or ingress. The current migrator
ensures the compatibility schema; migration status/history, advisory locking,
and the complete versioned expand/contract workflow remain required before the
database migration contract is considered complete.

The same manifest also includes a `prodex-control-plane` `Deployment`, `Service`,
and `NetworkPolicy` as an explicit enterprise control-plane surface. It is scaled
to `replicas: 0` and annotated as a placeholder until the control-plane async
adapter is wired to `prodex-application` and `prodex-control-plane`. Operators
must not scale it above zero until the `prodex-control-plane serve` command is
implemented and validated. The placeholder mounts only
`prodex-control-plane-secrets`, which contains PostgreSQL and Redis connection
references, so provider API keys and data-plane gateway tokens remain scoped to
the gateway workload.

The gateway ConfigMap sets `PRODEX_GATEWAY_REPLICA_COUNT=3` and
`PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS=true`, and the gateway
`ExternalSecret` includes both PostgreSQL and Redis URLs. This declares the
intended production accounting-concurrency topology, but the current legacy
gateway launch path still rejects startup in that mode until durable
reservation admission is wired:
PostgreSQL remains the durable ledger/counter source of truth while Redis is
only used for rate limiting, cache, and coordination primitives.

If the deployment also uses the current replicated file transport for
configuration publication, mount one shared durable filesystem path that is
visible to the control-plane publisher and every gateway replica. The current
one-shot operational flow is:

```bash
prodex-control-plane publish-config-publication --event <path> --transport <shared-path>
prodex-gateway consume-config-publication --transport <shared-path> --replica <gateway-id> --root <prodex-home>
prodex-control-plane compact-config-publication --transport <shared-path> --retain 10
```

`compact-config-publication` only removes records that every known replica has
already acknowledged and can retain the newest acknowledged records for
inspection. Shared-storage deployments should run that command periodically so
transport outbox/ack files do not grow without bound. Non-shared-storage
topologies still need the broker-backed publication transport staged in
`docs/adr/0984-config-publication-broker-transport-staging.md`; separate
node-local or cluster-local transport roots are not equivalent.

The namespace default-deny policy has no allow rules. The gateway and
placeholder control-plane NetworkPolicies intentionally avoid unbounded
in-cluster ingress and egress rules. They accept application ingress only from
namespaces labeled `prodex.dev/network-tier=ingress`, gateway metrics scraping
only from namespaces labeled `prodex.dev/network-tier=monitoring`, allow DNS to
`kube-dns`, and allow PostgreSQL (`5432`) and Redis (`6379`) only in namespaces
labeled `prodex.dev/network-tier=data-store`. Gateway pods additionally allow
outbound HTTPS (`443`) to public provider endpoints while excluding RFC1918
private ranges from that broad HTTPS egress; the control-plane placeholder does
not allow public provider egress. Label your ingress controller, monitoring,
and database/Redis namespaces, or replace the selectors with your private
endpoint policy before applying the manifest.

The `ServiceMonitor` authenticates with `PRODEX_GATEWAY_METRICS_TOKEN` from
`prodex-gateway-secrets`, not the gateway root token. The Secret is projected
read-only at `/run/secrets/prodex` with mode `0440`; it is not imported through
`envFrom` and the mount does not use `subPath`, so Kubernetes atomic projection
updates remain visible. The manifest also mounts `prodex-gateway-policy` at
`/var/lib/prodex/policy.toml`:

```toml
[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway]
require_auth = true
auth_token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_TOKEN" }
provider_api_key_ref = { provider = "kubernetes", name = "OPENAI_API_KEY" }

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }

[[gateway.admin_tokens]]
name = "prometheus"
token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_METRICS_TOKEN" }
role = "viewer"
```

The PostgreSQL state backend keeps admin keys, SCIM users, usage counters, and
billing ledger rows shared across Kubernetes gateway replicas. Add `tenant_id`
or `allowed_key_prefixes` when Prometheus should scrape only a tenant- or
prefix-scoped view of gateway metrics.

## Backup and Restore

Use `docs/backup-restore.md` as the backend-specific runbook for file, SQLite,
PostgreSQL, Redis, audit-log, and runtime-log backup and restore drills. Run a
restore drill before promoting a new state backend, changing migration behavior,
or scaling a gateway deployment across replicas.
