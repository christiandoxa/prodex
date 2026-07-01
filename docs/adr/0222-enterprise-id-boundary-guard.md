# 0222: Enterprise ID Boundary Guard

## Status

Accepted.

## Context

Enterprise multi-replica deployments require globally unique typed identifiers
for tenants, principals, requests, calls, reservations, virtual keys, policy
revisions, and audit events. Process-local counters such as `AtomicU64` can
collide across replicas and make idempotency, ledger uniqueness, audit
correlation, and replay protection unreliable.

The domain already exposes typed UUIDv7 identifiers. The risk is regression:
future boundary crates could reintroduce local monotonic counters for
request/call/resource identity.

## Decision

Add `ci:enterprise-id-boundary-guard`. The guard verifies that
`prodex-domain` keeps the full typed identifier set backed by `Uuid::now_v7()`
and rejects `AtomicU64` or `.fetch_add(...)` in the enterprise boundary crates:

- domain, application, authn, authz, config, control-plane;
- gateway-core and gateway-http;
- observability and provider-spi;
- storage, storage-postgres, storage-redis, and storage-sqlite.

Runtime compatibility code and tests may still use local numeric counters where
they are protocol/log compatibility values rather than enterprise resource
identifiers.

## Consequences

- Boundary crates cannot silently regress to process-local request or call ID
  generation.
- Existing runtime proxy compatibility counters remain outside this guard until
  their upstream-facing wire contracts can be changed deliberately.
- Multi-replica ID safety is enforced by both Rust tests and a lightweight CI
  source guard.
