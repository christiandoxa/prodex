# ADR 0977: Config publication replicated file transport

## Status

Accepted

## Context

Configuration publication already had:

- a side-effect-free publication event contract in `prodex-config`;
- a local runtime delivery adapter in `prodex-app`;
- one-shot `prodex-control-plane deliver-config-publication` wiring for manual
  local fan-out.

That still left a multi-replica gap: a separated control plane had no replicated
transport that could hand the same publication event to every gateway replica
without operator-managed per-replica command loops.

## Decision

Add a small shared-filesystem publication transport:

- `prodex-control-plane publish-config-publication --event <path> --transport <path>`
  writes one hashed event record into a transport outbox;
- `prodex-control-plane compact-config-publication --transport <path> [--retain <n>]`
  deletes fully acknowledged outbox records and their per-replica ack files
  while optionally retaining the newest acknowledged records for inspection;
- `prodex-gateway consume-config-publication --transport <path> --replica <id> --root <path>`
  scans that outbox, delivers any event not yet acknowledged by the named
  replica, then writes a per-replica acknowledgement file;
- event payloads reuse the existing publication event contract and remain
  transport-neutral at the boundary;
- local one-shot delivery stays available for targeted validation and fallback.

The transport record includes a schema version and opaque event ID so the
composition-root wiring can evolve without exposing tenant or revision material
in command output or metric labels.

## Consequences

- Replicated gateway fan-out now exists without wiring a long-lived control-plane
  server or message bus early.
- Publication delivery remains outside request-serving paths.
- Gateway replicas advance independently and idempotently through shared event
  state.
- Shared-storage deployments now also have an explicit retention compaction
  path instead of indefinite outbox/ack growth.
- The current transport still assumes a shared durable filesystem. Deployments
  that do not share storage can replace this composition-root adapter with a
  broker-backed outbox/watch transport later without changing the publication
  event contract; ADR 0984 captures that staging boundary.
