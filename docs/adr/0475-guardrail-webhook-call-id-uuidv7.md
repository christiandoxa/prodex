# ADR 0475: Guardrail webhook call IDs use UUIDv7

## Status

Accepted.

## Context

Gateway guardrail webhook payloads used `prodex-{request_id}` as `call_id`.
Runtime request IDs are local proxy sequence values and can collide across
multi-replica deployments.

## Decision

Generate guardrail webhook call IDs with the typed domain `CallId` UUIDv7
generator while preserving the existing `prodex-` payload prefix.

## Consequences

Guardrail webhook payloads no longer expose process-local request sequences as
call identifiers. The payload still includes the separate `request_id` field for
local runtime correlation.
