# ADR 0085: Config boundary crate

## Status
Accepted

## Context
Gateway and control-plane configuration must be revisioned, tenant-scoped,
invalidatable, and safe to cache with last-known-good semantics. The target
architecture calls for `prodex-config` and explicitly identifies policy/config
caches without revision, refresh, invalidation, or last-known-good behavior as a
priority risk. These decisions should be reusable without depending on HTTP,
filesystem, database, or async-runtime code.

## Decision
Introduce `prodex-config` as a boundary crate for revisioned configuration
publishing and gateway cache decisions. It models tenant-scoped
`ConfigRevision`, ordered cache windows, cache state, refresh decisions,
invalidation fallback, and publication validation.

Publication validation rejects cross-tenant configuration updates and candidate
revisions that are not newer than the active revision. Cache evaluation uses the
active revision while fresh, requests async refresh inside the refresh window,
falls back to last-known-good while still usable, and requires refresh once the
cache is expired or invalidated without a safe fallback.

## Consequences
Control-plane publishing and gateway configuration caches can migrate to a
shared contract incrementally. Concrete parsers, storage adapters, file loading,
and HTTP distribution remain in adapter/composition crates rather than the
configuration boundary itself.
