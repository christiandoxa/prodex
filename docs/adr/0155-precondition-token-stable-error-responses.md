# ADR 0155: Precondition token stable error responses

## Status

Accepted

## Context

The domain API governance boundary validates optimistic-concurrency tokens
before mutating control-plane operations can compare them. Raw `EntityTagError`
and `ResourceVersionError` values distinguish empty entity tags, overlong entity
tags, and zero resource versions. Those details are useful for trusted
diagnostics, but client-visible API responses should not expose parser limits,
request-controlled token lengths, or internal resource-version invariants.

## Decision

Add stable response planners to `prodex-domain`:

- `plan_entity_tag_error_response` maps every entity-tag parse failure to
  `entity_tag_invalid` with a bad-request status; and
- `plan_resource_version_error_response` maps invalid resource versions to
  `resource_version_invalid` with a bad-request status.

The response messages stay generic and exclude entity-tag lengths, entity-tag
values, and resource-version implementation details.

## Consequences

Gateway and control-plane composition roots can expose deterministic validation
failures for concurrency precondition tokens while retaining raw errors for
trusted diagnostics and audit trails.
