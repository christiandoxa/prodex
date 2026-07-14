# Runtime Policy Reference

Prodex reads `policy.toml` from the Prodex root, usually `~/.prodex/policy.toml` unless `PRODEX_HOME` is set.
Environment variables override policy values, and unset values fall back to built-in defaults.

Core runtime-proxy, broker, WebSocket, smart-context, request-body, and OIDC
timing settings are captured in one typed snapshot at startup. The environment
is read once per key and policy is loaded once for that snapshot; active
requests and broker polling loops do not re-read those inputs. Configuration
errors are aggregated, report key names without values, and stop proxy startup
before the listener is bound. Changing these inputs therefore requires a
restart. Legacy lenient inputs keep their documented/default behavior and emit
`runtime_config_compatibility_default` with the key name only.

Policy-file validation also evaluates independent runtime, secret, proxy, and
gateway sections before returning. A single invalid setting keeps its existing
error text; multiple invalid sections are reported in deterministic section
order without including configured values. Rust callers that need structured
diagnostics can downcast the error to `RuntimePolicyValidationErrors`.

Relative `runtime.log_dir` values are resolved under the Prodex root. `PRODEX_RUNTIME_LOG_DIR` is used as provided.
Use `prodex info` for effective tuning values and `prodex doctor --runtime --json` for the resolved runtime log directory, format, and current `log_path`.

```bash
prodex doctor --runtime --json
prodex doctor --runtime --json | jq -r '.log_path'
prodex doctor --runtime --json | jq -r '.runtime_logs.directory'
```

Defaults below are production defaults. Test builds use smaller timeouts and limits in several places.

## Service Mode

| Policy key | Default | Meaning |
| --- | --- | --- |
| `service_mode` | `gateway` | Typed process role. `gateway` preserves the data-plane policy contract. `control-plane` is accepted only by the dedicated control-plane entrypoint and requires production projected secrets, a projected `role = "admin"` token, and shared PostgreSQL or Redis state. Control-plane policies reject provider, data-plane auth, virtual-key, routing, SSO, outbound observability, request-constraint, and guardrail capabilities. |

`prodex-gateway` and the compatibility gateway command reject
`service_mode = "control-plane"`. Conversely, `prodex-control-plane serve`
requires that explicit mode; an absent policy therefore cannot silently start a
control-plane listener.

## Runtime Keys

| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `runtime.log_dir` | `PRODEX_RUNTIME_LOG_DIR` | OS temp directory, usually `/tmp` on Linux | Directory for `prodex-runtime-latest.path` and per-run `prodex-runtime-*.log` files. |
| `runtime.log_format` | `PRODEX_RUNTIME_LOG_FORMAT` | `text` | Runtime proxy log format. Valid values: `text`, `json`. |

## Gateway Keys

`prodex gateway` runs a standalone OpenAI-compatible HTTP gateway.
Native OpenAI-compatible upstreams are passed through for `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models`.
Provider bridges translate `/v1/responses` where supported and pass native-compatible side endpoints through to the selected upstream.

Production secret resolution is explicit:

| Policy key | Default | Meaning |
| --- | --- | --- |
| `secrets.production` | `false` | When `true`, gateway CLI/environment credential inputs and all gateway `*_env` secret sources are rejected. `gateway.require_auth=true`, `gateway.auth_token_ref` or a referenced virtual key, and `gateway.provider_api_key_ref` are required. |
| `secrets.projected_root` | empty | Read-only directory projected by External Secrets, Secrets Store CSI, or an equivalent workload-identity adapter. Relative paths resolve under the Prodex root. Required with `projected_provider`. |
| `secrets.projected_provider` | empty | Exact provider name that every production `SecretRef.provider` must match. Required with `projected_root`. |

Each `*_ref` is a TOML object with exact `provider`, `name`, and optional
`version` fields. Resolution happens during gateway startup, before request
serving; secret files are not read on the request hot path.

| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `gateway.listen_addr` | none | `127.0.0.1:4000` | Gateway bind address. Non-loopback binds require `--auth-token`, `PRODEX_GATEWAY_TOKEN`, or `gateway.virtual_keys`. |
| `gateway.provider` | CLI `--provider` | OpenAI-compatible upstream | Exact provider preset: `anthropic`, `copilot`, `deepseek`, `gemini`, or `kiro`. Unsupported values fail closed instead of falling back to the OpenAI-compatible default. |
| `gateway.harness` | CLI `--harness` | `auto`, resolving to `native` in v1 | Model-facing request policy: `auto`, `native`, or `minimal`. CLI selection wins; the resolved mode is fixed for the gateway lifetime. |
| `gateway.base_url` | CLI `--base-url`; `OPENAI_BASE_URL` for OpenAI-compatible mode | Provider default, or `https://api.openai.com/v1` | Upstream base URL. Explicit values must be non-empty and whitespace-free. OpenAI-compatible mode appends `/v1` when the URL has no path. |
| OpenAI-compatible upstream API keys | CLI `--api-key`; `OPENAI_API_KEYS`; `OPENAI_API_KEY` | empty unless gateway auth is enabled | Optional upstream provider credentials. Explicit values must resolve to at least one non-empty key; singular `--api-key` and `OPENAI_API_KEY` values must be whitespace-free, while `OPENAI_API_KEYS` may contain comma-, semicolon-, or newline-separated keys. The Mem0 memory gateway applies the same env boundary and treats selected-profile `auth.json` `OPENAI_API_KEY` as an exact singular key. |
| `gateway.require_auth` | none | `false` | Require gateway bearer auth even on loopback. Token value comes from non-empty, whitespace-free `--auth-token`, non-empty, whitespace-free `PRODEX_GATEWAY_TOKEN`, or configured virtual key env vars. |
| `gateway.auth_token_ref` | none | empty | Projected data-plane bearer-token reference. Required in production unless at least one `gateway.virtual_keys[].token_ref` exists. |
| `gateway.provider_api_key_ref` | none | empty | Projected upstream provider API-key reference. Required in production. |
| `gateway.adaptive_routing.enabled` | none | `false` | Enable adaptive routing telemetry and shadow recommendations. Live route selection remains deterministic until an explicit non-shadow policy is implemented. |
| `gateway.adaptive_routing.shadow_mode` | none | `true` | Keep adaptive routing recommendations observational only. Continuation affinity and quota/safety constraints still win. |
| `gateway.adaptive_routing.window_size` | none | `128` | Bounded owner-attributed feedback window size used by adaptive quality scoring. |
| `gateway.adaptive_routing.min_samples` | none | `8` | Minimum samples before a model can be recommended by the adaptive shadow scorer. |
| `gateway.adaptive_routing.exploration_rate` | none | `0.0` | Reserved exploration rate in the range `0.0..=1.0`; currently parsed and validated but not applied to live routing. |
| `gateway.state.backend` | none | `file` | Gateway admin/usage state backend. Valid values: `file`, `sqlite`, `postgres`, `redis`. `postgres` stores admin-managed virtual keys, cumulative per-key/grouped request counts, token/cost reservations, usage counters, and billing ledger rows in a shared Postgres database. `redis` stores compatibility state and is not a durable production accounting backend. |
| `gateway.state.sqlite_path` | none | `gateway-state.sqlite` under the Prodex root when `backend=sqlite` | Optional non-empty SQLite database path for admin-managed virtual keys, usage counters, and schema migrations. Relative paths are resolved under the Prodex root and non-blank values are preserved exactly. |
| `gateway.state.postgres_url_env` | none | empty | Environment variable containing a non-empty, whitespace-free Postgres connection URL. Required when `backend="postgres"`. |
| `gateway.state.postgres_url_ref` | none | empty | Projected Postgres connection-URL reference. Use instead of `postgres_url_env`; required for the production Postgres backend. |
| `gateway.state.postgres_tls_mode` | none | `verify-full` in production; `disable` otherwise | PostgreSQL transport mode. `verify-full` forces TLS plus certificate-chain and hostname verification for pooled, blocking, and migration connections. `disable` is rejected when `secrets.production=true`. |
| `gateway.state.postgres_tls_ca_path` | none | native trust store only | Optional PEM CA bundle added to native roots when `postgres_tls_mode="verify-full"`. Relative paths resolve under the Prodex root. It is invalid with `disable`. |
| `gateway.state.redis_url_env` | none | empty | Environment variable containing a non-empty, whitespace-free Redis connection URL. Required when `backend="redis"`; optional Redis coordination source when `backend="postgres"`. |
| `gateway.state.redis_url_ref` | none | empty | Projected Redis connection-URL reference. Use instead of `redis_url_env` in production; with `backend="postgres"`, configures rebuildable coordination beside durable PostgreSQL state. |
| `gateway.admin_tokens` | env vars named by `token_env` | empty | Additional admin-plane bearer tokens. Each configured `token_env` name must be exact and whitespace-free and resolve to a non-empty, whitespace-free secret. They protect `/v1/prodex/gateway/*` only and do not authorize model traffic. |
| `gateway.admin_tokens[].name` | none | required per token | Exact non-empty admin-token identifier without whitespace. |
| `gateway.admin_tokens[].token_ref` | none | empty | Projected control-plane bearer-token reference. Exactly one of `token_ref` or development-only `token_env` is required. |
| `gateway.admin_tokens[].role` | none | `viewer` | Admin-plane role: `admin` can create/update/delete keys; `viewer` can read keys, usage, metrics, and OpenAPI only. |
| `gateway.admin_tokens[].allowed_key_prefixes` | none | empty | Optional virtual-key name prefixes this admin token can see and mutate. Empty means global access. |
| `gateway.admin_tokens[].tenant_id` | none | empty | Optional tenant boundary for this admin token. Tenant-scoped admins only see and mutate keys, SCIM users, usage, ledger rows, summaries, CSV exports, and metrics in that tenant. |
| `gateway.admin_tokens[].team_id` / `project_id` / `user_id` / `budget_id` | none | empty | Optional governance boundaries for this admin token. Scoped admins only see, create, mutate, and export virtual keys, usage, ledger rows, summaries, CSV exports, and metrics matching those dimensions. |
| `gateway.sso.proxy_token_env` | named env var | empty | Enable trusted reverse-proxy SSO for admin endpoints. The env-var name and resolved secret value must be exact and whitespace-free. The proxy must send this shared token in `gateway.sso.token_header`; Prodex then trusts the configured identity headers. |
| `gateway.sso.proxy_token_ref` | none | empty | Projected trusted-proxy token reference. Mutually exclusive with `proxy_token_env`. |
| `gateway.sso.token_header` | none | `x-prodex-sso-token` | Exact whitespace-free header carrying the trusted proxy shared token. |
| `gateway.sso.user_header` | none | `x-prodex-sso-user` | Exact whitespace-free header carrying the authenticated user name/email from the upstream SSO proxy. |
| `gateway.sso.role_header` | none | `x-prodex-sso-role` | Exact whitespace-free optional header carrying `admin` or `viewer`; missing/invalid values fall back to an active matching SCIM user's role, then `viewer`. |
| `gateway.sso.key_prefixes_header` | none | `x-prodex-sso-key-prefixes` | Exact whitespace-free optional comma/semicolon/newline-separated virtual-key prefixes visible to the SSO principal. Empty means global access. |
| `gateway.sso.tenant_header` | none | `x-prodex-sso-tenant` | Exact whitespace-free optional tenant id header from a trusted SSO proxy. Missing values fall back to an active matching SCIM user's tenant. SCIM users can also carry `team_id`, `project_id`, `user_id`, and `budget_id`; SSO/OIDC admin requests inherit those dimensions from the matching active SCIM user. |
| `gateway.sso.require_tenant` | none | `false` | Reject SSO/OIDC admin authentication when no tenant is supplied by the trusted header, OIDC claim, or active SCIM user. Enable for multi-tenant deployments. |
| `gateway.sso.oidc_issuer` | none | empty | Enable native OIDC/JWT admin auth for bearer tokens issued by this exact normalized HTTPS issuer. The bounded URL must include a permitted host and no userinfo, query, or fragment. Requires `oidc_audience`; Prodex discovers JWKS from this issuer when `oidc_jwks_url` is omitted. |
| `gateway.sso.oidc_audience` | none | empty | Exact whitespace-free required audience for OIDC/JWT admin bearer tokens. |
| `gateway.sso.oidc_jwks_url` | none | issuer discovery | Optional exact HTTPS JWKS URL used to verify OIDC/JWT admin bearer token signatures. It must use the issuer origin unless its full origin and effective port appear in `oidc_jwks_origin_allowlist`. |
| `gateway.sso.oidc_jwks_origin_allowlist` | none | empty | Up to 16 exact HTTPS origins permitted for cross-origin JWKS. Entries contain only scheme, normalized host, and optional explicit port; paths, userinfo, queries, fragments, and private/loopback/metadata IP literals are rejected. Redirects remain disabled. |
| `gateway.sso.oidc_user_claim` | none | `email` | Exact whitespace-free claim used as the admin principal name before SCIM lookup. Runtime falls back to `email`, `preferred_username`, then `sub` only when the configured claim is absent from the token. |
| `gateway.sso.oidc_role_claim` | none | `prodex_role` | Exact whitespace-free optional claim carrying `admin` or `viewer`; missing values fall back to an active matching SCIM user's role, then `viewer`, while malformed/unknown explicit values resolve to `viewer`. |
| `gateway.sso.oidc_tenant_claim` | none | `prodex_tenant` | Exact whitespace-free optional claim carrying the admin tenant id; missing values fall back to an active matching SCIM user's tenant. |
| `gateway.sso.oidc_key_prefixes_claim` | none | `prodex_key_prefixes` | Exact whitespace-free optional string or string-array claim carrying visible virtual-key prefixes; missing values fall back to SCIM user prefixes. |
| `gateway.sso.default_role` | none | `viewer` | Compatibility setting retained for existing policy files. Missing or invalid role claims resolve to `viewer`; configure an explicit SSO/OIDC or SCIM admin role instead. |
| `gateway.route_aliases` | none | empty | Declarative exact model aliases. Matching request `model` values are rewritten according to each alias `strategy`; alias names and model IDs must be non-empty and whitespace-free. |
| `gateway.route_aliases[].strategy` | none | `fallback` | Routing strategy for the alias: `fallback` rewrites to `combo:...`, `round-robin` selects one target by request id, `least-busy` selects the target with the fewest in-flight gateway requests, `first` always picks the first target. |
| `gateway.route_aliases[].model_metrics` | none | catalog defaults where known | Optional per-model routing hints for metric strategies: cost, latency, RPM limit, and TPM limit. Metric model IDs must exactly match one configured alias model. Policy values override the embedded provider/model catalog. |
| `gateway.request_constraints.enabled` | none | `false` | Enable catalog-backed endpoint, feature, context-window, output-limit, and known reasoning-reserve admission before gateway ranking. Disabled preserves legacy alias selection. |
| `gateway.request_constraints.unknown_context` | none | `allow` | Handling for an unknown context window: `allow`, `safe_window`, or `reject`. Unknown values stay explicit in the decision trace. |
| `gateway.request_constraints.safe_window_tokens` | none | `128000` | Positive admission boundary used only when `unknown_context="safe_window"`. |
| `gateway.request_constraints.oversized_output` | none | `passthrough` | Handling for an explicit output limit above a known model limit: `passthrough`, `reject`, or `clamp_with_notice`. Clamping is never silent. |
| `gateway.virtual_keys` | env vars named by `token_env` | empty | Static virtual gateway keys. Each key can enforce model allowlists, persisted request/spend budgets, RPM, and TPM, and can carry governance dimensions for admin and FinOps reporting. |
| `gateway.virtual_keys[].name` | none | required per key | Exact non-empty virtual-key identifier without whitespace. |
| `gateway.virtual_keys[].token_env` | named env var | required per key | Environment variable containing the bearer token for this virtual key. Missing, empty, or whitespace-bearing env vars are configuration errors. |
| `gateway.virtual_keys[].token_ref` | none | empty | Projected data-plane bearer-token reference. Exactly one of `token_ref` or development-only `token_env` is required. |
| `gateway.virtual_keys[].tenant_id` | none | empty | Optional tenant id assigned to this policy-backed key for tenant-scoped admin visibility. |
| `gateway.virtual_keys[].team_id` / `project_id` / `user_id` / `budget_id` | none | empty | Optional governance dimensions returned by the admin API and SDK for team, project, user, and budget attribution. When multiple virtual keys share a non-empty `budget_id`, `request_budget` and `budget_usd` also act as shared caps for that budget bucket. |
| `gateway.virtual_keys[].allowed_models` | none | empty | Optional model allowlist checked against the request `model` before route alias rewrite. |
| `gateway.virtual_keys[].budget_usd` | none | empty | Optional persisted spend cap for estimated request cost when catalog or policy cost is available. |
| `gateway.virtual_keys[].request_budget` | none | empty | Optional persisted total request cap for the virtual key name. |
| `gateway.virtual_keys[].rpm_limit` / `gateway.virtual_keys[].tpm_limit` | none | empty | Optional per-minute request/token caps. TPM uses Prodex's semantic request-token estimator. |
| `gateway.observability.sinks` | none | `runtime-log` | Exact whitespace-free enabled gateway observability sinks. `runtime-log` is always enabled; `jsonl` and `http` are enabled automatically when their target fields are set. |
| `gateway.observability.call_id_header` | none | `x-prodex-call-id` | Response header containing a stable per-request UUIDv7 call id such as `prodex-018f2f9a-...`. |
| `gateway.observability.jsonl_path` | none | empty | Optional non-empty JSONL export path for structured `gateway_spend` events. Relative paths are resolved under the Prodex root and preserved exactly. |
| `gateway.observability.http_endpoint` | none | empty | Optional exact HTTP(S) JSON export endpoint with host and no userinfo for structured `gateway_spend` events. |
| `gateway.observability.http_schema` | none | `generic` | Exact HTTP export payload schema: `generic`, `otel`, `datadog`, or `langfuse`. Unsupported values fail closed. |
| `gateway.observability.http_bearer_token_env` | none | empty | Exact whitespace-free environment variable name containing a bearer token for `gateway.observability.http_endpoint`; configured refs must resolve to a non-empty, whitespace-free token. |
| `gateway.observability.http_bearer_token_ref` | none | empty | Projected telemetry-export bearer-token reference. Mutually exclusive with `http_bearer_token_env`. |
| `gateway.guardrails.blocked_keywords` | none | empty | Case-insensitive pre-call keyword blocks applied before upstream send. |
| `gateway.guardrails.blocked_output_keywords` | none | empty | Case-insensitive output keyword blocks. Buffered responses are replaced with `403 policy_violation`; streaming responses are stopped and logged when a keyword is observed. |
| `gateway.guardrails.allowed_models` | none | empty | Optional pre-call allowlist for request `model` values, checked before route alias rewrite. |
| `gateway.guardrails.presidio_redaction` | CLI `--presidio` / `--no-presidio` | `false` | Enable Presidio request-body redaction for gateway traffic. |
| `gateway.guardrails.prompt_injection_detection` | none | `false` | Enable built-in prompt-injection heuristic checks before upstream send. |
| `gateway.guardrails.pii_redaction` | none | `false` | Enable local best-effort request-body redaction for emails, secret-like bearer/API-key values, and long digit groups before upstream send. |
| `gateway.guardrails.webhook_url` | none | empty | Optional external guardrail HTTP endpoint. Prodex sends base64 request/response bodies and expects JSON such as `{"allow": false, "reason": "...", "message": "..."}` to block. |
| `gateway.guardrails.webhook_host_allowlist` | none | empty | Exact bounded DNS host allowlist for the guardrail endpoint. Required with HTTPS in `bank_enforce`; redirects are not followed. |
| `gateway.guardrails.webhook_phases` | none | both phases | External guardrail phases: `pre` for requests before upstream send, `post` for buffered responses before returning to caller. |
| `gateway.guardrails.webhook_bearer_token_env` | none | empty | Exact whitespace-free environment variable name containing a non-empty, whitespace-free bearer token for `gateway.guardrails.webhook_url`. |
| `gateway.guardrails.webhook_bearer_token_ref` | none | empty | Projected webhook bearer-token reference. Mutually exclusive with `webhook_bearer_token_env`. |
| `gateway.guardrails.webhook_fail_closed` | none | `false` | Block when the external guardrail endpoint fails or returns non-2xx. |

### Route decisions and request constraints

Fresh pre-commit selection emits one schema-versioned `route_decision` event for each decision. Its typed stages are ordered as affinity, model resolution, endpoint capability, request constraints, governance, authentication, quota, circuit/backoff, admission, ranking, and final selection when those stages apply. Schema v1 retains at most 11 stage summaries and 32 candidate records, caps safe identifiers at 96 bytes, and reports every omission or truncation. Candidate identities are trace-local opaque values; traces never contain request bodies, prompts, tool arguments, cookies, bearer values, credentials, tenant secrets, or filesystem paths. Request and configured call-ID correlation stays in log fields and is not added to metric labels.

When `[gateway.request_constraints] enabled = true`, Prodex resolves aliases to concrete upstream models, evaluates each concrete candidate against known endpoint/features, estimated input, output reservation, context window, and provider reasoning reservation, then applies governance and runtime ranking only to eligible candidates. Catalog values remain optional: `unknown_context` controls unknown windows, while unknown output limits are reported rather than guessed. `passthrough` preserves an oversized explicit output request, `reject` returns a pre-commit no-route result, and `clamp_with_notice` rewrites the limit once before the first upstream attempt and records both requested and applied values. The default disabled/passthrough policy does not reject malformed legacy limits or alter existing route selection.

Hard continuation ownership remains authoritative. `previous_response_id`, turn-state, and session-scoped unary ownership are validated only against their owner; constraint failure does not turn a continuation into a fresh request or select a larger alternate. Rotation and output adjustment occur only before commitment. After unary or streaming commitment, Prodex preserves the natural upstream/transport failure and never replays against another provider.

Authenticated `admin` and read-only `viewer` principals can `POST /v1/prodex/gateway/routes/explain` or use **Route Workbench** in `/v1/prodex/gateway/admin`. The bounded JSON response uses the same pure planner as live routing and includes alias resolution, requirements, candidate decisions, adjustments, warnings, and trace truncation. The optional global current-load snapshot is available only to unscoped principals; the dashboard uses the side-effect-free catalog-only view. Explain authenticates and authorizes through the normal admin control-plane boundary and writes only redacted audit metadata. It does not call upstreams, refresh credentials, reserve billing/quota, increment request/model/inflight counters, mutate affinity/round-robin/circuit/prompt-cache state, or persist runtime state. Submitted request JSON is neither logged nor stored by the dashboard. Historical trace storage is not provided; explain results are on-demand and non-durable.

For production multi-replica accounting, set `gateway.state.backend =
"postgres"`, configure `gateway.state.redis_url_ref`, run PostgreSQL migration
v2, set `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS=true`, and set
`PRODEX_GATEWAY_REPLICA_COUNT` to at least `2`. This topology uses PostgreSQL for
atomic cumulative request/token/cost admission and Redis for distributed
RPM/TPM. Production policy defaults PostgreSQL to `verify-full`; set a custom
CA path when the database certificate is issued by a private trust root.

Admin-managed virtual keys and SCIM users default to file state under the Prodex root as `gateway-virtual-keys.json`; request/spend usage defaults to `gateway-virtual-key-usage.json`, and response-reconciled billing ledger records default to `gateway-billing-ledger.jsonl`. Set `[gateway.state] backend = "sqlite"` to store admin-managed keys, SCIM users, usage counters, billing ledger records, and schema migrations in one SQLite database, `backend = "postgres"` with `postgres_url_env` to store the same admin/usage/ledger/SCIM state in a shared Postgres database, or `backend = "redis"` with `redis_url_env` to store gateway state in Redis. New SCIM users receive typed UUIDv7 `PrincipalId` resource IDs, and missing create-time `user_id` defaults to that generated resource ID when no admin user scope supplies one. Stored virtual-key token hashes/names/governance/model scopes and SCIM authentication fields must pass exact validation, file key-store scope fields must be strings when present, active SCIM loads omit malformed persisted authorization rows, file/SQL/Redis key-store budget/rate-limit/timestamp fields and Redis usage hash counters must be exact unsigned integers when present, file key-store `disabled` fields must be JSON booleans when present, SQLite/Redis key-store boolean fields must be exact `0` or `1`, and SQL/Redis key-store JSON array fields must contain non-empty whitespace-free string entries; malformed present fields fail the load instead of silently becoming absent, false, zero, renamed, rescaled, or empty unrestricted policy. The configured gateway admin token from `--auth-token` or `PRODEX_GATEWAY_TOKEN` has admin role and can `GET`/`POST` `/v1/prodex/gateway/keys`, `GET`/`PATCH`/`DELETE` `/v1/prodex/gateway/keys/{name}`, `GET`/`POST` `/v1/prodex/gateway/scim/v2/Users`, `GET`/`PATCH`/`PUT`/`DELETE` `/v1/prodex/gateway/scim/v2/Users/{id}`, read `/v1/prodex/gateway/usage`, read `/v1/prodex/gateway/ledger`, read aggregated billing totals from `/v1/prodex/gateway/ledger/summary`, export billing CSV from `/v1/prodex/gateway/ledger.csv` and `/v1/prodex/gateway/ledger/summary.csv`, scrape Prometheus text metrics from `/v1/prodex/gateway/metrics`, inspect provider adapter contracts through `/v1/prodex/gateway/providers`, inspect active observability and guardrail configuration through `/v1/prodex/gateway/observability` and `/v1/prodex/gateway/guardrails`, fetch `/v1/prodex/gateway/openapi.json`, and use the built-in gateway admin dashboard at `/v1/prodex/gateway/admin`. Prometheus virtual-key metrics expose a stable `key_hash` plus low-cardinality scope booleans such as `tenant_scoped`, `team_scoped`, `project_scoped`, `user_scoped`, and `budget_scoped`; they do not expose raw tenant, team, project, user, budget, or virtual-key names as metric labels. Additional `[[gateway.admin_tokens]]` entries can be `admin` or read-only `viewer`, and can set `allowed_key_prefixes`, `tenant_id`, `team_id`, `project_id`, `user_id`, and/or `budget_id` to restrict key list/read/mutation, SCIM user management, usage, ledger, summary, CSV, and metrics visibility. `[gateway.sso]` can trust an authenticated reverse proxy by requiring a shared proxy token header and mapping user, role, tenant, and key-prefix headers to the same admin RBAC model. It can also verify native OIDC/JWT bearer tokens against a configured issuer and audience, using either a configured JWKS URL or the issuer discovery document; role, tenant, key-prefix, and governance dimensions can come from an active matching SCIM user when token/header values are absent. An inactive matching SCIM user is rejected. Virtual-key bearer tokens cannot use these admin endpoints. `POST /keys` returns a generated bearer token once when `token` is omitted, while persisted state stores only its hash. Keys configured by `policy.toml` stay source `policy` and are read-only through the admin API. Admin-managed key create, update, rotate, delete, and SCIM user mutations are recorded as `gateway_admin` events in `prodex audit` without storing bearer token material. Gateway observability emits `gateway_spend` with `phase=request` after upstream response headers and `phase=response` after buffered response completion or streaming EOF/drop. Provider catalog edits should pass `npm run catalog:providers`; runtime catalog numeric overrides such as `model_context_window` and `model_auto_compact_token_limit` must be exact positive integers greater than one.

Gateway OIDC discovery/JWKS startup prefetch is bounded by
`PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS`, defaulting to 2000 ms. Request-path
OIDC authentication reads only the startup cache and fails closed when the cache
is unavailable. OIDC timing overrides retain their legacy compatibility rule:
valid unsigned values are bounded, HTTP cache TTL and last-known-good may be zero
for diagnostics, and malformed values use the default while logging only the
affected key name. Values are snapshotted before the listener is bound and do
not change until restart.

Example:

```toml
[gateway]
listen_addr = "127.0.0.1:4000"
provider = "gemini"
harness = "minimal"
require_auth = true

[gateway.state]
backend = "sqlite"
sqlite_path = "gateway-state.sqlite"

[[gateway.admin_tokens]]
name = "ops"
token_env = "PRODEX_GATEWAY_OPS_TOKEN"
role = "admin"

[[gateway.admin_tokens]]
name = "auditor"
token_env = "PRODEX_GATEWAY_AUDITOR_TOKEN"
role = "viewer"
allowed_key_prefixes = ["team-a-"]
tenant_id = "tenant-a"
team_id = "platform"
project_id = "codex-gateway"
user_id = "alice@example.com"
budget_id = "budget-platform"

[gateway.sso]
proxy_token_env = "PRODEX_GATEWAY_SSO_PROXY_TOKEN"
user_header = "x-auth-request-email"
role_header = "x-prodex-role"
key_prefixes_header = "x-prodex-key-prefixes"
tenant_header = "x-prodex-tenant"
require_tenant = true
default_role = "viewer"

# Or verify native OIDC/JWT admin bearer tokens directly.
# oidc_issuer = "https://idp.example"
# oidc_audience = "prodex-gateway"
# oidc_jwks_url = "https://idp.example/.well-known/jwks.json" # optional
# oidc_jwks_origin_allowlist = ["https://keys.idp.example:8443"] # only for cross-origin JWKS
# oidc_user_claim = "email"
# oidc_role_claim = "prodex_role"
# oidc_tenant_claim = "prodex_tenant"
# oidc_key_prefixes_claim = "prodex_key_prefixes"

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gemini-3-flash", "gemini-2.5-flash"]
strategy = "fallback"

[[gateway.route_aliases.model_metrics]]
model = "gemini-3-flash"
input_cost_per_million_microusd = 100
output_cost_per_million_microusd = 200
latency_ms = 300
rpm_limit = 60
tpm_limit = 100000

[[gateway.virtual_keys]]
name = "team-a"
token_env = "PRODEX_GATEWAY_TEAM_A_TOKEN"
tenant_id = "tenant-a"
team_id = "platform"
project_id = "codex-gateway"
user_id = "alice@example.com"
budget_id = "budget-platform"
allowed_models = ["prodex-fast"]
budget_usd = 10.0
request_budget = 1000
rpm_limit = 60
tpm_limit = 100000

[gateway.observability]
sinks = ["runtime-log", "jsonl", "http"]
call_id_header = "x-prodex-call-id"
jsonl_path = "gateway-spend.jsonl"
http_endpoint = "https://otel-collector.example/v1/events"
http_schema = "otel"
http_bearer_token_env = "PRODEX_GATEWAY_OBSERVABILITY_TOKEN"

[gateway.guardrails]
blocked_keywords = ["secret project"]
blocked_output_keywords = ["do not reveal"]
allowed_models = ["prodex-fast"]
presidio_redaction = true
prompt_injection_detection = true
pii_redaction = true
webhook_url = "https://guardrails.example/check"
webhook_host_allowlist = ["guardrails.example"]
webhook_phases = ["pre", "post"]
webhook_bearer_token_env = "PRODEX_GATEWAY_GUARDRAIL_TOKEN"
webhook_fail_closed = true
```

## Governance authority schemas

`governance.authority_tenants` is the bounded, typed tenant discovery source for
SQLite/PostgreSQL governance refresh. It is merged with tenant IDs on static admin
tokens, is limited to 64 unique UUIDs, and supports OIDC/SCIM deployments that have
no static tenant-bound admin token. `enterprise_enforce` and `bank_enforce` require
an authoritative SQLite or PostgreSQL store and at least one configured authority
tenant.

The stored `Policy` artifact is strict JSON using the same schema as
`RuntimePolicyGovernanceSettings`. Local `policy.toml` can configure the equivalent
fields directly. An empty `policy_rules` list keeps the mode baseline. Non-empty
custom rules are added to that baseline; they never replace it. IDs beginning with
`builtin.` are reserved. In particular, the immutable `bank_enforce` baseline keeps
its provider-trust, retention, training, response-inspection, audit, and fallback
obligations even when custom allow rules are present. Custom deny rules can tighten
the result.

```toml
[governance]
mode = "enterprise_observe"
authority_tenants = ["00000000-0000-7000-8000-000000000001"]

[[governance.policy_rules]]
id = "deny-revoked-tool-session"
effect = "deny"
obligations = []
reason_code = "policy.session_revoked"

[governance.policy_rules.condition]
channel = "api"
credential_scope = "data_plane"
session_revoked = true
requested_capability = "tools"
```

Each rule requires `id`, `condition`, `effect`, `obligations`, and `reason_code`.
The strict condition schema supports channel, principal kind, minimum role,
credential scope, action, canonical route, minimum data classification, inspection
coverage, minimum request risk, network zone, maximum session age/idle time,
session revoked/MFA state, minimum retained classification, minimum authentication
strength, environment MFA state, one requested capability, and quota
headroom/reservation flags. Tenant is implicit in the tenant-bound snapshot.
Group and department attributes are not currently supported. Provider eligibility
is expressed through typed obligations and the separately activated provider
registry/routing artifacts, not free-form policy attributes.

Effects are `allow`, `require_approval`, and `deny`. A deny rule cannot carry
obligations. Typed obligation kinds are `mask_finding`,
`minimum_provider_trust`, `allow_provider`, `deny_provider`,
`require_local_execution`, `prohibit_retention`, `prohibit_training_use`,
`require_region`, `disable_tools`, `allow_tool`, `allow_model`, `allow_modality`,
`max_input_tokens`, `max_output_tokens`, `max_context_tokens`,
`require_response_inspection`, `session_idle_timeout_seconds`,
`session_absolute_timeout_seconds`, `require_reauthentication`, `require_mfa`,
`audit_detail`, `require_human_approval`, `retention_seconds`, and
`deny_fallback_outside_eligibility`. Rules are limited to 256 and obligations to
64 per rule. Unknown fields, missing required rule fields, invalid tokens/selectors,
zero positive bounds, and over-limit artifacts are rejected before activation.

`ClassificationRules` is a separate strict artifact activated through the
`/v1/prodex/gateway/classification-rules` lifecycle. It owns both tenant detector
patterns/revision and the classification rule set used by the PDP; the `Policy`
artifact does not override its classification decision. Example:

```json
{
  "schema_version": 1,
  "detector_revision": "detectors-v1",
  "patterns": [
    {"id": "customer-secret", "pattern": "customer-*secret"}
  ],
  "classification_revision": "classification-v1",
  "classification_checksum": "867cd7a4ceb2c1f0beb5f0662d71c059ce65ff0e2056be7aa7d650e5772cf3d7",
  "unsupported_coverage_floor": "restricted",
  "classification_rules": [
    {"finding_kind": "tenant_sensitive", "classification": "confidential"}
  ]
}
```

The checksum is lowercase SHA-256 over canonical JSON containing
`unsupported_coverage_floor` and the classification rules sorted by finding kind
and classification. A mismatch is rejected. Detector patterns are limited to 16
per tenant, use bounded interior `*` globs, and retain Unicode-safe byte offsets.

The data plane reads immutable per-tenant snapshots only. SQLite/PostgreSQL reads
run at startup and in the bounded background refresher; invalid refreshes retain
the last-known-good snapshot. Enforcing modes have no cross-tenant/default fallback.
Artifact validation, activation, startup load, refresh, and committed swap also
enforce the process deployment-mode floor: `bank_enforce` accepts only bank policy
artifacts, and `enterprise_enforce` cannot be downgraded to observe or personal.

## Runtime Proxy Keys

`runtime_proxy.preset` selects a conservative preset before individual `runtime_proxy` keys are applied.
Valid values are `low`, `default`, `many-terminals`, and `aggressive`; `PRODEX_RUNTIME_PROXY_PRESET` selects the preset from the environment.
Specific environment overrides for individual keys still have highest priority.
Unknown policy preset values are rejected when `policy.toml` is parsed. Unknown environment preset values are ignored so the configured policy or built-in defaults still apply.
The preset changes only local concurrency and admission tuning; transport timeouts remain on their normal defaults unless configured directly.
Runtime proxy numeric environment overrides are exact values: unset means use policy/default tuning, while empty, whitespace-bearing, malformed, or non-positive values for positive-only settings fail closed. Overflow-capacity overrides that explicitly allow zero keep zero as the documented escape hatch.
`PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES` follows the same exact positive-integer boundary for the HTTP request capture limit.

## Runtime Proxy Contract

`runtime_proxy` tuning must preserve these invariants:

- Prodex stays a scoped Codex gateway, not a general-purpose LLM SDK.
- Profile selection must be visible through policy, `prodex info`, `prodex doctor`, and runtime logs.
- Pre-commit retry and fallback paths must stay bounded per request.
- Runtime hot paths must avoid broad disk reads, quota probes, or blocking state saves.
- Quota, budget, transport, and local pressure signals must stay classified separately.
- Selection, admission, affinity, backoff, and first-chunk events must be structured in runtime logs.
- Upstream HTTP/WebSocket connection reuse should be preserved where it does not change Codex semantics.
- Secrets remain profile-isolated, redacted in diagnostics, and covered by audit events for Prodex-owned mutations.

<!-- BEGIN GENERATED RUNTIME_PROXY_KEYS -->
| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `runtime_proxy.worker_count` | `PRODEX_RUNTIME_PROXY_WORKER_COUNT` | CPU parallelism clamped to `4..12` | Short-lived proxy worker pool size. |
| `runtime_proxy.long_lived_worker_count` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT` | `parallelism * 2` clamped to `8..24` | Worker pool for long-lived streams and websocket work. |
| `runtime_proxy.probe_refresh_worker_count` | `PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT` | CPU parallelism clamped to `2..4` | Background profile probe refresh workers. |
| `runtime_proxy.async_worker_count` | `PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT` | CPU parallelism clamped to `2..4` | Async runtime worker count. |
| `runtime_proxy.long_lived_queue_capacity` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY` | `long_lived_worker_count * 8` clamped to `128..1024` | Queue capacity for long-lived proxy work. |
| `runtime_proxy.active_request_limit` | `PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT` | `worker_count + long_lived_worker_count * 3` clamped to `64..512` | Global local admission cap for fresh runtime proxy requests. |
| `runtime_proxy.responses_active_limit` | `PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT` | `75%` of global limit, clamped to `4..global` | Lane cap for main Responses traffic. |
| `runtime_proxy.compact_active_limit` | `PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT` | `25%` of global limit, clamped to `2..6` | Lane cap for `/responses/compact`. |
| `runtime_proxy.websocket_active_limit` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT` | `long_lived_worker_count` clamped to `2..global` | Lane cap for websocket transport. |
| `runtime_proxy.standard_active_limit` | `PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT` | `worker_count / 2` clamped to `2..8` | Lane cap for other unary proxy traffic. |
| `runtime_proxy.profile_inflight_soft_limit` | `PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT` | `4` | Fresh selection starts penalizing profiles above this in-flight count. |
| `runtime_proxy.profile_inflight_hard_limit` | `PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT` | `8` | Fresh selection avoids profiles above this in-flight count; hard affinity still wins. |
| `runtime_proxy.admission_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS` | `750` | Normal wait budget for local admission pressure. |
| `runtime_proxy.pressure_admission_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS` | `200` | Shorter admission wait budget when proxy is already under pressure. |
| `runtime_proxy.long_lived_queue_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS` | `750` | Normal wait budget for long-lived queue pressure. |
| `runtime_proxy.pressure_long_lived_queue_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS` | `200` | Shorter long-lived queue wait budget under pressure. |
| `runtime_proxy.http_connect_timeout_ms` | `PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS` | `5000` | Upstream HTTP connect timeout. |
| `runtime_proxy.stream_idle_timeout_ms` | `PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS` | `300000` | Responses stream idle timeout, aligned with Codex behavior. |
| `runtime_proxy.compact_request_timeout_ms` | `PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS` | `90000` | Total request timeout for unary remote compact calls before Codex can observe failure and retry. |
| `runtime_proxy.sse_lookahead_timeout_ms` | `PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS` | `1000` | Pre-commit SSE lookahead timeout. |
| `runtime_proxy.prefetch_backpressure_retry_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS` | `10` | Retry delay while stream prefetch is backpressured. |
| `runtime_proxy.prefetch_backpressure_timeout_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS` | `1000` | Max wait for stream prefetch backpressure to clear. |
| `runtime_proxy.prefetch_max_buffered_bytes` | `PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES` | `786432` | Max buffered prefetch bytes before backpressure. |
| `runtime_proxy.websocket_connect_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS` | `15000` | Upstream websocket connect timeout. |
| `runtime_proxy.websocket_happy_eyeballs_delay_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS` | `200` | Delay before alternate websocket TCP connect attempt. |
| `runtime_proxy.websocket_precommit_progress_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS` | `8000` | Websocket pre-commit progress timeout. |
| `runtime_proxy.websocket_connect_worker_count` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT` | CPU parallelism clamped to `4..16` | Worker count for bounded websocket TCP connect executor. |
| `runtime_proxy.websocket_connect_queue_capacity` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY` | `websocket_connect_worker_count * 8` clamped to `32..128` | Bounded queue capacity for websocket TCP connect work; effective value is at least the worker count. |
| `runtime_proxy.websocket_connect_overflow_capacity` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY` | `websocket_connect_queue_capacity * 4` clamped to `32..512` | Overflow queue capacity for websocket TCP connect work after the bounded queue fills; `0` disables overflow buffering. |
| `runtime_proxy.websocket_dns_worker_count` | `PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT` | CPU parallelism clamped to `2..8` | Worker count for bounded websocket DNS resolution executor. |
| `runtime_proxy.websocket_dns_queue_capacity` | `PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY` | `websocket_dns_worker_count * 4` clamped to `16..64` | Bounded queue capacity for websocket DNS resolution work; effective value is at least the worker count. |
| `runtime_proxy.websocket_dns_overflow_capacity` | `PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY` | `websocket_dns_queue_capacity * 2` clamped to `16..128` | Overflow queue capacity for websocket DNS resolution work after the bounded queue fills; `0` disables overflow buffering. |
| `runtime_proxy.websocket_previous_response_reuse_stale_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS` | `60000` | Window for reusing a websocket previous-response binding before treating it as stale. |
| `runtime_proxy.broker_ready_timeout_ms` | `PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS` | `15000` | Startup wait for the runtime broker to become ready. |
| `runtime_proxy.broker_health_connect_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS` | `750` | Broker health check connect timeout. |
| `runtime_proxy.broker_health_read_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS` | `1500` | Broker health check read timeout. |
| `runtime_proxy.sync_probe_pressure_pause_ms` | `PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS` | `5` | Pause before synchronous probe work when local pressure is detected. |
| `runtime_proxy.responses_critical_floor_percent` | `PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT` | `2` | Minimum remaining Responses quota percentage treated as critical; valid range `1..10`. |
| `runtime_proxy.startup_sync_probe_warm_limit` | `PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT` | `1` | Startup synchronous quota probe warm-up limit, capped internally at `3`. |
<!-- END GENERATED RUNTIME_PROXY_KEYS -->

Positive integer values are required for numeric policy keys, except websocket overflow capacity keys, which may be `0`, and `responses_critical_floor_percent`, which must be between `1` and `10`.
Some effective values are clamped after env or policy resolution to protect runtime bounds.

## Example

```toml
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[runtime_proxy]
preset = "many-terminals"
worker_count = 16
active_request_limit = 128
responses_active_limit = 96
profile_inflight_soft_limit = 6
profile_inflight_hard_limit = 10
```
