# ADR 0537: Guardrail webhook request IDs use UUIDv7

## Status

Accepted

## Context

Guardrail webhook payloads are sent to an external policy service. They already
used a typed UUIDv7 `call_id`, but the `request_id` field still exposed the
legacy process-local runtime request sequence. Process-local counters can
collide across replicas and are not suitable as cross-service identifiers.

## Decision

Emit `request_id` as `prodex-{RequestId}` using the typed UUIDv7 domain ID.
Keep the old numeric value under `legacy_request_sequence` for local diagnostic
correlation only.

## Consequences

External guardrail services receive globally unique request identifiers. Legacy
numeric request sequences remain available for short-term log correlation, but
must not be used as durable idempotency or billing keys.
