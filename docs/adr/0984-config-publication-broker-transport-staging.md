# ADR 0984: Stage broker-backed config publication transport for non-shared-storage topologies

## Status

Accepted

## Context

ADR 0977 added a replicated file transport so one control-plane publication can
reach every gateway replica without per-replica operator loops. That closes the
shared-storage case, but it still assumes one durable filesystem path is visible
from the publisher and every consumer.

That assumption does not hold for the target enterprise topologies in this
repository goal:

- multi-AZ or multi-node Kubernetes clusters with node-local storage;
- separated control-plane and gateway workloads without a shared volume;
- regulated deployments that do not permit ad hoc shared writable filesystems
  between service tiers.

The publication event contract is already transport-neutral. The missing piece
is a composition-root adapter that can fan out the same event through a broker
without changing publication planning or local delivery semantics.

## Decision

Keep the publication event contract and local delivery adapter unchanged, and
stage the next transport as a broker-backed outbox/watch adapter with these
minimum rules:

- the control plane remains responsible for publishing one event per activated
  revision into a durable outbox;
- gateway replicas consume independently with per-replica acknowledgement or
  equivalent durable cursor ownership;
- delivery stays idempotent by event ID, matching the current file transport;
- local runtime delivery still goes through the existing
  `deliver_config_publication_event_to_gateway_runtime` adapter;
- transport metadata stays low-cardinality and must not expose tenant IDs,
  revision payloads, roots, or secrets in broker subjects, metric labels, or
  command output;
- retries may re-deliver an event before ack, but must not require a new
  publication plan or mutate the event payload;
- the broker adapter replaces the shared-filesystem composition-root wiring only
  for deployments that lack one shared durable path.

## Consequences

- Non-shared-storage topologies now have an explicit migration contract instead
  of a generic future-work note.
- The existing file transport remains the shortest path for shared-storage
  deployments; no new broker surface is forced into simple installs.
- Future broker work has a hard boundary: adapt transport only, do not reopen
  config publication planning or local gateway delivery semantics.
