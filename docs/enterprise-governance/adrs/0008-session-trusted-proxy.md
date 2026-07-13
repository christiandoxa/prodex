# ADR 0008: Sessions and Trusted Proxies

- Status: Proposed
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

Virtual-key auth/scopes, tenant boundaries and session primitives exist.
Unified OIDC/workload/mTLS authentication, trusted-proxy enforcement and durable
multi-replica revocation remain gaps.
