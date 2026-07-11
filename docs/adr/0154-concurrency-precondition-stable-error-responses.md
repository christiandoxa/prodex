# ADR 0154: Concurrency precondition stable error responses

## Status

Accepted

## Context

The domain API governance boundary validates optimistic concurrency
preconditions for mutating control-plane operations. Raw `ConcurrencyError`
values can include expected and actual resource versions or entity tags. Those
values are useful for trusted diagnostics and audit context, but client-visible
API envelopes should not expose stale version counters, current entity tags, or
request-controlled `If-Match` metadata.

## Decision

Add `plan_concurrency_error_response` to `prodex-domain`. It maps optimistic
concurrency failures into stable response plans:

- missing preconditions become `mutation_precondition_required` with a
  precondition-required status;
- stale resource versions become `resource_version_conflict` with a conflict
  status; and
- stale entity tags become `entity_tag_conflict` with a conflict status.

The response messages stay generic and exclude expected or actual versions,
entity-tag values, and request-controlled precondition metadata.

## Consequences

Gateway and control-plane composition roots can expose deterministic
machine-readable concurrency failures while retaining raw `ConcurrencyError`
values for trusted diagnostics, audit trails, and operator troubleshooting.
