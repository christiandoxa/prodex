# ADR 0027: Audit gateway admin idempotency denials without logging replay keys

## Status

Accepted

## Context

Gateway admin mutation endpoints accept an `Idempotency-Key` header to reject malformed
keys and duplicate mutation replays before the control-plane write handler runs. These
denials are security-sensitive because they can indicate replay attempts or clients using
invalid request fingerprints, but the previous response path returned 400/409 without an
immutable audit event.

The idempotency value itself can be user-controlled and may accidentally contain sensitive
correlation material, so audit records must not persist the raw header value.

## Decision

Admin idempotency denials now append a `gateway_admin` audit event with action
`request_denied` and outcome `failure`. The event records the authenticated admin actor,
role, method, path, backend label, and denial reason (`invalid_idempotency_key` or
`duplicate_idempotency_key`). It deliberately omits the raw `Idempotency-Key` value.

## Consequences

Operators can investigate replay and malformed-idempotency activity across replicas using
the audit log while avoiding credential-like or correlation-token leakage from
user-controlled headers. The data-plane behavior and HTTP compatibility remain unchanged.
