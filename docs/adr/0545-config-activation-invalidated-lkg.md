# ADR 0545: Do not promote invalidated config revisions to LKG

## Status

Accepted

## Context

Configuration activation preserved the previous active revision as
last-known-good. If that previous active revision was also the revision being
invalidated, a later cache fallback could re-serve configuration that the
control plane had just withdrawn.

## Decision

When activating a new configuration revision, only promote the previous active
revision to last-known-good if it is not the invalidated revision. If the prior
last-known-good revision is also invalidated, clear the fallback instead of
keeping an unsafe cache target.

## Consequences

Gateway cache state no longer resurrects an invalidated configuration revision
after a successful publish. Deployments that invalidate the only usable
fallback will fail closed if the new active revision later becomes unavailable.
