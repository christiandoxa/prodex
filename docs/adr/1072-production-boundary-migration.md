# ADR 1072: Production request authentication and authorization boundary migration

## Status

Accepted

## Context

The legacy gateway handler is the deployed production path. It parsed routes, verified gateway and
admin credentials, acquired admission, and dispatched providers directly. The canonical
`prodex-application` request-authentication and authorization planners had only test callers, so
they could not prevent a production route, scope, role, or tenant bypass.

Moving credential decoding, virtual-key budgeting, accounting, and provider streaming together
would be a high-risk rewrite. The first production slice therefore needs a narrow seam that is
authoritative without changing those mature side effects.

## Decision

Use a two-stage strangler boundary in the legacy production handler:

1. Parse one `CanonicalRequestTarget` and pass that exact object by reference to
   `plan_application_request_context`. The application boundary classifies its typed route, plane,
   and required credential scope. Unknown routes stop before authentication or backend work.
2. Keep bounded secret verification in the transport adapter. The explicit
   `local_rewrite_application_boundary` converts verified bearer, admin, and virtual-key outcomes
   into `VerifiedCredentialEvidence::Principal`. Verified OIDC credentials instead retain the
   actual JWT header/claims and immutable JWKS cache snapshot as typed OIDC evidence; the request
   path never performs discovery or a JWKS refresh.
3. The application planner invokes `prodex-authn::authenticate_verified_credential` with the
   credential scope from the existing `ApplicationRequestContext`. OIDC evidence also enters the
   canonical issuer, audience, algorithm, key, signature, time, principal, tenant, and role checks.
   Missing OIDC roles use an explicit trusted SCIM/viewer fallback; present roles must map to the
   resolved principal and unknown roles fail closed.
4. The authenticated principal enters the canonical authorization policy before local admission.
   Data-plane requests pass `prodex-authz` scope and role checks and resolve a typed tenant.
   Control-plane requests pass `prodex-control-plane` operation, role, resource, and tenant checks.
   Legacy verification, authentication, and authorization failures all stop before virtual-key
   reservation, admin mutation, or provider dispatch.
5. Control-plane dispatch requires a preauthorization wrapper carrying both the verified legacy
   admin identity and the typed application authorization context. The router no longer owns a
   duplicate viewer/admin mutation policy. A typed control-plane route that the compatibility
   backend does not implement is denied with the existing stable route-not-available response
   rather than falling through to a provider.
6. Forwarding copies the path and query from the same application request context. It does not
   reparse or normalize a second request-target string.

The context is immutable and borrows the canonical target. The authenticated and authorized
context owns the typed principal and tenant produced by this slice. Compatibility identities use
deterministic identifiers derived from non-secret stable names; they do not use random or secret
material. Deadline, trace, and audit correlation remain later production slices.

## Compatibility

Gateway/admin token, trusted-proxy SSO, OIDC signature, and virtual-key secret verification remain
in their bounded transport adapters. Existing authentication failure status, body, audit writes, runtime logs, provider
headers, reservation/accounting effects, streaming, and affinity behavior remain owned by their
existing adapters. Anonymous data-plane operation remains possible only when the legacy
configuration has no authentication mechanism enabled.

This is an application-policy cutover, not a transport cutover. The production pipeline still
accepts and answers a concrete `tiny_http::Request`; websocket handling also takes the
request-owned connection. `prodex_app::GatewayBackend` consequently exposes a loopback address and
drain handle, not a transport-neutral request handler. Adapting Hyper directly at this point would
either expose the blocking listener or recreate the socket bridge with synchronous in-memory I/O,
neither of which preserves the required streaming, cancellation, upgrade, and drain contracts.

The next transport slice must first make the typed pipeline return a response stream or explicit
upgrade handoff from transport-neutral request inputs. Hyper and `tiny_http` adapters must then pass
the same response/side-effect differential corpus, including partial-stream cancellation and
bounded drain, before the dedicated composition root removes the loopback backend.

Characterization coverage compares the established data-plane decision matrix with the application
gate. Focused negative coverage exercises anonymous denial, both scope crossings, insufficient
roles, unknown OIDC roles, mismatched OIDC principal/tenant evidence, and cross-tenant control-plane
access. Gateway integration coverage preserves unscoped OIDC admin behavior and SCIM role/tenant
fallback while remaining the response and side-effect oracle for bearer, virtual-key, admin,
health, provider, audit, and accounting behavior.

`scripts/ci/production-boundary-guard.mjs` is a positive and negative CI guard. It rejects builds
when the canonical application context is removed, data-plane authentication moves after
admission/provider calls, admin preauthorization moves after admission, forwarding stops using the
canonical context, the application planner stops invoking `prodex-authn`/`prodex-authz`, or a
duplicate router-level admin role policy is reintroduced.

## Consequences

- `prodex-authn` and `prodex-application` now authenticate typed evidence on real gateway traffic.
- Route, credential-scope, role, and tenant policy has one authoritative production boundary.
- Two uncompiled request-entry/preparation prototypes containing duplicate auth and admission
  branches are deleted (280 lines); the production handler is the single compatibility path.
- Low-level secret parsing and signature verification, durable admission, reconciliation,
  accounting, and provider invocation remain explicit migration gaps.
- The compatibility authentication API has no production callers and can be removed separately
  after downstream API compatibility is no longer required.
- The compatibility gateway adapter can be removed only after provider, streaming, accounting, and
  control-plane side-effect parity is proven on the dedicated server path, and after the pipeline
  no longer owns `tiny_http::Request` response or upgrade I/O.

## Verification

```bash
cargo test --locked -p prodex-authn -p prodex-authz -p prodex-application
cargo test --locked -p prodex-app gateway_application_boundary -- --test-threads=1
cargo test --locked -p prodex-gateway-server -- --test-threads=1
npm run ci:production-boundary-guard
npm run ci:enterprise-binaries-guard
cargo clippy --locked -p prodex-authn -p prodex-application -p prodex-app --all-targets -- -D warnings
```
