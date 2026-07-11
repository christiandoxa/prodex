# ADR 0547: Spend events reuse admission request IDs

## Status

Accepted

## Context

Gateway spend request and response events exposed UUIDv7 `request_id` fields,
but each event factory generated its own value. The legacy numeric request
sequence still linked the events locally, but cross-replica consumers of the
typed ID could not correlate the request and response phases for the same
admitted gateway call.

## Decision

Store the typed `prodex-{RequestId}` assigned during virtual-key admission in
the per-request gateway usage state. Before logging or exporting spend events,
copy that admission request ID and the admission call ID onto the event.

## Consequences

Spend request and response events for one admitted request now share the same
typed request and call identifiers. The numeric runtime request sequence remains
legacy-local metadata.
