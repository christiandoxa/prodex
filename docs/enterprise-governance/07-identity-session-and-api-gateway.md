# Identity, Session, and API Gateway

The target is one governed application boundary for CLI, human API clients,
services, compatibility routes, HTTP/SSE and WebSocket traffic. Existing
virtual-key authentication, scopes, tenant-aware application boundaries,
runtime admission and session/affinity primitives are reusable. The production
edge validates trusted proxies and a shared authority revocation epoch refreshes
session caches; anonymous compatibility paths, human/workload identity parity,
mTLS peer verification and deployed multi-replica validation remain gaps.
[`10-unified-gateway-and-identity.md`](10-unified-gateway-and-identity.md).

## Entry sequence

1. Terminate TLS at an approved internal gateway or the service.
2. Accept forwarded identity/network attributes only from configured trusted
   proxy hops and only after canonicalization.
3. Authenticate a human through approved SSO/OIDC or a service through a
   workload credential; use mTLS where the deployment profile requires it.
4. Resolve exactly one tenant and immutable principal; reject ambiguous or
   cross-tenant binding.
5. Authorize route and operation scope.
6. Establish or validate a bounded session and idempotency identity.
7. Run inspection, policy, obligations, reservation, governed routing and audit
   before provider dispatch.

Client-supplied `tenant_id`, `roles`, `principal_id`, forwarding headers and
approval claims are untrusted until independently bound. Provider credentials
never authenticate a caller.

## Session contract

A session stores only bounded metadata: opaque session ID, tenant, principal,
authentication method/assurance, created/last-used/absolute expiry, current
credential and policy epoch, revocation state and required continuation
bindings. IDs are random, hashed at rest where practical, rotated at privilege
change, and never placed in logs in raw form.

Session validation checks tenant/principal binding, idle and absolute expiry,
credential and policy epoch, revocation and channel binding. Logout, identity
disable, role reduction and incident response revoke sessions promptly. Redis
may cache revocation or coordination data but is not the sole durable authority.

## Continuations

`previous_response_id`, `x-codex-turn-state` and session-scoped continuation
keys remain pinned to the owning profile/provider when possible. Policy and
registry revocation always wins: a continuation cannot preserve eligibility
that has been explicitly revoked. No retry or rotation occurs after a stream is
committed.

## Mode behavior

| Mode | Identity/session requirement |
| --- | --- |
| `personal` | Local compatibility remains available within its documented trust boundary |
| `enterprise_observe` | Remote traffic is authenticated and tenant-bound before shadow governance |
| `enterprise_enforce` | Every supported channel uses the authenticated governed boundary |
| `bank_enforce` | Private/internal listener, approved identity, mandatory tenant/session controls and no anonymous compatibility bypass |

## API properties

- bounded bodies, headers, deadlines, concurrency and idempotency;
- stable machine-readable local error codes without rewriting upstream errors;
- request metadata preservation except hop-by-hop and selected-profile auth;
- WebSocket origin/subprotocol checks and governed client-frame inspection;
- no terminal output while the Codex TUI runs; and
- rate/admission controls keyed by authenticated tenant/principal, not
  forgeable headers.

## Acceptance evidence

- OIDC/workload/mTLS positive and negative integration tests;
- forged forwarded-header and confused-deputy tests;
- tenant/RLS/session fixation, expiry and revocation tests;
- HTTP/SSE/WebSocket parity and no-bypass architecture guards; and
- multi-replica revocation and continuation-affinity tests.
