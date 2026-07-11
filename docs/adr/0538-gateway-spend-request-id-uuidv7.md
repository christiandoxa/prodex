# ADR 0538: Gateway spend observability uses UUIDv7 request IDs

## Status

Accepted

## Context

Gateway spend events are written to observability sinks such as JSONL, HTTP,
OpenTelemetry-shaped payloads, Datadog, and Langfuse. The event still serialized
the process-local runtime request sequence as `request`, which can collide
across replicas and should not be used as a cross-service request identifier.

## Decision

Serialized spend events and structured spend log lines now expose `request_id`
as `prodex-{RequestId}` using the typed UUIDv7 domain ID. The numeric runtime
sequence is kept only for compatibility as `legacy_request_sequence`; older
structured log consumers may still see `request` during the transition, but it
is not the cross-service request identifier.

## Consequences

External observability sinks receive globally unique request IDs. Internal
runtime matching can keep using the existing numeric sequence until the broader
runtime proxy correlation type is replaced.
