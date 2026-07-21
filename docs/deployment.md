# Prodex Gateway Deployment

This page describes the repository-provided container scaffold for running the
dedicated `prodex-gateway` data-plane service.

## Docker Compose

Copy the example environment file, create private secret files, then start the
gateway. The environment file contains only file paths; Compose projects the
secret contents into the container instead of exporting them as application or
provider environment variables.

```bash
cp deploy/gateway.env.example deploy/gateway.env
install -d -m 0700 deploy/secrets
# Populate the five files named in deploy/gateway.env from your secret manager.
sudo chown 10001:10001 deploy/secrets/*
chmod 0600 deploy/secrets/*
node scripts/ci/deployment-security-guard.mjs --secret-env deploy/gateway.env

docker compose --env-file deploy/gateway.env build prodex-gateway
docker compose --profile postgres --profile redis \
  --env-file deploy/gateway.env up -d postgres redis
docker compose --env-file deploy/gateway.env run --rm prodex-gateway \
  migrate --backend postgres \
  --url-ref PRODEX_GATEWAY_POSTGRES_URL \
  --secret-provider compose --secret-root /run/secrets \
  --tls-mode disable
docker compose --env-file deploy/gateway.env up -d prodex-gateway
```

The default compose command runs:

```bash
/usr/local/bin/prodex-gateway serve --listen 0.0.0.0:4000
```

`deploy/gateway.env` points at five ignored local files:

- `PRODEX_GATEWAY_TOKEN`
- `OPENAI_API_KEY`
- `PRODEX_GATEWAY_POSTGRES_URL`
- `PRODEX_GATEWAY_REDIS_URL`
- `PRODEX_POSTGRES_PASSWORD`

Write each value without a trailing newline and keep the source files private.
The tracked `deploy/compose-gateway-policy.toml` resolves the first four through
`SecretRef`s rooted at `/run/secrets`. The Postgres image reads its password
through `POSTGRES_PASSWORD_FILE`. The Postgres connection URL and password files
must contain matching credentials; URL-encode the password in the connection
URL. Run the one-shot migration command above before starting the gateway;
request-serving containers never perform schema migration.

Docker Compose uses bind mounts for `file:` secret sources and therefore
silently ignores service-level `uid`, `gid`, and `mode` overrides, as documented
in the [Docker Compose service secrets reference](https://docs.docker.com/reference/compose-file/services/#secrets).
The Linux baseline consequently requires the host files to be owned by the
container UID `10001` and to have mode `0600`; the deployment security guard's
`--secret-env` check verifies that metadata without reading secret contents.
The tracked policy disables PostgreSQL TLS only for the bundled local Compose
database. Use the Kubernetes baseline or an external TLS-enabled database for
production.

## Persistent Paths

The compose stack mounts two named volumes:

- `/var/lib/prodex`: `PRODEX_HOME`, including file-backed gateway state when selected. The tracked Compose policy is mounted read-only at `/var/lib/prodex/policy.toml`; the default Postgres-backed deployment keeps admin keys, usage counters, and billing ledger rows in the configured database.
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

The metrics endpoint also exports live, closed-label authentication,
authorization, tenant-isolation, policy-lifecycle, and runtime secret-provider
counters. With Prometheus Operator installed, apply
`deploy/observability/prodex-alerts.yaml`; import
`deploy/observability/prodex-dashboard.json` into Grafana for the matching
operations dashboard. The checked-in `ServiceMonitor` uses the dedicated viewer
token and does not expose raw tenant, principal, request, or secret identifiers
as metric labels.

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

SCIM user create, replace, and patch operations accept `group_ids` and
`department_id` directly or through
`urn:prodex:params:scim:schemas:gateway:2.0:User`. For data-plane policy,
Prodex attaches these organization attributes to a virtual key only when one
active SCIM row has the same exact `tenant_id` and `user_id`; absent,
cross-tenant, malformed, or ambiguous linkage yields no organization
attributes. File, SQLite, PostgreSQL, and Redis state persist these fields.

Prodex can also verify admin OIDC/JWT bearer tokens directly when `[gateway.sso]` sets an HTTPS `oidc_issuer` and `oidc_audience`; `oidc_jwks_url` can be configured explicitly as an HTTPS URL or discovered from the issuer's OpenID configuration. JWKS is same-origin by default. A cross-origin JWKS endpoint requires its exact HTTPS origin and port in `oidc_jwks_origin_allowlist`; redirects, private/loopback/link-local/metadata addresses, and discovery issuer changes fail closed. Role, tenant, and key-prefix claims can come from the JWT or from SCIM users when those claims are absent.

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
Production policy defaults PostgreSQL transport to certificate and hostname
verification.

The tracked Compose policy uses projected references for shared database-backed
admin keys, usage counters, billing ledger rows, and Redis coordination:

```toml
version = 1

[secrets]
projected_root = "/run/secrets"
projected_provider = "compose"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_POSTGRES_URL" }
postgres_tls_mode = "disable" # local Compose only
redis_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_REDIS_URL" }
```

Production deployments must use `postgres_tls_mode = "verify-full"` (the
production default). Set `postgres_tls_ca_path` to a PEM CA bundle when native
roots do not contain the database issuer. The same mode applies to pooled
accounting, blocking admin/usage/ledger access, and external migrations.

The Compose file includes Postgres under the `postgres` profile. Its password is
read from the projected file named by `PRODEX_POSTGRES_PASSWORD_FILE`:

```bash
docker compose --profile postgres --profile redis \
  --env-file deploy/gateway.env up -d postgres redis
```

After the database is healthy, run the one-shot migration and gateway startup
commands from the initial Compose procedure above.

Redis remains the rebuildable coordination backend beside durable PostgreSQL
state. The policy reads its URL through:

```toml
redis_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_REDIS_URL" }
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
secret is reserved for the dedicated control-plane workload.

The data-plane container starts explicitly with
`/usr/local/bin/prodex-gateway serve`; it does not route deployment startup
through the compatibility `prodex gateway` CLI entrypoint.

The migration Job runs the dedicated external `prodex-gateway migrate` command
with `--url-ref PRODEX_GATEWAY_POSTGRES_URL --secret-provider kubernetes
--secret-root /run/secrets/prodex --tls-mode verify-full`;
gateway request-serving pods do not run schema migration during startup. The
Job is labeled separately, uses the dedicated `prodex-gateway-migration`
ServiceAccount, mounts only `prodex-gateway-migration-secrets` as private projected files, and has
a dedicated egress-only NetworkPolicy that permits DNS
and PostgreSQL but not Redis, provider HTTPS, or ingress. The current migrator
records enterprise migration checksum history, serializes PostgreSQL migrators
with a bounded advisory lock, and records/ensures compatibility schema versions. Destructive
contract migrations still require release-specific expand/backfill/contract
evidence before they are added to the catalog.

The same manifest runs a single `prodex-control-plane` replica behind its own
`Service` and restricted `NetworkPolicy`. The dedicated binary explicitly
requests `service_mode = "control-plane"` and listens on `0.0.0.0:4100`; normal
gateway startup rejects that policy. Its ExternalSecret contains only the
projected admin token plus PostgreSQL and Redis connection references. The
workload mounts those values as read-only files rather than importing secrets
through `env` or `envFrom`, and it uses a 45-second termination grace period
with a 15-second pre-stop delay before the bounded backend drain.

The gateway ConfigMap sets `PRODEX_GATEWAY_REPLICA_COUNT=3` and
`PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS=true`, and the gateway
`ExternalSecret` includes both PostgreSQL and Redis URLs. This declares the
production accounting-concurrency topology. Startup accepts it only when the
declared replica count is at least two, PostgreSQL is the durable
ledger/counter authority, and Redis is the rebuildable rate-limit, cache, and
coordination backend. Invalid multi-replica claims fail before the listener is
bound.

Multi-replica deployments should use the PostgreSQL configuration-publication
transport after migration schema 14 is installed. Give every live consumer a
unique stable replica ID, normally from the Kubernetes pod name through
`PRODEX_CONFIG_PUBLICATION_REPLICA_ID`, then start it with
`--config-publication-postgres`. The operational flow is:

```bash
prodex-control-plane publish-config-publication --event <path> --postgres
prodex-gateway serve --config-publication-postgres
prodex-control-plane compact-config-publication --postgres --retain 10
```

The transport is a durable notification channel, not a policy-content or
secret channel. Roll out the validated candidate `policy.toml` to every runtime
root before publishing its revision notification. A consumer acknowledges a
record only after it builds and atomically activates the replacement
application; failed activation remains eligible for retry. Compaction removes
only records acknowledged by every registered replica and can retain the newest
acknowledged records for inspection. PostgreSQL consumer batches are bounded
and an advisory lock prevents two processes from claiming one replica ID.
Compaction expires replicas that have not renewed their database-clock lease for
five minutes and always preserves at least the newest fully acknowledged event,
even when `--retain 0` is requested, so a returning replica can trigger a reload
of the current local policy.

Single-node or shared-storage deployments may instead mount one durable
filesystem path visible to the control-plane publisher and every gateway
replica:

```bash
prodex-control-plane publish-config-publication --event <path> --transport <shared-path>
prodex-gateway consume-config-publication --transport <shared-path> --replica <gateway-id> --root <prodex-home>
prodex-control-plane compact-config-publication --transport <shared-path> --retain 10
```

Run compaction periodically so transport records do not grow without bound.
Separate node-local or cluster-local roots are not equivalent to one replicated
filesystem transport; use one durable shared path or PostgreSQL.

The namespace default-deny policy has no allow rules. The gateway and
control-plane NetworkPolicies intentionally avoid unbounded
in-cluster ingress and egress rules. They accept application ingress only from
namespaces labeled `prodex.dev/network-tier=ingress`, gateway metrics scraping
only from namespaces labeled `prodex.dev/network-tier=monitoring`, allow DNS to
`kube-dns`, and allow PostgreSQL (`5432`) and Redis (`6379`) only in namespaces
labeled `prodex.dev/network-tier=data-store`. Gateway pods send provider HTTPS
traffic only through namespaces labeled `prodex.dev/network-tier=ai-egress`;
the control plane has no provider egress. Run an approved egress proxy or
gateway in that namespace and enforce its destination allowlist there. Label
your ingress controller, monitoring, data-store, and AI-egress namespaces, or
replace the selectors with your private endpoint policy before applying the
manifest.

The `ServiceMonitor` authenticates with `PRODEX_GATEWAY_METRICS_TOKEN` from
`prodex-gateway-secrets`, not the gateway root token. The Secret is projected
read-only at `/run/secrets/prodex` with mode `0440`; it is not imported through
`envFrom` and the mount does not use `subPath`, so Kubernetes atomic projection
updates remain visible. The manifest also mounts `prodex-gateway-policy` at
`/var/lib/prodex/policy.toml`:

```toml
version = 1

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
postgres_tls_mode = "verify-full"
redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_REDIS_URL" }

[[gateway.admin_tokens]]
name = "prometheus"
token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_METRICS_TOKEN" }
role = "viewer"
```

The dedicated control-plane policy is intentionally smaller:

```toml
version = 1
service_mode = "control-plane"

[secrets]
production = true
projected_root = "/run/secrets/prodex"
projected_provider = "kubernetes"

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }
postgres_tls_mode = "verify-full"
redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_REDIS_URL" }

[[gateway.admin_tokens]]
name = "operations"
token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
role = "admin"
```

It deliberately has no provider key, gateway root token, virtual keys, SSO,
outbound observability, routing, request constraints, or guardrail capability.

The PostgreSQL state backend keeps admin keys, SCIM users, usage counters, and
billing ledger rows shared across Kubernetes gateway replicas. Add `tenant_id`
or `allowed_key_prefixes` when Prometheus should scrape only a tenant- or
prefix-scoped view of gateway metrics.

## Backup and Restore

Use `docs/backup-restore.md` as the backend-specific runbook for file, SQLite,
PostgreSQL, Redis, audit-log, and runtime-log backup and restore drills. Run a
restore drill before promoting a new state backend, changing migration behavior,
or scaling a gateway deployment across replicas.
