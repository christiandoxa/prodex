# Unified Gateway and Identity

## Status and Scope

This document defines the target authenticated boundary for CLI, IDE, browser,
and service traffic. It separates repository evidence observed on 2026-07-13
from controls that still require implementation and environment-specific proof.
It is not evidence that a deployment is bank-ready, certified, or approved by a
regulator.

The target request sequence is:

```text
channel
  -> canonical HTTP/API boundary
  -> credential verification and identity construction
  -> tenant and credential-scope authorization
  -> access-session and risk evaluation
  -> inspection and classification
  -> policy decision and obligations
  -> bounded admission and accounting reservation
  -> governed provider routing
  -> provider dispatch and response guard
  -> reconciliation and durable audit
```

No channel, route alias, provider adapter, or compatibility entry point may
skip a stage required by the active governance mode.

## Existing Evidence and Gaps

| Area | Existing repository evidence | Gap against the target |
| --- | --- | --- |
| Identity domain | `prodex-domain` has typed issuer, audience, algorithm allowlists, principal kinds, tenant context, credential scopes, role mapping, and redacted error plans. | It does not define the complete human and workload authentication protocol or the requested access-session policy. |
| OIDC validation | `prodex-authn` validates exact HTTPS OIDC endpoints, restricts JWKS origins, rejects redirects, and provides bounded JWKS last-known-good behavior. The gateway verifies bearer tokens and implements browser Authorization Code with PKCE S256, state/nonce checks, bounded token exchange, and logout. | A CLI/IDE device flow, IdP back-channel logout, and deployed IdP rotation/failure exercises remain outside the current browser contract. |
| Service identity | The gateway verifies workload JWT signature, issuer, audience, subject, tenant, and required `data_plane` scope before building a service principal. JWKS discovery/cache follows the same bounded no-redirect origin policy as human OIDC. | Credential issuance and managed issuer/key rotation remain deployment responsibilities. |
| mTLS | The direct Rustls listener loads projected server identity and client CA material, verifies client certificate chains, computes the leaf SHA-256 fingerprint, and requires matching JWT `cnf.x5t#S256` evidence when configured. | Deployed CA revocation/rotation, certificate inventory, and bank PKI outage exercises remain environment acceptance work. |
| Gateway HTTP policy | `prodex-gateway-http` has strict request-target parsing, route/method classification, body limits, timeout budgets, stream concurrency, cancellation, backpressure, and drain plans. | Per-route header count/size limits, one derived request deadline for every stage, distributed admission parity, and full OpenAPI compatibility enforcement need completion. |
| Application admission | `prodex-gateway-core`, `prodex-application`, and the canonical runtime path enforce typed tenant, principal, authorization, reservation, provider invocation, and trace relationships. | Anonymous compatibility admission and early WebSocket/Gemini paths have not all been removed or proven to converge on the governed application use case. |
| Browser protections | The unified edge enforces canonical Host/Origin/CSRF policy and exposes opt-in login/callback/logout routes. PKCE state transactions and authenticated sessions are bounded; cookies are Secure, HttpOnly, SameSite, path-scoped, and explicitly cleared on logout. | Browser session state is process-local; shared multi-replica sessions and IdP logout propagation remain incomplete. |
| Trusted proxy | The production async edge preserves the TCP peer, requires explicit `gateway.expected_host` for non-loopback listeners, rejects untrusted forwarding, derives a bounded right-to-left client address from exact trusted proxies, strips forwarding headers before dispatch, and maps validated metadata to a low-cardinality governance zone. | Privacy-preserving persisted network-risk history and deployment-specific multi-proxy/geo evidence remain outside the in-process request contract. |
| Sessions | Governed sessions enforce tenant/principal binding, timeouts, concurrency, monotonic classification, revision pins and atomic revoke/audit/outbox; a shared authority revocation epoch triggers cache refresh on a 250-millisecond poll path. | A deployed two-gateway chaos proof, identity-provider logout propagation, re-authentication/MFA and ordinary data-plane self-service logout remain incomplete. |
| Deployment exposure | Kubernetes uses a ClusterIP-style internal service, non-root containers, read-only filesystems, dropped capabilities, resource bounds, probes, graceful drain scaffolding, and the runtime can terminate direct mTLS. | Compose currently publishes a host port broadly, and the checked-in deployment does not prove private ingress, IdP/Vault/SIEM egress restrictions, managed PKI rotation, or the full bank profile. |

Existing primitives are foundations, not proof of the Phase 5 exit gate.

## Deployment Modes

The gateway must consume one validated, versioned governance snapshot. The mode
names below are target configuration until the typed mode contract is present
in production.

| Mode | Listener and identity behavior | Governance failure behavior |
| --- | --- | --- |
| `personal` | Loopback-only operation may retain current local credentials and compatibility behavior. Remote human access is not implied. | Explicitly enabled controls apply; compatibility must not be mislabeled as enterprise enforcement. |
| `enterprise_observe` | Remote traffic is authenticated and tenant-bound before shadow governance runs. | Shadow findings are recorded without changing the legacy routing result, except existing security boundaries remain enforced. |
| `enterprise_enforce` | All supported channels use the authenticated application boundary. | Required identity, classification, policy, admission, routing, and audit decisions deny safely when unresolved. |
| `bank_enforce` | Internal/private listener only; remote humans require approved SSO; services require approved workload credentials and mTLS where supported by deployment. | Identity, tenant, required inspection, mandatory policy, approved registry, secret references, accounting, and durable audit fail closed according to the operation matrix. |

A flag that weakens a mandatory bank control must be revisioned, tenant-scoped,
audited, explicitly authorized as break-glass, and automatically expire. It may
not be a process-local environment toggle read in request handling.

## Unified Channel Contract

### Channel identity matrix

| Channel | Permitted target authentication | Binding requirements | Explicitly forbidden |
| --- | --- | --- | --- |
| Browser admin | OIDC Authorization Code with PKCE through an approved IdP or identity broker | Issuer, audience, nonce/state, redirect URI, tenant, principal, authentication strength, session, CSRF, Origin, and Host | Password collection by Prodex; SAML parsing in Prodex; bearer tokens in URLs |
| CLI and IDE human | Validated bearer token obtained through an IdP-supported secure device/login flow appropriate to that IdP contract | Issuer, audience, tenant, principal, channel, credential scope, expiry, revocation state, and session where configured | Invented password grant; tokens in argv, URLs, logs, or ordinary config |
| Internal API service | OAuth client credential, workload identity, or another approved non-human credential verified at the gateway | Tenant, service principal, audience, scope, workload or certificate identity, expiry, and revocation | Reusing a human browser session as service identity |
| Service-to-service in bank mode | Approved workload credential plus mTLS when the deployment supports the service path | Workload identity must agree with the authenticated TLS peer and requested tenant/scope | Trusting a caller-supplied certificate header without an authenticated trusted proxy boundary |
| Personal loopback client | Existing local credential boundary when explicitly in `personal` mode | Loopback binding and exact local scope | Treating local anonymous compatibility as enterprise identity |

If operational SAML federation is required, an OIDC-compatible identity broker
must terminate SAML. Prodex must not implement a SAML parser.

### Verified identity context

Authentication produces an immutable, content-free context before application
authorization:

- stable tenant ID and principal ID;
- principal kind: human, service, or an explicitly supported local principal;
- credential kind and credential scope;
- issuer and audience revision or approved identifier;
- authentication time, expiry, and bounded authentication-strength result;
- session ID when an access session exists;
- channel and trusted workload/certificate binding;
- bounded role/group attributes mapped through an approved revision;
- trusted network-risk digest or bucket, never a raw full IP address;
- request and trace correlation IDs;
- credential and revocation revision needed to explain the decision.

The context excludes raw tokens, certificate bytes, cookies, secrets, prompts,
responses, detector matches, and unrestricted identity-provider claims. Debug,
audit, and metrics representations must remain redacted and bounded.

### OIDC and bearer validation

The verifying adapter must validate, at minimum:

1. exact configured issuer and an allowed audience;
2. signature with an explicitly allowed algorithm and a usable key from the
   bounded JWKS snapshot;
3. token expiry, not-before, issued-at, and configured maximum token age with a
   bounded clock-skew policy;
4. token type, credential scope, tenant claim, subject, and authorized client
   or workload identity;
5. nonce, state, PKCE verifier, and exact redirect binding for browser flows;
6. required authentication method or strength when policy emits a re-auth/MFA
   obligation;
7. revocation/logout state when the IdP and session contract support it;
8. bounded claims and exact claim selectors before mapping to roles or scopes.

Discovery and JWKS retrieval belong to a bounded background adapter. The pure
authentication and policy boundaries perform no DNS or HTTP. A last-known-good
key may be used only within a configured bounded stale window; after that,
enterprise enforcement denies token verification as unavailable. Bank mode
must not silently disable OIDC when its configuration or key set is invalid.

### Service credentials and mTLS

Service credentials must be purpose-bound `SecretRef` or workload-identity
references. The gateway validates the expected audience, tenant, service
principal, scope, lifetime, and credential status. A service credential cannot
authorize a human-only control-plane operation merely because its role string
matches.

The bank mTLS target additionally requires:

- an approved trust bundle and exact server/client identity policy;
- TLS version and cipher policy owned by the deployment boundary;
- peer-name or workload-identity validation, with certificate expiry and
  revocation behavior;
- agreement between the TLS peer and bearer/workload identity;
- bounded trust-bundle and certificate rotation without accepting an unknown
  issuer during overlap;
- redacted telemetry that reports only stable verification reasons.

TLS termination at a proxy is acceptable only when that proxy is on a bounded
allowlist, its connection to Prodex is authenticated, and it supplies signed or
otherwise protected peer evidence. Unauthenticated certificate headers are
untrusted input.

## Canonical Gateway Enforcement

The HTTP adapter first rejects ambiguous targets, duplicate credential or
affinity headers, disallowed methods, and configured body/header limits. It
then constructs one application request. The application use case owns the
security sequence; provider adapters receive only an admitted invocation.

Required bounded controls are:

- canonical route and method allowlists with stable redacted errors;
- total and per-header byte limits, header-count limits, body limits, JSON
  nesting/value limits, and bounded upgrade frames;
- one absolute request deadline with smaller stage budgets, including external
  inspection when policy requires it;
- global, route/lane, tenant, principal, credential, and provider concurrency
  or rate controls as applicable;
- distributed rate and quota decisions that cannot make Redis authoritative;
- cancellation propagation, streaming backpressure, and graceful drain;
- no retry or provider rotation after a response/stream is committed;
- preservation of required Codex metadata and hard continuation affinity;
- no public listener by default in enterprise and bank profiles.

Rate, quota, and overload denials must be attributable by bounded reason code.
They must not disclose another tenant's existence, limits, provider state, or
policy internals.

## Trusted Proxy and Browser Boundary

Only direct peers in a validated, bounded trusted-proxy set may influence the
derived client network context. The derivation algorithm must:

1. start from the authenticated transport peer;
2. ignore `Forwarded` and `X-Forwarded-*` from untrusted peers;
3. parse a bounded number of syntactically valid hops;
4. remove trusted hops in a documented direction and select the first
   untrusted address;
5. store only a keyed prefix/digest or coarse approved risk bucket;
6. treat network changes as policy input, not a universal hard lockout.

Mobile and corporate-proxy transitions require explicit tests. Geo or network
zone can be used only when its source is authenticated and its staleness and
precision are documented.

Browser administration additionally requires exact allowed Host and Origin,
state-changing CSRF protection, secure and HTTP-only cookies, appropriate
SameSite policy, session rotation after authentication or privilege change,
`Cache-Control: no-store`, clickjacking/content-type protections, and no token
material in URLs or browser-readable persistent storage. The existing expose
controls are useful evidence but must be reviewed as part of this one boundary.

The async edge applies forwarding-header trust and safe client-address
derivation to every route. Exact Host and browser Origin/CSRF enforcement is
limited to the control plane because that is the browser/admin surface; data
plane routing never uses caller Host as identity and replaces transport-local
Host before upstream dispatch. Non-loopback listeners still require an exact
`gateway.expected_host` so later control-plane composition cannot inherit a
wildcard-bind authority.

## Access-Session Contract

The enterprise target uses an opaque, high-entropy session ID. Only a digest or
other non-reversible lookup representation is persisted where practical. Each
session is bounded and binds:

- tenant, principal, channel, and credential identity/scope;
- creation, last activity, absolute expiry, and idle expiry;
- active, revoked, expired, or risk-hold status;
- authentication strength and pending re-auth/MFA obligation;
- maximum concurrent-session policy and an atomic admission result;
- optional trusted device/workload/network-risk binding;
- active classification maximum for retained context;
- policy, classification, provider-registry, routing, and affinity revisions;
- bounded creation/revocation reason and audit links.

Session creation rotates any pre-authentication identifier to prevent fixation.
Replay of a revoked or replaced identifier is denied. Logout and credential
revocation invalidate all required replicas through authoritative PostgreSQL
state plus rebuildable invalidation caches. Redis may accelerate revocation
lookup but cannot be the only revocation record.

Classification is monotonic for retained context. A later public request in a
session that has held restricted context cannot silently lower the session's
effective classification. A deliberate declassification, if supported, is a
separate authorized and audited control-plane action.

Provider and policy revision pinning must preserve continuation behavior while
honoring emergency revocation. A pinned provider that becomes prohibited is
not replaced with an ineligible fallback; the continuation is denied before
commit with a stable reason.

## Bank-Mode Failure Matrix

This is the required target behavior, not a claim about current production
code.

| Failure before commit | Bank data-plane behavior | Operator signal |
| --- | --- | --- |
| Missing or invalid credential, issuer, audience, scope, tenant, or auth strength | Deny with a stable generic 401/403-class plan; do not reveal tenant existence | Bounded authn/authz reason counter and audit attempt where durable audit is available |
| OIDC keys unavailable beyond the permitted LKG window | Deny as identity service unavailable | Key age, refresh failure, LKG use, and unavailable alert |
| Required mTLS absent, invalid, expired, or inconsistent with workload identity | Deny before application admission | Stable mTLS verification reason; no certificate content |
| Session missing, expired, idle, revoked, replayed, over concurrency, or requiring step-up | Deny or return an approved re-auth obligation before provider work | Session lifecycle and step-up counters plus audit |
| Client network metadata supplied by an untrusted proxy | Ignore it; deny if policy requires trusted risk context | Proxy-trust violation counter |
| Classification or inspection unresolved when mandatory | Deny before routing | Coverage/classification failure and detector availability alert |
| Policy or approved provider-registry snapshot unavailable beyond bounded LKG | Deny; never use an unapproved default | Snapshot age/LKG/unavailable alert |
| Secret reference unresolved or expired | Deny before adapter use | Secret lease/resolve failure without secret identity cardinality |
| Reservation or mandatory local durable audit cannot commit | Do not invoke the provider or acknowledge success | Accounting/audit write failure alert |
| Redis unavailable | Use documented conservative admission; never infer authorization or durable state from Redis | Redis coordination failure and conservative-mode signal |
| Failure after stream commit | Terminate naturally or according to the already-admitted response obligation; never rotate provider mid-stream | Cancellation/stream failure plus reconciliation/audit outcome |

Control-plane mutations that cannot durably record their required audit,
approval, or activation transaction must not report success.

## Required Tests

The Phase 5 identity and gateway gate requires at least:

- CLI, IDE, browser/API, and service identity parity through the same
  application use case;
- OIDC issuer, audience, algorithm, key, expiry, nonce/state, PKCE, claim-size,
  and JWKS outage/LKG negative tests;
- secure device/login flow contract tests against the selected IdP adapter;
- client-credential/workload identity and credential-scope confusion tests;
- mTLS trust, name, expiry, revocation, bearer-binding, and rotation tests;
- session fixation, replay, logout, revocation, absolute/idle timeout,
  concurrency, MFA, and multi-replica invalidation tests;
- trusted-proxy chain, spoofed forwarding, mobile network, and corporate proxy
  tests;
- canonical target, request smuggling, duplicate header, body/header/frame
  limit, slow client, cancellation, and overload tests;
- browser Host, Origin, CSRF, cookie, cache, and security-header tests;
- distributed rate and quota tests with Redis loss and stale replicas;
- OpenAPI, stable error-envelope, and Codex upstream metadata compatibility;
- a complete `bank_enforce` fail-closed scenario with no public exposure.

## Exit Criteria

This boundary is complete only when every supported production channel produces
the same typed identity, tenant, session, inspection, policy, admission,
routing, response, accounting, and audit context; compatibility anonymous and
early dispatch bypasses are removed; the bank failure matrix is executable;
and current environment evidence proves private exposure, identity parity,
mTLS where required, multi-replica revocation, and bounded overload behavior.
