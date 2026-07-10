# Prodex Enterprise Readiness Audit

This audit maps the priority enterprise findings to the current boundary work,
regression coverage, decisions, and known migration gaps. It is intentionally
evidence-oriented so future refactors can preserve Prodex runtime invariants
while moving legacy adapter code behind enterprise boundaries.

## Scope

- Data plane: stateless gateway admission, provider invocation, reservation,
  trace propagation, and compatibility with existing Codex proxy behavior.
- Control plane: tenant, identity, authorization, key, policy, budget, audit,
  and configuration management planning boundaries.
- Storage: tenant-scoped, atomic, migration-safe PostgreSQL, Redis, and SQLite
  contracts. PostgreSQL remains the durable source of truth; Redis remains a
  rebuildable cache, limiter, or coordination primitive.
- Documentation: ADRs, threat model, migration guide, deployment notes, and CI
  guards that keep the enterprise boundaries visible.

## Audit Matrix

### 1. SSO Role Fallback Must Not Become Admin

- **Evidence:** `crates/prodex-domain/src/security.rs`
  (`ExplicitRoleMapper`, `Principal`, `Role`) and
  `crates/prodex-authn/src/lib.rs` (`authenticate_oidc_claims`).
- **Risk:** A missing or unknown SSO role claim could otherwise create a silent
  vertical privilege escalation to Admin.
- **Regression coverage:** `crates/prodex-domain/tests/security.rs`,
  `crates/prodex-authn/tests/oidc.rs`, and
  `scripts/ci/auth-boundary-guard.mjs`.
- **Local verification:** `npm run ci:gateway-security-smoke` exercises the
  focused gateway admin auth and usage suites after SSO/OIDC role-boundary
  changes.
- **Implemented fix or boundary:** Role mapping is explicit; missing or unknown
  role claims resolve to Viewer or denial, never Admin. Multi-tenant SSO/OIDC
  deployments can enable `gateway.sso.require_tenant` so admin authentication
  fails closed when no tenant header, tenant claim, or active SCIM tenant
  exists. The HTTP boundary now canonicalizes legacy
  `/prodex/gateway/...` admin mounts into explicit control-plane route
  operations, including gateway admin reads, SCIM reads, and virtual-key
  read/update/delete operations. The legacy mounted admin adapter now calls
  that planner after successful admin authentication and before route-specific
  handling, so method rejection uses the shared stable control-plane envelope.
  Domain role mapping matches claims exactly, ignores configured claims
  containing whitespace, and treats incoming whitespace-bearing claims as
  unknown so padded claims cannot be normalized into Admin. Legacy SCIM
  provisioning also treats `userName` and `role` as exact non-empty
  whitespace-free values so padded principals and roles cannot be normalized
  into an authorized identity. Gateway SSO/OIDC principal strings now match
  SCIM users exactly instead of trim-normalizing into a provisioned identity.
  Gateway OIDC issuer and JWKS URLs are exact HTTPS configuration boundaries
  instead of trim-normalized identity-provider endpoints, and OIDC audience
  values are exact identity-provider audience selectors. Malformed or partial
  direct OIDC launch settings now fail closed instead of silently disabling OIDC
  or dropping configured JWKS metadata. When gateway OIDC is configured, OIDC
  cache/refresh runtime env overrides now reject empty, whitespace-bearing,
  non-integer, or disallowed zero values during launch instead of silently using
  default cache timing.
  Gateway SSO header names and OIDC claim names are exact identity selectors
  instead of trim-normalized principal, role, tenant, or scope sources, and
  malformed configured selectors fail closed during runtime launch instead of
  falling back to defaults.
  The gateway admin dashboard preserves exact key,
  SCIM principal, model, and scope identifier inputs so the server-side
  validation boundary is not hidden by client-side trimming. The OpenAPI
  contract also advertises the exact identifier schema for key, model, SCIM
  principal, key-prefix, and scope fields. Stored virtual-key token hashes and
  names loaded from compatibility key stores now also use exact validation
  instead of trim-normalizing corrupt persisted credential/key fields into
  active runtime keys. Stored virtual-key governance scope identifiers and model
  allow-list entries loaded from compatibility key stores also fail closed when
  null or whitespace-bearing instead of becoming active tenant, team, project,
  user, budget, or model scopes. Persisted SCIM
  users used for SSO/OIDC authorization now also require exact principal, role,
  governance, and key-prefix scope fields before contributing auth state, and
  active SCIM store loads omit malformed persisted authorization rows before
  exposing admin state. Redis key-store loading also treats
  persisted key, model, SCIM principal, key-prefix, role, and scope identifiers
  exactly instead of trim-normalizing hash fields into active runtime state, and
  present file, SQL, and Redis key-store budget, rate-limit, and timestamp
  numeric fields now fail closed when malformed or negative instead of
  disappearing as `None` or `0`. Present file key-store `disabled` fields now
  fail closed unless they are exact JSON booleans, and SQLite/Redis key-store
  boolean fields such as `disabled` and `active` also fail closed unless they
  are exact `0` or `1`, so malformed disabled flags cannot silently re-enable a
  virtual key.
  Present SQL and Redis
  key-store JSON arrays for model allow-lists and SCIM/admin key-prefix scopes
  now also fail closed when malformed instead of silently becoming unrestricted
  empty policy.
  SQL and Redis key-store loaders share an exact parser for persisted model and
  key-prefix identifier arrays. Configured gateway admin tokens now fail
  closed: missing roles resolve to Viewer and unknown roles are rejected instead
  of becoming Admin. Gateway admin role parsing now rejects whitespace-bearing
  configured, SSO, OIDC, and SCIM role values instead of normalizing them into
  write access. Configured gateway admin key-prefix scopes are also exact
  non-empty values instead of trim-normalized authorization scopes. Configured
  gateway admin-token and virtual-key governance scope IDs also match exactly
  instead of being trimmed into tenant, team, project, user, or budget scope.
  Gateway virtual-key and guardrail model allow-list entries are exact
  non-empty model IDs instead of trim-normalized policy values. Gateway route
  alias names, model IDs, per-model metric IDs, and route strategy names are
  also exact fail-closed routing policy inputs; malformed direct launch route
  aliases no longer become empty aliases or silently lose target models and
  metric rows. Gateway provider selectors are exact
  fail-closed routing policy inputs instead of trim-normalized adapter names or
  unsupported names that silently fall back to the OpenAI-compatible default.
  Gateway base URLs are exact upstream routing endpoints instead of
  trim-normalized HTTP targets, and explicitly empty direct inputs fail closed
  instead of falling back to provider or OpenAI defaults. Gateway listen
  addresses are exact bind-address inputs instead of trim-normalized network
  exposure settings. Configured gateway admin token
  names and virtual key names also fail closed as exact identifiers instead of
  being trim-normalized or mapped to empty credential identifiers. Gateway
  credential environment references are exact secret-reference inputs instead of
  trim-normalized env var names, including admin-token, SSO proxy-token,
  guardrail webhook bearer-token, and observability HTTP sink bearer-token
  references; malformed, missing, or empty configured admin-token env references
  now fail closed during runtime launch instead of silently dropping a
  configured control-plane credential. Malformed configured SSO proxy-token env
  references also fail closed during runtime launch, and malformed, missing, or
  empty observability bearer env references no longer silently disable
  configured HTTP sink authentication. Configured gateway bearer secret values
  now also reject empty or whitespace-bearing admin-token, virtual-key, SSO
  proxy-token, observability HTTP bearer-token, and guardrail webhook
  bearer-token values instead of trim-normalizing them. Gateway root-token
  values now reject explicit empty or whitespace-bearing `--auth-token` and
  `PRODEX_GATEWAY_TOKEN` inputs instead of trim-normalizing bearer secrets.
  Gateway state URL values now reject explicit empty or whitespace-bearing
  PostgreSQL/Redis connection URLs instead of trim-normalizing them or treating
  blank shared Redis coordination as absent. OpenAI-compatible provider API-key
  inputs also fail closed when explicitly empty, and singular key inputs reject
  whitespace-bearing values instead of trim-normalizing upstream credentials;
  the Mem0 memory gateway now applies the same boundary to OpenAI env keys and
  selected-profile `auth.json` keys.
  Anthropic, Copilot, DeepSeek, and Gemini provider API-key inputs now use the
  same fail-closed empty-value boundary for CLI keys, plural env vars, and
  single-key env vars, and singular external provider key values now reject
  whitespace-bearing credentials instead of trim-normalizing them. DeepSeek beta
  base URL overrides are exact upstream routing endpoints and now reject empty,
  whitespace-bearing, or non-http(s) direct values instead of trim-normalizing
  them or falling back to the default beta endpoint silently. DeepSeek web
  search mode overrides are exact feature-mode selectors and now reject empty,
  whitespace-bearing, or unsupported configured values instead of silently
  falling back to the default mode. DeepSeek strict-tools overrides are exact
  boolean selectors and now reject empty, whitespace-bearing, unsupported, or
  non-boolean configured values instead of silently disabling strict tool
  handling. Local, DeepSeek, Gemini, Anthropic, and Copilot provider catalog
  numeric overrides now reject empty, whitespace-bearing, non-numeric, zero, or
  one values instead of silently falling back to default capability metadata.
  Gateway guardrail webhook phases are exact trigger selectors instead of
  trim-normalized request/response phase values, and guardrail keyword text is
  preserved exactly instead of trim-normalized before matching.
  Gateway guardrail webhook URLs are exact external security-service routing
  targets instead of trim-normalized URL values, and runtime launch enforces
  that boundary even when policy settings were constructed outside the file
  validator.
  Gateway observability HTTP schema names are exact fail-closed telemetry
  format selectors instead of trim-normalized or unsupported schema values, and
  observability sink names are exact fail-closed exporter selectors instead of
  trim-normalized or silently dropped sink labels. Gateway observability call-id
  header names are exact propagation header selectors.
  Gateway observability HTTP endpoint URLs are also exact telemetry routing
  targets instead of trim-normalized export endpoints, and runtime launch now
  enforces the same no-whitespace, host-present, no-userinfo boundary used by
  policy validation. Gateway observability JSONL paths are preserved as exact
  filesystem paths rather than trim-normalized export file names, and blank-only
  direct settings now fail closed instead of disabling the JSONL sink silently.
  Gateway state backend URL environment references are also exact non-empty
  values instead of trim-normalized PostgreSQL or Redis URL refs. Gateway
  SQLite state paths are preserved as exact filesystem paths instead of
  trim-normalized state database names, and blank-only direct settings now fail
  closed instead of falling back to the default SQLite state file silently.
  Runtime policy path values, including `runtime.log_dir`, are also preserved
  as exact filesystem paths. Domain
  security debug output redacts principal and tenant identifiers while
  preserving authorization shape. Role-mapper debug output redacts configured
  identity-provider claim mappings. Legacy gateway admin composition now also
  delegates `Idempotency-Key` and `If-Match` parsing to the shared
  `prodex-gateway-http` boundary, so duplicate governance headers fail closed
  instead of the adapter selecting one value ad hoc. The mounted admin adapter
  now also routes non-empty idempotency and precondition header validation
  through the shared `prodex-application` control-plane HTTP planners before the
  legacy execution branches run.
- **ADRs:** `docs/adr/0002-gateway-admin-auth-boundary.md`,
  `docs/adr/0081-authn-boundary-crate.md`,
  `docs/adr/0082-auth-boundary-guard.md`,
  `docs/adr/0161-domain-security-stable-error-responses.md`,
  `docs/adr/0533-sso-oidc-require-tenant-context.md`,
  `docs/adr/0591-gateway-http-legacy-admin-route-boundary.md`,
  `docs/adr/0615-domain-role-claim-empty-guard.md`,
  `docs/adr/0656-role-claim-exact-boundary.md`,
  `docs/adr/0669-scim-principal-role-exact-boundary.md`,
  `docs/adr/0670-gateway-dashboard-exact-identifier-inputs.md`,
  `docs/adr/0671-gateway-openapi-exact-identifier-contract.md`,
  `docs/adr/0674-sso-oidc-principal-exact-boundary.md`,
  `docs/adr/0675-redis-key-store-exact-identifier-load.md`,
  `docs/adr/0676-sql-key-store-exact-identifier-arrays.md`,
  `docs/adr/0677-gateway-admin-token-role-fail-closed.md`,
  `docs/adr/0678-gateway-admin-role-values-exact.md`,
  `docs/adr/0679-gateway-admin-key-prefix-scopes-exact.md`,
  `docs/adr/0680-gateway-governance-scope-identifiers-exact.md`,
  `docs/adr/0681-gateway-model-allow-lists-exact.md`,
  `docs/adr/0682-gateway-route-alias-model-identifiers-exact.md`,
  `docs/adr/0683-gateway-route-alias-names-exact.md`,
  `docs/adr/0684-gateway-configured-credential-names-exact.md`,
  `docs/adr/1044-stored-virtual-key-name-exact-load.md`,
  `docs/adr/1045-stored-virtual-key-scope-exact-load.md`,
  `docs/adr/1046-stored-scim-authz-fields-exact-load.md`,
  `docs/adr/1049-active-scim-store-exact-load.md`,
  `docs/adr/1050-stored-virtual-key-model-scope-exact-load.md`,
  `docs/adr/1051-stored-virtual-key-token-hash-exact-load.md`,
  `docs/adr/1052-file-key-store-disabled-null-fail-closed.md`,
  `docs/adr/1053-file-key-store-numeric-null-fail-closed.md`,
  `docs/adr/1054-file-key-store-scope-null-fail-closed.md`,
  `docs/adr/0685-gateway-credential-env-refs-exact.md`,
  `docs/adr/0686-gateway-sso-proxy-token-env-ref-exact.md`,
  `docs/adr/0687-gateway-guardrail-webhook-env-ref-exact.md`,
  `docs/adr/0688-gateway-observability-env-ref-exact.md`,
  `docs/adr/0689-gateway-state-url-env-refs-exact.md`,
  `docs/adr/1031-gateway-state-url-values-exact-boundary.md`,
  `docs/adr/1032-gateway-accounting-topology-env-exact-boundary.md`,
  `docs/adr/1033-gateway-configured-secret-values-exact-boundary.md`,
  `docs/adr/0697-gateway-route-strategy-exact-boundary.md`,
  `docs/adr/0698-guardrail-webhook-phase-exact-boundary.md`,
  `docs/adr/0699-observability-http-schema-exact-boundary.md`,
  `docs/adr/0700-observability-sink-exact-boundary.md`,
  `docs/adr/0701-guardrail-keyword-values-exact-boundary.md`, and
  `docs/adr/0702-observability-call-id-header-exact-boundary.md`, and
  `docs/adr/0703-observability-http-endpoint-exact-boundary.md`, and
  `docs/adr/0704-guardrail-webhook-url-exact-boundary.md`, and
  `docs/adr/0705-observability-jsonl-path-exact-boundary.md`, and
  `docs/adr/0706-gateway-sqlite-state-path-exact-boundary.md`, and
  `docs/adr/0707-gateway-oidc-url-exact-boundary.md`, and
  `docs/adr/0708-gateway-sso-header-claim-exact-boundary.md`, and
  `docs/adr/0709-gateway-provider-exact-boundary.md`, and
  `docs/adr/0710-gateway-base-url-exact-boundary.md`, and
  `docs/adr/0711-gateway-listen-addr-exact-boundary.md`,
  `docs/adr/0712-runtime-policy-path-exact-boundary.md`, and
  `docs/adr/1038-redis-key-store-numeric-field-fail-closed.md`, and
  `docs/adr/1039-redis-key-store-boolean-field-fail-closed.md`, and
  `docs/adr/1040-redis-key-store-json-array-fail-closed.md`, and
  `docs/adr/1041-sql-key-store-json-array-fail-closed.md`, and
  `docs/adr/1042-sqlite-key-store-boolean-field-fail-closed.md`, and
  `docs/adr/1043-sql-key-store-numeric-field-fail-closed.md`, and
  `docs/adr/0822-domain-security-principal-debug-redaction.md`, and
  `docs/adr/0823-domain-security-role-mapper-debug-redaction.md`.
- **Remaining gap:** Replace the remaining hand-written legacy admin execution
  branches with application/control-plane use-case calls before enabling
  multi-tenant production mode. The dedicated `prodex-control-plane` binary can
  now exercise control-plane HTTP route, audit, audit-correlation, audit span, idempotency, precondition, and page-request planning for those
  paths through the shared application and gateway-http boundaries instead of
  duplicating that logic in ad hoc scripts. The dedicated
  `prodex-control-plane` binary now also carries focused one-shot route-planning
  coverage for mounted SCIM read/create/update/delete control-plane HTTP paths, so
  those lifecycle routes stay pinned to the shared HTTP planner alongside the
  compatibility adapter regressions, and SCIM wrong-method plus missing-idempotency
  failures now also stay pinned to the shared stable error envelopes while the
  remaining SCIM user-lifecycle selectors now also carry exact `scim_user_*`
  operation names. The dedicated `prodex-control-plane`
  binary now also pins role-binding grant/revoke route planning through the
  same shared HTTP/application boundaries, and role-binding wrong-method plus
  missing-idempotency failures now also stay pinned to the shared stable error
  envelopes. The dedicated `prodex-control-plane`
  binary also pins service-identity create and provider-credential rotate route
  planning through those same shared boundaries, and service-identity plus provider-credential
  wrong-method plus missing-idempotency failures now also stay pinned to the
  shared stable error envelopes while those remaining shared selectors now also
  carry operation-specific names and exact failure names, including explicit
  missing-idempotency-key helpers. The dedicated `prodex-control-plane` binary
  also pins tenant create/update and user-invite
  route planning through the same shared boundaries, and tenant wrong-method
  plus missing-idempotency failures now also stay pinned to the shared stable
  error envelopes while those remaining shared selectors now also carry
  operation-specific names and exact failure names, and user-invite wrong-method plus missing-idempotency
  failures now also stay pinned there while those helper selectors now also
  carry exact failure names. The dedicated
  `prodex-control-plane` binary now also pins policy-publish route planning
  through the same shared boundaries, and policy-publish wrong-method plus
  missing-idempotency failures now also stay pinned to the shared stable error
  envelopes while those helper selectors now also carry exact failure names.
  The dedicated `prodex-control-plane` binary now also pins
  audit-retention-purge route planning plus wrong-method and missing-idempotency
  failures to those same shared stable envelopes while those helper selectors
  now also carry exact failure names. The dedicated `prodex-control-plane`
  binary now also pins audit-export route planning and wrong-method route
  rejection to those same shared boundaries while those helper selectors now
  also carry exact failure names. The dedicated
  `prodex-control-plane` binary now also pins virtual-key read route planning
  and wrong-method rejection alongside the existing create/rotate-secret
  coverage, and virtual-key delete now also stays pinned with its stable
  missing-idempotency envelope. The dedicated `prodex-control-plane` binary
  now also pins gateway-admin read route planning and wrong-method rejection to
  those same shared boundaries. The dedicated `prodex-control-plane` binary now
  also pins budget-update route planning and wrong-method rejection in a
  dedicated one-shot family instead of relying only on the shared generic
  audit/idempotency/precondition boundary tests, and the remaining
  budget-update shared helpers now also carry exact audit-required,
  trace-context, and wrong-method failure names.
  The dedicated
  `prodex-control-plane` binary now also pins billing-read route planning and
  wrong-method rejection in a dedicated one-shot family instead of relying only
  on the shared page-request boundary tests, and the remaining billing-read
  shared page-request selectors now also carry exact query/cursor names while
  the billing-read wrong-method helper now also carries the exact failure name. The dedicated
  `prodex-control-plane` binary now also names configuration-publish route
  planning, wrong-method rejection, and missing-idempotency failure as their
  own dedicated one-shot family instead of leaving that evidence under generic
  test wrappers, and the wrong-method helper now also carries the exact
  failure name. The dedicated
  `prodex-control-plane` binary now also pins virtual-key update route
  planning and wrong-method rejection in a dedicated one-shot family instead of
  relying only on the shared precondition boundary tests, and the remaining
  budget-update / virtual-key-update / virtual-key-create shared boundary
  selectors now also carry operation-specific names instead of generic
  wrappers while the virtual-key-update helpers now also carry exact
  precondition/entity-tag/route-invalid names. The dedicated
  `prodex-control-plane` binary also carries focused one-shot route-planning
  coverage for mounted virtual-key create and rotate-secret control-plane HTTP
  paths, and virtual-key wrong-method plus missing-idempotency failures now
  also stay pinned to the shared stable error envelopes even before the
  long-lived admin adapter is fully retired. The dedicated
  `prodex-control-plane` binary now also uses the shared
  `prodex-application` route-error response planner for one-shot HTTP planning
  failures instead of adapting `prodex-gateway-http` route errors directly at
  the composition root. Mounted admin route rejection now also uses the shared
  `prodex-application` route-error response planner rather than adapting
  `prodex-gateway-http` errors directly in the compatibility adapter. Legacy
  mounted gateway admin write-role rejection now also delegates allow/deny
  planning through `plan_application_control_plane` instead of branching on
  `RuntimeGatewayAdminRole::can_write()` in the adapter directly. Legacy
  mounted gateway admin virtual-key create and rotate-secret paths now also
  preflight through `plan_application_virtual_key_lifecycle` on
  SQLite/Postgres compatibility backends before mutating local state, while
  file/Redis compatibility behavior remains unchanged. Focused compatibility
  regressions now pin both SQLite and Postgres rotate-secret behavior for that
  mounted admin path. Legacy
  mounted gateway SCIM create/update/delete paths now also preflight through
  `plan_application_user_lifecycle` on SQLite/Postgres compatibility backends
  before mutating local state, and focused compatibility regressions now pin
  the mounted SCIM create/update/delete shapes on both SQLite and Postgres
  backends, so one actual admin mutation family now reuses the shared
  lifecycle planner without changing file/Redis compatibility behavior. Legacy mounted gateway admin ledger reads now also delegate pagination query
  validation through the shared `prodex-application` control-plane page-request
  planner instead of parsing query state in the adapter directly, shrinking one
  more adapter-local branch.

### 2. Root Token Must Not Bypass Data-Plane Authorization

- **Evidence:** `crates/prodex-domain/src/security.rs`
  (`CredentialScope`, `AuthorizationDecision`) and
  `crates/prodex-authz/src/lib.rs` (`BoundaryKind`,
  `authorize_boundary_resource`, and `plan_authorization_error_response`).
- **Risk:** Treating a gateway or root token as an inference credential can
  bypass tenant policy, budget, and audit controls.
- **Regression coverage:** `crates/prodex-domain/tests/security.rs`,
  `crates/prodex-authz/tests/boundary.rs`,
  `crates/prodex-gateway-core/tests/admission.rs`, and
  `scripts/ci/auth-boundary-guard.mjs`.
- **Local verification:** `npm run ci:gateway-security-smoke` exercises the
  focused gateway admin auth and usage suites after control-plane/data-plane
  credential-boundary or gateway usage/auth changes.
- **Implemented fix or boundary:** Data-plane and control-plane credential
  scopes are distinct at the domain primitive and boundary layers; break-glass
  access is separate, exact-scope, exact-principal-kind, short-lived, reasoned,
  and auditable. The auth boundary guard now also checks that `prodex-authz`
  keeps break-glass on an explicit boundary and compares principal scope to
  exact boundary requirements. Authz adapters can resolve data-plane,
  control-plane, and break-glass requirements through a fail-closed universal
  boundary resolver while the normal control-plane resolver continues to reject
  break-glass requirements. Gateway SSO proxy-token authentication now compares
  token material exactly instead of trim-normalizing padded control-plane
  credential headers. Domain authorization error debug output redacts expected
  and actual scope or role values while preserving failure shape. Domain
  authorization display output uses one generic denied message for scope and
  role failures.
  Authorization requirement debug output redacts resource, action, scope, and
  role policy values. Authz display output now uses one generic denied message
  for scope, role, principal-kind, and tenant-access failures, so local
  stringified errors do not expose authorization topology. Control-plane
  authorization display output uses the same single denied-message pattern for
  scope, role, resource, tenant, and break-glass failures.
  Gateway admission display output also uses generic
  wording for unavailable authorization boundaries and tenant mismatches so
  local stringified errors do not expose admission topology. Gateway
  authorization-boundary resolver display output uses the same generic
  temporary-unavailable wording.
- **ADRs:** `docs/adr/0003-gateway-admin-openapi-and-scope.md`,
  `docs/adr/0080-authz-boundary-crate.md`,
  `docs/adr/0125-authz-stable-error-responses.md`,
  `docs/adr/0126-control-plane-stable-authorization-errors.md`,
  `docs/adr/0839-domain-authorization-error-debug-redaction.md`,
  `docs/adr/0875-domain-authorization-display-redaction.md`,
  `docs/adr/0840-domain-authorization-requirement-debug-redaction.md`,
  `docs/adr/0864-authz-display-topology-redaction.md`,
  `docs/adr/0865-control-plane-authorization-display-topology-redaction.md`,
  `docs/adr/0860-gateway-core-admission-display-redaction.md`,
  `docs/adr/0863-gateway-core-authorization-boundary-display-redaction.md`,
  `docs/adr/0127-config-publication-stable-error-responses.md`,
  `docs/adr/0128-control-plane-configuration-publication-error-boundary.md`,
  `docs/adr/0129-application-configuration-publication-error-boundary.md`,
  `docs/adr/0130-application-usage-reconciliation-error-boundary.md`,
  `docs/adr/0131-application-expired-reservation-recovery-error-boundary.md`,
  `docs/adr/0132-application-recovery-lease-release-error-boundary.md`,
  `docs/adr/0133-application-runtime-topology-error-boundary.md`,
  `docs/adr/0134-application-control-plane-audit-error-boundary.md`,
  `docs/adr/0135-provider-spi-stable-error-responses.md`,
  `docs/adr/0136-gateway-admission-stable-error-responses.md`,
  `docs/adr/0137-gateway-usage-reconciliation-stable-error-responses.md`,
  `docs/adr/0138-gateway-expired-recovery-stable-error-responses.md`,
  `docs/adr/0139-application-data-plane-admission-error-boundary.md`,
  `docs/adr/0140-application-usage-reconciliation-gateway-error-boundary.md`,
  `docs/adr/0141-application-expired-recovery-gateway-error-boundary.md`,
  `docs/adr/0142-redis-stable-error-responses.md`,
  `docs/adr/0143-sql-storage-stable-error-responses.md`,
  `docs/adr/0144-core-storage-stable-error-responses.md`,
  `docs/adr/0145-gateway-core-storage-error-boundary.md`,
  `docs/adr/0146-observability-stable-error-responses.md`,
  `docs/adr/0147-trace-context-stable-error-responses.md`,
  `docs/adr/0148-secret-store-stable-error-responses.md`,
  `docs/adr/0149-session-resolve-stable-error-responses.md`,
  `docs/adr/0150-config-cache-window-stable-error-responses.md`,
  `docs/adr/0151-deployment-security-stable-error-responses.md`,
  `docs/adr/0152-api-version-stable-error-responses.md`,
  `docs/adr/0153-pagination-cursor-stable-error-responses.md`,
  `docs/adr/0154-concurrency-precondition-stable-error-responses.md`,
  `docs/adr/0155-precondition-token-stable-error-responses.md`,
  `docs/adr/0156-idempotency-stable-error-responses.md`,
  `docs/adr/0157-migration-stable-error-responses.md`,
  `docs/adr/0158-backup-restore-stable-error-responses.md`,
  `docs/adr/0159-minimum-enterprise-slo-baseline.md`,
  `docs/adr/0160-health-probe-stable-responses.md`,
  `docs/adr/0831-domain-health-probe-debug-redaction.md`,
  `docs/adr/0161-domain-security-stable-error-responses.md`,
  `docs/adr/0162-policy-stable-error-responses.md`,
  `docs/adr/0163-identity-jwks-stable-error-responses.md`,
  `docs/adr/0164-capability-negotiation-stable-error-responses.md`,
  `docs/adr/0165-rate-limit-stable-error-responses.md`,
  `docs/adr/0166-audit-stable-error-responses.md`,
  `docs/adr/0167-domain-secret-stable-error-responses.md`,
  `docs/adr/0168-domain-trace-id-stable-error-responses.md`,
  `docs/adr/0169-slo-alert-stable-responses.md`,
  `docs/adr/0170-domain-accounting-stable-error-responses.md`,
  `docs/adr/0171-domain-id-parse-stable-error-responses.md`,
  `docs/adr/0172-domain-telemetry-attribute-stable-error-responses.md`,
  `docs/adr/0173-domain-error-code-stable-validation-responses.md`,
  `docs/adr/0174-domain-audit-action-stable-validation-responses.md`,
  `docs/adr/0175-domain-audit-resource-kind-stable-validation-responses.md`,
  `docs/adr/0176-domain-audit-reason-code-stable-validation-responses.md`,
  `docs/adr/0177-domain-audit-resource-id-stable-validation-responses.md`,
  `docs/adr/0178-domain-audit-digest-stable-validation-responses.md`,
  `docs/adr/0179-domain-audit-timestamp-stable-validation-responses.md`,
  `docs/adr/0180-domain-audit-outcome-stable-validation-responses.md`,
  `docs/adr/0181-domain-audit-export-format-stable-validation-responses.md`,
  `docs/adr/0182-domain-audit-time-range-stable-validation-responses.md`,
  `docs/adr/0183-domain-audit-page-limit-stable-validation-responses.md`,
  `docs/adr/0184-domain-audit-sort-order-stable-validation-responses.md`,
  `docs/adr/0185-domain-audit-query-scope-stable-validation-responses.md`,
  `docs/adr/0186-domain-audit-query-plan.md`, and
  `docs/adr/0234-domain-break-glass-exact-scope.md`, and
  `docs/adr/0616-break-glass-principal-kind-boundary.md`, and
  `docs/adr/0660-authz-universal-boundary-resolver.md`, and
  `docs/adr/0673-sso-proxy-token-exact-boundary.md`.
- **Remaining gap:** Legacy composition paths must be converted to the
  boundary API before root-token compatibility code is removed.

### 3. Process-Local IDs Are Not Globally Unique

- **Evidence:** `crates/prodex-domain/src/ids.rs` defines typed UUIDv7 IDs for
  `TenantId`, `PrincipalId`, `RequestId`, `CallId`, `ReservationId`,
  `VirtualKeyId`, `PolicyRevisionId`, and `AuditEventId`.
  Typed domain ID debug output preserves the ID type while redacting the raw
  UUID value; see `docs/adr/0804-domain-id-debug-redaction.md`.
  Domain ID parse error display output redacts identifier kind while keeping
  response planning typed; see
  `docs/adr/0848-domain-id-parse-display-kind-redaction.md`.
  Domain ID parse error debug output redacts identifier kind while keeping
  response planning typed; see
  `docs/adr/0844-domain-id-parse-error-debug-redaction.md`.
- **Risk:** `AtomicU64` or process-local IDs collide across replicas and can
  corrupt accounting, tracing, or idempotency.
- **Regression coverage:** `crates/prodex-domain/tests/ids.rs`,
  `scripts/ci/domain-boundary-guard.mjs`,
  `scripts/ci/enterprise-id-boundary-guard.mjs` including its self-tests, and
  runtime request ID characterization tests in `crates/prodex-app/src/lib.rs`,
  plus
  `gateway_virtual_key_usage_is_persisted_and_visible_to_admin_endpoint` for
  gateway ledger UUID/scope JSON, CSV, and summary output.
- **Local verification:** `cargo test -q -p prodex-domain ids -- --test-threads=1`
  plus `npm run ci:enterprise-id-boundary-guard` run the guard self-test plus
  workspace scan to verify typed UUIDv7 domain IDs and keep enterprise crates
  from reintroducing process-local counter identities.
- **Implemented fix or boundary:** Enterprise domain use cases expose typed,
  globally unique identifiers and keep ID generation independent from HTTP,
  storage, and provider adapters; enterprise boundary crates are guarded
  against reintroducing `AtomicU64` or monotonic `fetch_add` identity
  generation. External guardrail webhook payloads now emit typed UUIDv7
  `request_id` and `call_id` values instead of using the process-local runtime
  request sequence as a cross-service request identifier. Serialized gateway
  spend observability events and structured spend log lines also expose typed
  UUIDv7 `request_id` generated at admission while keeping the numeric runtime
  sequence explicitly marked as legacy. New file/Redis gateway billing ledger entries also expose
  optional typed UUIDv7 `request_id` generated at admission for API and CSV
  consumers while retaining the numeric legacy `request` field for
  compatibility. New file/Redis ledger entries and fresh SQL-backed ledger
  schemas also include admission-time tenant and governance scope snapshots so
  billing summaries do not depend on mutable key configuration for historical
  rows. Scoped admin ledger reads now authorize from those row snapshots before
  falling back to the current key store for legacy rows. Gateway compatibility
  key stores now also persist canonical typed `virtual_key_id` values for
  admin-managed keys across file, SQLite, PostgreSQL, and Redis backends so
  durable reservation wiring can target tenant-owned key identity instead of
  display-name-only compatibility state. Newly created gateway SCIM user
  resource IDs now use typed UUIDv7 `PrincipalId` values instead of virtual-key
  token-shaped compatibility strings, and SCIM create defaults an omitted target
  `user_id` to that generated typed resource ID when no narrower admin user
  scope is present. Anthropic
  compatibility runtime tokens now use UUIDv7 values instead of process ID,
  nanoseconds, and `AtomicU64` sequence material. Runtime proxy internal `u64`
  request IDs now derive their high bits from fresh UUIDv7 `RequestId` entropy
  per request and keep the atomic sequence only as a local diagnostic suffix.
- **ADRs:** `docs/adr/0001-gateway-request-id-entropy.md`,
  `docs/adr/0222-enterprise-id-boundary-guard.md`,
  `docs/adr/0235-enterprise-id-guard-self-test.md`,
  `docs/adr/0537-guardrail-webhook-request-id-uuidv7.md`, and
  `docs/adr/0538-gateway-spend-request-id-uuidv7.md`, and
  `docs/adr/0539-gateway-ledger-request-id-uuidv7.md`, and
  `docs/adr/0541-gateway-ledger-scope-snapshot.md`, and
  `docs/adr/0543-ledger-scope-snapshot-read-authz.md`, and
  `docs/adr/0546-ledger-request-id-from-admission.md`, and
  `docs/adr/0547-spend-request-id-from-admission.md`, and
  `docs/adr/0586-anthropic-runtime-token-uuidv7.md`, and
  `docs/adr/0587-sql-ledger-request-scope-snapshot.md`, and
  `docs/adr/0713-runtime-request-id-per-request-entropy.md`, and
  `docs/adr/0980-gateway-compatibility-virtual-key-id.md`, and
  `docs/adr/1047-scim-resource-id-principal-id.md`, and
  `docs/adr/1048-scim-create-target-principal-default.md`, and
  `docs/adr/0978-gateway-compatibility-schema-versioned-migrations.md`.
- **Remaining gap:** The legacy numeric `request` / `legacy_request_sequence`
  field still remains in compatibility payloads and logs where upstream or
  existing operator workflows consume it. Typed UUIDv7 IDs are now the primary
  cross-process identifiers, but the numeric compatibility field cannot be
  removed until those external consumers are retired or versioned.

### 4. Ledger Uniqueness Must Survive Multi-Replica Writes

- **Evidence:** `crates/prodex-domain/src/accounting.rs`
  (`LedgerEvent`, reservation idempotency) plus storage SQL in
  `crates/prodex-storage-postgres/src/lib.rs` and
  `crates/prodex-storage-sqlite/src/lib.rs`.
- **Risk:** Weak unique constraints can collide or allow duplicate charges when
  multiple replicas commit usage concurrently.
- **Regression coverage:** `crates/prodex-domain/tests/accounting.rs`,
  `crates/prodex-storage/tests/storage_contract.rs`,
  `crates/prodex-storage-postgres/tests/postgres_storage.rs`,
  `crates/prodex-storage-sqlite/tests/sqlite_storage.rs`,
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_keys.rs`,
  and
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_state.rs`, and
  `scripts/ci/storage-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-storage multi_replica_accounting -- --test-threads=1`
  exercises the concurrency-evidence gate,
  `cargo test -q -p prodex-storage atomic_reservation -- --test-threads=1`
  exercises tenant-qualified reservation idempotency,
  `PRODEX_TEST_POSTGRES_URL=postgres://... cargo test -q -p prodex-storage-postgres postgres_atomic_reservation_allows_only_one_concurrent_claim_per_budget_scope -- --test-threads=1`
  exercises the concrete Postgres adapter against a real database, and
  `PRODEX_TEST_POSTGRES_URL=postgres://... npm run ci:storage-boundary-guard`
  runs the guard self-test plus workspace scan and keeps that optional Postgres
  execution proof wired into the storage boundary guard when a local database
  URL is available, and
  `npm run ci:storage-postgres-proof` provisions a temporary local Postgres
  fixture automatically when Docker and `psql` are available.
- **Implemented fix or boundary:** Ledger events are tenant-scoped and use
  reservation or call identifiers as idempotency keys; storage plans include
  tenant-qualified uniqueness. Multi-replica accounting evidence must now prove
  it ran against the same Postgres/Redis topology as the runtime spec and every
  required concurrency check before production readiness can claim the
  accounting gate passed. The compatibility file-backed gateway billing ledger
  now dedupes retried request-phase rows by
  `(call_id, lower(key_name), phase)` while holding its existing file lock,
  streaming existing JSONL ids instead of materializing full ledger records, and
  file ledger reads stream the requested tail window instead of reading the full
  JSONL file into memory before applying a limit. File-backed response
  reconciliation also streams JSONL into a temporary file and renames only when
  a matching `call_id` row changes. This matches SQL and Redis idempotency
  semantics without changing the JSONL format. SQLite execution coverage now
  also proves that one durable reservation wins and one is rejected under a
  shared budget scope at both the storage-plan boundary and the gateway adapter
  boundary. Postgres execution coverage now proves the same winner/loser budget
  race against a real Postgres database through the adapter's atomic
  reservation statement and RLS tenant-context setup. Billing ledger range validation
  now keeps query timestamps out of storage planner error strings while
  preserving structured enum fields for internal diagnostics. Billing ledger
  page-limit validation also keeps rejected limit values out of planner error
  strings and uses one stable display message for zero or oversized limits.
- **ADRs:** `docs/adr/0005-gateway-ledger-idempotency.md`,
  `docs/adr/0092-postgres-storage-rls-and-migration-boundary.md`,
  `docs/adr/0094-sqlite-storage-migration-boundary.md`,
  `docs/adr/0246-multi-replica-accounting-evidence-plan.md`,
  `docs/adr/0317-multi-replica-accounting-evidence-topology.md`, and
  `docs/adr/0542-file-ledger-call-id-dedupe.md`, and
  `docs/adr/0714-storage-billing-ledger-range-error-redaction.md`, and
  `docs/adr/0715-storage-billing-ledger-limit-error-redaction.md`, and
  `docs/adr/0905-storage-billing-ledger-limit-display-redaction.md`.
- **Remaining gap:** Replace the single-node temporary Postgres fixture with a
  repeatable shared-topology CI/readiness fixture so the same proof covers the
  full multi-replica enterprise readiness gate.

### 5. Read-Modify-Write Budget Updates Can Lose Usage

- **Evidence:** `crates/prodex-storage/src/lib.rs`
  (`AtomicReservationPlan`), `crates/prodex-storage-postgres/src/lib.rs`
  (atomic reservation DML), and `crates/prodex-storage-redis/src/lib.rs`
  (Lua operation plans).
- **Risk:** A read-modify-write update can lose usage under concurrent gateway
  replicas and permit budget overshoot or dropped billing events.
- **Regression coverage:** `crates/prodex-storage/tests` through the storage
  boundary, `crates/prodex-storage-postgres/tests/postgres_storage.rs`, and
  `crates/prodex-storage-redis/tests/redis_storage.rs`.
- **Local verification:** `cargo test -q -p prodex-storage atomic_reservation -- --test-threads=1`
  exercises atomic reservation planning,
  `cargo test -q -p prodex-domain accounting -- --test-threads=1` exercises
  reservation/reconciliation invariants, and
  `npm run ci:storage-boundary-guard` runs the guard self-test plus workspace
  scan to keep the storage boundary wired into local CI.
- **Implemented fix or boundary:** Reservation, commit, release, and expiry
  are planned as atomic operations with explicit idempotency. Redis limiter Lua
  results are normalized by `plan_redis_rate_limit_result` so adapters share one
  allow/deny and TTL interpretation. Domain commit and reconciliation reject
  actual usage above the reservation and committed usage overflow before
  updating the snapshot so post-provider accounting cannot silently saturate
  usage totals. Domain reservation rejects zero estimates before upstream
  dispatch so accounting cannot create no-op reservations that bypass budget
  admission. Domain reservation records also reject zero reserved usage so
  direct recovery callers cannot create no-op abandoned-reservation records.
  Domain commit rejects zero actual usage so direct commit callers cannot create
  no-op commits that only release reservations. Domain commit, reconciliation,
  and expired-reservation recovery reject reserved-balance underflow instead of
  silently saturating, so duplicate or stale accounting attempts cannot hide a
  missing reservation. Checked reservation commits also require the committed
  reserved amount to match the original reservation estimate before mutating
  accounting state. Domain reservation records reject zero TTLs so
  abandoned-reservation recovery cannot create immediately expired no-window
  reservations. Domain
  rate-limit evaluation rejects zero-request admissions so
  admission and atomic Redis mutation planning share the same fail-closed
  increment contract. Domain rate-limit evaluation also rejects zero-capacity or
  zero-window rules before a request can be admitted with an invalid reset
  horizon. Oversized rate-limit windows that would overflow millisecond reset
  computation also fail closed instead of relying on saturating arithmetic.
  Atomic rate-limit update planning rejects reset windows that are already
  expired at request time, so distributed limiter adapters cannot create
  non-expiring or immediately-expired increments from stale allowance data.
  Rate-limit rule debug output redacts configured request ceilings and windows;
  see `docs/adr/0845-domain-rate-limit-rule-debug-redaction.md`.
  Rate-limit debug output now redacts tenant IDs, virtual-key IDs, usage counts,
  windows, remaining capacity, and atomic update internals while preserving
  state shape for diagnostics. Rate-limit response debug output redacts typed
  retry-after timing while preserving stable response shape.
  Reservation commit error displays now keep tenant IDs, call IDs, reservation
  IDs, and usage amounts out of stringified domain errors while preserving
  structured enum fields for trusted diagnostics. Reservation reconciliation
  error displays also keep usage amounts out of stringified domain errors.
  Core storage reservation and usage-reconciliation display output also uses
  generic request-invalid wording for tenant mismatches; see
  `docs/adr/0871-storage-reservation-display-redaction.md`.
  Core storage billing query, expired recovery, append-only audit, retention
  purge, and audit export display output also uses generic request-invalid
  wording for tenant mismatches; see
  `docs/adr/0872-storage-query-audit-display-redaction.md`.
  Core storage lifecycle and mutation display output also uses generic
  request-invalid wording for tenant mismatches; see
  `docs/adr/0873-storage-lifecycle-display-redaction.md`.
  Core storage secret-reference and idempotency display output also uses
  generic request-invalid wording for tenant mismatches; see
  `docs/adr/0874-storage-secret-idempotency-display-redaction.md`.
  Reservation commit mismatch debug output now preserves mismatch shape while
  redacting tenant IDs, call IDs, reservation IDs, and usage amounts; see
  `docs/adr/0764-domain-reservation-commit-mismatch-debug-redaction.md`.
  Reservation commit error debug output preserves commit failure shape while
  redacting nested mismatch details and usage amounts; see
  `docs/adr/0765-domain-reservation-commit-error-debug-redaction.md`.
  Reservation recovery error debug output preserves recovery failure shape while
  redacting tenant IDs and usage amounts; see
  `docs/adr/0766-domain-reservation-recovery-error-debug-redaction.md`.
  Reservation reconciliation error debug output preserves reconciliation
  failure shape while redacting usage amounts; see
  `docs/adr/0767-domain-reservation-reconciliation-error-debug-redaction.md`.
  Budget rejection displays keep requested and available usage amounts out of
  stringified domain errors.
  Budget rejection debug output preserves rejection reason while redacting
  requested and available usage amounts; see
  `docs/adr/0768-domain-budget-rejection-debug-redaction.md`.
  Reservation request debug output preserves request shape while redacting
  tenant IDs, call IDs, reservation IDs, and usage estimates; see
  `docs/adr/0769-domain-reservation-request-debug-redaction.md`.
  Reservation record debug output preserves record shape while redacting tenant
  IDs, call IDs, reservation IDs, reserved usage, and recovery timestamps; see
  `docs/adr/0770-domain-reservation-record-debug-redaction.md`.
  Reservation commit debug output preserves commit shape while redacting tenant
  IDs, call IDs, reservation IDs, reserved usage, and actual usage; see
  `docs/adr/0771-domain-reservation-commit-debug-redaction.md`.
  Reservation reconciliation debug output preserves reason and release-event
  presence while redacting nested commit and ledger event details; see
  `docs/adr/0772-domain-reservation-reconciliation-debug-redaction.md`.
  Ledger event debug output preserves event kind while redacting tenant IDs,
  call IDs, reservation IDs, and usage amounts; see
  `docs/adr/0773-domain-ledger-event-debug-redaction.md`.
  Usage amount debug output preserves amount field shape while redacting token
  and cost values; see `docs/adr/0774-domain-usage-amount-debug-redaction.md`.
  Budget snapshot debug output preserves snapshot shape while redacting reserved
  and committed usage totals; see
  `docs/adr/0775-domain-budget-snapshot-debug-redaction.md`.
  Budget limit debug output preserves limit shape while redacting token and cost
  ceilings; see `docs/adr/0776-domain-budget-limit-debug-redaction.md`.
- **ADRs:** `docs/adr/0083-storage-boundary-contract.md`,
  `docs/adr/0092-postgres-storage-rls-and-migration-boundary.md`,
  `docs/adr/0093-redis-storage-rate-limit-boundary.md`,
  `docs/adr/0236-redis-rate-limit-result-contract.md`, and
  `docs/adr/0411-domain-reservation-reconciliation-overflow.md`, and
  `docs/adr/0610-domain-reservation-zero-estimate-guard.md`, and
  `docs/adr/0611-domain-reservation-underflow-guard.md`, and
  `docs/adr/0612-domain-reservation-commit-amount-mismatch-guard.md`, and
  `docs/adr/0613-domain-reservation-zero-actual-commit-guard.md`, and
  `docs/adr/0614-domain-reservation-zero-ttl-guard.md`, and
  `docs/adr/0632-domain-reservation-record-zero-reserved-guard.md`, and
  `docs/adr/0608-domain-rate-limit-zero-request-guard.md`, and
  `docs/adr/0609-domain-rate-limit-invalid-rule-guard.md`, and
  `docs/adr/0630-domain-rate-limit-expired-window-guard.md`, and
  `docs/adr/0631-domain-rate-limit-window-overflow-guard.md`, and
  `docs/adr/0721-domain-rate-limit-debug-redaction.md`, and
  `docs/adr/0845-domain-rate-limit-rule-debug-redaction.md`, and
  `docs/adr/0841-domain-rate-limit-response-debug-redaction.md`, and
  `docs/adr/0716-domain-reservation-commit-error-redaction.md`, and
  `docs/adr/0717-domain-reservation-reconciliation-error-redaction.md`, and
  `docs/adr/0718-domain-budget-rejection-error-redaction.md`.
- **Remaining gap:** Replace legacy gateway ledger mutations with the durable
  reservation backend behind the application boundary.

### 6. Admission Must Not Use Only Local In-Memory Usage

- **Evidence:** `crates/prodex-gateway-core/src/lib.rs`
  (`plan_data_plane_admission`), `crates/prodex-application/src/lib.rs`
  (`plan_data_plane_request`), and the legacy compatibility gate in
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_keys.rs`.
- **Risk:** Local-only admission accepts requests that exceed tenant budget
  when traffic is split across replicas.
- **Regression coverage:** `crates/prodex-gateway-core/tests/admission.rs`,
  `crates/prodex-application/tests/application_boundary.rs`,
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_budget.rs`, and
  `scripts/ci/gateway-core-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-gateway-core admission -- --test-threads=1`
  exercises gateway admission planning,
  `cargo test -q -p prodex-application application_atomic_reservation -- --test-threads=1`
  exercises durable reservation storage selection through the application
  boundary,
  `cargo test -q -p prodex-application application_data_plane -- --test-threads=1`
  exercises application-level admission composition, and
  `npm run ci:gateway-core-boundary-guard` runs the guard self-test plus
  workspace scan to keep the gateway-core boundary wired into local CI.
- **Implemented fix or boundary:** The gateway core requires an
  `AtomicReservationCommand` before provider invocation can proceed. The legacy
  compatibility gate now checks budget groups, checks per-key admission, and
  records accepted local usage while holding one usage lock, so same-process
  read-modify-write races do not admit unaccounted requests. Legacy budget
  groups now aggregate only across matching tenant/team/project/user governance
  scope, so same-name budgets in different tenants do not deny each other.
  Budget group IDs are matched exactly instead of being trim-normalized, so a
  padded budget scope cannot merge into a canonical compatibility group.
  `prodex-application` now exposes `plan_application_atomic_reservation` so
  composition roots can select PostgreSQL or SQLite durable reservation DML
  through one application boundary instead of branching on storage adapters in
  `prodex-app`.
  Legacy `prodex gateway` startup now also fails closed when
  `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS=true`: runtime topology
  planning still runs, and the composition root now emits the shared
  `runtime_accounting_verification_invalid` application-level failure envelope
  before refusing to claim multi-replica readiness while request admission
  remains on the local compatibility path instead of the durable reservation
  backend. Accounting topology env values are now exact runtime inputs, so
  padded replica-count or accounting-gate values fail closed instead of being
  trim-normalized into active topology intent. Runtime proxy tuning env values
  now use the same exact numeric boundary for local worker counts, queue
  capacities, active-request caps, lane caps, timeouts, and pressure budgets, so
  malformed explicit hot-path tuning cannot silently fall back to defaults.
  Storage atomic reservation debug output redacts request, accounting, and
  timing fields while preserving typed planner fields; see
  `docs/adr/0923-storage-atomic-reservation-debug-redaction.md`.
- **ADRs:** `docs/adr/0089-gateway-core-admission-boundary.md`,
  `docs/adr/0096-application-use-case-boundary.md`, and
  `docs/adr/0165-rate-limit-stable-error-responses.md`, and
  `docs/adr/0535-local-gateway-admission-critical-section.md`, and
  `docs/adr/0540-budget-group-tenant-scope.md`, and
  `docs/adr/0979-application-atomic-reservation-storage-boundary.md`, and
  `docs/adr/0672-budget-group-id-exact-boundary.md`.
- **Remaining gap:** The legacy `prodex-app` local usage path still remains an
  adapter migration target; request-serving admission still needs to execute
  the durable reservation backend through the new application boundary instead
  of mutating local compatibility usage state.

### 7. Tenant ID Must Be Mandatory and Keyed

- **Evidence:** `crates/prodex-domain/src/security.rs`
  (`TenantContext`), tenant-aware storage key builders in
  `crates/prodex-storage/src/lib.rs`, and tenant predicates in
  `crates/prodex-storage-postgres/src/lib.rs`. PostgreSQL request-path plans
  start with a shared `set_config('prodex.tenant_id', ..., true)` tenant context
  statement before tenant-owned DML.
- **Risk:** Optional or unkeyed tenant IDs allow cross-tenant reads, writes,
  cache collisions, and audit ambiguity.
- **Regression coverage:** `crates/prodex-domain/tests/security.rs`,
  `crates/prodex-storage-postgres/tests/postgres_storage.rs`, and
  `crates/prodex-storage-redis/tests/redis_storage.rs`.
- **Local verification:** `cargo test -q -p prodex-domain security -- --test-threads=1`
  exercises tenant-context and tenant-authorization boundaries,
  `cargo test -q -p prodex-storage-postgres tenant_context -- --test-threads=1`
  exercises SQL tenant-context planning, and
  `cargo test -q -p prodex-storage-redis redis_storage_use -- --test-threads=1`
  exercises Redis tenant-scoped storage-use boundaries.
- **Implemented fix or boundary:** Tenant context is required in domain use
  cases, storage keys include tenant IDs, Postgres plans include Row-Level
  Security plus a shared request tenant-context plan as defense in depth, and
  fresh SQL gateway ledger rows persist admission-time tenant and governance
  scope snapshots. Legacy SCIM provisioning, SSO/OIDC admin authentication, and
  gateway key CRUD now treat tenant/governance scope IDs and allowed key
  prefixes as exact non-empty, whitespace-free values instead of trimming padded
  request input into an authorized or unscoped admin scope. Gateway key names
  and allowed model IDs are also exact control-plane values instead of
  trim-normalized request input. Domain tenant-access denial display/debug
  strings and tenant-wrapped resource authorization debug strings no longer echo
  principal or resource tenant IDs while preserving structured fields for trusted
  diagnostics. Resource authorization display output uses one generic denied
  message for scope, role, and tenant failures.
- **ADRs:** `docs/adr/0092-postgres-storage-rls-and-migration-boundary.md`,
  `docs/adr/0228-postgres-request-tenant-context-plan.md`,
  `docs/adr/0587-sql-ledger-request-scope-snapshot.md`,
  `docs/adr/0665-scim-scope-id-exact-boundary.md`, and
  `docs/adr/0876-domain-resource-authorization-display-redaction.md`, and
  `docs/adr/0666-gateway-key-identifier-model-exact-boundary.md`, and
  `docs/adr/0719-domain-tenant-access-error-redaction.md`.
- **Remaining gap:** The migration guide now defines explicit
  expand/backfill/contract choreography for legacy optional tenant columns.
  Production cutovers still need to execute that guide before any compatibility
  table or nullable tenant shape can be removed.

### 8. DDL Must Not Run While Opening Request-Serving Backends

- **Evidence:** New migration policies live in `crates/prodex-storage/src`,
  `crates/prodex-storage-postgres/src/lib.rs`, and
  `crates/prodex-storage-sqlite/src/lib.rs`. The legacy SQL open helper now
  verifies `prodex_gateway_schema_migrations` by default in
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_backend_connection.rs`.
  `src/bin/prodex-gateway.rs` exposes the external `prodex-gateway migrate`
  entrypoint.
- **Risk:** Request-path DDL can stall traffic, deadlock replicas, and make
  rollbacks unsafe.
- **Regression coverage:** `crates/prodex-storage-postgres/tests/postgres_storage.rs`,
  `crates/prodex-storage-sqlite/tests/sqlite_storage.rs`, and
  `scripts/ci/storage-postgres-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-storage-postgres migration -- --test-threads=1`
  exercises Postgres migration/open-readiness planning,
  `cargo test -q -p prodex-storage-sqlite sqlite_migration -- --test-threads=1`
  exercises the SQLite migration boundary, and
  `npm run ci:storage-postgres-boundary-guard` runs the guard self-test plus
  workspace scan to keep production Postgres storage plans driver-free while
  allowing the test-only `postgres` execution proof, and
  `npm run ci:storage-sqlite-boundary-guard` runs the guard self-test plus
  workspace scan to keep production SQLite storage plans driver-free while
  allowing the test-only `rusqlite` execution coverage.
- **Implemented fix or boundary:** Enterprise storage plans require versioned
  migrations run by external migrators, not by request-serving gateway opens.
  Backend-open readiness planners now require a known current schema version for
  gateway startup/request paths and report zero DDL-eligible migrations there.
  The legacy SQL open helper now rejects fresh SQLite/Postgres compatibility
  state in every build mode unless it was migrated first. `prodex-gateway
  migrate` now ensures both the enterprise storage schema and the legacy
  gateway compatibility schema so the request path no longer needs a bootstrap
  escape hatch. Gateway compatibility tests also prepare migrated SQLite
  fixtures explicitly instead of relying on implicit or explicit request-path
  bootstrap. The compatibility schema itself now advances through explicit
  versioned external migrations instead of one current-state bootstrap blob, so
  future schema growth stays on the same migrator boundary as the enterprise
  storage plan. The production Kubernetes migration Job now runs
  `prodex-gateway migrate --backend postgres --url-env PRODEX_GATEWAY_POSTGRES_URL`
  with migration-only credentials instead of a placeholder shell command.
  Domain migration plan debug output redacts descriptions, lock owners, and
  version strings while preserving execution shape; see
  `docs/adr/0835-domain-migration-plan-debug-redaction.md`. Migration
  compatibility-window debug output redacts target and source version strings;
  see `docs/adr/0836-domain-migration-compatibility-debug-redaction.md`.
- **ADRs:** `docs/adr/0083-storage-boundary-contract.md`,
  `docs/adr/0092-postgres-storage-rls-and-migration-boundary.md`,
  `docs/adr/0094-sqlite-storage-migration-boundary.md`, and
  `docs/adr/0157-migration-stable-error-responses.md`,
  `docs/adr/0241-storage-backend-open-readiness-plan.md`, and
  `docs/adr/0532-gateway-open-schema-bootstrap-escape-hatch.md`, and
  `docs/adr/0588-gateway-external-migrator-command.md`, and
  `docs/adr/0976-gateway-test-schema-bootstrap-explicit.md`, and
  `docs/adr/0978-gateway-compatibility-schema-versioned-migrations.md`, and
  `docs/adr/0982-gateway-compatibility-contract-choreography.md`.
- **Remaining gap:** Compatibility-schema contract choreography is now
  documented. Future destructive migrations still need to execute that
  choreography with release-specific evidence before legacy gateway tables or
  columns can actually be removed.

### 9. Redis Must Not Store Whole-Map Durable Billing JSON

- **Evidence:** `crates/prodex-storage-redis/src/lib.rs` plans tenant-scoped
  keys, atomic Lua operations, cache entries, coordination locks, and an
  explicit storage-use planner that rejects durable usage, ledger, audit, and
  configuration state. The legacy whole-map/list rewrite risk is tracked in
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_redis_ledger.rs`.
- **Risk:** Whole-map JSON under a global lock creates lost updates, long lock
  holds, and non-rebuildable billing state.
- **Regression coverage:** `crates/prodex-storage-redis/tests/redis_storage.rs`
  and `scripts/ci/storage-redis-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-storage-redis redis_storage_use -- --test-threads=1`
  exercises the durable-state prohibition boundary, and
  `npm run ci:storage-redis-boundary-guard` runs the guard self-test plus
  workspace scan to keep the Redis storage boundary wired into local CI.
- **Implemented fix or boundary:** Redis is limited to distributed rate
  limiting, short-lived cache, and coordination primitives that can be rebuilt
  from durable storage; durable tenant-owned state is rejected by the Redis
  planning boundary. Redis rate-limit script outputs are parsed into typed
  decisions rather than adapter-specific tuple handling. Gateway Redis key-store
  and usage data are record/hash based, and the Redis ledger writes entry-scoped
  payloads with atomic `SETNX` dedupe instead of replacing the full ledger list
  under a global lock. Redis usage hash counters now fail closed when present
  fields are malformed or whitespace-bearing instead of silently loading request,
  token, or spend counters as zero.
- **ADRs:** `docs/adr/0093-redis-storage-rate-limit-boundary.md`,
  `docs/adr/0227-redis-durable-state-prohibition-plan.md`, and
  `docs/adr/0236-redis-rate-limit-result-contract.md`, and
  `docs/adr/1037-redis-usage-hash-counter-fail-closed.md`.
- **Remaining gap:** Keep migrating production deployments toward
  Postgres-backed durable accounting plus Redis limiter/cache usage; legacy
  Redis whole-list ledger payloads remain read-only compatibility data.

### 10. OIDC Discovery and JWKS Fetch Must Stay Off Request Path

- **Evidence:** `crates/prodex-domain/src/identity.rs` models JWKS refresh
  decisions and `crates/prodex-authn/src/lib.rs` authenticates against cached
  key snapshots while planning refreshes only for background control-plane mode.
- **Risk:** Network discovery during request authentication can create latency,
  outage coupling, stale-failure ambiguity, and request-path SSRF exposure.
- **Regression coverage:** `crates/prodex-domain/tests/identity.rs`,
  `crates/prodex-authn/tests/oidc.rs`, and
  `crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_auth.rs`,
  and `scripts/ci/auth-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-authn -- --test-threads=1`
  plus `cargo test -q -p prodex-domain identity -- --test-threads=1` cover the
  cached-key, issuer/audience, and request-path JWKS boundary decisions, and
  `npm run ci:auth-boundary-guard` runs the auth guard self-test plus workspace
  scan for forbidden runtime/JWT/OIDC dependencies.
- **Implemented fix or boundary:** Authentication is network-free; stale JWKS
  and unknown key IDs fail closed according to cached snapshot policy. OIDC
  discovery/JWKS fetch targets are rejected in gateway request-path mode and
  emitted only by a background refresh plan. The legacy gateway admin OIDC path
  also reads discovery/JWKS from the startup cache only during request
  authentication and fails closed when startup prefetch did not populate cache.
  Legacy runtime startup now moves that initial OIDC discovery/JWKS prefetch to
  a bounded background worker so proxy launch no longer blocks on the IdP;
  request authentication waits only for that bounded local prefetch attempt to
  finish and still does not perform discovery/JWKS network I/O itself.
  Configured JWKS refresh URLs must use HTTPS; plaintext HTTP, local files,
  empty, non-printable, non-ASCII, over-2048-byte, and whitespace-containing
  values are rejected before any transport adapter can fetch keys. Legacy
  startup OIDC discovery/JWKS prefetches now use an explicit bounded request
  timeout via `PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS`.
  The legacy gateway now also runs a bounded background discovery/JWKS refresh
  loop after startup and revalidates cached OIDC metadata based on upstream
  `Cache-Control: max-age` metadata, falling back to
  `PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS`, while keeping request-path
  authentication network-free and fail-closed. Expired discovery/JWKS cache
  entries remain usable only inside a bounded last-known-good window from
  upstream `Cache-Control: stale-while-revalidate` metadata or
  `PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS`. Failed refresh attempts retry
  after bounded backoff via `PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS`.
  That legacy loop now also emits refresh success/failure/backoff and cached
  JWKS age metric events with the existing low-cardinality observability
  planners, without logging issuer URLs, JWKS URLs, key IDs, tenants, or users
  as metric labels.
  Background discovery/JWKS refresh plans also require a non-empty HTTPS host
  before a transport adapter can fetch keys, and OIDC issuer configuration
  rejects non-HTTPS, hostless, whitespace-bearing, or userinfo-bearing issuer
  values before discovery URLs can be derived. Configured JWKS URLs now use the
  same no-userinfo HTTPS host boundary. OIDC audience configuration also
  rejects whitespace, control characters, and non-ASCII values before claim
  validation policy is built. Issuer, audience, and configured JWKS URL values
  are validated without trimming. Configured JWKS URLs must also use the same
  host as the validated issuer unless a future explicit trust-policy exception
  is added. Token key IDs must be non-empty bounded printable ASCII before the
  cached-key decision can accept them; see
  `docs/adr/0659-authn-exact-jwks-url-and-key-id-boundary.md`.
  OIDC issuer debug output preserves issuer presence while redacting raw issuer
  URLs; see `docs/adr/0800-domain-oidc-issuer-debug-redaction.md`.
  OIDC audience debug output preserves audience presence while redacting raw
  audience selectors; see
  `docs/adr/0801-domain-oidc-audience-debug-redaction.md`.
  OIDC policy debug output preserves policy shape and allowed algorithms while
  redacting raw issuer and audience values; see
  `docs/adr/0802-domain-oidc-policy-debug-redaction.md`.
  JWKS cache debug output preserves key-material presence while redacting raw
  freshness, last-known-good, refresh-error, backoff, and key-count internals;
  see `docs/adr/0803-domain-jwks-cache-debug-redaction.md`.
- **ADRs:** `docs/adr/0070-domain-jwks-refresh-decision.md`,
  `docs/adr/0081-authn-boundary-crate.md`, and
  `docs/adr/0163-identity-jwks-stable-error-responses.md`,
  `docs/adr/0226-authn-background-oidc-refresh-plan.md`, and
  `docs/adr/0412-authn-https-only-jwks-refresh-source.md`, and
  `docs/adr/0589-gateway-oidc-prefetch-timeout.md`, and
  `docs/adr/0606-authn-jwks-refresh-source-host-guard.md`, and
  `docs/adr/0607-domain-oidc-issuer-host-guard.md`, and
  `docs/adr/0617-authn-jwks-issuer-host-match.md`, and
  `docs/adr/0628-domain-oidc-issuer-host-shape-guard.md`, and
  `docs/adr/0629-domain-oidc-audience-character-guard.md`, and
  `docs/adr/0654-oidc-issuer-audience-exact-boundary.md`, and
  `docs/adr/0659-authn-exact-jwks-url-and-key-id-boundary.md`, and
  `docs/adr/0975-gateway-oidc-background-refresh-loop.md`.
- **Remaining gap:** Move refresh ownership, revisioning, and last-known-good
  rollover from the legacy gateway worker into a dedicated control-plane
  refresh service before production multi-tenant mode.

### 11. Blocking HTTP, Worker Threads, and Mutex Hot Paths Must Be Bounded

- **Evidence:** `crates/prodex-gateway-http/src/lib.rs` defines route
  classification, method policy, body limits, timeout budgets, trace
  requirements, header preserve/strip rules, and a bounded execution plan for
  concurrency, cancellation propagation, streaming backpressure, and graceful
  drain without selecting a blocking runtime. Timeout budgets reject stream-idle
  values that exceed the request timeout, and drain planning validates that
  a non-zero preStop delay is present and termination grace covers both preStop
  delay and connection drain budget. Drain-plan display output uses generic
  wording so local stringified errors do not expose deployment hook names; see
  `docs/adr/0855-gateway-http-drain-display-redaction.md`. Gateway HTTP policy
  display output uses generic wording so local stringified errors do not expose
  configuration knob names; see
  `docs/adr/0856-gateway-http-policy-display-redaction.md`.
- **Risk:** Blocking HTTP servers, unbounded worker threads, or hot
  `Arc<Mutex<...>>` maps can make one tenant or route starve the data plane.
- **Regression coverage:** `crates/prodex-gateway-http/tests`,
  `scripts/ci/gateway-http-boundary-guard.mjs`,
  `scripts/ci/runtime-hotpath-guard.mjs`, and existing runtime proxy overload
  characterization tests.
- **Local verification:** `cargo test -q -p prodex-gateway-http drain_plan -- --test-threads=1`
  exercises graceful-drain bounds,
  `cargo test -q -p prodex-gateway-http timeout -- --test-threads=1`
  exercises timeout-budget bounds, and
  `npm run ci:runtime-hotpath-guard` pins the hot-path guard parser,
  `#[cfg(test)]` exclusions, allowlist-overuse detection, and workspace scan,
  and `npm run ci:gateway-http-boundary-guard` runs the guard self-test plus
  workspace scan to keep the HTTP boundary wired into local CI.
- **Implemented fix or boundary:** The enterprise HTTP boundary records the
  async adapter contract and deployment drain timing contract before a concrete
  Axum/Hyper/Tower composition root is wired. The legacy gateway JSONL
  observability sink now schedules filesystem writes on the existing blocking
  pool instead of creating directories and appending files inline from the
  spend-event emitter and redacts path/error text before logging export
  failures. The legacy HTTP observability sink also schedules its blocking
  export on that pool instead of sending inline from the emitter, and redacts
  endpoint/error text before logging export failures. Gateway admin ledger CSV
  and summary exports now use an explicit bounded ledger load limit instead of
  `usize::MAX`, and Redis ledger compatibility reads cap accidental unbounded
  loads before issuing `LRANGE`. Runtime proxy request body limit env overrides
  now reject empty, whitespace-bearing, non-numeric, or zero values instead of
  silently falling back to the default capture limit.
- **ADRs:** `docs/adr/0095-gateway-http-policy-boundary.md`,
  `docs/adr/0229-gateway-http-execution-plan.md`,
  `docs/adr/0238-gateway-http-timeout-ordering.md`,
  `docs/adr/0281-runtime-hotpath-guard-test-fixture-scope.md`,
  `docs/adr/0319-gateway-http-drain-plan.md`, and
  `docs/adr/0548-gateway-jsonl-observability-blocking-pool.md`, and
  `docs/adr/0549-gateway-http-observability-blocking-pool.md`, and
  `docs/adr/0550-gateway-http-observability-failure-redaction.md`, and
  `docs/adr/0551-gateway-jsonl-observability-failure-redaction.md`, and
  `docs/adr/0664-gateway-admin-ledger-export-limit.md`, and
  `docs/adr/0010-runtime-request-body-limit.md`.
- **Remaining gap:** The remaining `tiny_http` and mutex-backed
  compatibility paths in `prodex-app` are now staged behind an explicit async
  serve migration contract (`docs/adr/0983-async-serve-composition-root-staging.md`),
  but the actual gateway/control-plane serve adapters and the separate
  loopback-only `dashboard` / `expose` migrations still need to land before the
  legacy compatibility code can be removed.

### 12. Dependency Inversion Must Keep Domain and Shared Logic Clean

- **Evidence:** Boundary crates include `prodex-domain`, `prodex-application`,
  `prodex-provider-spi`, `prodex-authn`, `prodex-authz`,
  `prodex-gateway-core`, `prodex-gateway-http`, `prodex-control-plane`,
  `prodex-storage`, `prodex-observability`, and `prodex-config`.
- **Risk:** Business rules depending on CLI, reports, database drivers,
  filesystem, HTTP frameworks, or provider SDKs make testing and HA deployment
  brittle.
- **Regression coverage:** `scripts/ci/domain-boundary-guard.mjs`,
  `scripts/ci/application-boundary-guard.mjs`, and the per-boundary crate
  tests.
- **Local verification:** `npm run ci:domain-boundary-guard` runs the domain
  guard self-test plus workspace scan for the dependency boundary, and
  `npm run ci:application-boundary-guard` runs the application guard self-test
  plus workspace scan for the use-case boundary, and
  `npm run ci:control-plane-boundary-guard` runs the control-plane guard
  self-test plus workspace scan for the admin planning boundary, and
  `npm run ci:provider-spi-boundary-guard` runs the provider SPI guard self-test
  plus workspace scan for the provider dependency boundary.
- **Implemented fix or boundary:** Domain and application use-case crates are
  side-effect-free and adapter-neutral; `prodex-app` is being reduced toward a
  composition root.
- **ADRs:** `docs/adr/0078-provider-spi-boundary.md`,
  `docs/adr/0083-storage-boundary-contract.md`, and
  `docs/adr/0096-application-use-case-boundary.md`.
- **Remaining gap:** Continue extracting legacy runtime business logic from
  `prodex-app` without changing upstream proxy semantics.

### 13. Telemetry Must Propagate End-to-End Trace Context

- **Evidence:** `crates/prodex-observability/src/lib.rs` parses and renders
  W3C traceparent, `crates/prodex-gateway-http/src/lib.rs` requires
  traceparent on enterprise routes, and runtime compatibility code propagates
  `traceparent`, `tracestate`, and W3C `baggage`.
- **Risk:** Custom logs without trace context cannot diagnose tenant incidents,
  streaming stalls, quota decisions, or cross-service failures.
- **Regression coverage:** `crates/prodex-domain/tests/observability.rs`,
  `crates/prodex-observability/tests/observability_boundary.rs`,
  `crates/prodex-gateway-http/tests`, `scripts/ci/domain-boundary-guard.mjs`,
  and `scripts/ci/observability-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-domain observability -- --test-threads=1`
  exercises domain telemetry label boundaries,
  `cargo test -q -p prodex-observability trace_context -- --test-threads=1`
  exercises W3C trace parsing/propagation, and
  `npm run ci:observability-boundary-guard` runs the guard self-test plus
  workspace scan to keep the observability boundary wired into local CI.
- **Implemented fix or boundary:** Trace context, metric label guardrails, and
  span planning are first-class boundary types; outbound propagation uses a
  shared plan for canonical `traceparent`, validated optional `tracestate`, and
  validated optional W3C `baggage`; gateway HTTP preserves those three context
  headers while still stripping auth and hop-by-hop headers. Gateway HTTP rejects
  duplicate `traceparent` headers with the stable redacted trace-context
  envelope so propagation is not adapter-order dependent. Tenant correlation has
  an explicit `tenant_trace_attribute` helper so raw tenant IDs are carried as
  trace-only context instead of metric labels. Gateway HTTP request plans now
  also carry closed-label trace propagation metric plans for `traceparent`,
  `tracestate`, and `baggage` through request and response plans, so exporters
  can publish propagation counters without inspecting raw headers or adding
  high-cardinality labels. Metric label keys and values are validated exactly
  without trimming before the low-cardinality and secret-like denylist checks.
  Domain observability debug output redacts telemetry values, attribute lists,
  span names, and rejected lengths while preserving span and validation shape.
  Legacy gateway operational probes expose public `/livez`, `/readyz`, and
  `/startupz` `gateway.health` JSON plus bodyless `HEAD` responses and stable
  `405` method denial, and the health payload includes active local-overload,
  draining, request-limit, active-request, and policy-version fields without
  requiring admin credentials or prompt/user identifiers as metric labels.
  Focused health coverage now also proves `/readyz` returns `503` during local
  overload or draining while `HEAD /readyz` stays bodyless and `/livez` plus
  `/startupz` remain passing for orchestrator liveness/startup checks.
  The dedicated `prodex-control-plane` and `prodex-gateway` binaries now also
  emit concrete OTLP/HTTP log records for their live control-plane planning,
  config-publication publish/delivery, external gateway migration, and
  gateway-side replicated consumption workflows when standard OpenTelemetry
  endpoint env vars are configured, so the composition root no longer stops at
  planning-only telemetry for those shipped enterprise commands. Dedicated
  `enterprise_binaries` coverage now also pins the positive `"otlp_log_export":
  "exported"` path for shipped `prodex-control-plane plan-http-control-plane`,
  `prodex-control-plane plan-config-publication`,
  `prodex-gateway migrate`,
  `prodex-control-plane publish-config-publication`,
  `prodex-control-plane compact-config-publication`,
  `prodex-control-plane deliver-config-publication`, and
  `prodex-gateway consume-config-publication` binary flows against a local
  OTLP/HTTP capture server. That same `enterprise_binaries` coverage now also
  pins exported-path denial telemetry for `prodex-control-plane
  plan-config-publication`, and denied publication planning now also pins the
  `"otlp_log_export": "failed"` fallback when the configured OTLP/HTTP
  endpoint is unreachable, and
  pins exported-path planner failures for the shared control-plane HTTP route,
  trace-context, idempotency, precondition, and page-request envelopes against
  the same local capture server. Gateway-migrate, control-plane plan,
  denied publication planning, control-plane HTTP plan, control-plane HTTP
  trace-context failure, control-plane HTTP route failure, control-plane HTTP
  idempotency failure, control-plane HTTP precondition failure,
  control-plane HTTP page-request failure, control-plane publish,
  control-plane compact, delivery, and gateway-consume coverage now also pin the
  `"otlp_log_export": "failed"` fallback when the configured OTLP/HTTP
  endpoint is unreachable, so every current OTLP-enabled one-shot enterprise
  binary path under `enterprise_binaries` now reports exporter failure without
  turning that transport problem into a command failure. The shared
  `src/enterprise_observability.rs` helper now also has direct unit coverage
  for OTLP endpoint fallback, logs-endpoint precedence, header parsing,
  disabled `Ok(false)` behavior, configured `Ok(true)` export behavior,
  shared `"exported" | "disabled" | "failed"` status mapping behavior,
  invalid header name/value rejection, invalid env-driven header name/value
  rejection, malformed env-driven header-entry rejection, configured
  transport-failure handling on the public helper, OTLP timeout-env parsing,
  OTLP timeout failure handling, invalid OTLP timeout-env rejection, and non-200 upstream OTLP failure handling.
  `enterprise_binaries` now also pins
  malformed-header failure reporting on both dedicated binaries plus the
  `OTEL_EXPORTER_OTLP_ENDPOINT -> /v1/logs` fallback path for every
  current OTLP-enabled one-shot enterprise binary path at the composition-root
  boundary instead of covering that fallback only in the shared helper unit
  tests, and that composition-root fallback matrix now also includes the blank
  `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` compatibility case for every current
  one-shot enterprise binary path. Gateway migrate and control-plane delivery
  now also pin OTLP timeout failure reporting at the composition root when the
  collector accepts the connection but does not respond before the configured
  timeout, including the generic `OTEL_EXPORTER_OTLP_TIMEOUT` fallback path
  when logs-specific timeout config is unset, and those same dedicated binaries
  now also pin invalid timeout-env failure reporting, including the generic
  timeout fallback path. The shared helper now also treats blank
  `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` the same as unset so the generic OTLP
  endpoint fallback still applies. Successful control-plane HTTP planning OTLP
  records now also carry the validated request, call, trace, tenant, and
  audit-event correlation identifiers from the shared audit-correlation planner, and
  `enterprise_binaries` pins those fields against the local OTLP/HTTP capture
  server. The shared OTLP helper now also promotes a valid `trace_id` attribute
  into native OTLP log-record `traceId`, with direct helper coverage, so
  collectors can correlate one-shot logs with traces without parsing
  attributes, and `enterprise_binaries` pins that native field at the
  control-plane HTTP planning composition-root boundary. The shared helper now
  also promotes a valid `span_id` attribute into native OTLP log-record
  `spanId`, with direct helper coverage, and control-plane HTTP planning now
  derives that `span_id` from the already-validated `traceparent` header with
  `enterprise_binaries` coverage, so one-shot workflow logs can attach to the
  exact upstream span without adding a second exporter path. The shared helper
  also promotes W3C `trace_flags` into native OTLP log-record `flags`, and
  control-plane HTTP planning derives those flags from the already-validated
  `traceparent` header with direct and composition-root coverage. Control-plane
  HTTP planning failures now also copy valid incoming `traceparent` trace IDs,
  span IDs, and flags into OTLP records, with `enterprise_binaries`
  route-failure, idempotency-failure, precondition-failure, and
  page-request-failure coverage, so collector correlation survives before audit
  correlation exists.
  The shared OTLP helper now also stamps every log record with
  `timeUnixNano` and `observedTimeUnixNano`, with direct helper coverage, so
  one-shot binary logs have collector-sortable event time instead of
  timestamp-less payloads. The shared helper also emits standard OTLP
  `severityNumber: 9` next to `severityText: "INFO"` with helper coverage, so
  collectors receive a numeric INFO severity instead of relying on text-only
  inference. The shared helper also mirrors the log body into low-cardinality
  `event.name` with helper coverage, so collectors can query one-shot workflow
  outcomes without parsing body text, and `enterprise_binaries` pins that field
  at the control-plane HTTP planning composition root. The shared helper now
  also emits `service.version` alongside
  `service.name`, with helper coverage, so one-shot gateway/control-plane
  telemetry can be grouped by deployed Prodex binary version, and
  `enterprise_binaries` pins that resource version at the control-plane HTTP
  planning composition-root boundary. It also sets the instrumentation scope
  `version` from that same package version, with helper coverage, so downstream
  collectors can distinguish event schema versions, and `enterprise_binaries`
  pins that scope version at the same boundary.
- **ADRs:** `docs/adr/0062-gateway-trace-context-propagation.md`,
  `docs/adr/0087-observability-boundary-crate.md`,
  `docs/adr/0088-observability-boundary-guard.md`, and
  `docs/adr/0225-observability-trace-propagation-plan.md`,
  `docs/adr/0279-w3c-baggage-propagation-boundary.md`,
  `docs/adr/0318-tenant-trace-attribute-boundary.md`,
  `docs/adr/0536-runtime-baggage-propagation.md`, and
  `docs/adr/0592-gateway-http-trace-propagation-metric-plan.md`, and
  `docs/adr/0011-gateway-operational-health-endpoints.md`, and
  `docs/adr/0160-health-probe-stable-responses.md`, and
  `docs/adr/0492-gateway-health-policy-version.md`, and
  `docs/adr/0499-health-probe-method-contract.md`, and
  `docs/adr/0502-application-readiness-draining.md`, and
  `docs/adr/0829-domain-observability-debug-redaction.md`, and
  `docs/adr/0847-domain-observability-span-name-debug-redaction.md`, and
  `docs/adr/0981-enterprise-binaries-otlp-http-log-export.md`.
- **Local verification:** `cargo test -q enterprise_observability::tests:: -- --test-threads=1`
  exercises the shared OTLP helper paths directly,
  `cargo test -q -p prodex-app runtime_launch::proxy_startup::local_rewrite_tests::gateway_health -- --test-threads=1`
  exercises the public gateway health probes, and
  `cargo test -q --test enterprise_binaries -- --test-threads=1`
  exercises the current dedicated binary OTLP export/fallback matrix end to
  end.
- **Remaining gap:** Carry the same OTLP exporter path into the future async
  `serve` composition root so long-lived gateway/control-plane traffic emits the
  same native telemetry without relying on one-shot command wiring.

### 14. Config and Policy Cache Need Revision and Last-Known-Good Semantics

- **Evidence:** `crates/prodex-domain/src/policy.rs` and
  `crates/prodex-config/src/lib.rs` model revisioned policy/config cache
  refresh, invalidation, publication validation, and last-known-good fallback.
- **Risk:** Unrevisioned caches can serve stale policy, fail open, or produce
  inconsistent decisions across replicas.
- **Regression coverage:** `crates/prodex-domain/tests/policy.rs`,
  `crates/prodex-config/tests/config_boundary.rs`, and
  `scripts/ci/config-boundary-guard.mjs`.
- **Local verification:** `cargo test -q -p prodex-domain policy -- --test-threads=1`
  exercises policy revision and cache semantics,
  `cargo test -q -p prodex-config config_refresh -- --test-threads=1`
  exercises config refresh/last-known-good behavior, and
  `npm run ci:config-boundary-guard` runs the guard self-test plus workspace
  scan to keep the config boundary wired into local CI.
- **Implemented fix or boundary:** Published config is revisioned, validated,
  invalidated deliberately through `plan_config_invalidation`, activated through
  a shared cache-state transition, routed through an application-level
  authorized-publication activation boundary, optionally reflected into
  tenant-scoped Redis policy revision cache plans with explicit revision IDs and
  TTLs, and evaluated with explicit last-known-good behavior. Config cache
  window debug output redacts refresh/stale/expiry timings; see
  `docs/adr/0909-config-cache-window-debug-redaction.md`. Config cache-state
  debug output redacts tenant and revision metadata; see
  `docs/adr/0911-config-cache-state-debug-redaction.md`. Config revision debug
  output redacts tenant, revision, timestamp, and payload fields; see
  `docs/adr/0910-config-revision-debug-redaction.md`. Config activation-plan
  debug output redacts direct revision fields and nested cache state; see
  `docs/adr/0912-config-activation-plan-debug-redaction.md`. Config publication
  event-plan debug output redacts tenant and revision metadata while preserving
  delivery targets; see
  `docs/adr/0913-config-publication-event-plan-debug-redaction.md`. Config
  invalidation-plan debug output redacts direct tenant/revision metadata and
  nested cache state; see
  `docs/adr/0914-config-invalidation-plan-debug-redaction.md`. Config
  invalidation-error debug output redacts tenant and revision identifiers while
  preserving failure shape; see
  `docs/adr/0915-config-invalidation-error-debug-redaction.md`. Config
  publication-error debug output redacts tenant and revision identifiers while
  preserving failure shape; see
  `docs/adr/0916-config-publication-error-debug-redaction.md`. Secret-bearing
  config entries are accepted only as
  `SecretRef` references and raw secret material is rejected before publication
  planning. Malformed secret references with empty, non-printable, non-ASCII,
  over-128-byte, or whitespace-containing provider, name, or version parts are
  also rejected with redacted stable configuration errors; see
  `docs/adr/0658-secret-ref-printable-boundary.md`. Domain secret debug output
  keeps secret references, versions, and refresh timings redacted while
  preserving request and rotation shape; see
  `docs/adr/0830-domain-secret-debug-redaction.md`. Domain `SecretRef`
  formatter output redacts provider, name, and version fields while preserving
  structured accessors for lookup; see
  `docs/adr/0849-domain-secret-ref-formatter-redaction.md`. Config secret
  source debug output redacts provider, name, and version metadata at the config
  boundary; see `docs/adr/0917-config-secret-source-debug-redaction.md`. Config secret
  reference plan debug output redacts tenant and reference metadata; see
  `docs/adr/0908-config-secret-reference-plan-debug-redaction.md`. The
  config boundary guard now pins the `ConfigSecretSource`,
  `plan_config_secret_reference`, malformed-reference, and raw-secret rejection
  contracts so publication cannot drift back to raw secret material; see
  `docs/adr/0086-config-boundary-guard.md`. Domain secret provider
  debug output redacts provider names while preserving provider kind and
  rotation support; see
  `docs/adr/0832-domain-secret-provider-debug-redaction.md`. Secret rotation
  policy debug output redacts timing values while preserving audit-event
  requirement shape; see
  `docs/adr/0837-domain-secret-rotation-policy-debug-redaction.md`. Runtime
  policy validation
  treats `secrets.keyring_service` as an exact non-empty identifier instead
  of trimming it into a different secret namespace; see
  `docs/adr/0690-keyring-service-exact-policy-identifier.md`. The secret-store
  keyring backend constructor enforces the same exact service-name boundary; see
  `docs/adr/0691-keyring-service-backend-exact-identifier.md`. Secret backend
  kind parsing also rejects whitespace-padded backend names before policy
  activation; see
  `docs/adr/0692-secret-backend-kind-whitespace-boundary.md`. Keyring location
  accounts must also be non-empty at the secret-store boundary; see
  `docs/adr/0693-keyring-location-account-nonempty.md`. File secret locations
  must also have a non-empty path before filesystem access; see
  `docs/adr/0694-file-secret-path-nonempty.md`. Secret-store debug output
  redacts locations, values, keyring backend selections, refresh-lease payloads,
  secret revision metadata, paths, accounts, and backend reasons while
  preserving variant names; see
  `docs/adr/0985-secret-store-debug-redaction.md`. Gateway runtime launch
  config debug output redacts admin-token hashes, SSO/OIDC metadata, state-store
  URLs, local paths, observability bearer tokens, and guardrail webhook tokens
  while preserving backend and feature shape; see
  `docs/adr/0986-gateway-runtime-config-debug-redaction.md`. Gateway billing
  ledger and summary debug output redacts key names, tenant and identity
  selectors, request/call identifiers, model names, token/cost/byte counters,
  and timestamps while preserving DTO shape; see
  `docs/adr/0987-gateway-billing-debug-redaction.md`. Gateway virtual-key store
  and governance-scope debug output redacts token hashes, key names, SCIM user
  identifiers, tenant and identity selectors, model allow-lists, budgets, rate
  limits, and timestamps while preserving store shape; see
  `docs/adr/0988-gateway-store-debug-redaction.md`. Provider spend-event debug
  output redacts key names, tenant selectors, request/call identifiers, paths,
  model names, token and byte counts, latency, and costs while preserving
  event/status shape; see `docs/adr/0989-provider-spend-debug-redaction.md`.
  Gemini OAuth profile debug output redacts profile names, Codex home paths,
  account emails, access tokens, and project IDs while preserving DTO shape; see
  `docs/adr/0990-gemini-oauth-profile-debug-redaction.md`. Anthropic OAuth
  profile debug output redacts profile names and access tokens while preserving
  DTO shape; see `docs/adr/0991-anthropic-oauth-profile-debug-redaction.md`.
  Gemini OAuth pool debug output redacts profile lists, affinity bindings, quota
  headers, model cooldowns, model-unavailable markers, model preferences, and
  timing data while preserving pool shape; see
  `docs/adr/0992-gemini-oauth-pool-debug-redaction.md`. DeepSeek request DTO
  debug output redacts prompt messages, translated request bodies, and response
  metadata while preserving DTO shape; see
  `docs/adr/0993-deepseek-request-debug-redaction.md`. DeepSeek SSE debug
  output redacts response IDs, model names, streamed text, reasoning, tool
  payloads, usage, logprobs, fingerprints, response metadata, and conversation
  messages while preserving stream state shape; see
  `docs/adr/0994-deepseek-sse-debug-redaction.md`. Gateway usage-delta debug
  output redacts request/call identifiers, key names, tenant and identity
  selectors, model names, token counters, cost estimates, and timestamps while
  preserving accounting DTO shape; see
  `docs/adr/0995-gateway-usage-delta-debug-redaction.md`. Copilot profile and
  OAuth pool debug output redacts profile names, API keys, API URLs, model
  catalogs, rotation cursors, and response/profile affinity bindings while
  preserving provider DTO shape; see
  `docs/adr/0996-copilot-profile-debug-redaction.md`. Gemini OAuth secret,
  token-response, and user-info debug output redacts persisted and freshly
  exchanged tokens, token metadata, expiry timing, account emails, project IDs,
  and auth-mode strings while preserving OAuth DTO shape; see
  `docs/adr/0997-gemini-oauth-secret-debug-redaction.md`. Claude OAuth secret,
  credentials-file, nested token, and auth-status debug output redacts tokens,
  expiry timing, subscription/auth method labels, and account identifiers while
  preserving auth DTO shape; see
  `docs/adr/0998-claude-oauth-debug-redaction.md`. Copilot profile import and
  runtime API auth debug output redacts GitHub hosts, account logins,
  OAuth/import tokens, runtime API keys, and model catalogs while preserving auth
  DTO shape; see `docs/adr/0999-copilot-import-auth-debug-redaction.md`.
  Managed Mem0 secrets debug output redacts PostgreSQL passwords, admin API
  keys, and JWT secrets while preserving secret bundle shape; see
  `docs/adr/1000-mem0-secrets-debug-redaction.md`. Kiro import/auth debug
  output redacts auth keys, auth kinds, raw auth JSON, identity metadata, AWS
  profile selectors, start URLs, and regions while preserving auth DTO shape;
  see `docs/adr/1001-kiro-auth-debug-redaction.md`. Gateway root/data-plane
  bearer tokens no longer authenticate admin endpoints; admin routes require
  explicit admin tokens, trusted-proxy SSO, or OIDC identities and report
  `admin_auth_not_configured` when no control-plane credential source exists;
  see `docs/adr/1002-gateway-root-token-control-plane-denial.md`. Explicitly
  empty root/data-plane gateway bearer-token inputs now fail closed during
  launch instead of silently disabling the configured credential; see
  `docs/adr/1024-gateway-root-token-empty-fail-closed.md`. File and Redis
  billing ledger de-duplication now uses the exact, length-prefixed `(call_id,
  key_name, phase)` identity so case-distinct key names and delimiter-bearing
  fields cannot collide; see
  `docs/adr/1003-gateway-ledger-entry-identity.md`. Durable SQLite/PostgreSQL
  per-key cost reservations now take precedence over stale local spend for
  admin-managed keys with valid tenant and virtual-key IDs while preserving
  local request-count, RPM/TPM, model, and grouped-budget checks; see
  `docs/adr/1004-durable-budget-admission-precedence.md`. Durable
  SQLite/PostgreSQL admin key lifecycle planning now rejects missing or
  malformed key tenant IDs instead of synthesizing a random tenant; see
  `docs/adr/1005-durable-key-lifecycle-tenant-fail-closed.md`. Durable
  SQLite/PostgreSQL admin key lifecycle planning also rejects missing or
  malformed virtual-key IDs instead of synthesizing a random key identity; see
  `docs/adr/1006-durable-key-lifecycle-virtual-key-id-fail-closed.md`. Durable
  SQLite/PostgreSQL SCIM lifecycle planning now rejects missing or malformed
  SCIM tenant IDs instead of synthesizing a random tenant; see
  `docs/adr/1007-durable-scim-lifecycle-tenant-fail-closed.md`. Durable
  SQLite/PostgreSQL SCIM lifecycle planning also rejects missing or malformed
  target user IDs instead of synthesizing a random principal; see
  `docs/adr/1008-durable-scim-lifecycle-principal-id-fail-closed.md`. Durable
  SQLite/PostgreSQL SCIM lifecycle planning now derives a stable compatibility
  actor principal from admin name and tenant instead of generating a random
  actor principal per request; see
  `docs/adr/1009-durable-scim-lifecycle-actor-principal.md`. Durable
  SQLite/PostgreSQL virtual-key lifecycle planning now derives the same kind of
  stable compatibility actor principal for create and rotate-secret operations;
  see `docs/adr/1010-durable-key-lifecycle-actor-principal.md`. Mounted gateway
  admin route/idempotency/precondition planning now also uses stable
  compatibility tenant and principal IDs instead of fresh random IDs; see
  `docs/adr/1011-admin-control-plane-compat-principal.md`. SSO and OIDC admin
  authentication now use SCIM role and key-prefix metadata only when an explicit
  tenant claim matches the SCIM tenant, preventing cross-tenant role reuse by
  username; see `docs/adr/1012-sso-scim-tenant-claim-isolation.md`. Explicit
  SSO/OIDC role claims now also fail closed to Viewer when unknown, malformed,
  or non-string instead of falling through to SCIM Admin metadata; see
  `docs/adr/1013-sso-unknown-role-claim-fail-closed.md`. Trusted-proxy SSO
  admin authentication now rejects missing user identity headers instead of
  synthesizing `sso-admin`; see
  `docs/adr/1014-sso-missing-user-claim-fail-closed.md`. OIDC admin identity
  derivation now rejects malformed explicit user identity claims instead of
  silently falling back to another principal claim; see
  `docs/adr/1015-oidc-configured-user-claim-fail-closed.md`. Admin token
  `allowed_key_prefixes` now fail closed on malformed values instead of dropping
  invalid prefixes and widening scope to all keys; see
  `docs/adr/1016-admin-token-key-prefix-fail-closed.md`. Admin token governance
  scopes now also fail closed on malformed tenant/team/project/user/budget
  values instead of dropping them and widening control-plane access; see
  `docs/adr/1017-admin-token-governance-scope-fail-closed.md`. Configured
  virtual-key governance scopes now use the same fail-closed behavior so
  malformed scope values cannot widen data-plane access or accounting scope; see
  `docs/adr/1018-virtual-key-governance-scope-fail-closed.md`. Configured
  virtual-key model scopes now also fail closed so malformed `allowed_models`
  cannot be dropped into unrestricted model access; see
  `docs/adr/1019-virtual-key-model-scope-fail-closed.md`. Global guardrail
  model scopes now fail closed for the same reason, preventing malformed
  `allowed_models` entries from disabling the configured allowlist; see
  `docs/adr/1020-guardrail-model-scope-fail-closed.md`. Guardrail webhook
  phase configuration now also fails closed so malformed phase values cannot
  silently skip intended request or response checks; see
  `docs/adr/1021-guardrail-webhook-phase-fail-closed.md`. Guardrail keyword
  blank entries now fail closed instead of being dropped from the active
  security policy; see
  `docs/adr/1030-guardrail-keyword-blank-fail-closed.md`. Guardrail webhook
  bearer secret references now fail closed when malformed, missing, or empty so
  configured webhook auth is not silently disabled; see
  `docs/adr/1022-guardrail-webhook-bearer-secret-fail-closed.md`. Runtime proxy
  preset names reject whitespace padding before policy activation; see
  `docs/adr/0695-runtime-proxy-preset-exact-boundary.md`. Gateway state backend
  names also reject whitespace padding before activating storage selection; see
  `docs/adr/0696-gateway-state-backend-exact-boundary.md`. Policy snapshot
  validation rejects zero issued-at timestamps and malformed digest or signature
  metadata containing empty, whitespace, control-character, non-ASCII, or
  over-512-byte
  values before activation while keeping raw values redacted from responses; see
  `docs/adr/0644-policy-integrity-metadata-length-boundary.md`. Domain policy
  digest debug output also redacts opaque integrity metadata; see
  `docs/adr/0824-domain-policy-digest-debug-redaction.md`. Policy validation
  debug output redacts verifier outcomes; see
  `docs/adr/0843-domain-policy-validation-debug-redaction.md`. Domain policy
  snapshot debug output redacts payloads, timestamps, revision identifiers, and
  integrity metadata; see
  `docs/adr/0825-domain-policy-snapshot-debug-redaction.md`. Policy activation
  state debug output reports only active/last-known-good presence; see
  `docs/adr/0826-domain-policy-activation-state-debug-redaction.md`. Policy
  audit record debug output redacts revision IDs and free-form reason text; see
  `docs/adr/0827-domain-policy-audit-record-debug-redaction.md`. Policy cache
  debug output redacts refresh-window timings and revision identity; see
  `docs/adr/0828-domain-policy-cache-debug-redaction.md`.
  Successful activations now also produce a required
  publication event for gateway cache refresh and runtime policy reload; the runtime policy
  cache exposes per-root invalidation, redacted invalidation plans, and explicit
  reload so event consumers do not keep serving stale local policy or log full
  policy material. Runtime policy cache keys are normalized across equivalent
  `.` root aliases before lookup or invalidation. Configuration activation does
  not promote an invalidated active revision into last-known-good fallback, and
  clears invalidated fallback state instead of resurrecting withdrawn config.
  Refresh evaluation also refuses to select a last-known-good revision after
  that same revision was explicitly invalidated. Policy refresh evaluation also
  uses last-known-good only when a non-invalidated last-known-good revision is
  present; stale policy caches without a usable fallback keep async refresh
  pressure instead of claiming a fallback revision exists.
  Unservable config refresh decisions are converted through
  `config_refresh_error_for_decision` and
  `plan_config_refresh_error_response` so refresh-required and
  invalidated-without-last-known-good states fail closed behind stable,
  redacted responses. The legacy production composition root now consumes
  publication events through
  `deliver_config_publication_event_to_gateway_runtime`, refreshes the local
  gateway cache target, invalidates the runtime policy cache for the configured
  root, and reloads runtime policy before subsequent reads can reuse stale
  cached state. That adapter also returns low-cardinality publication delivery
  metric plans for gateway cache refresh and runtime policy reload so delivery
  outcomes can be observed without tenant, revision, root, or payload labels.
  The dedicated `prodex-control-plane` binary now exposes one-shot
  `plan-config-publication`, `deliver-config-publication`, and
  `publish-config-publication`, and `compact-config-publication` commands,
  while `prodex-gateway` exposes `consume-config-publication`. That shared file
  transport writes one hashed event record into a transport outbox, lets each
  gateway replica advance independently through per-replica acknowledgements,
  and now provides one-shot retention compaction for fully acknowledged records,
  so separated control-plane and gateway processes no longer require manual
  per-replica fan-out or indefinite shared-storage growth for this path.
- **ADRs:** `docs/adr/0068-domain-policy-cache-refresh.md`,
  `docs/adr/0085-config-boundary-crate.md`,
  `docs/adr/0086-config-boundary-guard.md`,
  `docs/adr/0150-config-cache-window-stable-error-responses.md`,
  `docs/adr/0162-policy-stable-error-responses.md`, and
  `docs/adr/0223-config-activation-plan.md`,
  `docs/adr/0224-application-config-activation-boundary.md`,
  `docs/adr/0231-config-secret-reference-plan.md`, and
  `docs/adr/0237-config-invalidation-plan.md`, and
  `docs/adr/0242-config-publication-refresh-event-plan.md`, and
  `docs/adr/0243-runtime-policy-explicit-reload-cache-invalidation.md`, and
  `docs/adr/0280-runtime-policy-redacted-cache-invalidation-plan.md`, and
  `docs/adr/0316-config-refresh-error-response-boundary.md`, and
  `docs/adr/0544-runtime-policy-cache-root-normalization.md`, and
  `docs/adr/0545-config-activation-invalidated-lkg.md`, and
  `docs/adr/0590-config-publication-runtime-delivery-adapter.md`, and
  `docs/adr/0605-config-invalidated-lkg-refresh-guard.md`, and
  `docs/adr/0618-config-secret-ref-shape-guard.md`, and
  `docs/adr/0633-policy-metadata-character-guard.md`, and
  `docs/adr/0634-policy-issued-at-zero-guard.md`, and
  `docs/adr/0977-config-publication-replicated-file-transport.md`, and
  `docs/adr/0984-config-publication-broker-transport-staging.md`.
- **Remaining gap:** Non-shared-storage publication transport is now staged
  behind an explicit broker-backed outbox/watch contract, but the actual broker
  composition-root adapter still needs to land before this path is fully
  topology-neutral.

## Cross-Cutting Release Gates

- Keep characterization tests for continuation affinity, pre-commit rotation,
  provider fallback, transport transparency, streaming cancellation, and
  upstream error compatibility green before replacing legacy adapters.
- Keep model/provider capability negotiation on the side-effect-free
  `prodex-provider-spi` and `prodex-domain` boundary before dispatching to a
  concrete provider adapter.
  Application provider-capability debug output redacts capability requests,
  route candidates, negotiation plans, and negotiation errors while preserving
  planner and error variant names; see
  `docs/adr/0972-application-provider-capability-debug-redaction.md`.
  Model route candidate debug output preserves capability shape while redacting
  raw provider and model route names; see
  `docs/adr/0805-domain-model-route-candidate-debug-redaction.md`.
  Capability decision debug output preserves negotiation outcome shape while
  redacting route candidates and missing capability internals; see
  `docs/adr/0806-domain-capability-decision-debug-redaction.md`.
  Capability request debug output preserves request shape while redacting
  required capability internals; see
  `docs/adr/0807-domain-capability-request-debug-redaction.md`.
  Capability set debug output preserves capability count while redacting exact
  capability internals; see
  `docs/adr/0808-domain-capability-set-debug-redaction.md`.
- Keep provider retries behind `plan_provider_retry` so only bounded pre-commit
  attempts are allowed and retries after stream commit or cancellation remain
  denied; expose retry denials through `plan_provider_retry_decision_response`
  so stages, attempts, routes, and provider internals stay out of client-visible
  responses.
- Keep provider circuit breakers behind `plan_provider_circuit_breaker` so
  repeated upstream failures open bounded cooldown windows before concrete
  transport retry.
- Keep boundary guards in CI so new crates do not import HTTP frameworks,
  database drivers, provider SDKs, filesystem operations, or CLI/reporting
  modules into domain/application logic.
- Keep the GitHub Actions `process-guard` job running the self-tested enterprise
  docs, binaries, boundary, storage, gateway, and deployment security guards so
  local preflight and CI enforce the same modular-monolith contracts.
- Persist every `ControlPlaneAuditWritePlan` in append-only hash-chain mode
  before treating a security-sensitive control-plane action as durably
  completed.
- Use `ControlPlaneOperation::audit_requirement` as the mandatory immutable
  audit contract for every control-plane operation so success and denial paths
  both require append-only hash-chain audit writes.
- Keep `ControlPlaneOperation::ALL` lifecycle coverage exhaustive so each
  control-plane operation has an explicit resource kind, action, role, and
  audited resource-kind mismatch denial.
- Use explicit `BoundaryKind::ControlPlaneTenant*`,
  `BoundaryKind::ControlPlaneUser*`,
  `BoundaryKind::ControlPlaneRoleBinding*`, and
  `BoundaryKind::ControlPlaneServiceIdentityCreate` contracts for lifecycle
  adapters so tenant, identity, and role mutations authorize against exact
  resource/action pairs before storage mutation.
- Use explicit `BoundaryKind::ControlPlaneVirtualKey*`,
  `BoundaryKind::ControlPlaneProviderCredentialRotate`, and
  `BoundaryKind::ControlPlaneBudgetUpdate` contracts for key, provider
  credential, and budget adapters so secret rotation and spending-limit
  mutations authorize against exact resource/action pairs before storage
  mutation.
- Use `control_plane_boundary_for_requirement` before adapting normal
  control-plane operation authorization so supported resource/action
  requirements resolve to explicit authz boundaries instead of silently falling
  back to generic admin checks.
- Use `data_plane_boundary_for_requirement` before adapting inference or quota
  authorization so only data-plane `VirtualKey/Read` and `Budget/Read`
  operator requirements can resolve to data-plane boundaries.
- Use `boundary_for_requirement` when an adapter accepts a generic domain
  `AuthorizationRequirement` so data-plane, normal control-plane, and
  break-glass requirements resolve to explicit boundaries or fail closed.
- Use the data-plane authz resolver inside `plan_data_plane_admission` so
  gateway inference admission records the explicit `DataPlaneInference`
  boundary before reservation or provider invocation.
  Application data-plane debug output redacts HTTP request metadata,
  admission planning, and route/admission error details while preserving
  planner and error variant names; see
  `docs/adr/0974-application-data-plane-debug-redaction.md`.
- Use `plan_gateway_quota_read_authorization` before adapting quota read
  surfaces so `DataPlaneQuota` authorization, tenant matching, and trace spans
  are planned before budget state is read or returned. Quota authorization
  display output uses generic wording for unavailable authorization boundaries
  and tenant mismatches so local stringified errors do not expose quota-read
  topology; see
  `docs/adr/0861-gateway-core-quota-authorization-display-redaction.md`.
- Use `plan_application_quota_read` for quota endpoints so `/quota` and
  `/v1/quota` are classified as `DataPlaneQuota` routes before gateway
  authorization and budget state access.
  Application quota-read debug output redacts HTTP request metadata,
  authorization planning, and route/authorization error details while
  preserving planner and error variant names; see
  `docs/adr/0973-application-quota-read-debug-redaction.md`.
- Use `plan_application_request_authentication` before data-plane or
  control-plane use cases so authenticated OIDC claims are bound to the
  classified HTTP route and data-plane/control-plane credential scopes cannot
  bypass each other. Request-authentication display output uses one generic
  denial message for wrong-route and credential-scope mismatch failures so
  local stringified errors do not expose route or scope topology; see
  `docs/adr/0866-application-request-authentication-display-redaction.md`.
  Request-authentication debug output redacts HTTP metadata, token claims,
  principals, credential scopes, and route topology while preserving planner
  and error variant names; see
  `docs/adr/0957-application-request-authentication-debug-redaction.md`.
- Use `plan_control_plane_route` for `/admin`, `/v1/admin`, `/scim`, and
  `/v1/scim` adapters so HTTP method/path pairs resolve to explicit
  control-plane operations with idempotency and audit requirements before
  authorization or storage mutation.
- Keep `ci:gateway-http-boundary-guard` checking that the control-plane route
  planner remains segment-boundary based, method-specific, fail-closed, and
  surfaces idempotency/audit requirements for adapters.
- Use `plan_gateway_control_plane_route_error_response` before exposing
  control-plane route planner failures so unknown paths, unsupported methods,
  operations, and HTTP methods stay out of client-visible errors.
  Control-plane route display output also uses generic HTTP wording so local
  stringified errors do not expose route topology; see
  `docs/adr/0853-gateway-http-control-plane-route-display-redaction.md`.
- Use `plan_application_control_plane_http_route` and the HTTP idempotency
  helpers so composition roots reject mismatches between a control-plane HTTP
  route and the canonical `ControlPlaneOperation` before accepting replay keys
  or mutating storage, even when an adapter supplies a precomputed fingerprint.
- Delegate application HTTP-route idempotency failures to
  `plan_gateway_control_plane_route_error_response` so unsupported methods keep
  their method-not-allowed status while remaining redacted.
- Keep `ci:application-boundary-guard` checking that both HTTP idempotency
  helpers call the shared route/action validator and that route mismatch
  responses stay stable and redacted.
- Use `plan_application_break_glass_with_audit_storage` for emergency
  control-plane actions so break-glass credentials stay on an explicit,
  short-lived path with a non-empty, bounded, non-control-character reason, and
  authorized or denied decisions are persisted to append-only audit storage.
- Use `plan_application_provider_capability` before building provider
  invocations so data-plane adapters select a route that satisfies required
  model/provider capabilities and expose only stable redacted compatibility
  errors. Domain negotiation skips malformed provider/model route names before
  compatibility checks so blank, over-128-byte, whitespace/control-character,
  or non-ASCII routes cannot be selected.
- Use `plan_application_provider_retry`,
  `plan_application_provider_circuit_breaker`, and
  `plan_application_provider_circuit_breaker_event` before concrete provider
  transports retry or expose provider health so post-commit retries stay denied
  and repeated failures open bounded cooldown windows.
- Route SCIM user create, update, and delete through explicit
  `ControlPlaneOperation::ScimUser*` variants so provisioning cannot bypass
  admin authorization, idempotency, or immutable audit requirements.
- Route role grants and revocations through
  `ControlPlaneOperation::RoleBinding*` variants on `ResourceKind::RoleBinding`
  so permission management cannot be hidden inside generic user updates.
- Persist service identity creation through tenant-scoped
  `ServiceIdentityCreateCommand` plans and
  `plan_application_service_identity_lifecycle` so service account lifecycle
  uses authorization, append-only audit, and authorized-only storage mutation.
  Application service-identity create debug output redacts service-identity
  commands, storage plans, and nested storage errors while preserving planner
  and error variant names; see
  `docs/adr/0968-application-service-identity-create-debug-redaction.md`.
  Storage service identity debug output redacts tenant, principal, display-name,
  and timestamp fields while preserving command/plan shape; see
  `docs/adr/0931-storage-service-identity-debug-redaction.md`.
  Storage service identity error debug output redacts mismatched tenant fields
  while preserving the error variant name; see
  `docs/adr/0948-storage-service-identity-error-debug-redaction.md`.
- Persist role grants and revocations through the storage
  `RoleBindingMutationCommand` and adapter DML plans so role state is
  tenant-keyed, durable, and request-path DDL free.
  Storage role-binding mutation debug output redacts tenant, principal,
  role-binding, and timestamp fields while preserving role/kind shape; see
  `docs/adr/0930-storage-role-binding-mutation-debug-redaction.md`.
  Storage role-binding mutation error debug output redacts mismatched tenant
  fields while preserving the error variant name; see
  `docs/adr/0947-storage-role-binding-error-debug-redaction.md`.
- Use `plan_application_role_binding_mutation` from composition roots so
  role-binding storage backend selection is not reimplemented in HTTP adapters.
  Application role-binding mutation debug output redacts role-binding
  commands, storage plans, and nested storage errors while preserving planner
  and error variant names; see
  `docs/adr/0969-application-role-binding-mutation-debug-redaction.md`.
- Use `plan_application_role_binding_lifecycle` from control-plane adapters so
  role grants and revocations compose authorization, append-only audit, and
  authorized-only tenant-scoped role-binding storage in one application
  boundary.
  Application role-binding lifecycle debug output redacts control-plane action,
  role-binding mutation command, decision, storage-plan, and route/resource
  mismatch details while preserving planner and error variant names; see
  `docs/adr/0970-application-role-binding-lifecycle-debug-redaction.md`.
- Persist virtual key creation and secret rotation through tenant- and
  virtual-key-scoped `VirtualKeySecretReferenceCommand` storage plans that keep
  `SecretRef` fields instead of raw key material.
  Storage virtual-key secret reference debug output redacts tenant, virtual-key,
  principal, display-name, secret-reference, and timestamp fields while
  preserving mutation kind; see
  `docs/adr/0935-storage-virtual-key-secret-reference-debug-redaction.md`.
  Storage virtual-key secret reference error debug output redacts mismatched
  tenant and virtual-key fields while preserving error variant names; see
  `docs/adr/0952-storage-virtual-key-error-debug-redaction.md`.
- Use `plan_application_virtual_key_lifecycle` from control-plane adapters so
  virtual key create and rotate-secret requests compose authorization,
  append-only audit, and authorized-only `SecretRef` storage in one application
  boundary.
- Persist provider credential rotations as tenant-scoped
  `ProviderCredentialReferenceCommand` plans that store `SecretRef` fields, not
  raw credential material.
  Storage provider credential debug output redacts tenant, credential,
  provider-name, secret-reference, and timestamp fields; see
  `docs/adr/0936-storage-provider-credential-debug-redaction.md`.
  Storage provider credential error debug output redacts mismatched tenant
  fields while preserving the error variant name; see
  `docs/adr/0953-storage-provider-credential-error-debug-redaction.md`.
- Use `plan_application_provider_credential_reference` from composition roots
  so provider credential storage backend selection stays outside HTTP adapters.
  Application provider-credential reference debug output redacts provider
  credential commands, storage plans, and nested storage errors while
  preserving planner and error variant names; see
  `docs/adr/0964-application-provider-credential-reference-debug-redaction.md`.
- Use `plan_application_provider_credential_rotation` from control-plane
  adapters so provider credential rotation composes authorization,
  append-only audit, and authorized-only `SecretRef` reference storage in one
  application boundary.
  Application provider-credential rotation debug output redacts control-plane
  action, provider credential command, decision, storage-plan, and
  route/resource mismatch details while preserving planner and error variant
  names; see
  `docs/adr/0965-application-provider-credential-rotation-debug-redaction.md`.
- Persist control-plane budget updates through tenant-scoped
  `BudgetPolicyUpdateCommand` plans and
  `plan_application_budget_policy_lifecycle` so budget policy changes are
  authorized, append-only audited, and stored only after authorization.
  Application budget-policy update debug output redacts budget policy commands,
  storage plans, and nested storage errors while preserving planner and error
  variant names; see
  `docs/adr/0966-application-budget-policy-update-debug-redaction.md`.
  Application budget-policy lifecycle debug output redacts control-plane
  action, budget policy command, decision, storage-plan, and route/resource
  mismatch details while preserving planner and error variant names; see
  `docs/adr/0962-application-budget-policy-lifecycle-debug-redaction.md`.
  Storage budget policy debug output redacts tenant, scope, limit, and
  timestamp fields while preserving command/plan shape; see
  `docs/adr/0933-storage-budget-policy-debug-redaction.md`.
  Storage budget policy error debug output redacts mismatched tenant fields
  while preserving error variant names; see
  `docs/adr/0950-storage-budget-policy-error-debug-redaction.md`.
- Persist tenant create and update through `TenantLifecycleCommand` plans and
  `plan_application_tenant_lifecycle` so tenant registry changes are
  authorized, append-only audited, and stored only after authorization.
  Application tenant-lifecycle debug output redacts control-plane action,
  tenant command, decision, storage-plan, and route/resource mismatch details
  while preserving planner and error variant names; see
  `docs/adr/0960-application-tenant-lifecycle-debug-redaction.md`.
  Storage tenant lifecycle debug output redacts tenant, display-name, and
  timestamp fields while preserving lifecycle kind; see
  `docs/adr/0934-storage-tenant-lifecycle-debug-redaction.md`.
  Storage tenant lifecycle error debug output redacts mismatched tenant fields
  while preserving error variant names; see
  `docs/adr/0951-storage-tenant-lifecycle-error-debug-redaction.md`.
- Persist user invite and SCIM create/update/delete through
  `UserLifecycleCommand` plans and `plan_application_user_lifecycle` so user
  registry changes are tenant-keyed, append-only audited, and stored only after
  authorization.
  Application user-lifecycle debug output redacts control-plane action, user
  command, decision, storage-plan, and route/resource mismatch details while
  preserving planner and error variant names; see
  `docs/adr/0961-application-user-lifecycle-debug-redaction.md`.
  Storage user lifecycle debug output redacts tenant, principal, external-ID,
  display-name, and timestamp fields while preserving lifecycle kind; see
  `docs/adr/0932-storage-user-lifecycle-debug-redaction.md`.
  Storage user lifecycle error debug output redacts mismatched tenant fields
  while preserving error variant names; see
  `docs/adr/0949-storage-user-lifecycle-error-debug-redaction.md`.
- Route audit export through `AuditExportQueryCommand` and
  `plan_application_audit_export` so export reads are tenant-keyed,
  append-only audited, and selected only after authorization.
  Application audit-export debug output redacts control-plane action,
  audit-query, decision, storage-plan, and route/resource mismatch details
  while preserving planner and error variant names; see
  `docs/adr/0959-application-audit-export-debug-redaction.md`.
  Storage audit export query debug output redacts tenant, query, page-limit, and
  storage-key details while preserving sort direction; see
  `docs/adr/0928-storage-audit-export-query-debug-redaction.md`.
  Storage audit export query error debug output redacts mismatched tenant
  fields while preserving the error variant name; see
  `docs/adr/0946-storage-audit-export-query-error-debug-redaction.md`.
- Route billing reads through `BillingLedgerQueryCommand` and
  `plan_application_billing_read` so usage ledger reads are tenant-keyed,
  append-only audited, and selected only after authorization.
  Application billing-read debug output redacts control-plane action, billing
  query, decision, storage-plan, and route/resource mismatch details while
  preserving planner and error variant names; see
  `docs/adr/0958-application-billing-read-debug-redaction.md`.
  Storage billing ledger range display output uses the stable query request
  message while preserving structured timestamps for trusted diagnostics; see
  `docs/adr/0714-storage-billing-ledger-range-error-redaction.md`.
  Storage billing ledger query debug output redacts tenant, storage-key,
  time-range, and page-limit details while preserving sort direction; see
  `docs/adr/0925-storage-billing-ledger-query-debug-redaction.md`.
  Storage billing ledger error debug output redacts tenant and time-range
  fields while preserving error variant names; see
  `docs/adr/0942-storage-billing-ledger-error-debug-redaction.md`.
  Storage billing ledger limit debug output redacts rejected numeric limits
  while preserving error variant names; see
  `docs/adr/0956-storage-billing-ledger-limit-debug-redaction.md`.
- Application audit export, billing read, and tenant lifecycle display output
  uses generic request-invalid wording for tenant mismatches so local
  stringified errors do not expose tenant topology; see
  `docs/adr/0868-application-control-plane-tenant-display-redaction.md`.
- Application user lifecycle, service identity lifecycle, and budget policy
  lifecycle display output uses the same generic request-invalid wording for
  tenant mismatches; see
  `docs/adr/0869-application-lifecycle-tenant-display-redaction.md`.
- Application service-identity lifecycle debug output redacts control-plane
  action, service-identity command, decision, storage-plan, and
  route/resource mismatch details while preserving planner and error variant
  names; see
  `docs/adr/0967-application-service-identity-lifecycle-debug-redaction.md`.
- Storage user lifecycle, budget policy, and tenant lifecycle display output
  uses the same stable planner messages for empty request fields; see
  `docs/adr/0907-storage-lifecycle-field-display-redaction.md`.
- Storage virtual-key secret-reference display output uses the same stable
  planner message for storage-key and request virtual-key mismatches; see
  `docs/adr/0918-storage-virtual-key-secret-reference-display-redaction.md`.
- Application role-binding lifecycle, virtual-key lifecycle, and provider
  credential rotation display output also uses generic request-invalid wording
  for tenant mismatches; see
  `docs/adr/0870-application-secret-lifecycle-tenant-display-redaction.md`.
- Application virtual-key lifecycle debug output redacts control-plane action,
  virtual-key reference command, decision, storage-plan, and route/resource
  mismatch details while preserving planner and error variant names; see
  `docs/adr/0963-application-virtual-key-lifecycle-debug-redaction.md`.
- Application virtual-key secret-reference debug output redacts virtual-key
  commands, storage plans, and nested storage errors while preserving planner
  and error variant names; see
  `docs/adr/0971-application-virtual-key-secret-reference-debug-redaction.md`.
- Use `BoundaryKind::ControlPlaneBillingRead` for billing read adapters so
  billing records require control-plane credentials and tenant access while
  remaining viewer-eligible instead of falling back to generic admin checks.
- Use `BoundaryKind::ControlPlanePolicyPublish` and
  `BoundaryKind::ControlPlaneConfigurationPublish` for publication adapters so
  policy and configuration revisions authorize against explicit publish
  resource/action pairs rather than generic admin update checks.
- Use `BoundaryKind::ControlPlaneAuditExport` and
  `BoundaryKind::ControlPlaneAuditRetentionPurge` for audit adapters so export
  and retention delete paths authorize against explicit audit-log
  resource/actions before storage query or purge execution.
- Use the storage `AppendOnlyAuditPlan` contract for durable audit persistence
  so tenant mismatches are rejected before adapter-specific writes.
- Gateway usage reconciliation and expired-reservation recovery display output
  uses generic request-invalid wording for tenant mismatches so local
  stringified errors do not expose accounting topology; see
  `docs/adr/0862-gateway-core-accounting-display-redaction.md`.
- Storage atomic reservation and usage reconciliation display output uses
  generic request-invalid wording for tenant mismatch, expiry overflow, and
  usage arithmetic failures; see
  `docs/adr/0449-storage-accounting-display-redaction.md`.
  Storage accounting error debug output redacts tenant and usage amount fields
  while preserving error variant names; see
  `docs/adr/0954-storage-accounting-error-debug-redaction.md`.
- Use the audit response planners before exposing audit digest or hash-chain
  verification failures so digest values, tenant IDs, principal IDs, resource
  IDs, action names, and chain topology stay out of client-visible error
  envelopes.
  Storage append-only audit debug output redacts storage keys, audit events,
  digest material, and envelopes while preserving append mode; see
  `docs/adr/0926-storage-append-only-audit-debug-redaction.md`.
  Storage append-only audit error debug output redacts mismatched tenant fields
  while preserving the error variant name; see
  `docs/adr/0944-storage-append-only-audit-error-debug-redaction.md`.
- Use `plan_postgres_append_only_audit` as the PostgreSQL DML plan for
  tenant-scoped control-plane audit events; migrations remain external-only.
- Use `plan_sqlite_append_only_audit` for local compatibility audit appends
  without reintroducing SQLite DDL on request-serving paths.
- Use `plan_usage_reconciliation` after upstream completion, cancellation, or
  stream interruption so actual usage is committed and unused reservation
  capacity is released through tenant-scoped ledger events.
  Storage usage reconciliation debug output redacts storage keys, reservation
  records, usage values, reconciliation payloads, and ledger events while
  preserving typed planner fields; see
  `docs/adr/0924-storage-usage-reconciliation-debug-redaction.md`.
- Use `plan_postgres_usage_reconciliation` for PostgreSQL DML that locks the
  reservation, updates durable counters, and writes idempotent committed plus
  released ledger events.
- Use `plan_sqlite_usage_reconciliation` for local compatibility reconciliation
  with `BEGIN IMMEDIATE`, DML-only counter updates, and idempotent ledger rows.
- Use `plan_gateway_usage_reconciliation` after provider completion,
  cancellation, or interruption so the data plane explicitly plans
  commit/release ledger work with trace propagation.
- Use `plan_application_usage_reconciliation` from composition roots so
  post-upstream accounting is not reimplemented in legacy adapters.
- Let `prodex-application` select the durable reconciliation SQL plan for
  PostgreSQL or SQLite so composition roots do not duplicate storage-adapter
  accounting decisions.
- Use `plan_application_control_plane_with_audit_storage` so authorized and
  denied control-plane actions both select append-only PostgreSQL or SQLite
  audit storage plans at the application boundary.
- Use `plan_application_configuration_publication` so accepted and rejected
  configuration revision publications select append-only audit storage through
  the same application boundary.
- Use `plan_config_publication_event` and the application activation event plan
  after accepted configuration revisions so gateway cache refresh and runtime
  policy reload are both scheduled explicitly.
- Use `reload_runtime_policy_cached` only from publication-event consumers or
  other bounded background paths so runtime policy refresh invalidates the
  affected root without turning request handlers into filesystem reloaders.
- Use `plan_expired_reservation_recovery` for abandoned reservation cleanup so
  expired reserved capacity is released with tenant-scoped idempotent ledger
  events.
  Expired reservation recovery display output uses the stable planner message;
  see
  `docs/adr/0904-storage-expired-reservation-recovery-display-redaction.md`.
  Storage expired-reservation recovery debug output redacts storage keys,
  reservation records, recovery timestamps, and ledger events; see
  `docs/adr/0929-storage-expired-reservation-recovery-debug-redaction.md`.
  Storage expired-reservation recovery error debug output redacts mismatched
  tenant fields while preserving error variant names; see
  `docs/adr/0943-storage-expired-reservation-error-debug-redaction.md`.
- Use `plan_postgres_expired_reservation_recovery` for PostgreSQL DML that
  locks expired reservations, releases durable counters, and writes an
  idempotent released ledger event.
- Use `plan_sqlite_expired_reservation_recovery` for local compatibility
  cleanup with `BEGIN IMMEDIATE`, DML-only release updates, and tenant-scoped
  idempotent ledger rows.
- Use `plan_gateway_expired_reservation_recovery` and
  `plan_application_expired_reservation_recovery` so abandoned-reservation
  cleanup goes through tenant validation, trace propagation, and durable-store
  selection instead of bypassing boundary crates.
- Use `plan_recovery_lease_acquire` and `plan_recovery_lease_release` when
  multi-replica recovery workers coordinate cleanup shards through Redis; the
  durable source of truth remains PostgreSQL/SQLite storage plans.
- Use the optional coordination field on
  `plan_application_expired_reservation_recovery` so composition roots select
  Redis recovery leases and durable recovery writes through one application
  boundary contract.
- Use `plan_application_recovery_lease_release` after a recovery worker finishes
  or aborts a cleanup shard so owner-token based lease release also stays behind
  the application boundary.
- Use `plan_multi_replica_accounting_concurrency_spec` to define the required
  PostgreSQL/Redis two-replica accounting checks before running deployment
  integration tests.
- Use `plan_multi_replica_accounting_verification` to accept database-backed
  concurrency evidence only when every required check passed with the planned
  replica count and documented limit-overshoot tolerance.
  Storage multi-replica accounting display output uses the stable accounting
  configuration message while preserving structured topology, replica, and
  evidence-check variants; see
  `docs/adr/0920-storage-multi-replica-accounting-display-redaction.md`.
  Storage multi-replica accounting error debug output redacts topology,
  replica-count, and evidence-check fields while preserving error variant
  names; see
  `docs/adr/0955-storage-multi-replica-accounting-error-debug-redaction.md`.
  Storage multi-replica accounting debug output redacts topology, replica count,
  and checklist details in spec, evidence, and verification plans; see
  `docs/adr/0921-storage-multi-replica-accounting-debug-redaction.md`.
  Storage topology debug output redacts durable-store, cache-store, and
  migration-policy details while preserving typed planner fields; see
  `docs/adr/0922-storage-topology-debug-redaction.md`.
- Use `plan_application_runtime_accounting_verification` at composition roots
  so startup/readiness accepts multi-replica accounting evidence through the
  same application-level redaction boundary as runtime topology planning.
- Enable `require_multi_replica_accounting_checks` in
  `plan_application_runtime` for production-like deployment plans so invalid
  single-replica, SQLite, or Redis-free accounting topologies fail before
  adapters start.
- Keep deployment manifests and environment examples aligned with the runtime
  accounting gate by setting `PRODEX_GATEWAY_REPLICA_COUNT`,
  `PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS`, and shared PostgreSQL/Redis
  connection secrets deliberately. The legacy `prodex gateway` launch path now
  derives the same prelaunch gate from those deployment env vars, validates the
  production posture through `plan_production_deployment_readiness` first, and
  still fails closed until the durable reservation backend replaces local
  admission.
- Use `plan_production_deployment_readiness` before claiming production
  horizontal readiness so replica count, accounting gate intent, and shared
  PostgreSQL/Redis dependencies are validated by one domain contract.
  Deployment posture debug output exposes only violation counts while keeping
  typed posture fields available to callers; see
  `docs/adr/0838-domain-deployment-posture-debug-redaction.md`.
  Deployment security report debug output exposes only violation count while
  keeping the typed violation list available to callers; see
  `docs/adr/0834-domain-deployment-security-report-debug-redaction.md`.
  Production readiness topology debug output preserves low-cardinality readiness
  booleans while redacting exact replica-count topology; see
  `docs/adr/0812-domain-production-readiness-topology-debug-redaction.md`.
  Deployment readiness plan debug output preserves accounting-gate intent while
  redacting exact replica-count topology; see
  `docs/adr/0813-domain-deployment-readiness-plan-debug-redaction.md`.
- Keep Kubernetes NetworkPolicy egress restricted to DNS, labeled data-store
  namespaces for PostgreSQL/Redis, and public HTTPS provider traffic with
  private-network exclusions.
- Keep Kubernetes NetworkPolicy ingress restricted to namespaces labeled
  `prodex.dev/network-tier=ingress`, not every namespace in the cluster.
- Keep gateway metrics ingress restricted to namespaces labeled
  `prodex.dev/network-tier=monitoring` so the `ServiceMonitor` can scrape
  metrics without opening gateway ingress to the whole cluster.
- Keep Kubernetes metrics scraping on a dedicated viewer admin token instead of
  `PRODEX_GATEWAY_TOKEN`, and bind that token through `policy.toml` as a
  read-only `[[gateway.admin_tokens]]` entry.
- Keep runtime policy docs aligned with the Prometheus label-cardinality
  contract: virtual-key metrics expose `key_hash` and low-cardinality scope
  booleans, not raw tenant, team, project, user, budget, or virtual-key names.
- Keep `scripts/docs/runtime-policy.mjs --self-test` green so the docs guard
  proves that raw Prometheus governance labels are rejected and bounded label
  contract phrases remain mandatory.
- Keep Kubernetes `policy.toml` mounted from `prodex-gateway-policy` so the
  metrics viewer token is registered in the checked-in deployment artifact.
- Keep Kubernetes `policy.toml` configured with `[gateway.state] backend =
  "postgres"` and `postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"` so
  multi-replica gateway pods do not fall back to per-pod file state.
- Keep a namespace-wide Kubernetes default-deny NetworkPolicy with no allow
  rules so new pods in the `prodex` namespace are isolated until a dedicated
  workload allow policy is added.
- Keep the Kubernetes `prodex` namespace labeled for restricted Pod Security
  Admission in enforce, audit, and warn modes so future pods cannot bypass the
  workload hardening baseline silently.
- Keep Kubernetes workload security contexts on `RuntimeDefault` seccomp so
  gateway, migration, and control-plane pods cannot silently fall back to an
  unconfined syscall profile.
- Keep Kubernetes gateway, migration, and control-plane workloads bound to
  distinct explicit ServiceAccounts with token automount disabled so production
  identity does not fall back to the namespace default or share RBAC across
  operational planes.
- Keep Kubernetes gateway topology spread constraints across zone and node
  failure domains so the multi-replica baseline does not collapse onto one
  scheduler domain.
- Keep Kubernetes gateway termination grace and `preStop` drain hooks aligned
  with the HTTP connection-drain budget so rolling updates do not sever
  streaming requests immediately.
- Keep Kubernetes image references pinned by 64-character `sha256` digest and
  reject malformed, all-zero, repeated-character, or repeated-pattern
  placeholders so manifests cannot appear immutable while still carrying an
  unreplaced sample digest.
- Keep the Kubernetes migration Job behind a dedicated egress-only NetworkPolicy
  so external migrators can reach DNS/PostgreSQL without inheriting gateway
  ingress, Redis, or provider egress.
- Keep migration database credentials in a migration-only Kubernetes secret
  instead of mounting gateway/provider tokens into the external migrator path.
- Keep gateway data-plane credentials in the gateway Kubernetes secret instead
  of mounting control-plane storage credentials into the gateway workload.
- Keep control-plane storage and coordination credentials in a control-plane
  Kubernetes secret instead of mounting gateway/provider tokens into the
  control-plane workload.
- Keep control-plane Kubernetes egress limited to DNS and data-store namespaces;
  only gateway data-plane pods should have public provider HTTPS egress.
- Keep `npm run ci:deployment-security-guard` green so the guard self-test plus
  workspace scan continue proving
  credential defaults, default ServiceAccounts, missing workload bindings,
  enabled service-account token automount, missing or weakened namespace Pod
  Security Admission labels, missing namespace default-deny, non-global or
  rule-bearing default-deny policies, missing gateway monitoring metrics
  ingress, unauthenticated ServiceMonitor scrapes, ServiceMonitor reuse of the
  gateway root token, missing gateway policy ConfigMaps, metrics policy bindings
  to the root token, non-viewer metrics policy roles, missing or non-PostgreSQL
  gateway shared state policy, missing gateway policy mounts, missing topology
  spreading, missing or weakened seccomp profiles, unrestricted namespace
  ingress, missing gateway drain grace, missing migration ServiceAccount
  bindings, migration gateway ServiceAccount reuse, migration network labels,
  missing migration-only secrets, missing gateway workload secrets, gateway
  control-plane secret mounts, missing control-plane storage secrets,
  control-plane gateway secret mounts, and control-plane public provider
  egress are proven negative cases.
- See `docs/adr/0594-control-plane-gateway-secret-mount-guard.md` for the
  control-plane guard that prevents data-plane gateway/provider credentials
  from being mounted into the control-plane workload.
- Use `plan_deployment_security_error_response` before exposing deployment
  security validation failures through readiness, startup, or control-plane APIs
  so concrete manifest topology and hardening checks stay in trusted diagnostics
  rather than client-visible responses.
- Use `plan_api_version_error_response` before exposing unsupported or sunset
  API version failures so requested versions, sunset timestamps, and API policy
  internals stay out of stable client-visible error envelopes.
  API version error debug output preserves lifecycle failure shape while
  redacting requested versions and sunset timestamps; see
  `docs/adr/0777-domain-api-version-error-debug-redaction.md`.
  API version error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0778-domain-api-version-error-response-plan-debug-redaction.md`.
  API version lifecycle debug output redacts policy versions and lifecycle
  timestamps while preserving current/deprecated/sunset shape; see
  `docs/adr/0842-domain-api-version-lifecycle-debug-redaction.md`.
- Use `plan_gateway_http_api_version` before dispatching gateway HTTP requests
  so explicit `/vN/...` paths and legacy unversioned paths are evaluated against
  the same domain API lifecycle policy before reaching data-plane or
  control-plane handlers.
- Normalize gateway HTTP request targets before route and API-version planning
  so query strings or fragments cannot make valid data-plane or control-plane
  routes appear unknown; see
  `docs/adr/0596-gateway-http-request-target-route-normalization.md`.
- Keep overflowing explicit `/vN/...` prefixes as unsupported explicit API
  versions instead of falling back to the legacy default; see
  `docs/adr/0597-gateway-http-api-version-overflow-fail-closed.md`.
- Use `plan_cursor_error_response` before exposing pagination cursor validation
  failures so cursor parser limits, request-controlled cursor lengths, and
  cursor encoding details stay out of stable client-visible error envelopes.
  Cursor debug output preserves cursor presence while redacting opaque token
  values; see `docs/adr/0787-domain-cursor-debug-redaction.md`.
  Page debug output preserves item count and next-cursor presence while
  redacting item payloads and opaque cursor values; see
  `docs/adr/0816-domain-page-debug-redaction.md`.
  Page request debug output preserves limit and cursor presence while redacting
  opaque cursor values; see
  `docs/adr/0817-domain-page-request-debug-redaction.md`.
  Cursor error debug output preserves validation failure shape while redacting
  lengths, indexes, and rejected characters; see
  `docs/adr/0779-domain-cursor-error-debug-redaction.md`.
  Cursor error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0780-domain-cursor-error-response-plan-debug-redaction.md`.
  Domain cursors reject empty, whitespace, control-character, and non-ASCII bytes
  at the shared API boundary before storage or control-plane query planning, and
  cursor values are validated without trimming; see
  `docs/adr/0652-api-cursor-entity-tag-exact-boundary.md`.
- Use `page_request_from_query` at gateway HTTP adapters before list/query
  control-plane operations so `limit` and `cursor` query parameters are
  normalized into the domain `PageRequest` contract, duplicate pagination
  parameters fail closed, and parse errors stay redacted.
- Decode pagination query parameter names before matching `limit` and `cursor`
  so percent-encoded duplicates cannot bypass the duplicate-parameter guard; see
  `docs/adr/0595-gateway-http-pagination-query-name-decoding.md`.
- Use `plan_application_control_plane_page_request_from_http_query` before
  control-plane list/query storage planning so parsed pagination metadata is
  accepted only after the HTTP route matches the authorized action operation.
- Use `plan_concurrency_error_response` before exposing mutation precondition
  failures so expected/current resource versions, entity tags, and
  request-controlled `If-Match` metadata stay out of stable client-visible error
  envelopes.
  Concurrency error display output also redacts expected/current resource
  versions; see
  `docs/adr/0846-domain-concurrency-error-display-redaction.md`.
  Concurrency error debug output preserves precondition failure shape while
  redacting expected/current resource versions and entity tags; see
  `docs/adr/0785-domain-concurrency-error-debug-redaction.md`.
  Concurrency error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0786-domain-concurrency-error-response-plan-debug-redaction.md`.
- Use `plan_entity_tag_error_response` and
  `plan_resource_version_error_response` before exposing optimistic-concurrency
  token validation failures so parser limits, token lengths, token values, and
  resource-version invariants stay out of stable client-visible error
  envelopes. Domain resource-version advancement is fallible and rejects
  `u64::MAX` overflow instead of silently reusing the saturated value. Domain
  entity tags reject empty, whitespace, control-character, and non-ASCII values
  before mutation precondition planning, and entity-tag values are validated without
  trimming; see `docs/adr/0652-api-cursor-entity-tag-exact-boundary.md`.
  Entity tag debug output preserves token presence while redacting raw ETag
  values; see `docs/adr/0788-domain-entity-tag-debug-redaction.md`.
  Entity tag error debug output preserves validation failure shape while
  redacting lengths, indexes, and rejected characters; see
  `docs/adr/0781-domain-entity-tag-error-debug-redaction.md`.
  Entity tag error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0782-domain-entity-tag-error-response-plan-debug-redaction.md`.
  Resource version debug output preserves version presence while redacting the
  raw numeric value; see
  `docs/adr/0789-domain-resource-version-debug-redaction.md`.
  Resource version error debug output keeps only low-cardinality version
  validation failure shape; see
  `docs/adr/0783-domain-resource-version-error-debug-redaction.md`.
  Resource version error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0784-domain-resource-version-error-response-plan-debug-redaction.md`.
  The legacy gateway admin adapter also compares `If-Match` values exactly, so
  padded wildcard or ETag values no longer satisfy key mutation preconditions;
  see `docs/adr/0668-gateway-admin-if-match-exact-boundary.md`.
- Use `entity_tag_from_if_match_headers` at gateway HTTP adapters before
  control-plane mutations so request-controlled `If-Match` values are parsed
  once through the domain `EntityTag` contract, duplicate precondition headers
  fail closed, and invalid values are rejected with redacted gateway/domain
  error envelopes.
- Construct `ErrorEnvelope` through the domain constructor so invalid
  machine-readable codes are replaced with `internal.error` instead of being
  serialized into public API responses; see
  `docs/adr/0642-error-envelope-invalid-code-fail-closed.md`.
- The same constructor sanitizes empty, overlong, control-character, and
  non-ASCII public messages to `request failed`; see
  `docs/adr/0643-error-envelope-message-sanitization-boundary.md`.
- Use `plan_application_control_plane_precondition_from_http` before
  control-plane mutation planning so parsed `If-Match` metadata is accepted only
  after the HTTP route matches the authorized action operation.
- Use `plan_application_control_plane_with_audit_storage_from_http` before
  control-plane mutation audit persistence so append-only audit commands are
  planned only after the HTTP route matches the authorized action operation and
  the route advertises its audit requirement.
- Use `plan_application_control_plane_audit_correlation_from_http` after audit
  storage planning so request ID, optional call ID, W3C trace ID, tenant ID, and
  immutable audit event ID share the canonical domain `CorrelationContext`
  without becoming high-cardinality metric labels.
- Use `plan_application_control_plane_audit_emission_span` before emitting
  control-plane audit telemetry so `GatewaySpanKind::AuditEmission` carries
  tenant ID and audit event ID as trace-scoped attributes rather than metric
  labels.
- Use `plan_application_control_plane_audit_persistence_span` around append-only
  audit storage writes so `GatewaySpanKind::Persistence` exposes only the typed
  low-cardinality `storage_backend` metric label while tenant and audit event IDs
  remain trace-scoped.
- Use `plan_idempotency_key_error_response` and
  `plan_idempotency_conflict_response` before exposing replay-key validation or
  idempotency replay conflicts so raw keys, token lengths, invalid characters,
  tenant-affinity details, and request fingerprints stay out of stable
  client-visible error envelopes. Domain idempotency keys and request
  fingerprints are validated without trimming; empty values are distinct from
  whitespace, control-character, or non-ASCII bytes before replay, storage
  materialization, or control-plane mutation planning; see
  `docs/adr/0657-idempotency-key-exact-boundary.md` and
  `docs/adr/0662-idempotency-fingerprint-exact-boundary.md`. The legacy
  gateway admin adapter also validates exact `Idempotency-Key` header values
  without trimming and rejects whitespace-bearing replay keys before they can
  enter the compatibility replay cache; see
  `docs/adr/0667-gateway-admin-idempotency-exact-boundary.md`.
- Domain idempotency `Debug` output redacts tenant IDs, replay keys, request
  fingerprints, and stored replay responses while preserving state shape for
  diagnostics; see `docs/adr/0720-domain-idempotency-debug-redaction.md`.
  Pending idempotency entry debug output also redacts replay-guard timestamps;
  see `docs/adr/0820-domain-idempotency-entry-debug-redaction.md`.
  Replay-decision debug output redacts in-progress timestamps while keeping the
  typed decision value unchanged; see
  `docs/adr/0821-domain-idempotency-replay-decision-debug-redaction.md`.
  Idempotency key error debug output preserves validation failure shape while
  redacting rejected lengths, indexes, and characters; see
  `docs/adr/0818-domain-idempotency-key-error-debug-redaction.md`.
  Idempotent operation error debug output preserves fingerprint validation
  failure shape while redacting rejected indexes and characters; see
  `docs/adr/0819-domain-idempotent-operation-error-debug-redaction.md`.
- Use `plan_migration_version_error_response`,
  `plan_migration_plan_error_response`, and
  `plan_migration_compatibility_error_response` before exposing migration
  startup, readiness, status, or control-plane failures so lock ownership,
  migration versions, step ordering, request-path DDL policy, and compatibility
  window internals stay out of stable client-visible error envelopes. Migration
  versions reject empty, whitespace, control-character, and non-ASCII values at
  the domain boundary before status or compatibility planning, without trimming; see
  `docs/adr/0653-backup-migration-identifier-exact-boundary.md`.
  Migration version debug output preserves identifier presence while redacting
  raw migration identifiers; see
  `docs/adr/0791-domain-migration-version-debug-redaction.md`.
  Migration version error debug output preserves validation failure shape while
  redacting rejected indexes and characters; see
  `docs/adr/0795-domain-migration-version-error-debug-redaction.md`.
- Use `plan_backup_id_error_response` and `plan_restore_error_response` before
  exposing backup/restore validation failures so backup IDs, timestamps,
  checksum values, backend restore internals, and drill evidence stay out of
  stable client-visible error envelopes. Restore validation rejects missing,
  blank, malformed, padded, over-512-byte, or mismatched checksums and invalid
  expiry windows before an operator can treat a backup as restorable. Backup IDs
  reject empty, whitespace, control-character, and non-ASCII values at the domain
  boundary before restore planning, without trimming; see
  `docs/adr/0653-backup-migration-identifier-exact-boundary.md` and
  `docs/adr/0661-restore-checksum-exact-boundary.md`.
  Backup ID debug output preserves identifier presence while redacting raw
  recovery identifiers; see
  `docs/adr/0790-domain-backup-id-debug-redaction.md`.
  Backup ID error debug output preserves validation failure shape while
  redacting rejected indexes and characters; see
  `docs/adr/0794-domain-backup-id-error-debug-redaction.md`.
  Backup ID display output also uses the stable planner message; see
  `docs/adr/0900-domain-recovery-display-redaction.md`.
  Backup snapshot debug output preserves backup status while redacting raw
  identifiers, timestamps, and checksums; see
  `docs/adr/0814-domain-backup-snapshot-debug-redaction.md`.
  Restore plan debug output preserves restore shape while redacting expected
  checksums and evaluation timestamps; see
  `docs/adr/0815-domain-restore-plan-debug-redaction.md`.
  Restore display output also uses the stable planner messages; see
  `docs/adr/0902-domain-restore-display-redaction.md`.
- Use `minimum_enterprise_slo_objectives` as the shared operational baseline for
  availability, p95 latency, error rate, quota correctness, provider
  degradation, and persistence failure alerts before adapter-specific overrides
  are applied. SLO evaluation ignores non-finite or negative observed/target
  values so invalid telemetry cannot raise misleading alerts.
- Use `plan_slo_alert_response` before exposing SLO alert decisions through
  readiness, admin, or operational APIs so objective names, observed values,
  targets, tenant names, and metric-label internals stay out of client-visible
  responses. Domain SLO debug output redacts objective names, observed values,
  and targets while preserving alert shape; see
  `docs/adr/0833-domain-slo-debug-redaction.md`.
- Use `plan_slo_alert_metric` before publishing SLO alert counters so only
  closed low-cardinality `slo_sli` and `slo_severity` labels appear while
  objective names, observed values, targets, tenants, request IDs, and raw alert
  routing details stay out of metric labels.
- Use `plan_health_probe_response` for public `/livez`, `/readyz`, and
  `/startupz` responses so the active policy revision is exposed as typed
  metadata while dependency messages, backend endpoints, revision values, and
  drain internals stay out of free-form probe messages.
- Use `plan_trace_propagation_metric` before publishing trace propagation
  counters so only closed low-cardinality `trace_carrier` and
  `trace_propagation_result` labels appear while trace IDs, span IDs, tenant
  baggage values, request IDs, and raw header values stay out of metric labels.
- Use `TraceId::new_w3c` when adapting W3C `traceparent` trace IDs so the
  domain boundary rejects non-32-hex and all-zero trace IDs while preserving the
  older compatibility parser for non-W3C correlation sources. Trace ID
  constructors validate exact values without trimming; see
  `docs/adr/0655-trace-id-exact-boundary.md`.
  Trace ID debug output preserves identifier presence while redacting raw
  propagation identifiers; see
  `docs/adr/0792-domain-trace-id-debug-redaction.md`.
  Trace ID error debug output preserves validation failure shape while redacting
  rejected lengths; see
  `docs/adr/0793-domain-trace-id-error-debug-redaction.md`.
- Use `plan_structured_log_correlation` before writing structured logs so
  `RequestId`, `CallId`, trace ID, tenant, and audit event identifiers are
  emitted only as trace/log fields and never as metric labels.
  Correlation context debug output preserves field presence while redacting raw
  request, call, trace, tenant, and audit-event identifiers; see
  `docs/adr/0809-domain-correlation-context-debug-redaction.md`.
- Use `plan_enterprise_id_metric` before publishing typed enterprise ID
  generation or parse counters so only closed low-cardinality
  `enterprise_id_kind` and `enterprise_id_result` labels appear while raw
  tenant, principal, request, call, reservation, virtual-key, policy revision,
  audit event IDs, and UUID parser details stay out of metric labels.
- Use `plan_application_configuration_readiness_snapshot` before readiness
  adapters serialize health probes so active, async-refresh, last-known-good,
  invalidated, and refresh-required config states map to one canonical
  `HealthSnapshot` with a `configuration` health check that marks async refresh
  and last-known-good fallback as degraded, and missing or rejected config as
  failing.
  Health check debug output preserves check name and state while redacting
  free-form readiness messages; see
  `docs/adr/0810-domain-health-check-debug-redaction.md`.
  Health snapshot debug output preserves readiness booleans, active-policy
  presence, and check count while redacting raw revision and check details; see
  `docs/adr/0811-domain-health-snapshot-debug-redaction.md`.
- Use `plan_jwks_cache_age_metric` before publishing JWKS cache telemetry so
  cache age is recorded as a measurement while only low-cardinality
  `jwks_cache_state` appears as a metric label.
- Use `plan_policy_snapshot_age_metric` before publishing policy snapshot age
  telemetry so age is recorded as a measurement while only low-cardinality
  `policy_cache_state` appears as a metric label and tenant/revision/digest
  details stay out of labels.
- Use `plan_policy_refresh_outcome_metric` before publishing policy refresh
  counters so only low-cardinality `policy_refresh_result` appears as a metric
  label and tenant/revision/digest/error details stay out of labels.
- Use `plan_policy_rollback_metric` before publishing policy rollback or
  last-known-good activation counters so only closed low-cardinality
  `policy_rollback_operation` and `policy_rollback_result` labels appear while
  tenant, policy revision, digest, policy body, request, caller identity, and
  raw validation error details stay out of labels.
- Use `plan_config_activation_metric` before publishing configuration revision
  activation, last-known-good fallback, rollback, or invalidation fallback
  counters so only closed low-cardinality `config_activation_source` and
  `config_activation_result` labels appear while tenant, revision, digest,
  configuration body, request, caller identity, and raw validation error details
  stay out of labels.
- Use `plan_config_publication_delivery_metric` before publishing
  configuration publication delivery counters so only closed low-cardinality
  `config_publication_target` and `config_publication_result` labels appear
  while tenant, revision, event topic, event payload, request, caller identity,
  and raw delivery error details stay out of labels.
- Use `plan_config_cache_invalidation_metric` before publishing configuration
  cache invalidation counters so only closed low-cardinality
  `config_invalidation_target` and `config_invalidation_result` labels appear
  while tenant, revision, root path, cache key, payload, caller identity, and raw
  invalidation error details stay out of labels.
- Use `plan_jwks_refresh_outcome_metric` before publishing JWKS refresh result
  counters so only low-cardinality `jwks_refresh_result` appears as a metric
  label and issuer/key/error details stay out of metrics.
- Use `plan_oidc_refresh_metric` before publishing background OIDC discovery,
  JWKS fetch, snapshot validation, or cache-write counters so only closed
  low-cardinality `oidc_refresh_operation` and `oidc_refresh_result` labels
  appear while issuer, tenant, key ID, endpoint, token material, JWKS payload,
  and raw refresh error details stay out of labels.
- Use `plan_dropped_telemetry_metric` before publishing telemetry-drop counters
  so only closed low-cardinality `telemetry_drop_reason` values appear as metric
  labels and tenant/request/exporter error details stay out of metrics.
- Use `plan_queue_depth_metric` before publishing bounded queue depth gauges so
  only closed low-cardinality `queue_kind` values appear as metric labels while
  depth, capacity, tenant, and request details stay out of labels.
- Use `plan_connection_pool_saturation_metric` before publishing connection
  pool saturation gauges so only closed low-cardinality `pool_kind` values
  appear as metric labels while endpoint, credential, capacity, tenant, and
  request details stay out of labels.
- Use `plan_api_red_metric` before publishing API RED metrics so request counts
  and durations use only closed low-cardinality `api_route` and `status_class`
  labels while raw paths, exact status codes, tenants, prompts, and request IDs
  stay out of labels.
- Keep the domain telemetry metric-label guard checking both separator
  normalized and compact key forms so camelCase identifiers such as `tenantId`,
  `requestId`, and `apiKey` cannot bypass the high-cardinality label denylist;
  see `docs/adr/0598-domain-telemetry-camelcase-metric-label-guard.md`.
- Reject empty, whitespace-bearing, control-character, and non-ASCII metric
  label keys before identifier denylist matching so malformed labels fail
  closed at the domain boundary; see
  `docs/adr/0640-domain-telemetry-printable-metric-label-keys.md`.
- Reject metric label keys longer than 128 bytes so malformed or
  request-controlled key names cannot create unbounded exporter cardinality; see
  `docs/adr/0641-domain-telemetry-metric-label-key-length.md`.
- Reject exact base metric label keys such as `tenant`, `user`, `principal`,
  `request`, `call`, and `prompt` while preserving closed lifecycle labels; see
  `docs/adr/0599-domain-telemetry-base-identifier-metric-label-guard.md`.
- Reject canonical UUID-shaped metric label values so raw tenant, principal,
  request, call, and audit identifiers cannot be smuggled through otherwise
  allowed label keys; see
  `docs/adr/0600-domain-telemetry-uuid-metric-label-value-guard.md`.
- Reject 32-character hexadecimal metric label values so undashed UUIDs and W3C
  trace IDs cannot become high-cardinality metrics; see
  `docs/adr/0601-domain-telemetry-undashed-hex-metric-label-value-guard.md`.
- Reject exact secret/credential metric label keys such as `key`, `token`,
  `secret`, `credential`, and `password` while preserving closed categorical
  labels; see
  `docs/adr/0602-domain-telemetry-secret-base-metric-label-key-guard.md`.
- Reject secret-like metric label values with credential prefixes such as
  `Bearer `, `sk-`, `ghp_`, `gho_`, and `github_pat_` so otherwise allowed
  categorical keys cannot carry tokens; see
  `docs/adr/0603-domain-telemetry-secret-prefix-metric-label-value-guard.md`.
- Reject JWT-shaped metric label values so raw bearer tokens cannot bypass the
  prefix guard when callers pass the token without the `Bearer ` scheme; see
  `docs/adr/0604-domain-telemetry-jwt-metric-label-value-guard.md`.
- Reject empty, whitespace-bearing, control-character, and non-ASCII metric
  label values so exporters only receive bounded printable low-cardinality
  labels; see
  `docs/adr/0639-domain-telemetry-printable-metric-label-values.md`.
- Use `plan_api_schema_validation_metric` before publishing request, response,
  OpenAPI, or error-envelope validation counters so only closed
  low-cardinality `api_schema_surface` and `api_schema_result` labels appear
  while tenant, raw route, schema path, field path, request, payload, and raw
  validation error details stay out of labels.
- Use `plan_api_deprecation_metric` before publishing API lifecycle counters so
  only closed low-cardinality `api_deprecation_surface` and
  `api_deprecation_signal` labels appear while API versions, raw paths, tenant,
  client, request, and compatibility details stay out of labels.
- Use `plan_api_pagination_metric` before publishing pagination or cursor
  counters so only closed low-cardinality `api_pagination_surface` and
  `api_pagination_result` labels appear while cursors, query strings, raw
  paths, tenants, resources, and request details stay out of labels.
- Use `plan_api_precondition_metric` before publishing ETag or version
  precondition counters so only closed low-cardinality
  `api_precondition_surface` and `api_precondition_result` labels appear while
  raw ETags, resource identifiers, version strings, tenants, and request
  details stay out of labels.
- Use `plan_api_idempotency_metric` before publishing mutating admin
  idempotency counters so only closed low-cardinality `api_idempotency_surface`
  and `api_idempotency_result` labels appear while raw idempotency keys, replay
  fingerprints, tenants, resources, and request details stay out of labels.
- Use `plan_idempotency_record_metric` before publishing durable idempotency
  record storage counters so only closed low-cardinality
  `idempotency_record_backend`, `idempotency_record_operation`, and
  `idempotency_record_result` labels appear while raw idempotency keys,
  fingerprints, tenants, request details, and stored response bodies stay out
  of labels.
- Use `plan_api_compatibility_metric` before publishing backward-compatibility
  check counters so only closed low-cardinality `api_compatibility_surface` and
  `api_compatibility_result` labels appear while raw routes, schema hashes, API
  versions, tenants, contract payloads, and request details stay out of labels.
- Use `plan_api_mutation_audit_metric` before publishing mutation audit coverage
  counters so only closed low-cardinality `api_mutation_audit_surface` and
  `api_mutation_audit_result` labels appear while audit event IDs, resources,
  tenants, raw failure text, and request details stay out of labels.
- Use `plan_api_version_metric` before publishing version-negotiation counters
  so only closed low-cardinality `api_version_surface` and
  `api_version_result` labels appear while raw version strings, paths, tenants,
  compatibility payloads, and request details stay out of labels.
- Use `plan_api_spec_publication_metric` before publishing OpenAPI or schema
  artifact counters so only closed low-cardinality `api_spec_surface` and
  `api_spec_publication_result` labels appear while schema hashes, raw paths,
  tenants, payloads, and request details stay out of labels.
- Use `plan_api_error_envelope_metric` before publishing stable error-envelope
  counters so only closed low-cardinality `api_error_envelope_surface` and
  `api_error_envelope_result` labels appear while raw error text, provider
  secrets, response bodies, tenants, and request details stay out of labels.
- Use `plan_api_body_limit_metric` before publishing request-body-limit counters
  so only closed low-cardinality `api_body_limit_surface` and
  `api_body_limit_result` labels appear while raw content lengths, paths,
  tenants, payloads, and request details stay out of labels.
- Use `plan_api_timeout_budget_metric` before publishing timeout-budget
  counters so only closed low-cardinality `api_timeout_budget_surface` and
  `api_timeout_budget_result` labels appear while raw deadlines, durations,
  paths, tenants, and request details stay out of labels.
- Use `plan_api_cancellation_metric` before publishing cancellation propagation
  counters so only closed low-cardinality `api_cancellation_surface` and
  `api_cancellation_source` labels appear while session IDs, connection IDs,
  paths, tenants, and request details stay out of labels.
- Use `plan_api_stream_backpressure_metric` before publishing streaming
  backpressure counters so only closed low-cardinality
  `api_stream_backpressure_surface` and `api_stream_backpressure_state` labels
  appear while stream IDs, session IDs, connection IDs, tenants, and request
  details stay out of labels.
- Use `plan_api_admission_metric` before publishing bounded HTTP admission
  decisions so accepted, global-limit, route-limit, queue-full, and draining
  outcomes use only closed low-cardinality `api_admission_route` and
  `api_admission_result` labels while tenant IDs, request IDs, paths,
  connection IDs, raw capacities, queue depths, and policy internals stay out of
  labels.
- Use `plan_provider_metric` before publishing provider latency and error
  metrics so counts and durations use only closed low-cardinality `provider`
  and `provider_result` labels while models, endpoints, tenants, prompts,
  credential names, request IDs, and raw upstream error text stay out of labels.
- Use `plan_provider_capability_negotiation_metric` before publishing provider
  capability negotiation counters so only closed low-cardinality `provider`,
  `provider_capability`, and `provider_capability_result` labels appear while
  model names, endpoints, tenants, request IDs, and raw negotiation errors stay
  out of labels.
- Use `plan_provider_retry_metric` before publishing provider retry decision
  counters so only closed low-cardinality `provider`, `provider_retry_stage`,
  and `provider_retry_outcome` labels appear while retry keys, request IDs,
  endpoints, models, tenants, retry-after values, and raw upstream errors stay
  out of labels.
- Use `plan_provider_circuit_breaker_metric` before publishing provider
  circuit-breaker counters so only closed low-cardinality `provider`,
  `provider_circuit_breaker_decision`, and `provider_circuit_breaker_event`
  labels appear while retry-after values, endpoints, models, tenants, request
  IDs, and raw upstream errors stay out of labels.
- Use `plan_provider_degradation_metric` before publishing provider degradation
  alert counters so error-rate, latency, overload, transport, and circuit-open
  signals use only closed low-cardinality `provider`,
  `provider_degradation_signal`, and `provider_degradation_severity` labels
  while tenant, model, endpoint, credential, request, prompt, and raw upstream
  error details stay out of labels.
- Use `plan_streaming_lifecycle_metric` before publishing streaming lifecycle
  telemetry so completions, cancellations, interruptions, and guardrail blocks
  use only closed low-cardinality `stream_transport` and `stream_outcome`
  labels while tenant, request, provider endpoint, model, prompt, and raw error
  details stay out of labels.
- Use `plan_routing_decision_metric` before publishing routing decision
  counters so selected, fallback, rejected, and no-candidate outcomes use only
  closed low-cardinality `routing_lane` and `routing_outcome` labels while
  tenant, profile, request, provider endpoint, model, prompt, and raw error
  details stay out of labels.
- Use `plan_audit_metric` before publishing audit emission, persistence, export,
  or drop counters so only closed low-cardinality `audit_operation` and
  `audit_result` labels appear while tenant, audit event, principal, resource,
  endpoint, and storage error details stay out of labels.
- Use `plan_audit_query_lifecycle_metric` before publishing audit query and
  export planning, pagination, serialization, denial, empty-result, or failure
  counters so only closed low-cardinality `audit_query_operation` and
  `audit_query_result` labels appear while tenant, principal, audit event ID,
  cursor, time range, export format, request payload, and raw storage errors stay
  out of labels.
- Use `plan_audit_chain_metric` before publishing append-only audit chain
  append, link verification, range verification, proof export, conflict, digest
  failure, gap, or storage failure counters so only closed low-cardinality
  `audit_chain_operation` and `audit_chain_result` labels appear while tenant,
  audit event ID, digest value, chain topology, principal ID, resource ID,
  request payload, and raw storage error details stay out of labels.
- Use `plan_audit_retention_purge_metric` before publishing destructive audit
  retention purge progress counters so only closed low-cardinality
  `audit_retention_operation` and `audit_retention_result` labels appear while
  tenant, audit event ID, batch ID, cursor, storage error, and raw retention
  policy details stay out of labels.
- Use `plan_security_decision_metric` before publishing authentication, tenant
  resolution, authorization, or credential-scope decision counters so only
  closed low-cardinality `security_decision` and `security_result` labels appear
  while tenant, principal, role, claim, token, key, request, and policy details
  stay out of labels.
- Use `plan_authn_token_validation_metric` before publishing OIDC/JWT token
  decode, signature, claim, tenant-claim, role-claim, or JWKS-cache validation
  counters so only closed low-cardinality `authn_validation_stage` and
  `authn_validation_result` labels appear while issuer, key ID, tenant ID,
  principal ID, raw claim value, token material, JWKS key material, cache
  internals, and raw validation errors stay out of labels.
- Use `plan_authz_decision_metric` before publishing data-plane, quota,
  control-plane read, control-plane mutation, billing, or break-glass
  authorization counters so only closed low-cardinality `authz_boundary` and
  `authz_result` labels appear while tenant, principal, role name, resource ID,
  action name, policy revision, credential internals, request payload, and raw
  authorization errors stay out of labels.
- Use `plan_credential_scope_mismatch_metric` before publishing route-scoped
  data-plane-to-control-plane, control-plane-to-data-plane, break-glass misuse,
  missing-credential, rejection, audit, or failure counters so only closed
  low-cardinality `credential_scope_direction` and `credential_scope_result`
  labels appear while tenant, principal, token material, credential ID, route
  path, route internals, request payload, and raw authorization errors stay out
  of labels.
- Use `plan_tenant_isolation_metric` before publishing authentication,
  authorization, storage-predicate, cache-key, audit-query, cross-tenant denial,
  missing-tenant denial, mismatch rejection, or failure counters so only closed
  low-cardinality `tenant_isolation_surface` and `tenant_isolation_result`
  labels appear while tenant ID, principal ID, resource ID, storage key, cache
  key, query predicate, request payload, and raw storage or authorization errors
  stay out of labels.
- Use `plan_postgres_tenant_context_metric` before publishing PostgreSQL
  tenant-context setup, verification, RLS policy, tenant DML, missing-context,
  mismatch, denial, or failure counters so only closed low-cardinality
  `postgres_tenant_context_operation` and `postgres_tenant_context_result`
  labels appear while tenant ID, SQL statement text, query text, database
  endpoint, storage key, request payload, and raw storage errors stay out of
  labels.
- Use `plan_redis_coordination_metric` before publishing Redis distributed
  limiter, short-lived cache, recovery lease, unavailable, limited, cache-miss,
  or failure counters so only closed low-cardinality
  `redis_coordination_operation` and `redis_coordination_result` labels appear
  while tenant ID, Redis key, lease-owner token, endpoint, script SHA, request
  payload, and raw storage errors stay out of labels.
- Use `plan_identity_context_metric` before publishing authentication,
  authorization, audit, control-plane, data-plane, missing principal,
  missing tenant, tenant mismatch, correlation-missing, or failure counters so
  only closed low-cardinality `identity_context_surface` and
  `identity_context_result` labels appear while tenant ID, principal ID, request
  ID, audit event ID, trace ID, raw context values, raw claim values, request
  payload, and raw validation errors stay out of labels.
- Use `plan_break_glass_lifecycle_metric` before publishing break-glass request,
  approval, activation, revocation, expiry, denial, persistence, or failure
  counters so only closed low-cardinality `break_glass_operation` and
  `break_glass_result` labels appear while tenant, principal ID, emergency token,
  TTL, request payload, authorization topology, and raw storage error details
  stay out of labels.
- Use `plan_user_lifecycle_metric` before publishing user invite or SCIM
  create, update, and delete counters so only closed low-cardinality
  `user_lifecycle_operation` and `user_lifecycle_result` labels appear while
  tenant, user ID, email, SCIM resource ID, role assignment, request payload,
  identity-provider detail, and raw storage error details stay out of labels.
- Use `plan_service_identity_lifecycle_metric` before publishing service
  identity creation, secret rotation, disablement, authorization, persistence,
  denial, or failure counters so only closed low-cardinality
  `service_identity_operation` and `service_identity_result` labels appear
  while tenant, principal ID, client ID, service name, secret reference, request
  payload, and raw storage error details stay out of labels.
- Use `plan_role_binding_lifecycle_metric` before publishing role grant,
  revoke, authorization, persistence, denial, or failure counters so only closed
  low-cardinality `role_binding_operation` and `role_binding_result` labels
  appear while tenant, principal ID, role name, role binding ID, request payload,
  caller identity, and raw storage error details stay out of labels.
- Use `plan_provider_credential_lifecycle_metric` before publishing provider
  credential rotation, reference validation, persistence, authorization, denial,
  or failure counters so only closed low-cardinality
  `provider_credential_operation` and `provider_credential_result` labels
  appear while tenant, provider credential ID, provider account, secret
  reference, request payload, caller identity, and raw storage error details
  stay out of labels.
- Use `plan_virtual_key_lifecycle_metric` before publishing virtual key
  creation, secret rotation, secret-reference persistence, authorization,
  denial, or failure counters so only closed low-cardinality
  `credential_lifecycle_operation` and `credential_lifecycle_result` labels
  appear while tenant, virtual key ID, key name, secret reference, generated key
  material, request payload, caller identity, and raw storage error details stay
  out of labels.
- Use `plan_budget_policy_lifecycle_metric` before publishing budget policy
  update, scope validation, persistence, authorization, denial, or failure
  counters so only closed low-cardinality `budget_policy_operation` and
  `budget_policy_result` labels appear while tenant, budget scope, limit amount,
  request payload, caller identity, and raw storage error details stay out of
  labels.
- Use `plan_policy_lifecycle_metric` before publishing policy create, update,
  publish, invalidation, authorization, persistence, denial, or failure counters
  so only closed low-cardinality `policy_lifecycle_operation` and
  `policy_lifecycle_result` labels appear while tenant, policy ID, policy
  revision, digest, signature, request payload, caller identity, and raw storage
  error details stay out of labels.
- Use `plan_tenant_lifecycle_metric` before publishing tenant create, update,
  authorization, persistence, denial, or failure counters so only closed
  low-cardinality `account_lifecycle_operation` and `account_lifecycle_result`
  labels appear while tenant ID, display name, request payload, caller identity,
  and raw storage error details stay out of labels.
- Use `plan_accounting_metric` before publishing reservation, reconciliation,
  and budget rejection counters so only closed low-cardinality
  `accounting_operation` and `accounting_result` labels appear while tenant,
  virtual key, reservation ID, call ID, ledger ID, prompt, and raw rejection
  details stay out of labels.
- Use `plan_reservation_recovery_metric` before publishing expired reservation
  scan, recovery lease, budget release, ledger write, skip, contention, or
  failure counters so only closed low-cardinality
  `reservation_recovery_operation` and `reservation_recovery_result` labels
  appear while tenant, reservation ID, lease key, token amount, request, prompt,
  and raw storage error details stay out of labels.
- Use `plan_billing_ledger_metric` before publishing append-only billing ledger
  append, skip, query, or failure counters so only closed low-cardinality
  `billing_ledger_operation` and `billing_ledger_result` labels appear while
  tenant, virtual key, reservation ID, call ID, ledger row ID, token amount,
  request, prompt, and raw storage error details stay out of labels.
- Use `plan_budget_rejection_metric` before publishing budget rejection counters
  so tenant-budget, virtual-key-budget, rate-limit, reservation-unavailable,
  and policy-denied cases use only the closed low-cardinality
  `budget_rejection_reason` label while tenant ID, virtual key ID, request,
  prompt, policy revision, caller identity, and raw rejection details stay out
  of labels.
- Use `plan_rate_limit_decision_metric` before publishing rate-limit decision
  counters so tenant, virtual-key, principal, and provider scopes use only
  closed low-cardinality `rate_limit_scope` and `rate_limit_decision` labels
  while tenant ID, virtual key ID, principal ID, provider endpoint, request,
  retry key, prompt, and raw limiter/storage error details stay out of labels.
- Use `plan_quota_correctness_metric` before publishing quota correctness
  counters so reservation overshoot, duplicate-charge prevention, missing commit
  recovery, missing release recovery, and ledger mismatch events use only the
  closed low-cardinality `quota_correctness_event` label while tenant ID,
  virtual key ID, reservation ID, call ID, ledger ID, amount, request, prompt,
  and raw storage error details stay out of labels.
- Use `plan_shutdown_lifecycle_metric` before publishing graceful shutdown and
  drain lifecycle counters so signal receipt, draining start, readiness
  disablement, in-flight drain completion, timeout, and final completion use
  only closed low-cardinality `shutdown_event` and `shutdown_result` labels
  while pod, node, signal value, tenant, request, connection, profile, and raw
  shutdown error details stay out of labels.
- Use `plan_health_probe_metric` before publishing `/livez`, `/readyz`, and
  `/startupz` result counters so passing, degraded, failing, and draining states
  use only closed low-cardinality `health_probe` and `health_result` labels
  while raw path, tenant, dependency, backend endpoint, policy revision, pod,
  request, and raw dependency error details stay out of labels.
- Use `plan_secret_provider_metric` before publishing secret-provider operation
  counters so only closed low-cardinality `secret_backend`,
  `secret_operation`, and `secret_result` labels appear while tenant, secret
  ID, path, keyring account, credential value, key material, request, and raw
  provider error details stay out of labels.
- Use `plan_secret_rotation_metric` before publishing secret rotation counters
  so provider credentials, OIDC clients, signing keys, storage credentials, and
  webhook secrets use only closed low-cardinality `secret_scope` and
  `secret_rotation_result` labels while tenant, secret ID, path, endpoint,
  credential value, key material, request, and raw provider error details stay
  out of labels.
- Use `plan_backup_restore_metric` before publishing backup, restore,
  verification, or drill counters so only closed low-cardinality
  `backup_restore_operation` and `backup_restore_result` labels appear while
  tenant, backup ID, path, object key, checksum, timestamp, endpoint, request,
  and raw storage error details stay out of labels.
- Use `plan_deployment_rollout_metric` before publishing deployment rollout
  counters so apply, verify, promote, and rollback operations use only closed
  low-cardinality `deployment_rollout_operation` and
  `deployment_rollout_result` labels while namespace, pod, image digest,
  revision, tenant, environment, request, and raw deployment error details stay
  out of labels.
- Use `plan_load_soak_metric` before publishing load, soak, spike, or recovery
  test counters and durations so only closed low-cardinality
  `load_soak_scenario` and `load_soak_result` labels appear while tenant,
  profile, run ID, environment, threshold value, request, prompt, and raw
  failure details stay out of labels.
- Use `plan_fault_injection_metric` before publishing PostgreSQL, Redis, IdP,
  or provider fault-injection counters so only closed low-cardinality
  `fault_injection_target` and `fault_injection_result` labels appear while
  tenant, backend endpoint, provider account, run ID, request, prompt, and raw
  injected error details stay out of labels.
- Use `plan_migration_lifecycle_metric` before publishing migration status,
  compatibility, apply, or rollback counters so only closed low-cardinality
  `migration_operation` and `migration_result` labels appear while tenant,
  migration version, lock owner, database endpoint, schema, request, and raw
  migration error details stay out of labels.
- Use `plan_persistence_metric` before publishing storage operation counters so
  reads, writes, commits, rollbacks, and health checks use only closed
  low-cardinality `persistence_operation` and `persistence_result` labels while
  tenant, resource, storage key, backend endpoint, query, request, and raw
  storage error details stay out of labels.
- Use the domain security response planners before exposing tenant resolution,
  tenant access, role-claim mapping, credential-scope, or resource-action
  authorization failures so tenant IDs, role claim values, scopes, role names,
  and authorization topology stay out of client-visible error envelopes.
- Use `plan_gateway_http_error_response` for local pre-upstream HTTP policy
  failures so client-visible errors stay stable and do not leak parser, route,
  byte-count, or policy internals.
- Use `plan_application_data_plane_error_response` from composition roots so
  HTTP-policy and gateway-admission errors receive stable redacted envelopes
  and wrong-route data-plane requests receive stable redacted route-denial
  responses before admission or provider invocation. Data-plane and quota-read
  wrong-route display output uses one generic unavailable-route message so
  local stringified errors do not expose route topology; see
  `docs/adr/0867-application-route-display-redaction.md`.
- Keep local buffered provider response transformation failures behind the
  stable `provider response could not be transformed` 502 body so Copilot,
  Gemini, DeepSeek, and other compatibility rewrites do not expose parser,
  provider-payload, or implementation diagnostics.
- Keep Gemini Live local/provider stream client errors stable and redact
  secret-like bearer token or endpoint query material from detailed runtime log
  diagnostics.
- Redact secret-like material in shared provider `response.failed` SSE messages
  so Gemini and DeepSeek compatibility stream failures do not expose bearer
  tokens or key-bearing provider URLs.
- Redact runtime HTTP/websocket dispatch error log values before persistence so
  capture, rewrite, transport, and session diagnostics do not expose bearer
  tokens or key-bearing URLs.
- Redact runtime auto-redeem quota error log values before persistence so usage
  availability, reset-credit consumption, and refresh failures do not expose
  bearer tokens or key-bearing URLs.
- Redact runtime streaming writer/read error log values before persistence so
  transport diagnostics keep chunk and byte context without exposing bearer
  tokens or key-bearing URLs.
- Redact runtime WebSocket transport error log values before persistence so
  connect pressure, proxy tunnel, upstream read, and upstream send diagnostics
  do not expose bearer tokens or key-bearing URLs.
- Redact runtime prefetch upstream error values before storing terminal
  prefetch state, runtime log fields, or queued prefetch error chunks so
  lookahead and handoff diagnostics do not expose bearer tokens or key-bearing
  URLs.
- Redact runtime local-rewrite upstream error log values before persistence so
  provider gateway failures keep context without exposing bearer tokens or
  key-bearing URLs.
- Redact Gemini semantic compact fallback reason log values before persistence
  so remote compact fallback diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact Gemini OAuth auth-refresh and quota-pool error log values before
  persistence so provider profile diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact smart-context token calibration save error log values before
  persistence so context budgeting diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Keep `prodex-inspect` and `prodex-memory` MCP invalid-params and
  method-not-found responses stable so tool argument values, unknown tool
  names, method names, local paths, and Mem0 upstream diagnostics are not echoed
  in client-visible JSON-RPC responses.
- Redact root CLI stderr errors at `main_entry` so any unhandled command error
  chain keeps context without exposing bearer tokens or key-bearing URLs.
- Redact secret-like material in `prodex capability` setup/status diagnostics
  so asset, memory, and Presidio check failures keep operator signal without
  exposing bearer tokens or key-bearing URLs.
- Redact `prodex capability` install-check and Super Doctor `fail (...)`
  status rows so capability probe failures do not expose bearer tokens or
  key-bearing URLs.
- Redact secret-like material at the dashboard JSON and request-loop diagnostic
  boundaries so local API failures do not echo bearer tokens, key-bearing URLs,
  or request-derived secrets.
- Redact profile identity auth/quota fallback error chains before composing
  account email or account identity lookup failures, preserving fallback context
  without exposing auth-secret or quota diagnostic material.
- Redact `prodex expose` tunnel startup error chains before terminal status
  rendering so cloudflared diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact Presidio helper response-body and health-probe diagnostics so Analyzer
  and Anonymizer failures do not leak bearer tokens or key-bearing URLs through
  higher-level callers.
- Redact `prodex info` secret-backend selection errors before human or JSON
  rendering so backend misconfiguration diagnostics do not expose bearer tokens
  or key-bearing URLs.
- Redact `prodex doctor --quota` per-profile error summaries before terminal
  rendering so quota probe diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact runtime selection quota probe errors before storing them in
  `RunProfileProbeReport` so launch preflight, doctor, info, and selection views
  do not render bearer tokens or key-bearing URLs.
- Redact selected-profile runtime launch quota preflight warnings before
  terminal rendering so fail-open launch notices do not expose bearer tokens or
  key-bearing URLs.
- Redact aggregated runtime provider profile auth errors for Anthropic, Copilot,
  and Gemini OAuth launch paths so profile diagnostics do not expose bearer
  tokens or key-bearing URLs.
- Redact runtime broker persistence-owner promotion errors before writing
  runtime logs so owner-lock diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact runtime scheduled-save error fields before writing runtime logs so
  state, continuation-journal, and artifact persistence failures do not expose
  bearer tokens or key-bearing URLs.
- Redact smart-context token calibration save error fields before writing
  runtime logs so calibration persistence failures do not expose bearer tokens
  or key-bearing URLs.
- Redact runtime profile probe refresh error fields before writing runtime logs
  so quota probe and state-update diagnostics do not expose bearer tokens or
  key-bearing URLs.
- Redact runtime background worker spawn and panic error fields before writing
  runtime logs so worker diagnostics do not expose bearer tokens or key-bearing
  URLs.
- Redact quota command/watch/provider diagnostics before storing them in
  `QuotaReport`, watch snapshots, or external detail rows so quota views do not
  render bearer tokens or key-bearing URLs.
- Redact quota watch TUI fallback diagnostics before writing stderr so terminal
  setup failures do not expose bearer tokens or key-bearing URLs.
- Redact non-validation Gemini quota error bodies before building local errors
  so generic Code Assist failures do not expose bearer tokens or key-bearing
  URLs while validation guidance remains visible.
- Redact non-validation Gemini Code Assist setup error bodies before building
  local errors so onboarding failures do not expose bearer tokens or key-bearing
  URLs while validation guidance remains visible.
- Redact Gemini OAuth token and user-info error bodies before building local
  errors so sign-in and refresh failures do not expose bearer tokens or
  key-bearing URLs.
- Use `plan_authentication_error_response` for client-visible authn failures so
  JWKS cache state, key IDs, IdP claim details, and role-mapping internals stay
  out of API responses.
- Use the identity/JWKS response planners before exposing identity
  configuration, token-validation, or key-cache unavailability failures so
  issuer, audience, key IDs, JWKS key counts, cache timing, retry/backoff, and
  last-known-good internals stay out of client-visible error envelopes.
- Use `plan_authorization_error_response` for client-visible authz failures so
  boundary kind, credential-scope internals, role names, and tenant IDs stay out
  of API responses.
- Use `plan_control_plane_authorization_error_response` for admin API denial
  responses so operation/resource topology, break-glass timing, scope internals,
  role names, and tenant IDs stay out of API responses.
- Use `plan_config_publication_error_response` for admin configuration
  publication failures so tenant IDs, policy revision IDs, payload contents, and
  cache internals stay out of API responses.
- Use `plan_config_cache_window_error_response` for readiness, admin, or
  validation APIs that expose configuration cache-window failures so refresh,
  stale, expiry, TTL, revision, and last-known-good internals stay out of API
  responses.
- Use `plan_config_invalidation_error_response` for config invalidation
  failures so tenant IDs, revision IDs, invalidation targets, and cache
  internals stay out of API responses.
- Use the policy response planners before exposing policy activation,
  refresh-window, expiration, or invalidation failures so revision IDs,
  digest/signature values, cache timing, invalidation targets, and
  last-known-good internals stay out of client-visible error envelopes.
- Use `plan_configuration_publication_error_response` for the composed
  control-plane publication use case so authorization and revision-validation
  failures share one stable admin API response boundary.
- Use `plan_application_configuration_publication_decision_error_response` and
  `plan_application_configuration_publication_error_response` at composition
  roots so denied publication decisions and audit-storage failures stay behind
  stable, redacted admin API responses.
- Use `plan_secret_error_response` and
  `plan_refresh_lease_error_response` before exposing secret store or token
  refresh coordination failures so filesystem paths, keyring accounts, and
  refresh-token-derived lock details stay out of API responses.
- Use the domain secret response planners before exposing secret resolution or
  rotation-policy failures so secret names, provider paths, versions, refresh
  timing, raw material, and backend topology stay out of client-visible error
  envelopes.
- Use `plan_session_resolve_error_response` before exposing resume/session
  lookup failures so session selectors, matching session IDs, paths, and
  affinity metadata stay out of API responses.
- Use `plan_gateway_admission_error_response` before provider dispatch. It
  adapts `plan_atomic_reservation_error_response` for reservation planning
  failures and `plan_span_error_response` for telemetry planning failures while
  keeping authorization and provider invocation failures behind their own stable
  boundaries.
- Use the domain accounting response planners before exposing budget,
  reservation commit, recovery, or reconciliation failures so tenant IDs, call
  IDs, reservation IDs, usage amounts, ledger keys, and arithmetic internals
  stay out of client-visible error envelopes.
  Domain accounting error-response plan debug output is restricted to the
  stable client-facing status, code, and message fields; see
  `docs/adr/0763-domain-accounting-error-response-plan-debug-redaction.md`.
- Use `plan_provider_capability_negotiation_error_response` before exposing
  model/provider negotiation failures so provider names, model names, missing
  capability sets, and route topology stay out of client-visible error
  envelopes. Provider route display output uses generic wording so local
  stringified errors do not expose model-field metadata; see
  `docs/adr/0858-provider-route-display-redaction.md`.
- Use `plan_provider_retry` before concrete provider transport retry so retry
  budgets, stream commit state, cancellation state, provider routes, and attempt
  counts do not leak into client-visible behavior.
- Provider invocation display output uses generic authorization wording so local
  stringified errors do not expose tenant or credential-scope topology; see
  `docs/adr/0859-provider-invocation-display-topology-redaction.md`.
- Use `plan_provider_circuit_breaker` and
  `plan_provider_circuit_breaker_event` before exposing provider health or retry
  behavior so model names, failure counts, cooldown timestamps, and route
  topology stay out of client-visible responses.
- Use the rate-limit response planners before exposing distributed limiter
  denials or atomic-update planning failures so tenant IDs, virtual-key IDs,
  bucket cache keys, reset timestamps, remaining capacity, and usage counters
  stay out of client-visible error envelopes.
- Use `plan_gateway_http_error_response` for HTTP-local policy failures. It
  adapts `plan_trace_context_error_response` for invalid or duplicate
  traceparent headers so trace parsing internals, ambiguous header ordering, and
  timeout budget internals stay out of API responses.
  Gateway HTTP plan display output redacts duplicate authorization, account,
  session, Codex metadata, and trace-context header names while preserving typed
  variants for response planning; see
  `docs/adr/0850-gateway-http-plan-display-header-redaction.md`.
  Gateway HTTP idempotency-key and entity-tag display errors also use generic
  request metadata wording while preserving typed response planners; see
  `docs/adr/0851-gateway-http-metadata-display-redaction.md`.
  Gateway HTTP pagination display errors use generic metadata wording while
  preserving typed pagination response planners; see
  `docs/adr/0852-gateway-http-pagination-display-redaction.md`.
  Gateway HTTP route method display output uses generic method-not-allowed
  wording while preserving route and method fields for response planning; see
  `docs/adr/0854-gateway-http-route-method-display-redaction.md`.
- Use `plan_trace_id_error_response` before exposing domain trace ID validation
  failures so trace values, lengths, request IDs, call IDs, tenant IDs, audit
  event IDs, and traceparent internals stay out of client-visible error
  envelopes.
  Trace ID display output also uses the stable planner message; see
  `docs/adr/0899-domain-boundary-token-display-redaction.md`.
- Use `plan_telemetry_attribute_error_response` before exposing telemetry
  attribute validation failures so metric keys, values, lengths, tenant/user
  IDs, principal IDs, virtual-key identifiers, API keys, prompts, request/call
  IDs, and high-cardinality telemetry material stay out of client-visible error
  envelopes.
- Use `ErrorCode::try_new` or `ErrorCode::validate` plus
  `plan_error_code_error_response` before exposing dynamic error-envelope codes
  so uppercase text, path fragments, raw rejected codes, lengths, tenant IDs,
  request IDs, audit IDs, and credential-like material stay out of
  client-visible error envelopes.
  Error code debug output preserves code presence while redacting raw code
  strings; see `docs/adr/0796-domain-error-code-debug-redaction.md`.
  Error code error debug output preserves validation failure shape while
  redacting rejected lengths; see
  `docs/adr/0797-domain-error-code-error-debug-redaction.md`.
  Error code display output also uses the stable planner message; see
  `docs/adr/0899-domain-boundary-token-display-redaction.md`.
  Error metadata debug output preserves identifier presence and retryability
  while redacting raw request, tenant, and audit-event identifiers; see
  `docs/adr/0798-domain-error-metadata-debug-redaction.md`.
  Error envelope debug output preserves version and category while redacting
  message content and nested code/metadata values; see
  `docs/adr/0799-domain-error-envelope-debug-redaction.md`.
- Use `AuditAction::try_new` or `AuditAction::validate` plus
  `plan_audit_action_error_response` before accepting dynamic audit action
  names so raw routes, rejected action strings, lengths, tenant IDs, principal
  IDs, resource IDs, and credential-like material stay out of audit-facing or
  client-visible error envelopes. The legacy `AuditEvent::new` constructor
  also replaces raw actions that fail this validator with
  `audit.invalid_action` before they become durable audit metadata; see
  `docs/adr/0647-audit-event-raw-action-boundary.md`.
  Domain audit action debug output redacts exact action names while durable
  serialization remains exact; see
  `docs/adr/0737-domain-audit-action-debug-redaction.md`.
  Domain audit action validation-error debug output redacts rejected input
  lengths while preserving variant shape; see
  `docs/adr/0739-domain-audit-action-error-debug-redaction.md`.
  Domain audit event debug output redacts event IDs, timestamps, tenant IDs,
  principal IDs, resource identifiers, and reason codes while durable
  serialization remains complete; see
  `docs/adr/0723-domain-audit-event-debug-redaction.md`.
- Use `AuditResource::try_new` or `AuditResource::validate_kind` plus
  `plan_audit_resource_kind_error_response` before accepting dynamic audit
  resource kinds so raw routes, rejected kind strings, lengths, tenant IDs,
  principal IDs, resource IDs, and credential-like material stay out of
  audit-facing or client-visible error envelopes. The legacy
  `AuditResource::new` constructor also replaces raw kinds that fail this
  validator with `audit.invalid_resource` before they become durable audit
  metadata; see `docs/adr/0648-audit-resource-raw-kind-boundary.md`.
  Domain audit resource-kind validation-error debug output redacts rejected
  input lengths while preserving variant shape; see
  `docs/adr/0741-domain-audit-resource-kind-error-debug-redaction.md`.
- Use `AuditResourceId` plus `plan_audit_resource_id_error_response` before
  accepting dynamic audit resource identifiers so raw routes, rejected IDs,
  lengths, tenant IDs, principal IDs, credential-like material, and provider
  account material stay out of audit-facing or client-visible error envelopes.
  The legacy `AuditResource::new` constructor also drops raw resource IDs that
  fail this validator before they become durable audit metadata; see
  `docs/adr/0646-audit-resource-raw-id-boundary.md`.
  Domain audit resource debug output redacts resource IDs and tenant IDs while
  preserving the resource kind for low-cardinality diagnostics; see
  `docs/adr/0722-domain-audit-resource-debug-redaction.md`.
  Domain audit resource-ID validation-error debug output redacts rejected input
  lengths while preserving variant shape; see
  `docs/adr/0740-domain-audit-resource-id-error-debug-redaction.md`.
- Use `AuditReasonCode` plus `plan_audit_reason_code_error_response` before
  accepting dynamic audit reason codes so raw provider errors, routes, rejected
  reason strings, lengths, tenant IDs, principal IDs, resource IDs, and
  credential-like material stay out of audit-facing or client-visible error
  envelopes. The legacy `AuditEvent::new` constructor also drops raw reason
  strings that fail this validator before they become durable audit metadata;
  see `docs/adr/0645-audit-event-raw-reason-code-boundary.md`.
  Domain audit reason-code debug output redacts exact reason codes while
  durable serialization remains exact; see
  `docs/adr/0738-domain-audit-reason-code-debug-redaction.md`.
  Domain audit reason-code validation-error debug output redacts rejected input
  lengths while preserving variant shape; see
  `docs/adr/0742-domain-audit-reason-code-error-debug-redaction.md`.
  Domain audit reason-code display output also uses the stable planner message;
  see `docs/adr/0898-domain-audit-policy-display-redaction.md`.
- Use `AuditDigest::new` plus `plan_audit_digest_error_response` before
  accepting dynamic audit chain digest values so raw digests, chain material,
  rejected digest strings, lengths, tenant IDs, principal IDs, resource IDs,
  and credential-like material stay out of audit-facing or client-visible error
  envelopes. Digest values are validated and stored exactly, without trimming;
  see `docs/adr/0649-audit-digest-exact-boundary.md`.
  Domain audit digest/envelope debug output redacts digest material and nested
  event identifiers while serialization and chain verification remain exact; see
  `docs/adr/0730-domain-audit-envelope-debug-redaction.md`.
  Domain audit chain validation-error debug output keeps only low-cardinality
  chain verification failure shape; see
  `docs/adr/0762-domain-audit-chain-error-debug-redaction.md`.
  Domain audit digest validation-error debug output redacts rejected input
  lengths while preserving variant shape; see
  `docs/adr/0743-domain-audit-digest-error-debug-redaction.md`.
  Domain audit digest display output also uses the stable planner message; see
  `docs/adr/0898-domain-audit-policy-display-redaction.md`.
- Use `AuditTimestamp` plus `plan_audit_timestamp_error_response` before
  accepting dynamic audit event occurrence timestamps so rejected timestamp
  values, tenant IDs, principal IDs, resource IDs, audit chain material, and
  credential-like material stay out of audit-facing or client-visible error
  envelopes.
  Domain audit timestamp debug output redacts exact millisecond values while
  preserving ordering and cutoff behavior; see
  `docs/adr/0736-domain-audit-timestamp-debug-redaction.md`.
  Domain audit timestamp validation-error debug output redacts rejected
  timestamp values while preserving variant shape; see
  `docs/adr/0744-domain-audit-timestamp-error-debug-redaction.md`.
- Use `AuditTimeRange` plus `plan_audit_time_range_error_response` before
  accepting dynamic audit query/export time ranges so rejected range bounds,
  tenant IDs, principal IDs, resource IDs, audit chain material, and
  credential-like material stay out of audit-facing or client-visible error
  envelopes.
  Domain audit time range debug output redacts exact query boundary timestamps
  while preserving bound presence; see
  `docs/adr/0732-domain-audit-time-range-debug-redaction.md`.
  Domain audit time-range validation-error debug output keeps only
  low-cardinality range validation failure shape; see
  `docs/adr/0760-domain-audit-time-range-error-debug-redaction.md`.
  Domain audit time-range display output also uses the stable planner message;
  see `docs/adr/0898-domain-audit-policy-display-redaction.md`.
- Use `AuditPageLimit` plus `plan_audit_page_limit_error_response` before
  accepting dynamic audit query/export page limits so rejected values, tenant
  IDs, principal IDs, resource IDs, audit chain material, and credential-like
  material stay out of audit-facing or client-visible error envelopes.
  Domain audit page-limit debug output redacts exact limit values while
  preserving pagination behavior; see
  `docs/adr/0733-domain-audit-page-limit-debug-redaction.md`.
  Domain audit page-limit validation-error debug output redacts rejected limit
  values while preserving variant shape; see
  `docs/adr/0745-domain-audit-page-limit-error-debug-redaction.md`.
- Use `AuditQueryScope` plus `plan_audit_query_scope_error_response` before
  serializing audit query/export results so cross-tenant events fail closed and
  tenant IDs, principal IDs, resource IDs, audit event IDs, and audit payload
  material stay out of audit-facing or client-visible error envelopes.
  Domain audit query-scope debug output redacts tenant IDs, including when the
  scope is nested inside query/export/retention plans; see
  `docs/adr/0724-domain-audit-query-scope-debug-redaction.md`.
  Domain audit query-scope validation-error debug output keeps only
  low-cardinality tenant-isolation failure shape; see
  `docs/adr/0756-domain-audit-query-scope-error-debug-redaction.md`.
- Use `AuditQueryPlan` plus `plan_audit_query_plan_error_response` before
  applying audit query/export filters so tenant scoping, time ranges, page
  limits, and sort order share one domain contract and raw tenant IDs, event
  timestamps, event IDs, and audit payload material stay out of client-visible
  errors.
  Domain audit query/export plan debug output redacts tenant scope, time-range,
  and page-limit internals while preserving low-cardinality sort/format shape;
  see `docs/adr/0725-domain-audit-query-plan-debug-redaction.md`.
  Domain audit query-plan validation-error debug output keeps scope, timestamp,
  and cursor-sort failure shape while preserving nested redaction; see
  `docs/adr/0753-domain-audit-query-plan-error-debug-redaction.md`.
  Domain audit query-plan cursor mismatch display output uses the stable cursor
  planner message; see
  `docs/adr/0919-domain-audit-cursor-mismatch-display-redaction.md`.
  Domain audit sort-order validation-error debug output keeps only
  low-cardinality sort validation failure shape; see
  `docs/adr/0757-domain-audit-sort-order-error-debug-redaction.md`.
- Use `AuditQueryPlan::select_events` before serializing in-memory audit query
  or export results so tenant scope, timestamp validation, sort order, and page
  limits are enforced by one fail-closed domain path.
- Use `AuditQueryCursor` plus `plan_audit_query_cursor_error_response` for
  audit query/export pagination so cursor version, sort order, timestamp, and
  audit event ID validation share one redacted domain contract.
  Domain audit query cursor debug output redacts cursor position timestamp and
  audit event ID while preserving opaque cursor compatibility; see
  `docs/adr/0731-domain-audit-query-cursor-debug-redaction.md`.
  Domain audit query-cursor validation-error debug output keeps parser,
  sort-order, timestamp, and event-ID failure shape while preserving nested
  redaction; see
  `docs/adr/0755-domain-audit-query-cursor-error-debug-redaction.md`.
  Domain audit query-cursor display output also uses the stable planner message;
  see `docs/adr/0903-domain-audit-query-cursor-display-redaction.md`.
- Use `AuditQueryPlan::select_events_after` or
  `AuditExportPlan::select_events_after` when applying audit pagination cursors
  so cursor position, sort order, tenant scope, timestamp validation, and page
  limits use the same fail-closed domain ordering.
- Use `AuditQueryPlan::page_events` or `AuditExportPlan::page_events` before
  returning audit query/export pages so page items and next cursors are built
  from one domain-owned pagination contract.
  Domain audit query-page validation-error debug output keeps query, timestamp,
  and cursor failure shape while preserving nested redaction; see
  `docs/adr/0754-domain-audit-query-page-error-debug-redaction.md`.
- Use `AuditRetentionPolicy` plus
  `plan_audit_retention_policy_error_response` before accepting control-plane
  audit retention settings so storage adapters receive bounded retention days
  and raw retention values stay out of client-visible errors.
  Domain audit retention-policy debug output redacts exact retention windows
  while preserving cutoff behavior; see
  `docs/adr/0734-domain-audit-retention-policy-debug-redaction.md`.
  Domain audit retention-policy validation-error debug output redacts rejected
  retention windows while preserving variant shape; see
  `docs/adr/0746-domain-audit-retention-policy-error-debug-redaction.md`.
  Domain audit retention-policy display output also uses the stable planner
  message; see `docs/adr/0898-domain-audit-policy-display-redaction.md`.
- Use `AuditRetentionPlan` plus
  `plan_audit_retention_plan_error_response` before retention cleanup selects
  purge candidates so tenant scope, cutoff calculation, and event timestamp
  validation remain fail-closed in the domain boundary.
  Domain audit retention-plan debug output redacts tenant scope, policy, and
  timestamp internals; see
  `docs/adr/0726-domain-audit-retention-plan-debug-redaction.md`.
  Domain audit retention-plan validation-error debug output keeps scope and
  timestamp failure shape while preserving nested redaction; see
  `docs/adr/0751-domain-audit-retention-plan-error-debug-redaction.md`.
- Use `AuditRetentionBatchLimit` and `AuditRetentionPlan::purge_candidates`
  for retention cleanup batches so storage deletes are bounded, deterministic,
  tenant-scoped, and oldest-first.
  Domain audit retention batch-limit debug output redacts exact batch sizes
  while preserving bounded cleanup behavior; see
  `docs/adr/0735-domain-audit-retention-batch-limit-debug-redaction.md`.
  Domain audit retention batch-limit validation-error debug output redacts
  rejected batch sizes while preserving variant shape; see
  `docs/adr/0747-domain-audit-retention-batch-limit-error-debug-redaction.md`.
- Use `AuditRetentionPlan::purge_candidate_page` for resumable retention
  cleanup so repeated bounded purge passes share one ascending cursor contract.
  Domain audit retention-page validation-error debug output keeps retention,
  decision, cursor, and timestamp failure shape while preserving nested
  redaction; see
  `docs/adr/0752-domain-audit-retention-page-error-debug-redaction.md`.
  Domain audit retention-page cursor mismatch display output uses the stable
  cursor planner message; see
  `docs/adr/0919-domain-audit-cursor-mismatch-display-redaction.md`.
- Use `AuditRetentionHold` plus
  `plan_audit_retention_hold_error_response` before deleting held audit events
  so legal-hold protection remains tenant-scoped, timestamp-validated, and
  redacted.
  Domain audit retention-hold debug output redacts tenant IDs, audit event IDs,
  reason codes, and expiry timestamps while durable serialization remains
  complete; see
  `docs/adr/0727-domain-audit-retention-hold-debug-redaction.md`.
  Domain audit retention-hold validation-error debug output keeps scope and
  timestamp failure shape while preserving nested redaction; see
  `docs/adr/0748-domain-audit-retention-hold-error-debug-redaction.md`.
- Use `AuditRetentionPlan::purge_decision` before executing retention deletes
  so each event is retained, protected by hold, or purge-eligible through one
  domain-owned decision contract.
  Domain audit retention decision-error debug output keeps retention-vs-hold
  failure shape while preserving nested redaction; see
  `docs/adr/0749-domain-audit-retention-decision-error-debug-redaction.md`.
- Use `AuditRetentionPlan::purgeable_candidates` for delete batches that must
  account for active legal holds before storage adapters execute purge
  operations.
- Use `AuditRetentionPlan::purgeable_candidate_page` for resumable legal-hold
  aware retention cleanup so storage adapters receive bounded purge pages,
  oldest-first cursors, and fail-closed hold validation from the domain.
- Use `AuditRetentionPurgeKey` or
  `AuditRetentionPlan::purgeable_candidate_key_page` before executing retention
  deletes so storage predicates carry both tenant ID and audit event ID.
  Domain audit retention purge-key debug output redacts tenant IDs and audit
  event IDs while serialization remains complete; see
  `docs/adr/0728-domain-audit-retention-purge-key-debug-redaction.md`.
- Use `AuditRetentionPurgeBatch` before issuing retention storage deletes so
  purge keys are tenant-homogeneous and bounded by the retention batch limit.
  Domain audit retention purge-batch debug output redacts tenant IDs and audit
  event IDs while preserving batch size for diagnostics; see
  `docs/adr/0729-domain-audit-retention-purge-batch-debug-redaction.md`.
  Domain audit retention purge-batch validation-error debug output redacts
  rejected batch counts and limits while preserving variant shape; see
  `docs/adr/0750-domain-audit-retention-purge-batch-error-debug-redaction.md`.
  Domain audit retention purge-batch display output also uses the stable
  planner message; see `docs/adr/0900-domain-recovery-display-redaction.md`.
- Use `plan_audit_retention_purge` at the storage boundary before concrete
  retention delete adapters build DML so `TenantStorageKey` and purge batch
  tenants cannot diverge.
  Storage audit retention purge debug output redacts storage keys and purge
  batch predicates while preserving command/plan shape; see
  `docs/adr/0927-storage-audit-retention-purge-debug-redaction.md`.
  Storage audit retention purge error debug output redacts mismatched tenant
  fields while preserving the error variant name; see
  `docs/adr/0945-storage-audit-retention-purge-error-debug-redaction.md`.
- Use `plan_postgres_audit_retention_purge` or
  `plan_sqlite_audit_retention_purge` for concrete audit retention deletes so
  SQL remains DML-only and predicates include tenant ID plus audit event IDs.
- Use `plan_application_audit_retention_purge` so composition roots select
  durable audit retention purge storage through one side-effect-free application
  use case with stable redacted storage failures.
- Use `ControlPlaneOperation::AuditRetentionPurge` before starting retention
  purge execution so audit log deletes require Admin/Delete authorization and
  emit immutable control-plane audit events.
- Route `ControlPlaneOperation::AuditRetentionPurge` through
  `plan_application_control_plane_with_audit_storage` so authorized and denied
  retention purge attempts select append-only audit storage before purge delete
  execution is adapted.
- Use `ControlPlaneOperation::requires_idempotency` and
  `ControlPlaneActionRequest::idempotent_operation` before mutating admin
  operations so retention purge, policy, configuration, and key mutations carry
  tenant-scoped idempotency replay keys.
  Request-fingerprint display output uses stable validation wording; see
  `docs/adr/0901-domain-idempotency-fingerprint-display-redaction.md`.
- Use `plan_application_control_plane_idempotency` before adapting mutating
  control-plane requests so missing idempotency keys are rejected with stable
  redacted application errors before audit or mutation storage execution.
  Idempotency key display output also uses the stable planner message; see
  `docs/adr/0899-domain-boundary-token-display-redaction.md`.
- Use `plan_application_control_plane_idempotency_replay` before adapting
  mutating control-plane storage so first execution, pending duplicates,
  completed replays, and reused-key conflicts follow one tenant-scoped replay
  decision boundary.
- Use `plan_idempotency_pending_record` at the storage boundary before
  executing mutating admin storage operations so pending replay entries are
  tenant-scoped and adapter-neutral.
  Storage idempotency pending-record debug output redacts tenant,
  idempotency-key, operation-response-hash, and timestamp fields; see
  `docs/adr/0937-storage-idempotency-pending-debug-redaction.md`.
- Use `plan_postgres_idempotency_pending_record` and
  `plan_sqlite_idempotency_pending_record` to persist pending admin mutation
  replay markers with `(tenant_id, idempotency_key)` uniqueness, PostgreSQL
  RLS, and request-path DML only.
- Use `plan_idempotency_completed_record`,
  `plan_postgres_idempotency_completed_record`, and
  `plan_sqlite_idempotency_completed_record` after successful mutating admin
  storage execution so retries replay a completed response only when tenant,
  idempotency key, and request fingerprint match.
  Storage idempotency completed-record debug output redacts tenant,
  idempotency-key, operation-response-hash, timestamp, and response-payload
  fields; see
  `docs/adr/0938-storage-idempotency-completed-debug-redaction.md`.
- Use `plan_idempotency_record_lookup`,
  `plan_postgres_idempotency_record_lookup`, and
  `plan_sqlite_idempotency_record_lookup` before mutating admin storage
  execution so durable replay state is loaded by `(tenant_id, idempotency_key)`
  and fingerprint mismatch remains a domain conflict.
  Storage idempotency lookup debug output redacts tenant, idempotency-key, and
  operation-response-hash fields; see
  `docs/adr/0939-storage-idempotency-lookup-debug-redaction.md`.
- Use `materialize_idempotency_record_lookup_row` after SQL replay lookup so
  raw rows become `IdempotencyEntry<Vec<u8>>` only when tenant and key match,
  completed rows include a completion timestamp and response body, and request
  fingerprint mismatch is preserved for the domain replay guard.
  Idempotency lookup-row display output uses the stable materializer response
  message; see
  `docs/adr/0906-storage-idempotency-lookup-row-display-redaction.md`.
  Idempotency lookup-row debug output redacts tenant, idempotency-key,
  operation-response-hash, timestamp, and response-payload fields while
  preserving row status; see
  `docs/adr/0940-storage-idempotency-lookup-row-debug-redaction.md`.
  Idempotency storage error debug output redacts mismatched tenant fields while
  preserving error variant names; see
  `docs/adr/0941-storage-idempotency-error-debug-redaction.md`.
- Use `plan_application_control_plane_idempotency_storage_prepare` and
  `plan_application_control_plane_idempotency_storage_complete` around mutating
  admin storage execution so composition roots plan replay lookup, pending
  marker insertion, mutation execution, and completion recording in one
  tenant-scoped lifecycle.
- Use `plan_application_control_plane_idempotency_replay_from_lookup_row` after
  durable idempotency lookup so missing rows execute, pending rows block
  duplicates, completed rows replay bytes, malformed rows fail closed, and
  reused-key fingerprint mismatches remain domain conflicts.
- Use `idempotency_key_from_headers` and
  `plan_application_control_plane_idempotency_from_http` for control-plane HTTP
  adapters so mutating admin requests share one `Idempotency-Key` header
  contract, duplicate replay headers fail closed, and invalid-key errors stay
  redacted before replay storage planning.
- Use `control_plane_request_fingerprint` and
  `plan_application_control_plane_idempotency_from_http_digest` so HTTP
  composition roots derive control-plane idempotency fingerprints from method,
  path, and adapter-supplied body digest instead of ad hoc strings.
  Request-fingerprint display output uses generic wording so local stringified
  errors do not expose path or body-digest metadata; see
  `docs/adr/0857-gateway-http-request-fingerprint-display-redaction.md`.
- Treat `/v1/admin` and `/v1/scim` as versioned control-plane route prefixes in
  gateway HTTP classification while keeping legacy `/admin` and `/scim`
  prefixes compatible.
- Match control-plane HTTP mounts on segment boundaries so prefix-confusable
  paths such as `/administrator` or `/scimitar` fail closed as unknown routes.
- Use `AuditExportPlan` for audit export use cases so validated query scope,
  time range, page limit, sort order, content type, and file extension metadata
  travel together, and export matching preserves the same fail-closed
  cross-tenant behavior as audit queries.
- Use `AuditSortOrder::parse` plus `plan_audit_sort_order_error_response`
  before accepting dynamic audit query/export sort order values so rejected
  sort strings, tenant IDs, principal IDs, resource IDs, audit chain material,
  and credential-like material stay out of audit-facing or client-visible error
  envelopes. Sort order parsing matches exact low-cardinality values without
  trimming; see `docs/adr/0651-audit-sort-order-parse-exact-boundary.md`.
- Use `AuditOutcome::parse` plus `plan_audit_outcome_error_response` before
  accepting dynamic audit outcome values so rejected outcome strings, tenant
  IDs, principal IDs, resource IDs, audit chain material, and credential-like
  material stay out of audit-facing or client-visible error envelopes.
  Domain audit outcome validation-error debug output keeps only
  low-cardinality outcome validation failure shape; see
  `docs/adr/0758-domain-audit-outcome-error-debug-redaction.md`.
- Use `AuditExportFormat::parse` plus
  `plan_audit_export_format_error_response` before accepting dynamic audit
  export format values so rejected format strings, tenant IDs, principal IDs,
  resource IDs, audit chain material, and credential-like material stay out of
  audit-facing or client-visible error envelopes. Audit enum parsers match
  exact low-cardinality values without trimming; see
  `docs/adr/0650-audit-enum-parse-exact-boundary.md`.
  Domain audit export-format validation-error debug output keeps only
  low-cardinality format validation failure shape; see
  `docs/adr/0759-domain-audit-export-format-error-debug-redaction.md`.
- Use `plan_application_usage_reconciliation_error_response` for post-provider
  accounting failures. It delegates gateway reconciliation failures to
  `plan_gateway_usage_reconciliation_error_response`, which adapts the shared
  `plan_usage_reconciliation_error_response` storage boundary, while keeping
  durable storage failures behind `usage_reconciliation_storage_unavailable`
  adapted from `plan_postgres_storage_error_response` or
  `plan_sqlite_storage_error_response`, so tenant IDs, usage amounts, SQL
  backend names, idempotency values, and telemetry internals stay out of API
  responses.
- Use `plan_application_expired_reservation_recovery_error_response` for
  background recovery failures. It delegates gateway recovery failures to
  `plan_gateway_expired_reservation_recovery_error_response`, which adapts the
  shared `plan_expired_reservation_recovery_error_response` storage boundary,
  while keeping Redis coordination and durable storage failures behind
  application-owned stable responses adapted from storage-level planners, so
  tenant IDs, usage amounts, Redis lease owners, backend names, and telemetry
  internals stay out of API responses.
- Use `plan_application_recovery_lease_release_error_response` for recovery
  lease release failures. It adapts the redacted `plan_redis_error_response`
  boundary to a use-case-specific code, so Redis details, tenant IDs, shard
  names, and owner tokens stay out of API responses.
- Use `plan_application_runtime_error_response` for startup/readiness topology
  failures. It adapts core storage topology/accounting failures from
  `plan_storage_topology_error_response` or
  `plan_multi_replica_accounting_error_response` and SQL storage planning
  failures from
  `plan_postgres_storage_error_response` or
  `plan_sqlite_storage_error_response` so migration policy, replica counts, SQL
  backend details, Redis topology requirements, and DDL planning internals stay
  out of API responses.
- Use `plan_application_control_plane_audit_error_response` for control-plane
  audit persistence failures. It adapts `plan_postgres_storage_error_response`
  or `plan_sqlite_storage_error_response` so SQL backend details, tenant IDs,
  storage-key mismatches, and migration/DDL internals stay out of API
  responses.
  Domain audit error-response plan debug output is restricted to the stable
  client-facing status, code, and message fields; see
  `docs/adr/0761-domain-audit-error-response-plan-debug-redaction.md`.
- Use `plan_provider_route_error_response` and
  `plan_provider_invocation_error_response` before provider dispatch so model
  names or lengths, tenant IDs, credential scope internals, and provider secret
  references stay out of API responses.
- Use `plan_gateway_admission_error_response` for pre-provider data-plane
  admission failures so tenant IDs, credential scope internals, reservation
  overflow details, provider secret references, and telemetry internals stay out
  of API responses.
- Use `plan_gateway_usage_reconciliation_error_response` for post-provider
  reconciliation failures so tenant IDs, usage amounts, reservation arithmetic,
  and telemetry internals stay out of API responses.
- Use `plan_gateway_expired_reservation_recovery_error_response` for abandoned
  reservation recovery failures so tenant IDs, usage amounts, reservation
  timing, and telemetry internals stay out of API responses.
- Treat any compatibility path listed as a remaining gap as non-enterprise
  until it is wired to the corresponding boundary crate and covered by a
  regression test.
- Use `plan_id_parse_error_response` before exposing typed domain identifier
  parse failures so raw submitted IDs, UUID parser details, tenant names,
  credential-like fragments, and high-cardinality values stay out of
  client-visible error envelopes.
- Config publication and invalidation tenant mismatch raw `Display` output is
  now the low-cardinality `configuration request is invalid` string. Keep
  structured variants for trusted diagnostics and expose client-facing failures
  through the existing response planners; see
  `docs/adr/0877-config-tenant-display-redaction.md`.
- Domain identity configuration and token-validation raw `Display` output now
  uses the same stable messages as the response planners, so issuer, audience,
  JWT algorithm, and claim mismatch classes stay out of accidental auth-boundary
  error strings; see
  `docs/adr/0878-domain-identity-display-redaction.md`.
- Domain idempotency tenant/key replay conflicts now share the same
  low-cardinality raw `Display` string as the response planner, keeping tenant
  and key relationship details in typed diagnostics only; see
  `docs/adr/0879-domain-idempotency-conflict-display-redaction.md`.
- Domain reservation commit mismatch and reconciliation raw `Display` output
  now uses the same low-cardinality messages as the accounting response
  planners, keeping tenant, call, reservation, amount, underflow, and overflow
  classifications in typed diagnostics only; see
  `docs/adr/0880-domain-accounting-display-redaction.md`.
- Domain tenant resolution, tenant access, and role-claim raw `Display` output
  now uses the same low-cardinality messages as the security response planners,
  keeping missing-claim and cross-tenant classifications in typed diagnostics
  only; see `docs/adr/0881-domain-security-display-redaction.md`.
- Domain reservation commit raw `Display` output now uses the same
  low-cardinality message for every commit failure, keeping zero-actual,
  underflow, overflow, and usage-comparison classifications in typed diagnostics
  only; see `docs/adr/0882-domain-reservation-commit-display-redaction.md`.
- Domain audit query scope raw `Display` output now uses the same
  low-cardinality message as the audit response planner, keeping cross-tenant
  event classification in typed diagnostics only; see
  `docs/adr/0883-domain-audit-query-scope-display-redaction.md`.
- Config publication and invalidation revision raw `Display` output now uses
  stable response-planner wording, keeping cache membership and active-revision
  implementation detail in typed diagnostics only; see
  `docs/adr/0884-config-revision-display-redaction.md`.
- Config secret-reference raw `Display` output now uses the same stable message
  as the response planner, keeping raw-material vs malformed-reference
  classification and `SecretRef` implementation wording in typed diagnostics
  only; see `docs/adr/0885-config-secret-reference-display-redaction.md`.
- Config cache-window raw `Display` output now uses the same stable message as
  the response planner, keeping refresh/stale/expiry ordering classifications in
  typed diagnostics only; see
  `docs/adr/0886-config-cache-window-display-redaction.md`.
- Config publication-event raw `Display` output now uses the same stable message
  as the response planner, keeping gateway cache and runtime-policy target
  classifications in typed diagnostics only; see
  `docs/adr/0887-config-publication-event-display-redaction.md`.
- Domain policy activation raw `Display` output now uses one stable message for
  timestamp, digest, signature, and verification failures, keeping integrity
  classification in typed diagnostics only; see
  `docs/adr/0888-domain-policy-activation-display-redaction.md`.
- Domain migration plan raw `Display` output now uses the same stable messages
  as the response planner, keeping lock-owner and specific step-order
  classifications in typed diagnostics only; see
  `docs/adr/0889-domain-migration-plan-display-redaction.md`.
- Domain migration version and compatibility raw `Display` output now uses the
  same stable messages as response planners, keeping rejected input shape and
  compatibility-window detail in typed diagnostics only; see
  `docs/adr/0890-domain-migration-validation-display-redaction.md`.
- Config refresh raw `Display` output now uses the same stable message as the
  response planner for refresh-required and invalidated-revision states, keeping
  cache-state classification in typed diagnostics only; see
  `docs/adr/0891-config-refresh-display-redaction.md`.
- Domain API version sunset raw `Display` output now uses the same public
  response-planner message, keeping lifecycle terminology and rejected version
  metadata in typed diagnostics only; see
  `docs/adr/0892-domain-api-version-display-redaction.md`.
- Domain pagination cursor and entity-tag raw `Display` output now uses the same
  stable messages as response planners, keeping empty, length, and
  invalid-character classifications in typed diagnostics only; see
  `docs/adr/0893-domain-api-token-display-redaction.md`.
- Domain resource-version and concurrency-precondition raw `Display` output now
  uses the same stable messages as response planners, keeping token type,
  zero/overflow, and mismatch classifications in typed diagnostics only; see
  `docs/adr/0894-domain-api-precondition-display-redaction.md`.
- Domain audit timestamp, page-limit, and retention-batch-limit raw `Display`
  output now uses the same stable messages as response planners, keeping
  timestamp and limit validation shape in typed diagnostics only; see
  `docs/adr/0895-domain-audit-limit-display-redaction.md`.
- Domain audit outcome, export-format, and sort-order raw `Display` output now
  uses the same stable messages as response planners, keeping empty vs unknown
  enum validation shape in typed diagnostics only; see
  `docs/adr/0896-domain-audit-enum-display-redaction.md`.
- Domain audit action, resource-kind, and resource-id raw `Display` output now
  uses the same stable messages as response planners, keeping empty, length,
  segment, and character validation shape in typed diagnostics only; see
  `docs/adr/0897-domain-audit-resource-display-redaction.md`.
- Run full database concurrency tests before declaring the multi-replica
  accounting path production-ready.
