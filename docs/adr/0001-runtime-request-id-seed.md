# ADR 0001: Runtime request ID allocation

## Status

Accepted.

## Context

Prodex runtime logs and compatibility helpers currently carry `request_id` as a
`u64`. A process-local counter starting at `1` is easy to correlate in a single
process, but it can collide when multiple gateway replicas write diagnostics or
usage records for the same tenant environment.

The enterprise target is a typed globally unique `RequestId`. Moving every hot
path and log/event DTO to a structured ID should be done incrementally to avoid a
large compatibility break.

## Decision

Keep the existing `u64` runtime request ID wire/log shape for now. Seed each
runtime instance's `AtomicU64` suffix with runtime identity material, but derive
the high bits of each emitted request ID from fresh UUIDv7 `RequestId` entropy.
The low 32 bits remain a monotonically incrementing local diagnostic suffix
starting at `1`. This is used by both the normal rotation proxy and the local
rewrite/gateway startup path so gateway-only deployments do not fall back to
process-local `1, 2, 3...` identifiers.

This is a compatibility-preserving Phase 0 hardening step. It materially reduces
multi-replica collision risk without changing existing CLI output, log parsers,
or provider-facing behavior.

## Consequences

- Existing downstream `u64` consumers keep compiling.
- Runtime request IDs are no longer trivially process-local `1, 2, 3...` values
  in production startup paths, and emitted IDs use UUIDv7 domain-ID entropy
  rather than relying only on local clock/process material.
- The workspace now includes `prodex-domain`, which defines UUIDv7-backed typed
  identifiers for `TenantId`, `PrincipalId`, `RequestId`, `CallId`,
  `ReservationId`, `VirtualKeyId`, `PolicyRevisionId`, and `AuditEventId`.
  Runtime hot paths can migrate to those types incrementally while preserving the
  current numeric request ID log shape until compatibility surfaces are updated.
  The same crate now contains canonical `Principal` and `TenantContext` types, tenant-owned resource
  authorization, immutable audit-event value objects, idempotency/replay primitives, stable machine-readable error envelopes, pagination/cursor semantics, optimistic concurrency primitives, health/readiness status primitives, provider/model capability negotiation primitives, trace/correlation context primitives, migration plan/status primitives, backup/restore validation primitives, SLO/alert primitives, OIDC/JWKS validation and cache primitives, resource/action authorization matrix, explicit role-claim mapping that never promotes missing/unknown claims to
  `Admin`, credential-scope checks that keep data-plane and control-plane
  credentials separated, `SecretRef` for non-raw secret references, and pure
  reservation/accounting and rate-limit value objects for pre-upstream admission and
  ledger idempotency keys, plus versioned policy snapshot typestates so only
  validated signed snapshots can become active or last-known-good policy.
- A later migration should update durable billing/audit schemas to use typed
  `RequestId`/`CallId`/`ReservationId` values as explicit idempotency keys.
