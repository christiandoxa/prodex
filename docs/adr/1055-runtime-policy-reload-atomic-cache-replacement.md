# ADR 1055: Runtime policy reload uses atomic cache replacement

## Status

Accepted.

## Context

Runtime policy reload previously removed the normalized per-root cache entry
before reading, parsing, and validating the replacement `policy.toml`. A failed
reload therefore discarded a usable cached policy or a cached decision that no
policy existed.

## Decision

Reload reads and validates the candidate before changing cached state. A
successful load atomically replaces the per-root cache entry and reports the
replaced entry through `RuntimePolicyCacheInvalidationPlan`. A failed load
returns the original error and leaves the exact previous cache entry untouched,
including cached policy absence.

Direct cache invalidation remains destructive. Filesystem reload remains limited
to publication consumers and other bounded background paths. Failed publication
delivery is not acknowledged, so correcting the policy and retrying installs the
replacement exactly once.

## Consequences

- Malformed or unreadable replacements do not evict last-known-good runtime
  policy.
- Concurrent readers never observe an intentional empty-cache window during
  reload.
- Failed publication remains visible and retryable instead of being reported as
  delivered.
- Urgent revocation still requires explicit invalidation or another fail-closed
  control because last-known-good policy remains active after reload failure.

## Regression Coverage

- `reload_runtime_policy_cached_atomically_replaces_and_reports_previous_entry`
- `failed_runtime_policy_reload_preserves_last_known_good_entry`
- `failed_runtime_policy_reload_preserves_cached_missing_policy`
- `failed_config_publication_reload_keeps_last_known_good_and_retries_event`

## Related Decisions

This decision amends ADR 0243 and clarifies ADR 0590. Revision-aware domain
policy fallback remains governed by ADR 0068 and ADR 0627.
