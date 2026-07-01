# 0637: Resource Version Overflow Guard

## Status

Accepted

## Context

`ResourceVersion::next` used saturating addition. At `u64::MAX`, a mutation could
silently keep the same version value, weakening optimistic concurrency and ETag
progression guarantees.

## Decision

`ResourceVersion::next` now returns `Result<ResourceVersion, ResourceVersionError>`
and rejects overflow with `ResourceVersionError::Overflow`. The response planner
keeps the stable redacted `resource_version_invalid` envelope.

## Consequences

- Version advancement cannot silently reuse an old resource version.
- Mutation planners must handle an impossible-to-advance resource as a domain
  error instead of committing a non-monotonic version.
- Client-visible error text still hides concrete version values.
