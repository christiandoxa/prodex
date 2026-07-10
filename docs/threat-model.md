# Prodex Enterprise Threat Model

## Scope

This threat model covers the enterprise target architecture for Prodex as a
multi-tenant, multi-replica platform. It focuses on the gateway data plane,
control plane, provider boundary, storage backends, identity integration,
configuration publication, audit trail, and production deployment artifacts.

The current repository is moving incrementally toward this target. Existing
legacy runtime proxy paths remain compatibility surfaces until they are migrated
behind the new boundary crates.

## Assets

- Tenant data and tenant-scoped configuration.
- Virtual keys, service identities, user identities, and break-glass identities.
- Provider credential references and secret-provider configuration.
- Budget reservations, usage counters, and append-only billing ledger events.
- Policy and configuration revisions, signatures, digests, and last-known-good
  cache state.
- Audit events and audit hash-chain digests.
- Provider traffic, streaming responses, continuation identifiers, and trace
  context.
- Deployment secrets, runtime policy, backup artifacts, and recovery metadata.

## Trust Boundaries

### Public HTTP Boundary

All inbound data-plane and control-plane requests cross an untrusted HTTP
boundary. The gateway HTTP boundary must enforce route classification, method
checks, request body limits, timeout budgets, trace propagation, and auth header
replacement before provider invocation.

### Authentication Boundary

OIDC tokens, service credentials, virtual keys, and break-glass credentials are
untrusted until validated by authentication code that uses cached metadata and
JWKS state. OIDC discovery and JWKS network fetches must not happen on the request path.

### Authorization Boundary

Data-plane, control-plane, and break-glass credentials are not interchangeable.
Every resource operation must authorize against canonical `Principal` and
`TenantContext` values.

### Provider Boundary

Provider adapters are outside the trusted application domain. The provider SPI
must receive only validated invocation plans, `SecretRef` credential references,
and pre-commit retry decisions. It must not weaken continuation affinity or
rotate after a stream is committed; no mid-stream rotation is allowed.

### Durable Storage Boundary

PostgreSQL is the production source of truth for tenant-owned durable state,
budget reservations, and usage ledger events. Application authorization remains
mandatory, while PostgreSQL Row-Level Security provides defense in depth.

### Redis Boundary

Redis is not durable billing state. It is limited to distributed rate limiting,
short-lived cache, and rebuildable coordination. Redis must not store the full
usage map or billing ledger as one JSON/list state that is loaded, mutated, and
rewritten.

### Local SQLite Boundary

SQLite is a compatibility and local-development backend. SQLite schema DDL must
be versioned and invoked from explicit migration flows, not from hot request
paths.

## Threats and Controls

| Threat | Risk | Required controls |
| --- | --- | --- |
| Missing or unknown role claim becomes admin | Vertical privilege escalation | Explicit role mapper; missing or unknown role denies or maps to Viewer only; negative tests |
| Root/admin token used for inference | Data-plane bypass and quota bypass | Distinct credential scopes; data-plane requires data-plane credentials; control-plane credentials cannot call inference |
| Virtual key used for admin endpoints | Tenant or policy takeover | Control-plane routes require control-plane scope and per-resource authorization |
| Break-glass becomes universal bypass | Unbounded emergency privilege | Separate scope, expiry, reason, and audit; no implicit data-plane/control-plane bypass |
| Cross-tenant resource access | Tenant data leakage | Mandatory tenant context, tenant-scoped keys, query predicates, FK/unique/index tenant columns, RLS |
| Process-local request or call IDs collide | Multi-replica id collision and ledger corruption | Typed globally unique IDs, preferably UUIDv7, for request/call/reservation/audit/policy IDs |
| Read-modify-write budget accounting | Lost updates and quota overshoot | Reservation-based accounting and atomic storage plans using SQL transactions/conflict-safe updates |
| Duplicate ledger events on retry | Double charging or inconsistent billing | Idempotency keys and tenant-scoped uniqueness by reservation/call/event kind |
| Stream cancellation loses accounting | Underbilling or leaked reservations | Reconciliation for completed, cancelled, and interrupted streams; expiry recovery |
| DDL during request handling | Availability impact and lock contention | External migrator-only DDL; request paths reject migration planning |
| Redis whole-map JSON state | Lost updates and global lock contention | Atomic Lua/hash/counter operations only; durable ledger remains in PostgreSQL |
| JWKS fetch on request path | Request stalls and identity availability coupling | Cached JWKS decisions, stale-while-revalidate/LKG semantics, no network in auth boundary |
| Secret value leaks in domain or logs | Credential compromise | `SecretRef` in domain, redacted secret material display, log/error redaction |
| Upstream provider error rewriting | Compatibility breakage and debugging loss | Pass-through upstream status/body/stream after upstream response exists |
| Mid-stream rotation | Broken transport semantics and affinity | Rotation only pre-commit; continuation bindings preserved |
| High-cardinality telemetry labels | Metrics cardinality explosion | Telemetry attribute validation and bounded labels |

## Required Negative Tests

The following tests must exist at the boundary or adapter layer before a feature
is considered enterprise-ready:

- Cross-tenant resource access is denied.
- Horizontal and vertical privilege escalation are denied.
- Missing/unknown role claim never becomes Admin.
- Malformed, expired, unknown-key, and stale-JWKS token cases are denied.
- Admin/control-plane credential cannot call data-plane inference.
- Data-plane/virtual-key credential cannot call control-plane routes.
- Break-glass requires separate scope, non-empty reason, expiry, and audit.
- Replayed idempotency key does not double charge.
- Cancellation or stream interruption reconciles reserved usage.
- Request path cannot plan or execute DDL.
- Redis plans avoid whole-map JSON or whole-list rewrite patterns.

## Audit Requirements

Security-sensitive actions must produce immutable tenant-scoped audit events:

authentication failures, authorization denials, key creation/rotation/revocation,
policy/config publication, provider credential rotation, budget changes, guardrail
denials, request body limit denials, backup/restore operations, and break-glass
use. Audit records must include tenant ID, principal ID, action, resource,
outcome, reason code, event ID, and hash-chain digest when persisted.

## Residual Risks and Migration Notes

Legacy runtime paths still contain compatibility logic while the modular
boundaries are being introduced. Those paths must be migrated behind
`prodex-application`, `prodex-gateway-http`, `prodex-control-plane`, and storage
adapter crates with characterization tests for transport transparency,
continuation affinity, upstream error compatibility, and CLI compatibility.
Non-shared-storage config-publication deployments also remain dependent on the
broker-backed transport staging captured in ADR 0984 until a real outbox/watch
adapter replaces the shared-filesystem composition-root path.
