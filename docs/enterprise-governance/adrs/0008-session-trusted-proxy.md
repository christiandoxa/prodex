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

The candidate implements typed identity evidence, explicit role/tenant
resolution, browser Authorization Code with PKCE S256, secure bounded cookies,
edge-header canonicalization, governed session binding/expiry/concurrency/
revocation, workload JWT verification, direct Rustls client-certificate
verification, and JWT `cnf.x5t#S256` peer binding.
Evidence includes `production_edge_security_uses_peer_trust_and_rejects_host_origin_csrf_spoofing`,
`validated_peer_metadata_maps_to_low_cardinality_governance_zone`,
`workload_evidence_requires_exact_identity_and_bound_mtls_when_configured`, and
`session_reuse_with_another_principal_is_revoked`, and
`cross_replica_revocation_epoch_invalidates_cached_sessions_promptly`. Shared
browser-session storage, IdP logout propagation, managed PKI rotation, and a
deployed two-gateway chaos proof remain acceptance gaps.
