# ADR 0242: Configuration publication refresh event plan

## Status

Accepted

## Context

Configuration publication already validates revision ordering, activates cache
state, preserves last-known-good revisions, and can write tenant-scoped Redis
policy revision cache entries. The remaining production wiring gap is the event
that tells gateway replicas to refresh configuration caches and reload runtime
policy after a control-plane publication is accepted.

If that event is implicit or adapter-specific, replicas can diverge: one process
may use a newer policy revision while another continues using stale policy until
TTL expiry.

## Decision

`prodex-config` now plans a `ConfigPublicationEventPlan` from a successful
activation. The plan carries the tenant, activated revision, previous active
revision, last-known-good revision, and two required delivery targets:

- gateway cache refresh;
- runtime policy reload.

Missing either delivery target is an explicit planning error with a stable,
redacted response. `prodex-application` returns this publication event from the
configuration activation use case so production adapters can publish one
side-effect-free plan instead of reconstructing refresh semantics locally.

## Consequences

Production control-plane adapters have a concrete event contract to connect to
gateway cache refresh and runtime policy reload mechanisms. Denied publications
still produce no activation, no Redis cache plan, and no publication event.

The boundary remains transport-neutral: it does not choose a message bus,
watcher, filesystem reload, or async runtime.
