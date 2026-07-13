# ADR 0008: Sessions and Trusted Proxies

- Status: Accepted
- Scope: remote human and service gateway access

## Context

Caller-controlled forwarding, tenant or role headers can create confused-deputy
and cross-tenant failures. Local compatibility identity is insufficient for a
multi-tenant enterprise gateway.

## Decision

Accept forwarded network/identity attributes only from configured proxy hops,
after canonicalization, with a documented single header precedence rule.
Authenticate humans through approved OIDC/SSO and workloads through scoped
credentials plus mTLS where required. Resolve exactly one tenant and principal
server-side. Sessions are opaque, bounded and tenant/principal-bound, with idle
and absolute expiry, credential/policy epochs, fixation resistance, rotation on
privilege change and durable revocation. Store only hashed/opaque identifiers in
logs and evidence. Redis may accelerate coordination but is not durable
authority.

## Consequences

Untrusted forwarding headers are stripped. Logout, role reduction, credential
disable and incident response invalidate sessions across replicas. Bank mode
requires private/internal exposure and forbids anonymous compatibility bypass.

## Implementation status

The candidate implements typed OIDC/workload evidence, optional bound mTLS,
explicit role/tenant resolution, PKCE S256 capability validation, edge-header
canonicalization, and governed session binding/expiry/concurrency/revocation.
Evidence includes `edge_security_rejects_forwarding_spoof_and_host_origin_csrf_mismatch`,
`workload_evidence_requires_exact_identity_and_bound_mtls_when_configured`, and
`session_reuse_with_another_principal_is_revoked`. Durable multi-replica
revocation validation remains pending with PostgreSQL.
