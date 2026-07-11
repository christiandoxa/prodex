# ADR 0015: Document Gateway Health and Admin Concurrency in OpenAPI

## Status

Accepted

## Context

The gateway exposes root-level operational probes and admin write safeguards that are important for enterprise operators and generated clients:

- `/livez`, `/readyz`, and `/startupz` are unauthenticated probe endpoints for orchestration.
- Admin mutations may carry `Idempotency-Key` to reject accidental replay.
- Virtual-key reads return an `ETag`; virtual-key `PATCH` and `DELETE` may use `If-Match` and can return `412` on stale versions.

Leaving these surfaces out of the OpenAPI contract makes Kubernetes health integration and safe admin automation harder to discover.

## Decision

The gateway OpenAPI document now includes:

- Root-level operational probe paths with `GET` and `HEAD` operations and the shared `GatewayHealth` schema.
- `Idempotency-Key` headers on admin mutation operations.
- `ETag` response metadata on virtual-key `GET`.
- Optional `If-Match` request headers and `412` responses on virtual-key `PATCH` and `DELETE`.

## Consequences

Generated clients can model health checks and optimistic concurrency without out-of-band documentation. The contract remains backward-compatible because all new request headers are optional and existing response shapes are unchanged.
