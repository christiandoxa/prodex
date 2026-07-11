# ADR 0082: Auth boundary guard

## Status
Accepted

## Context
`prodex-authn` and `prodex-authz` are boundary crates that should compose domain
security primitives without owning HTTP routing, storage, OIDC discovery, JWKS
fetching, JWT implementation details, async runtimes, or process/environment
side effects. If those dependencies enter the auth boundary crates, request-path
authentication and authorization can become blocking, network-dependent, or tied
to a serving framework.

## Decision
Add `scripts/ci/auth-boundary-guard.mjs` and wire it into npm scripts and local
preflight. The guard enforces that both auth crates depend only on
`prodex-domain`, have no dev-dependencies, avoid target-specific dependencies,
and do not import filesystem, environment, process, network, HTTP, database,
transport, async-runtime, JWT implementation, or OIDC implementation code.

The guard also checks core `prodex-authz` semantics that keep break-glass
credentials from becoming implicit data-plane or normal control-plane bypasses:
the explicit break-glass boundary must require `CredentialScope::BreakGlass`,
data-plane/control-plane resolvers must reject other scopes, and boundary
authorization must compare the principal scope to the exact boundary
requirement.
The npm script and local preflight run the guard self-test before the workspace
scan so forbidden-dependency, unsafe-forbid, plaintext-JWKS, and scope-separation
checks cannot silently rot.

## Consequences
Auth boundary crates remain deterministic and reusable from gateway and
control-plane composition roots. Network fetches, JWT decoding libraries, HTTP
middleware, and storage adapters must live in adapter/composition crates that
feed verified metadata or snapshots into these boundary crates. Break-glass
scope separation remains protected by both Rust tests and the boundary guard.
