# Prodex Enterprise Threat Model

## Scope

This threat model covers the implemented enterprise architecture for Prodex as a
multi-tenant, multi-replica platform. It focuses on the gateway data plane,
control plane, provider boundary, storage backends, identity integration,
configuration publication, audit trail, and production deployment artifacts.

Upstream and legacy compatibility mechanics remain explicit composition-adapter
responsibilities. Authorization, governance mutations, storage decisions, and
runtime policy enforcement cross the typed boundary crates and retain
characterization coverage.

This document states required controls and test targets. It does not by itself
prove that a deployment has immutable retention, break-glass evidence, backup
or disaster-recovery coverage, approved RPO/RTO, or complete native-provider
fidelity; deployment acceptance remains environment-specific.

## Personal ChatGPT expose boundary

`prodex s expose` is a separate personal-development boundary from the
enterprise gateway. It binds the local listener to loopback, captures one
workspace as the Super process's initial directory, and places a fresh
256-bit capability in one exact path segment:

```text
https://<random>.trycloudflare.com/pdx/v1/<capability>/mcp
```

The endpoint stores only a SHA-256 digest in memory and compares the incoming
segment with the existing constant-time helper. Invalid or revoked paths return
404, the public Host admits only the exact MCP route, and legacy browser routes
are not available on the default public host. The capability is never persisted
or placed in child arguments, child environment, telemetry, or routine logs.

This is Ephemeral Capability Authentication, not OAuth, account linking, or
identity authentication. Anyone with the complete URL can control the full
Super capability granted by that process. ChatGPT stores the URL and Cloudflare
carries the path, so URL disclosure is credential disclosure. The mode is
short-lived and intended for personal development, not a public multi-user
plugin or plugin-directory publication. Stopping the process revokes access;
rerunning creates a new capability. Quick Tunnel also creates a new random
hostname, while an existing user-managed hostname remains external
infrastructure owned by the user.

Each expose process owns its workspace context, MCP identity, selected endpoint,
run manager, bounded output buffers, capability digest, and child process groups.
Only a Prodex-managed Quick Tunnel child belongs to its cleanup lifecycle;
user-managed Cloudflare services remain external and are never stopped.
Separate processes may share the established Prodex profile/quota/health and
preference persistence, but active expose configuration and run tables are
not shared. The initial workspace is not claimed to be a stronger filesystem
sandbox than the underlying local Super permission model.

The MCP surface is deliberately lifecycle-oriented: `prodex_super_start`
returns a run ID, and status/events/result/cancel/list operate on that explicit
ID. It does not expose a raw shell MCP tool. Quick Tunnel mode uses bounded
JSON responses over Streamable HTTP and no SSE dependency; readiness requires a
public protocol probe and `tools/list` before the URL is printed.

## Assets

- Tenant data and tenant-scoped configuration.
- Virtual keys, service identities, user identities, and break-glass identities.
- Provider credential references and secret-provider configuration.
- Budget reservations, usage counters, and append-only billing ledger events.
- Policy and configuration revisions, signatures, digests, and last-known-good
  cache state.
- Tenant-scoped classification-rule, provider-registry, pricing, and routing-score
  revisions and their active/last-known-good pointers.
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

### Rust-Mojo ABI Boundary

Mojo receives only bounded deterministic inputs selected by safe Rust wrappers. Numeric buffers
and non-secret diagnostic text views are caller-owned for one synchronous call; Mojo cannot
retain them or return heap ownership. Text records carry explicit pointers and byte lengths,
validate UTF-8 before `StringSlice` use, permit embedded nul bytes, and reject ABI, capacity,
null/length, tag, and malformed-text errors. Prompts, credentials, auth/session values, cookies,
secrets, and filesystem paths do not cross this text seam. Release builds pin and verify the Mojo
compiler, compare ABI layouts, fail closed when the archive is absent, and reject an unexpected
Mojo compiler-runtime dependency.

Rich ABI v6 extends this boundary without changing text ABI v1. It carries only bounded,
non-secret UTF-8 views and fixed-layout record tables. Mojo's domain objects and collection
algorithms use caller-owned output/scratch arenas and offset/index relationships; no package-owned
or Mojo-owned object crosses into Rust. Rust validates every rich result's ABI version, status,
capacity, count, offset, length, UTF-8 slice, index, type/reason tag, uniqueness, and ordering.
Invalid output is a hard internal failure on Mojo-enabled builds, not a Rust semantic fallback.

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
| Blocking I/O, unbounded workers, or mutex-held I/O on request paths | Queue starvation and gateway stalls | Async transport, bounded worker/queue limits, immutable snapshots, and no broad file or network reads while request-path locks are held |
| Domain or shared crates depend on runtime adapters | Policy bypass and untestable side effects | Dependency inversion toward domain/application ports; composition roots alone select concrete adapters |
| Trace context is dropped at a boundary | Incomplete incident and audit correlation | Validate and propagate bounded end-to-end trace context through gateway, application, provider, storage, and export boundaries |
| Secret value leaks in domain, logs, environment, or projected paths | Credential compromise | `SecretRef` in domain, production rejection of CLI/environment credential sources, redacted secret material and provider debug output, canonical projected-root containment, private file modes, and log/error redaction |
| Custom provider inherits ambient OpenAI authentication | OpenAI bearer or account ID disclosure to a non-OpenAI endpoint | Generated Prodex bridge providers require only an explicit local placeholder credential; real provider keys stay in the proxy process, unrelated custom providers remain Codex-owned, and child secret stripping is regression-tested |
| Upstream provider error rewriting | Compatibility breakage and debugging loss | Pass-through upstream status/body/stream after upstream response exists |
| Malformed or incompatible Rust-Mojo record | In-process crash, corruption, or incorrect deterministic plan | Versioned `repr(C)` DTOs, compile/runtime layout probes, bounded counts/capacities, safe Rust wrappers, explicit status tags, and no pointer retention |
| Secret-bearing text enters experimental computation | Credential exposure through crashes, diagnostics, or memory dumps | Restrict the production text ABI to non-secret diagnostic lines; no content logging or persistence; explicit future threat review for prompts or secrets |
| Heap-owning Mojo code changes release runtime | Hidden shared-library, GLIBC, or clean-machine regression | Inspect undefined symbols and final dependencies; reject `KGEN_CompilerRT_*`, RPATH/RUNPATH, or unapproved runtime bundles |
| Mid-stream rotation | Broken transport semantics and affinity | Rotation only pre-commit; continuation bindings preserved |
| High-cardinality telemetry labels | Metrics cardinality explosion | Telemetry attribute validation and bounded labels |
| Failed runtime-policy reload evicts usable policy | Availability loss or hidden partial configuration | Validate before atomic cache replacement, propagate failure, withhold publication acknowledgement, and retry delivery |
| Governance artifact is loaded for the wrong tenant | Cross-tenant policy or provider selection | Explicit configured authority tenant set; tenant-keyed immutable snapshots; no fallback snapshot in enforcing modes; cross-tenant negative tests |
| Stored revision ID differs from the compiled artifact revision | Approval provenance bypass or misleading audit evidence | Create, activation, startup, and refresh require the stored and artifact-internal revisions to match exactly |
| Invalid active provider registry or routing scores replace usable state | Routing outage or attacker-controlled weights | Strict bounded typed compilers; SQLite/PostgreSQL active-to-LKG validation; compile before atomic `ArcSwap`; retain the prior in-memory snapshot on refresh failure |
| Provider is revoked or repriced after route planning | Dispatch through stale authority | Re-read the tenant registry and routing-score snapshots at dispatch; require registry, descriptor, pricing, credential, and score revisions to remain current |
| Backup exists but cannot restore tenant/accounting state | Extended outage, billing loss, or isolation regression | Automated PostgreSQL logical dump/restore drill, checksum, recovery-point-age/restore-time gates, full tenant-table fingerprint, ledger uniqueness, and `NOBYPASSRLS` negative checks; target environments separately prove WAL/PITR and RPO |

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
- Malformed runtime-policy reload preserves cached policy or cached absence, returns an error, remains unacknowledged, and installs the corrected replacement on retry.
- Restored PostgreSQL state preserves all tenant/accounting rows, excludes post-backup writes, and denies cross-tenant reads and writes under a non-owner role.
- Provider-registry and routing-score artifacts reject malformed bounds,
  cross-tenant lookup, unsupported executable adapters, revision tampering, and
  invalid refresh without evicting last-known-good state.
- Revocation and pricing revision changes between planning and dispatch deny the
  stale route before provider invocation.
- Rust-Mojo text calls reject malformed/truncated UTF-8, null with non-zero length,
  incompatible versions, invalid counters, and undersized output buffers.
- Rust-Mojo layout probes match size, alignment, and field offsets on each Mojo release target;
  repeated and concurrent calls use disjoint buffers and return exact Rust-oracle results.

## Audit Requirements

Security-sensitive actions must produce immutable tenant-scoped audit events:

authentication failures, authorization denials, key creation/rotation/revocation,
policy/config publication, provider credential rotation, budget changes, guardrail
denials, request body limit denials, backup/restore operations, and break-glass
use. Audit records must include tenant ID, principal ID, action, resource,
outcome, reason code, event ID, and hash-chain digest when persisted.

## Residual Risks and Deployment Boundaries

The live config-publication adapter supports both a durable shared-filesystem
transport and a PostgreSQL outbox with replica-scoped acknowledgements and
advisory-lock ownership. Separate node-local roots must not be treated as
replicas of one filesystem transport. Publication records carry revision IDs
and activation targets only; policy artifact distribution remains a separate
deployment responsibility and must make the candidate policy available at each
runtime root before publishing the notification.
Last-known-good runtime policy can delay an intended policy update until retry
succeeds. Reload failures must remain observable, and urgent revocation must use
an explicit fail-closed invalidation path.

The governed registry deliberately exposes only the adapter attached to the
current process as executable. Other provider descriptors may be retained as
non-executable metadata, but this runtime does not claim heterogeneous
cross-adapter fallback. Model aliases and context limits remain route-policy
authority; deployment timeouts and concurrency remain adapter/runtime authority;
live health, quota, circuit, and load remain runtime-state authority. Registry
snapshots carry bounded compliance metadata plus explicit pricing, cost,
latency, risk, and priority revisions without duplicating those live signals.
The lifecycle HTTP adapter parses transport input and performs bounded reads.
All four artifact mutation paths cross `ApplicationGovernanceLifecycleService`
and the atomic idempotent SQLite/PostgreSQL repository contracts before success.
