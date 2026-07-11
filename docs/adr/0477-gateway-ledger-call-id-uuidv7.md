# ADR 0477: Gateway ledger call IDs use UUIDv7

## Status

Accepted.

## Context

Gateway virtual-key billing ledger entries used `prodex-{request_id}` as
`call_id`. Runtime request IDs are local proxy sequence values and are not a
valid multi-replica billing idempotency key.

## Decision

Generate one typed domain `CallId` UUIDv7 value when a virtual-key request is
admitted. Store that value in the usage delta, billing ledger entry, spend event,
and optional gateway call ID response header.

## Consequences

Billing ledger call IDs no longer depend on process-local request sequences, and
the response header still correlates with the persisted ledger record for
virtual-key requests.
