# ADR 0546: Ledger request IDs originate at admission

## Status

Accepted

## Context

Gateway billing ledger entries exposed a UUIDv7 `request_id`, but the value was
generated while converting an in-memory usage delta into a ledger row. That made
the serialized ledger ID independent from the request admission identity and
left each backend free to invent a different typed request identifier for the
same accepted request.

## Decision

Generate the typed `prodex-{RequestId}` value when the virtual-key request is
admitted, store it on the usage delta, and copy that value into billing ledger
entries.

## Consequences

File and Redis billing rows now keep the same typed request identity that was
assigned at admission. The numeric `request` sequence remains for legacy
compatibility and local cleanup only.
