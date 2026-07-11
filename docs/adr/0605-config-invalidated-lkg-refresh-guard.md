# ADR 0605: Config Invalidated LKG Refresh Guard

## Status

Accepted.

## Context

The configuration cache can carry an active revision, a last-known-good
revision, and an invalidated revision marker. Refresh evaluation rejected an
invalidated active revision, but the stale-window fallback could still select a
last-known-good revision that had itself been invalidated.

## Decision

Configuration refresh evaluation now uses last-known-good only when that
revision is present and is not the invalidated revision.

## Consequences

Gateways fail closed with `RefreshRequired` instead of serving an explicitly
invalidated fallback revision. Valid active revisions can still trigger async
refresh while fresh enough.
